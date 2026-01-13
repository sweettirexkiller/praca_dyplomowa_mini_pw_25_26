//! Main entry point for the "Collaborative Whiteboard" collaborative.
//!
//! This crate implements a real-time collaborative whiteboard
//! using [LiveKit](https://livekit.io/) for data transport and [Automerge](https://automerge.org/)
//! for Conflict-Free Replicated Data Type (CRDT) state management.
//!
//! # Modules
//!
//! - `backend_api`: Defines the core document backend traits and data structures.
//! - `automerge_backend`: Implements the `DocBackend` using Automerge.
//! - `ui`: Contains the `eframe`/`egui` user interface and network handling logic.
//! - `ui_panels`: Submodules for different UI panels (sidebar, editor, status_bar etc.).

mod backend_api;
mod automerge_backend;
mod ui;

use crate::automerge_backend::AutomergeBackend;
use crate::ui::AppView;
use eframe::NativeOptions;

/// The main entry point of the application.
///
/// Initializes the application window, loads environment variables,
/// and starts the `eframe` event loop with the `AppView`.
///
/// # Returns
///
/// * `eframe::Result<()>` - Result of the application execution.
fn main() -> eframe::Result<()> {
    println!("Starting Collaborative Whiteboard...");
    let mut native_options = NativeOptions::default();
    native_options.centered = true;
    dotenv::dotenv().ok();

    // In a real app, this ID should be unique per client (e.g., random or assigned by server)
    // let local_replica_id = 1;

    eframe::run_native(
        "Collaborative Whiteboard - Automerge + LiveKit",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(AppView::new(Box::new(AutomergeBackend::new()))))
        }),
    )
}
