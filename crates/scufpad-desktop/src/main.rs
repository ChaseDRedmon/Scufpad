mod app;
mod edits;
mod self_test;
mod worker;

fn main() -> eframe::Result<()> {
    // This branch must precede creation of a window, worker, IPC client or process.
    if std::env::args_os().skip(1).any(|arg| arg == "--self-test") {
        match self_test::run() {
            Ok(()) => {
                println!(
                    "PASS: desktop pure edits, reference updates, validation, draft guards, setup and fake bridge launch policy; no UI, IPC, files or hardware initialized."
                );
                return Ok(());
            }
            Err(error) => {
                eprintln!("FAIL: {error:#}");
                std::process::exit(1);
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Scufpad · Control studio")
            .with_inner_size([1460.0, 940.0])
            .with_min_inner_size([1120.0, 720.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Scufpad · Control studio",
        options,
        Box::new(|cc| Ok(Box::new(app::Studio::new(cc)))),
    )
}
