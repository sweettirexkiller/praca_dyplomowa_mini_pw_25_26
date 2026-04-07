//! Stress test: CRDT convergence under heavy concurrent load.
//!
//! Simulates N peers (2–5) each drawing M strokes concurrently, then syncing
//! through a hub peer (star topology). Verifies all peers converge to the same
//! state and measures sync time per peer count.
//!
//! Also tests clear-during-draw (add-wins semantics) under load.
//!
//! Usage:
//!   cargo run --release --bin bench_convergence

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};
use std::time::Instant;

const STROKES_PER_PEER: usize = 50;

fn generate_stroke(peer: usize, i: usize) -> Stroke {
    Stroke {
        points: vec![
            Point { x: (peer * 100 + i) as i32, y: (peer * 100 + i) as i32 },
            Point { x: (peer * 100 + i + 10) as i32, y: (peer * 100 + i + 10) as i32 },
        ],
        color: [(peer * 60 % 256) as u8, (i % 256) as u8, 128, 255],
        width: 2.0 + (peer % 5) as f32,
    }
}

fn sync_loop(a: &mut AutomergeBackend, a_label: &str, b: &mut AutomergeBackend, b_label: &str) {
    for _ in 0..30 {
        let msg_ab = a.generate_sync_message(b_label);
        let msg_ba = b.generate_sync_message(a_label);
        if msg_ab.is_none() && msg_ba.is_none() {
            break;
        }
        if let Some(m) = msg_ab {
            b.receive_sync_message(a_label, m);
        }
        if let Some(m) = msg_ba {
            a.receive_sync_message(b_label, m);
        }
    }
}

fn run_star_sync(peers: &mut [AutomergeBackend], labels: &[String]) {
    // Hub is peers[0]. Sync hub ↔ each spoke, repeat until stable.
    for _round in 0..10 {
        let mut any_msg = false;
        for i in 1..peers.len() {
            let (left, right) = peers.split_at_mut(i);
            let hub = left.last_mut().unwrap();
            let spoke = &mut right[0];

            // hub → spoke
            if let Some(m) = hub.generate_sync_message(&labels[i]) {
                spoke.receive_sync_message(&labels[0], m);
                any_msg = true;
            }
            // spoke → hub
            if let Some(m) = spoke.generate_sync_message(&labels[0]) {
                hub.receive_sync_message(&labels[i], m);
                any_msg = true;
            }
        }
        if !any_msg {
            break;
        }
    }
}

fn main() {
    println!("=== CRDT Convergence Stress Test ===");
    println!();

    // ---- Test 1: Scalability with 2–5 concurrent peers ----
    println!("--- Test 1: Multi-peer scalability ---");
    println!("num_peers,strokes_per_peer,total_strokes_expected,draw_ms,sync_ms,converged,final_strokes");

    for num_peers in 2..=5 {
        let labels: Vec<String> = (0..num_peers).map(|i| format!("peer_{}", i)).collect();
        let mut peers: Vec<AutomergeBackend> = (0..num_peers).map(|_| AutomergeBackend::new()).collect();

        // Register connections (star topology: peer_0 is hub)
        for i in 1..num_peers {
            peers[0].peer_connected(&labels[i]);
            peers[i].peer_connected(&labels[0]);
        }

        // Seed shared strokes list via hub
        peers[0].apply_intent(Intent::Draw(generate_stroke(0, 0)));
        run_star_sync(&mut peers, &labels);

        // Each peer draws STROKES_PER_PEER strokes concurrently (no sync between draws)
        let draw_start = Instant::now();
        for p in 0..num_peers {
            for i in 1..=STROKES_PER_PEER {
                peers[p].apply_intent(Intent::Draw(generate_stroke(p, i)));
            }
        }
        let draw_time = draw_start.elapsed();

        // Sync everything through hub
        let sync_start = Instant::now();
        // Multiple full rounds to propagate from spoke→hub→other spokes
        for _ in 0..5 {
            run_star_sync(&mut peers, &labels);
        }
        let sync_time = sync_start.elapsed();

        // Verify convergence
        let stroke_counts: Vec<usize> = peers.iter().map(|p| p.get_strokes().len()).collect();
        let expected = 1 + num_peers * STROKES_PER_PEER; // seed + all draws
        let converged = stroke_counts.iter().all(|&c| c == stroke_counts[0]);

        println!(
            "{},{},{},{:.2},{:.2},{},{}",
            num_peers,
            STROKES_PER_PEER,
            expected,
            draw_time.as_secs_f64() * 1000.0,
            sync_time.as_secs_f64() * 1000.0,
            converged,
            stroke_counts[0],
        );

        if !converged {
            eprintln!("  WARNING: Peers did NOT converge! Counts: {:?}", stroke_counts);
        }
        if stroke_counts[0] != expected {
            eprintln!(
                "  WARNING: Expected {} strokes, got {}",
                expected, stroke_counts[0]
            );
        }
    }

    println!();

    // ---- Test 2: Clear during concurrent draw (add-wins) ----
    println!("--- Test 2: Clear during concurrent draw (add-wins semantics) ---");

    let mut hub = AutomergeBackend::new();
    let mut spoke = AutomergeBackend::new();
    hub.peer_connected("spoke");
    spoke.peer_connected("hub");

    // Seed 100 strokes
    for i in 0..100 {
        hub.apply_intent(Intent::Draw(generate_stroke(0, i)));
    }
    sync_loop(&mut hub, "hub", &mut spoke, "spoke");
    println!("Initial strokes on both: {}", hub.get_strokes().len());

    // Hub clears, spoke draws 20 new strokes concurrently
    hub.apply_intent(Intent::Clear);
    for i in 0..20 {
        spoke.apply_intent(Intent::Draw(generate_stroke(1, 1000 + i)));
    }

    // Sync
    let sync_start = Instant::now();
    for _ in 0..10 {
        sync_loop(&mut hub, "hub", &mut spoke, "spoke");
    }
    let sync_time = sync_start.elapsed();

    let hub_strokes = hub.get_strokes().len();
    let spoke_strokes = spoke.get_strokes().len();
    let converged = hub_strokes == spoke_strokes;

    println!("After clear + concurrent draw:");
    println!("  Hub strokes:   {}", hub_strokes);
    println!("  Spoke strokes: {}", spoke_strokes);
    println!("  Converged:     {}", converged);
    println!("  Sync time:     {:.2} ms", sync_time.as_secs_f64() * 1000.0);
    println!(
        "  Add-wins:      {} (spoke's new strokes survived the clear)",
        hub_strokes == 20
    );

    println!();
    println!("=== Done ===");
}
