//! KaadanEngine core utilities — logging, error types, engine-wide traits.

mod error;
mod logging;

pub use error::{KaadanError, KaadanResult};
pub use logging::init_logging;
pub use tracing;
