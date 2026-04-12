//! Generates .crdt files with a specified number of strokes for FPS testing.
//!
//! Usage:
//!   cargo run --release --bin generate_crdt -- <stroke_count> <output_path>
//!
//! Example:
//!   cargo run --release --bin generate_crdt -- 100 /tmp/100_strokes.crdt
//!   cargo run --release --bin generate_crdt -- 500 /tmp/500_strokes.crdt
//!   cargo run --release --bin generate_crdt -- 1000 /tmp/1000_strokes.crdt

use collaboratite_editor::automerge_backend::AutomergeBackend;
use collaboratite_editor::backend_api::{DocBackend, Intent, Point, Stroke};

fn generate_stroke(i: usize) -> Stroke {
    Stroke {
        points: (0..10)
            .map(|p| Point {
                x: ((i * 3 + p * 7) % 800) as i32,
                y: ((i * 5 + p * 11) % 600) as i32,
            })
            .collect(),
        color: [
            (i % 256) as u8,
            ((i * 7) % 256) as u8,
            ((i * 13) % 256) as u8,
            255,
        ],
        width: 2.0 + (i % 5) as f32,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: generate_crdt <stroke_count> <output_path>");
        eprintln!("Example: generate_crdt 500 /tmp/500_strokes.crdt");
        std::process::exit(1);
    }

    let count: usize = args[1].parse().expect("Invalid stroke count");
    let path = &args[2];

    let mut backend = AutomergeBackend::new();
    for i in 0..count {
        backend.apply_intent(Intent::Draw(generate_stroke(i)));
    }

    let data = backend.save();
    let size_kb = data.len() as f64 / 1024.0;
    std::fs::write(path, data).expect("Failed to write file");

    println!("Generated {} strokes → {} ({:.1} KB)", count, path, size_kb);
}
