//! Benchmark: CRDT sync latency between two local peers.
//!
//! Measures the time it takes for a stroke drawn on Peer A to be fully
//! synced to Peer B via the Automerge sync protocol (no network — pure CRDT overhead).
//! This isolates serialization + diff + merge cost from network latency.
//!
//! Usage:
//!   cargo run --release --bin bench_sync

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};
use std::time::Instant;

const TRIALS: usize = 50;

fn generate_stroke(i: usize) -> Stroke {
    Stroke {
        points: vec![
            Point { x: (i % 800) as i32, y: (i % 600) as i32 },
            Point { x: ((i + 50) % 800) as i32, y: ((i + 50) % 600) as i32 },
            Point { x: ((i + 100) % 800) as i32, y: ((i + 100) % 600) as i32 },
        ],
        color: [(i % 256) as u8, ((i * 7) % 256) as u8, ((i * 13) % 256) as u8, 255],
        width: 2.0 + (i % 10) as f32,
    }
}

/// Runs the sync loop until both peers have no more messages.
/// Returns the number of round trips needed.
fn sync_loop(a: &mut AutomergeBackend, b: &mut AutomergeBackend) -> usize {
    let mut rounds = 0;
    for _ in 0..20 {
        let msg_ab = a.generate_sync_message("b");
        let msg_ba = b.generate_sync_message("a");
        if msg_ab.is_none() && msg_ba.is_none() {
            break;
        }
        if let Some(m) = msg_ab {
            b.receive_sync_message("a", m);
        }
        if let Some(m) = msg_ba {
            a.receive_sync_message("b", m);
        }
        rounds += 1;
    }
    rounds
}

fn main() {
    println!("=== CRDT Sync Latency Benchmark (local, no network) ===");
    println!();

    let mut peer_a = AutomergeBackend::new();
    let mut peer_b = AutomergeBackend::new();
    peer_a.peer_connected("b");
    peer_b.peer_connected("a");

    // Seed a shared list so both peers operate on the same Automerge object
    peer_a.apply_intent(Intent::Draw(generate_stroke(0)));
    sync_loop(&mut peer_a, &mut peer_b);

    println!("trial,draw_us,sync_us,total_us,rounds,strokes_after");

    let mut all_total = Vec::with_capacity(TRIALS);

    for i in 1..=TRIALS {
        let stroke = generate_stroke(i);

        // Measure: apply intent on A
        let t0 = Instant::now();
        peer_a.apply_intent(Intent::Draw(stroke));
        let draw_time = t0.elapsed();

        // Measure: sync A → B until convergence
        let t1 = Instant::now();
        let rounds = sync_loop(&mut peer_a, &mut peer_b);
        let sync_time = t1.elapsed();

        let total = draw_time + sync_time;
        all_total.push(total.as_secs_f64() * 1_000_000.0); // microseconds

        let strokes_b = peer_b.get_strokes().len();

        println!(
            "{},{:.1},{:.1},{:.1},{},{}",
            i,
            draw_time.as_secs_f64() * 1_000_000.0,
            sync_time.as_secs_f64() * 1_000_000.0,
            total.as_secs_f64() * 1_000_000.0,
            rounds,
            strokes_b,
        );
    }

    println!();

    // Statistics
    let avg = all_total.iter().sum::<f64>() / all_total.len() as f64;
    let min = all_total.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = all_total.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut sorted = all_total.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
    let variance = all_total.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / all_total.len() as f64;
    let std_dev = variance.sqrt();

    println!("=== Summary (microseconds) ===");
    println!("  avg:    {:.1} µs", avg);
    println!("  min:    {:.1} µs", min);
    println!("  max:    {:.1} µs", max);
    println!("  p95:    {:.1} µs", p95);
    println!("  stddev: {:.1} µs", std_dev);
    println!();
    println!("Final stroke count on Peer B: {}", peer_b.get_strokes().len());
    println!("NF-07 threshold: sync ≤ 2,000,000 µs (2 s) — this measures only CRDT overhead, not network.");
}
