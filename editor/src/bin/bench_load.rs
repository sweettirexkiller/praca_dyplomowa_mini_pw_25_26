//! Benchmark: .crdt file load time for various document sizes.
//!
//! Generates documents with N strokes, saves them to .crdt, then measures load time.
//! Outputs a CSV table suitable for the thesis (Section 3.4.2, NF-08).
//!
//! Usage:
//!   cargo run --release --bin bench_load

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};
use std::time::Instant;

const STROKE_COUNTS: &[usize] = &[100, 500, 1000, 2000, 5000];
const TRIALS: usize = 5;

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

fn main() {
    println!("=== .crdt File Load Benchmark ===");
    println!();
    println!("stroke_count,file_size_kb,avg_load_ms,min_load_ms,max_load_ms,std_dev_ms");

    for &count in STROKE_COUNTS {
        // Generate document with `count` strokes
        let mut backend = AutomergeBackend::new();
        for i in 0..count {
            backend.apply_intent(Intent::Draw(generate_stroke(i)));
        }

        let data = backend.save();
        let file_size_kb = data.len() as f64 / 1024.0;

        // Measure load time over multiple trials
        let mut times_ms = Vec::with_capacity(TRIALS);
        for _ in 0..TRIALS {
            let mut loader = AutomergeBackend::new();
            let data_clone = data.clone();

            let start = Instant::now();
            loader.load(data_clone);
            let elapsed = start.elapsed();
            times_ms.push(elapsed.as_secs_f64() * 1000.0);
        }

        let avg = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let min = times_ms.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let variance = times_ms.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / times_ms.len() as f64;
        let std_dev = variance.sqrt();

        println!(
            "{},{:.2},{:.3},{:.3},{:.3},{:.3}",
            count, file_size_kb, avg, min, max, std_dev
        );
    }

    println!();
    println!("NF-08 threshold: load ≤ 1000 ms");
}
