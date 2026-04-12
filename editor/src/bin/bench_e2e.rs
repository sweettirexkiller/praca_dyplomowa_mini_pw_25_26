//! End-to-end sync latency benchmark over a real LiveKit SFU.
//!
//! Two-process design: run a **sender** and a **receiver** in separate terminals,
//! each with a single LiveKit connection (mirrors the real GUI app exactly).
//!
//! Latency is measured using wall-clock timestamps (both processes on the same machine).
//! The sender embeds `SystemTime` in a Chat message alongside each CRDT sync.
//! The receiver records when the stroke actually appears in its local document.
//!
//! Terminal 1 (receiver — start first):
//!   cargo run --release --bin bench_e2e -- receiver <room_name>
//!
//! Terminal 2 (sender — start after receiver is connected):
//!   cargo run --release --bin bench_e2e -- sender <room_name> [trials] [delay_ms]
//!
//! Requires .env with LIVEKIT_URL, LIVEKIT_API_KEY, LIVEKIT_API_SECRET.

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};

use livekit::prelude::*;
use livekit_api::access_token;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ---- protocol types (mirrors ui.rs) ----------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
enum TransportPacket {
    Message(Vec<u8>),
    Chunk {
        id: u64,
        index: u32,
        total: u32,
        data: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum NetworkMessage {
    Sync(Vec<u8>),
    Chat(String),
    Cursor { x: i32, y: i32 },
}

// ---- helpers ---------------------------------------------------------------

fn create_token(room: &str, identity: &str) -> String {
    let api_key = std::env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY not set");
    let api_secret = std::env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET not set");
    access_token::AccessToken::with_api_key(&api_key, &api_secret)
        .with_identity(identity)
        .with_name(identity)
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: room.to_string(),
            can_publish: true,
            can_publish_data: true,
            ..Default::default()
        })
        .to_jwt()
        .expect("Failed to create token")
}

fn livekit_url() -> String {
    let host = std::env::var("LIVEKIT_URL").expect("LIVEKIT_URL not set");
    if host.starts_with("ws://") || host.starts_with("wss://") {
        host
    } else if host.starts_with("http://") {
        host.replacen("http://", "ws://", 1)
    } else if host.starts_with("https://") {
        host.replacen("https://", "wss://", 1)
    } else {
        format!("ws://{}", host)
    }
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn generate_stroke(i: usize) -> Stroke {
    Stroke {
        points: vec![
            Point {
                x: (i % 800) as i32,
                y: (i % 600) as i32,
            },
            Point {
                x: ((i + 50) % 800) as i32,
                y: ((i + 50) % 600) as i32,
            },
            Point {
                x: ((i + 100) % 800) as i32,
                y: ((i + 100) % 600) as i32,
            },
        ],
        color: [
            (i % 256) as u8,
            ((i * 7) % 256) as u8,
            ((i * 13) % 256) as u8,
            255,
        ],
        width: 2.0 + (i % 10) as f32,
    }
}

/// Publish a NetworkMessage via LiveKit data channel (broadcast), with chunking for >14KB.
async fn publish_msg(room: &Room, msg: &NetworkMessage) {
    publish_msg_inner(room, msg, Vec::new()).await;
}

/// Publish a NetworkMessage to a specific participant (directed), with chunking for >14KB.
async fn publish_msg_to(room: &Room, msg: &NetworkMessage, identity: &str) {
    let dest: Vec<ParticipantIdentity> = vec![identity.to_string().into()];
    publish_msg_inner(room, msg, dest).await;
}

async fn publish_msg_inner(room: &Room, msg: &NetworkMessage, destination_identities: Vec<ParticipantIdentity>) {
    let data = serde_json::to_vec(msg).unwrap();
    let chunk_size = 14_000;
    let chunks_count = (data.len() + chunk_size - 1) / chunk_size;

    if chunks_count <= 1 {
        let packet = TransportPacket::Message(data);
        let payload = serde_json::to_vec(&packet).unwrap();
        let _ = room
            .local_participant()
            .publish_data(DataPacket {
                payload,
                reliable: true,
                destination_identities: destination_identities.clone(),
                ..Default::default()
            })
            .await;
    } else {
        let id: u64 = rand::random();
        for (i, chunk) in data.chunks(chunk_size).enumerate() {
            let packet = TransportPacket::Chunk {
                id,
                index: i as u32,
                total: chunks_count as u32,
                data: chunk.to_vec(),
            };
            let payload = serde_json::to_vec(&packet).unwrap();
            let _ = room
                .local_participant()
                .publish_data(DataPacket {
                    payload,
                    reliable: true,
                    destination_identities: destination_identities.clone(),
                    ..Default::default()
                })
                .await;
        }
    }
}

/// Decode a raw LiveKit payload into a NetworkMessage (handles chunking).
fn decode_payload(
    transfers: &mut HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>,
    payload: &[u8],
) -> Option<NetworkMessage> {
    match serde_json::from_slice::<TransportPacket>(payload) {
        Ok(TransportPacket::Message(data)) => serde_json::from_slice(&data).ok(),
        Ok(TransportPacket::Chunk {
            id,
            index,
            total,
            data,
        }) => {
            let entry = transfers
                .entry(id)
                .or_insert_with(|| (0, vec![None; total as usize]));
            if (index as usize) < entry.1.len() && entry.1[index as usize].is_none() {
                entry.1[index as usize] = Some(data);
                entry.0 += 1;
            }
            if entry.0 == total {
                let full: Vec<u8> = entry
                    .1
                    .iter()
                    .flat_map(|c| c.as_ref().unwrap().clone())
                    .collect();
                transfers.remove(&id);
                serde_json::from_slice(&full).ok()
            } else {
                None
            }
        }
        Err(_) => serde_json::from_slice::<NetworkMessage>(payload).ok(),
    }
}

// ---- SENDER MODE -----------------------------------------------------------

async fn run_sender(room_name: &str, trials: usize, delay_ms: u64, suffix: Option<&str>) {
    let url = livekit_url();
    let identity = match suffix {
        Some(s) => format!("bench_sender_{}", s),
        None => "bench_sender".to_string(),
    };
    let token = create_token(room_name, &identity);

    println!("=== E2E Benchmark — SENDER ===");
    println!("  Server:  {}", url);
    println!("  Room:    {}", room_name);
    println!("  Trials:  {}", trials);
    println!("  Delay:   {} ms", delay_ms);
    println!();
    println!("[sender] Connecting...");

    let (room, mut events) = match Room::connect(&url, &token, RoomOptions::default()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("[sender] Connection error: {}", e);
            return;
        }
    };
    let room = Arc::new(room);
    println!("[sender] Connected!");

    let mut backend = AutomergeBackend::new();
    let mut transfers: HashMap<String, HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>> = HashMap::new();

    // Register already-present peers
    for (_, p) in room.remote_participants() {
        let pid = p.identity().to_string();
        println!("[sender] Pre-existing peer: {}", pid);
        backend.peer_connected(&pid);
    }

    // Wait for at least one receiver
    if room.remote_participants().is_empty() {
        println!("[sender] Waiting for receiver to join...");
        loop {
            match events.recv().await {
                Some(RoomEvent::ParticipantConnected(p)) => {
                    let pid = p.identity().to_string();
                    println!("[sender] Peer joined: {}", pid);
                    backend.peer_connected(&pid);
                    break;
                }
                Some(_) => {}
                None => {
                    eprintln!("[sender] Event stream closed");
                    return;
                }
            }
        }
    }

    // Seed shared strokes list + initial sync
    println!("[sender] Seeding initial stroke...");
    backend.apply_intent(Intent::Draw(generate_stroke(0)));
    for (_, p) in room.remote_participants() {
        let pid = p.identity().to_string();
        if let Some(msg) = backend.generate_sync_message(&pid) {
            publish_msg_to(&room, &NetworkMessage::Sync(msg), &pid).await;
        }
    }

    // Process sync replies for a few seconds
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            event = events.recv() => {
                match event {
                    Some(RoomEvent::DataReceived { payload, participant, .. }) => {
                        if let Some(p) = participant {
                            let sid = p.identity().to_string();
                            let t = transfers.entry(sid.clone()).or_default();
                            if let Some(NetworkMessage::Sync(data)) = decode_payload(t, &payload) {
                                backend.receive_sync_message(&sid, data);
                                if let Some(reply) = backend.generate_sync_message(&sid) {
                                    publish_msg_to(&room, &NetworkMessage::Sync(reply), &sid).await;
                                }
                            }
                        }
                    }
                    Some(RoomEvent::ParticipantConnected(p)) => {
                        backend.peer_connected(&p.identity().to_string());
                    }
                    None => break,
                    _ => {}
                }
            }
        }
    }
    println!(
        "[sender] Initial sync done (strokes: {}). Starting trials...",
        backend.get_strokes().len()
    );
    println!();

    // --- Run trials ---
    for trial in 1..=trials {
        let stroke = generate_stroke(trial);
        let send_us = now_us();

        // Draw + sync + send timestamp via Chat
        backend.apply_intent(Intent::Draw(stroke));
        for (_, p) in room.remote_participants() {
            let pid = p.identity().to_string();
            if let Some(msg) = backend.generate_sync_message(&pid) {
                publish_msg_to(&room, &NetworkMessage::Sync(msg), &pid).await;
            }
        }
        // Send timestamp beacon so receiver can compute latency
        publish_msg(
            &room,
            &NetworkMessage::Chat(format!("BENCH:{}:{}", trial, send_us)),
        )
        .await;

        println!("[sender] trial {} sent (stroke count: {})", trial, backend.get_strokes().len());

        // Process any incoming sync replies while waiting
        let wait_until =
            tokio::time::Instant::now() + std::time::Duration::from_millis(delay_ms);
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(wait_until) => break,
                event = events.recv() => {
                    match event {
                        Some(RoomEvent::DataReceived { payload, participant, .. }) => {
                            if let Some(p) = participant {
                                let sid = p.identity().to_string();
                                let t = transfers.entry(sid.clone()).or_default();
                                if let Some(NetworkMessage::Sync(data)) = decode_payload(t, &payload) {
                                    backend.receive_sync_message(&sid, data);
                                    if let Some(reply) = backend.generate_sync_message(&sid) {
                                        publish_msg_to(&room, &NetworkMessage::Sync(reply), &sid).await;
                                    }
                                }
                            }
                        }
                        Some(RoomEvent::ParticipantConnected(p)) => {
                            backend.peer_connected(&p.identity().to_string());
                        }
                        Some(RoomEvent::Disconnected { .. }) | None => {
                            eprintln!("[sender] Disconnected during trial {}", trial);
                            room.close().await.ok();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Send end signal
    publish_msg(&room, &NetworkMessage::Chat("BENCH:END".to_string())).await;
    println!();
    println!("[sender] All {} trials sent. Stroke count: {}", trials, backend.get_strokes().len());

    // Keep alive briefly for final sync replies
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    room.close().await.ok();
    println!("[sender] Done.");
}

// ---- RECEIVER MODE ---------------------------------------------------------

async fn run_receiver(room_name: &str, suffix: Option<&str>) {
    let url = livekit_url();
    let identity = match suffix {
        Some(s) => format!("bench_receiver_{}", s),
        None => "bench_receiver".to_string(),
    };
    let token = create_token(room_name, &identity);

    println!("=== E2E Benchmark — RECEIVER ===");
    println!("  Server:  {}", url);
    println!("  Room:    {}", room_name);
    println!();
    println!("[receiver] Connecting...");

    let (room, mut events) = match Room::connect(&url, &token, RoomOptions::default()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("[receiver] Connection error: {}", e);
            return;
        }
    };
    let room = Arc::new(room);
    println!("[receiver] Connected! Waiting for sender...");

    let mut backend = AutomergeBackend::new();
    let mut transfers_by_sender: HashMap<String, HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>> =
        HashMap::new();

    // Register already-present peers
    for (_, p) in room.remote_participants() {
        let pid = p.identity().to_string();
        backend.peer_connected(&pid);
        println!("[receiver] Pre-existing peer: {}", pid);
    }

    let mut last_stroke_count = backend.get_strokes().len();
    // Pending timestamp from BENCH: chat messages (trial -> send_us)
    let mut pending_timestamps: HashMap<usize, u64> = HashMap::new();
    // Track which strokes arrived but haven't been matched to a timestamp yet (trial -> recv_us)
    let mut pending_strokes: HashMap<usize, u64> = HashMap::new();
    let mut all_latencies: Vec<f64> = Vec::new();
    let mut initial_strokes: Option<usize> = None;

    println!();
    println!("trial,latency_us,latency_ms");

    loop {
        match events.recv().await {
            Some(RoomEvent::ParticipantConnected(p)) => {
                let pid = p.identity().to_string();
                println!("[receiver] Peer joined: {}", pid);
                backend.peer_connected(&pid);
            }
            Some(RoomEvent::ParticipantDisconnected(p)) => {
                let pid = p.identity().to_string();
                transfers_by_sender.remove(&pid);
                backend.peer_disconnected(&pid);
                println!("[receiver] Peer left: {}", pid);
            }
            Some(RoomEvent::DataReceived {
                payload,
                participant,
                ..
            }) => {
                if let Some(p) = participant {
                    let sender_id = p.identity().to_string();
                    let transfers = transfers_by_sender
                        .entry(sender_id.clone())
                        .or_default();

                    match decode_payload(transfers, &payload) {
                        Some(NetworkMessage::Sync(sync_data)) => {
                            backend.receive_sync_message(&sender_id, sync_data);

                            // Check if new strokes arrived
                            let current = backend.get_strokes().len();
                            if current > last_stroke_count {
                                let recv_us = now_us();

                                // Record initial stroke count (after seed sync)
                                if initial_strokes.is_none() {
                                    initial_strokes = Some(current);
                                    last_stroke_count = current;
                                } else {
                                    let base = initial_strokes.unwrap();
                                    let new_strokes = current - last_stroke_count;
                                    last_stroke_count = current;

                                    // Each new stroke maps to a trial number
                                    for offset in 0..new_strokes {
                                        let trial = current - base - new_strokes + offset + 1;
                                        // Try to match immediately with a pending timestamp
                                        if let Some(send_us) = pending_timestamps.remove(&trial) {
                                            let latency_us = recv_us.saturating_sub(send_us) as f64;
                                            all_latencies.push(latency_us);
                                            println!("{},{:.1},{:.2}", trial, latency_us, latency_us / 1000.0);
                                        } else {
                                            // Timestamp hasn't arrived yet, park it
                                            pending_strokes.insert(trial, recv_us);
                                        }
                                    }
                                }
                            }

                            // Send sync reply
                            if let Some(reply) = backend.generate_sync_message(&sender_id) {
                                publish_msg_to(&room, &NetworkMessage::Sync(reply), &sender_id).await;
                            }
                        }
                        Some(NetworkMessage::Chat(text)) => {
                            if text == "BENCH:END" {
                                println!();
                                println!("[receiver] Sender finished.");
                                break;
                            }
                            // Parse "BENCH:<trial>:<timestamp_us>"
                            if let Some(rest) = text.strip_prefix("BENCH:") {
                                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    if let (Ok(trial), Ok(ts)) =
                                        (parts[0].parse::<usize>(), parts[1].parse::<u64>())
                                    {
                                        // Check if this stroke already arrived
                                        if let Some(recv_us) = pending_strokes.remove(&trial) {
                                            let latency_us = recv_us.saturating_sub(ts) as f64;
                                            all_latencies.push(latency_us);
                                            println!("{},{:.1},{:.2}", trial, latency_us, latency_us / 1000.0);
                                        } else {
                                            // Stroke hasn't arrived yet, park the timestamp
                                            pending_timestamps.insert(trial, ts);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some(RoomEvent::Disconnected { reason }) => {
                eprintln!("[receiver] Disconnected: {:?}", reason);
                break;
            }
            None => {
                eprintln!("[receiver] Event stream ended");
                break;
            }
            _ => {}
        }
    }

    // --- Summary ---
    println!();
    if !all_latencies.is_empty() {
        let n = all_latencies.len();
        let avg = all_latencies.iter().sum::<f64>() / n as f64;
        let min = all_latencies.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = all_latencies
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sorted = all_latencies.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = sorted[n / 2];
        let p95 = sorted[((n as f64 * 0.95) as usize).min(n - 1)];
        let p99 = sorted[((n as f64 * 0.99) as usize).min(n - 1)];
        let variance =
            all_latencies.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / n as f64;
        let std_dev = variance.sqrt();

        println!("=== End-to-End Latency Summary ===");
        println!("  samples: {}", n);
        println!("  avg:     {:.1} µs  ({:.2} ms)", avg, avg / 1000.0);
        println!("  min:     {:.1} µs  ({:.2} ms)", min, min / 1000.0);
        println!("  p50:     {:.1} µs  ({:.2} ms)", p50, p50 / 1000.0);
        println!("  p95:     {:.1} µs  ({:.2} ms)", p95, p95 / 1000.0);
        println!("  p99:     {:.1} µs  ({:.2} ms)", p99, p99 / 1000.0);
        println!("  max:     {:.1} µs  ({:.2} ms)", max, max / 1000.0);
        println!("  stddev:  {:.1} µs  ({:.2} ms)", std_dev, std_dev / 1000.0);
        println!();
        println!(
            "NF-07: sync ≤ 2000 ms  →  {}",
            if avg / 1000.0 <= 2000.0 {
                "PASS"
            } else {
                "FAIL"
            }
        );
    } else {
        println!("No latency samples collected!");
    }

    // Brief wait then clean up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    room.close().await.ok();
    println!("[receiver] Done.");
}

// ---- MAIN ------------------------------------------------------------------

fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("");

    match mode {
        "sender" => {
            let room = args
                .get(2)
                .expect("Usage: bench_e2e sender <room> [trials] [delay_ms] [id_suffix]");
            let trials: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);
            let delay_ms: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(200);
            let suffix = args.get(5).map(|s| s.as_str());

            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(run_sender(room, trials, delay_ms, suffix));
        }
        "receiver" => {
            let room = args
                .get(2)
                .expect("Usage: bench_e2e receiver <room> [id_suffix]");
            let suffix = args.get(3).map(|s| s.as_str());

            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(run_receiver(room, suffix));
        }
        _ => {
            eprintln!("Usage:");
            eprintln!("  Terminal 1 (start first):");
            eprintln!("    cargo run --release --bin bench_e2e -- receiver <room_name> [id_suffix]");
            eprintln!();
            eprintln!("  Terminal 2 (start after receiver connects):");
            eprintln!("    cargo run --release --bin bench_e2e -- sender <room_name> [trials] [delay_ms] [id_suffix]");
            eprintln!();
            eprintln!("  Example (single receiver):");
            eprintln!("    cargo run --release --bin bench_e2e -- receiver test_room");
            eprintln!("    cargo run --release --bin bench_e2e -- sender test_room 30 200");
            eprintln!();
            eprintln!("  Example (multi-receiver scalability test):");
            eprintln!("    cargo run --release --bin bench_e2e -- receiver scale_room 1");
            eprintln!("    cargo run --release --bin bench_e2e -- receiver scale_room 2");
            eprintln!("    cargo run --release --bin bench_e2e -- sender scale_room 30 200");
            std::process::exit(1);
        }
    }
}
