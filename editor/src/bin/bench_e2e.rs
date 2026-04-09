//! End-to-end sync latency benchmark over a real LiveKit SFU.
//!
//! Connects N headless clients (1 sender + receivers) to a LiveKit room,
//! then measures the time from "sender draws a stroke and publishes CRDT
//! sync data" to "receiver receives, applies, and has the new stroke".
//!
//! The measurement includes: CRDT diff generation + JSON serialization +
//! LiveKit data-channel transport (WebRTC via SFU) + deserialization +
//! CRDT merge on receiver.
//!
//! Architecture:
//!   - Each client runs in its own std::thread with a dedicated tokio Runtime
//!     (matching the GUI app's approach — required by LiveKit's WebRTC internals).
//!   - Sender draws, generates per-peer sync messages, publishes via LiveKit.
//!   - Receivers apply incoming sync data, send replies (multi-round sync).
//!   - Latency = Instant::now() at sender draw → receiver confirms new stroke count.
//!
//! Requires .env with LIVEKIT_URL, LIVEKIT_API_KEY, LIVEKIT_API_SECRET.
//!
//! Usage:
//!   cargo run --release --bin bench_e2e             # 1 receiver, 30 trials
//!   cargo run --release --bin bench_e2e -- 2 50     # 2 receivers, 50 trials

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};

use livekit::prelude::*;
use livekit_api::access_token;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, Notify};

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

/// Commands from main thread → sender thread.
enum SenderCmd {
    /// Draw a stroke and publish sync to all peers.
    DrawAndSync { stroke: Stroke },
    /// Shut down.
    Stop,
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

/// Publish a NetworkMessage through the LiveKit data channel, with chunking
/// for payloads above 14 KB (matches the app's chunking logic in ui.rs).
async fn publish_msg(room: &Room, msg: &NetworkMessage) {
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
                    ..Default::default()
                })
                .await;
        }
    }
}

/// Extract a NetworkMessage from a raw LiveKit payload, handling both
/// TransportPacket-wrapped and direct formats.
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

// ---- receiver (runs in its own std::thread + tokio runtime) ----------------

/// Spawn a receiver in a dedicated OS thread with its own tokio runtime.
/// This matches how the GUI app connects to LiveKit (required by WebRTC internals).
fn spawn_receiver(
    identity: String,
    room_name: String,
    url: String,
    ready: Arc<Notify>,
    stop: Arc<Notify>,
    result_tx: mpsc::UnboundedSender<f64>,
    timestamp_rx: Arc<Mutex<mpsc::UnboundedReceiver<Instant>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create receiver runtime");
        rt.block_on(async move {
            let token = create_token(&room_name, &identity);
            println!("[{}] Connecting to room '{}'...", identity, room_name);

            let (room, mut events) = match Room::connect(&url, &token, RoomOptions::default()).await
            {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("[{}] Connection error: {}", identity, e);
                    ready.notify_one();
                    return;
                }
            };
            let room = Arc::new(room);
            println!("[{}] Connected", identity);

            let mut backend = AutomergeBackend::new();
            let mut transfers_by_sender: HashMap<
                String,
                HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>,
            > = HashMap::new();

            // Register participants already in the room
            for (_, p) in room.remote_participants() {
                let pid = p.identity().to_string();
                backend.peer_connected(&pid);
                println!("[{}] Pre-existing peer: {}", identity, pid);
            }

            let mut last_stroke_count = backend.get_strokes().len();

            // Signal ready AFTER successful connect
            ready.notify_one();

            loop {
                tokio::select! {
                    _ = stop.notified() => break,
                    Some(event) = events.recv() => {
                        match event {
                            RoomEvent::ParticipantConnected(p) => {
                                let pid = p.identity().to_string();
                                println!("[{}] Peer joined: {}", identity, pid);
                                backend.peer_connected(&pid);
                            }
                            RoomEvent::ParticipantDisconnected(p) => {
                                let pid = p.identity().to_string();
                                transfers_by_sender.remove(&pid);
                                backend.peer_disconnected(&pid);
                            }
                            RoomEvent::DataReceived { payload, participant, .. } => {
                                if let Some(p) = participant {
                                    let sender_id = p.identity().to_string();
                                    let transfers = transfers_by_sender
                                        .entry(sender_id.clone())
                                        .or_default();

                                    if let Some(NetworkMessage::Sync(sync_data)) =
                                        decode_payload(transfers, &payload)
                                    {
                                        backend.receive_sync_message(&sender_id, sync_data);

                                        let current = backend.get_strokes().len();
                                        if current > last_stroke_count {
                                            last_stroke_count = current;
                                            if let Ok(sent_at) =
                                                timestamp_rx.lock().await.try_recv()
                                            {
                                                let latency_us =
                                                    sent_at.elapsed().as_secs_f64() * 1_000_000.0;
                                                let _ = result_tx.send(latency_us);
                                            }
                                        }

                                        if let Some(reply) =
                                            backend.generate_sync_message(&sender_id)
                                        {
                                            publish_msg(&room, &NetworkMessage::Sync(reply)).await;
                                        }
                                    }
                                }
                            }
                            RoomEvent::Disconnected { reason } => {
                                eprintln!("[{}] Disconnected: {:?}", identity, reason);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }

            room.close().await.ok();
            println!(
                "[{}] Final stroke count: {}",
                identity,
                backend.get_strokes().len()
            );
        });
    })
}

// ---- sender (runs in its own std::thread + tokio runtime) ------------------

/// Spawn the sender in a dedicated OS thread with its own tokio runtime.
fn spawn_sender(
    room_name: String,
    url: String,
    num_receivers: usize,
    ready: Arc<Notify>,
    _stop: Arc<Notify>,
    cmd_rx: Arc<Mutex<mpsc::UnboundedReceiver<SenderCmd>>>,
    _result_tx: mpsc::UnboundedSender<f64>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create sender runtime");
        rt.block_on(async move {
            let sender_identity = "sender_0";
            let token = create_token(&room_name, sender_identity);
            println!("[sender] Connecting...");

            let (room, mut events) = match Room::connect(&url, &token, RoomOptions::default()).await
            {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("[sender] Connection error: {}", e);
                    ready.notify_one();
                    return;
                }
            };
            let room = Arc::new(room);
            println!("[sender] Connected");

            let mut backend = AutomergeBackend::new();
            let mut seen_peers = 0usize;

            // Register peers already present
            for (_, p) in room.remote_participants() {
                let pid = p.identity().to_string();
                println!("[sender] Pre-existing peer: {}", pid);
                backend.peer_connected(&pid);
                seen_peers += 1;
            }

            // Wait for remaining peers via events
            while seen_peers < num_receivers {
                match events.recv().await {
                    Some(RoomEvent::ParticipantConnected(p)) => {
                        let pid = p.identity().to_string();
                        println!("[sender] Peer joined: {}", pid);
                        backend.peer_connected(&pid);
                        seen_peers += 1;
                    }
                    Some(_) => {}
                    None => {
                        eprintln!("[sender] Event stream ended while waiting for peers");
                        ready.notify_one();
                        return;
                    }
                }
            }
            println!("[sender] All {} receivers visible", num_receivers);

            // Seed shared strokes list
            backend.apply_intent(Intent::Draw(generate_stroke(0)));
            for (_, p) in room.remote_participants() {
                let pid = p.identity().to_string();
                if let Some(msg) = backend.generate_sync_message(&pid) {
                    publish_msg(&room, &NetworkMessage::Sync(msg)).await;
                }
            }

            // Process initial sync replies for a few seconds
            let deadline =
                tokio::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut transfers: HashMap<String, HashMap<u64, (u32, Vec<Option<Vec<u8>>>)>> =
                HashMap::new();
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
                                            publish_msg(&room, &NetworkMessage::Sync(reply)).await;
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
                "[sender] Initial sync complete, seed strokes: {}",
                backend.get_strokes().len()
            );

            // Signal ready
            ready.notify_one();

            // Main loop: handle commands from main thread + incoming events
            let mut cmd_rx = cmd_rx.lock().await;
            loop {
                tokio::select! {
                    Some(event) = events.recv() => {
                        match event {
                            RoomEvent::DataReceived { payload, participant, .. } => {
                                if let Some(p) = participant {
                                    let sid = p.identity().to_string();
                                    let t = transfers.entry(sid.clone()).or_default();
                                    if let Some(NetworkMessage::Sync(data)) = decode_payload(t, &payload) {
                                        backend.receive_sync_message(&sid, data);
                                        if let Some(reply) = backend.generate_sync_message(&sid) {
                                            publish_msg(&room, &NetworkMessage::Sync(reply)).await;
                                        }
                                    }
                                }
                            }
                            RoomEvent::ParticipantConnected(p) => {
                                backend.peer_connected(&p.identity().to_string());
                            }
                            RoomEvent::Disconnected { .. } => break,
                            _ => {}
                        }
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(SenderCmd::DrawAndSync { stroke }) => {
                                backend.apply_intent(Intent::Draw(stroke));
                                for (_, p) in room.remote_participants() {
                                    let pid = p.identity().to_string();
                                    if let Some(msg) = backend.generate_sync_message(&pid) {
                                        publish_msg(&room, &NetworkMessage::Sync(msg)).await;
                                    }
                                }
                            }
                            Some(SenderCmd::Stop) | None => break,
                        }
                    }
                }
            }

            room.close().await.ok();
            println!(
                "[sender] Final stroke count: {}",
                backend.get_strokes().len()
            );
        });
    })
}

// ---- main ------------------------------------------------------------------

fn main() {
    dotenv::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    let num_receivers: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let trials: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);

    let total_peers = num_receivers + 1;
    let room_name = format!("bench_{}", rand::random::<u32>() % 100_000);
    let url = livekit_url();

    println!("=== End-to-End LiveKit Sync Benchmark ===");
    println!("  Server:    {}", url);
    println!("  Room:      {}", room_name);
    println!(
        "  Peers:     {} (1 sender + {} receivers)",
        total_peers, num_receivers
    );
    println!("  Trials:    {}", trials);
    println!();

    // Use a small coordinator runtime on the main thread for mpsc/notify coordination
    let coord_rt = tokio::runtime::Runtime::new().expect("Failed to create coordinator runtime");

    // Shared stop signal
    let stop = Arc::new(Notify::new());

    // Result channel: receivers → main
    let (result_tx, result_rx) = mpsc::unbounded_channel::<f64>();
    let result_rx = Arc::new(Mutex::new(result_rx));

    // Timestamp channels: main → each receiver
    let mut ts_txs: Vec<mpsc::UnboundedSender<Instant>> = Vec::new();
    let mut receiver_handles = Vec::new();

    // --- Spawn receivers (one at a time, each in its own OS thread) ---
    for i in 0..num_receivers {
        let identity = format!("receiver_{}", i);
        let ready = Arc::new(Notify::new());
        let (ts_tx, ts_rx) = mpsc::unbounded_channel::<Instant>();
        ts_txs.push(ts_tx);

        let handle = spawn_receiver(
            identity,
            room_name.clone(),
            url.clone(),
            ready.clone(),
            stop.clone(),
            result_tx.clone(),
            Arc::new(Mutex::new(ts_rx)),
        );
        receiver_handles.push(handle);

        // Wait for this receiver to be connected before spawning the next
        let connected = coord_rt.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(30), ready.notified()).await
        });
        if connected.is_err() {
            eprintln!("Receiver {} failed to connect within 30s, aborting.", i);
            return;
        }
        println!("Receiver {} ready.", i);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Extra settle time for WebRTC data channels
    std::thread::sleep(std::time::Duration::from_secs(2));

    // --- Spawn sender in its own OS thread ---
    let sender_ready = Arc::new(Notify::new());
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SenderCmd>();

    let sender_handle = spawn_sender(
        room_name.clone(),
        url.clone(),
        num_receivers,
        sender_ready.clone(),
        stop.clone(),
        Arc::new(Mutex::new(cmd_rx)),
        result_tx.clone(),
    );

    // Wait for sender to be connected and initial sync done
    let sender_ok = coord_rt.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            sender_ready.notified(),
        )
        .await
    });
    if sender_ok.is_err() {
        eprintln!("Sender failed to connect within 30s, aborting.");
        return;
    }
    println!("[main] Sender ready, starting trials...");
    println!();
    println!("trial,latency_us");

    // --- Run trials ---
    let mut all_latencies: Vec<f64> = Vec::new();

    // Drain any latencies from initial sync
    coord_rt.block_on(async {
        let mut rx = result_rx.lock().await;
        while rx.try_recv().is_ok() {}
    });

    for trial in 1..=trials {
        let stroke = generate_stroke(trial);
        let now = Instant::now();

        // Send timestamp to each receiver
        for ts_tx in &ts_txs {
            let _ = ts_tx.send(now);
        }

        // Tell sender to draw + publish
        let _ = cmd_tx.send(SenderCmd::DrawAndSync { stroke });

        // Wait for latency reports from all receivers
        let latencies = coord_rt.block_on(async {
            let mut rx = result_rx.lock().await;
            let mut received = 0;
            let mut trial_latencies = Vec::new();
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

            while received < num_receivers {
                tokio::select! {
                    Some(latency) = rx.recv() => {
                        trial_latencies.push(latency);
                        received += 1;
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        eprintln!("  trial {} TIMEOUT: {}/{} receivers responded", trial, received, num_receivers);
                        break;
                    }
                }
            }
            trial_latencies
        });

        for lat in &latencies {
            println!("{},{:.1}", trial, lat);
            all_latencies.push(*lat);
        }

        // Pace trials
        std::thread::sleep(std::time::Duration::from_millis(200));
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
        let timeouts = (trials * num_receivers) - n;
        if timeouts > 0 {
            println!("  timeouts: {}/{}", timeouts, trials * num_receivers);
        }
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

    // Cleanup
    let _ = cmd_tx.send(SenderCmd::Stop);
    stop.notify_waiters();

    sender_handle.join().ok();
    for h in receiver_handles {
        h.join().ok();
    }
    println!("=== Done ===");
}
