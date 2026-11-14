//! The desktop binary: finds or starts the daemon and shows its sessions in a window.
//! Everything GPUI lives in `ui-kit`; the API client runs on its own tokio runtime.

use almahrya_app::sessions_view::{self, UiCommand};
use almahrya_local::paths::Paths;
use almahrya_ui_kit::{WindowConfig, run_sessions_window};
use std::process::ExitCode;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;

fn main() -> ExitCode {
    let paths = match Paths::from_env() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("could not start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (ui_tx, ui_rx) = unbounded_channel();
    let (command_tx, command_rx) = unbounded_channel();
    runtime.spawn(sessions_view::run(paths, ui_tx, command_rx));

    // tokio channels need no tokio runtime to be polled, so GPUI's executor reads this.
    let updates = Box::pin(futures_util::stream::unfold(ui_rx, |mut rx| async move {
        rx.recv().await.map(|update| (update, rx))
    }));
    run_sessions_window(WindowConfig::default(), updates, move || {
        let _ = command_tx.send(UiCommand::NewChatSession);
    });

    runtime.shutdown_timeout(Duration::from_millis(500));
    ExitCode::SUCCESS
}
