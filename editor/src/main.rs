mod backend_api;
mod automerge_backend;
mod ui;

use crate::automerge_backend::AutomergeBackend;
use crate::ui::AppView;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    println!("Starting Mac TextPad...");
    let mut native_options = NativeOptions::default();
    native_options.centered = true;
    dotenv::dotenv().ok();

    // In a real app, this ID should be unique per client (e.g., random or assigned by server)
    // let local_replica_id = 1;

    eframe::run_native(
        "Collaborative Text Editor",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(AppView::new(Box::new(AutomergeBackend::new()))))
        }),
    )
}
