use tracing_subscriber::{fmt, EnvFilter};

/// Initialize tracing with env filter. Safe to call more than once
/// (e.g. when the editor and the embedded runtime both start up) — subsequent
/// calls are no-ops rather than panicking.
/// Set `RUST_LOG=kaadan=debug` for engine debug output.
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kaadan=info,wgpu=warn"));

    // `try_init` returns Err if a global subscriber is already set; ignore it
    // so a second call is harmless instead of a panic.
    let _ = fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_is_idempotent() {
        // Calling twice must not panic even though a global subscriber
        // can only be installed once.
        init_logging();
        init_logging();
    }
}
