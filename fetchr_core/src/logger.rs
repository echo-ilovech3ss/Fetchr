use std::path::PathBuf;
use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

/// Initializes the tracing logging framework.
/// Sets up structured JSON log files that rotate daily, plus nice console logging.
pub fn init_logger(portable_mode: bool, debug: bool) -> Result<WorkerGuard> {
    let logs_dir = if portable_mode {
        PathBuf::from("./logs")
    } else {
        match dirs::home_dir() {
            Some(home) => home.join(".videosaver").join("logs"),
            None => PathBuf::from("./.videosaver").join("logs"),
        }
    };

    std::fs::create_dir_all(&logs_dir)?;

    // Daily rotating logs: videosaver.log.2026-05-27 etc.
    let file_appender = tracing_appender::rolling::daily(&logs_dir, "videosaver.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Dynamic filters
    let filter = if debug {
        EnvFilter::new("fetchr_core=debug,fetchr_desktop=debug,info")
    } else {
        EnvFilter::new("fetchr_core=info,fetchr_desktop=info,warn")
    };

    // 1. JSON Layer for rotating files (perfect for parsing diagnostic dumps)
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_ansi(false);

    // 2. Clear format Layer for standard output
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true);

    let subscriber = Registry::default()
        .with(filter)
        .with(json_layer)
        .with(stdout_layer);

    // Set global subscriber (ignore double registration if Tauri already registered one)
    let _ = tracing::subscriber::set_global_default(subscriber);

    tracing::info!("Tracing Logger initialized successfully. Logs dir: {:?}", logs_dir);

    Ok(guard)
}
