use tracing_subscriber::{fmt, EnvFilter};

/// Initialize tracing with env filter. Call once at engine startup.
/// Set `RUST_LOG=kaadan=debug` for engine debug output.
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kaadan=info,wgpu=warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .init();
}
