//! KaadanEngine core utilities — logging, error types, engine-wide traits.

mod error;
mod logging;

pub use error::{KaadanError, KaadanResult};
pub use logging::init_logging;
pub use tracing;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = KaadanError::AssetNotFound("hero.png".to_string());
        assert_eq!(err.to_string(), "Asset not found: hero.png");

        let err = KaadanError::InvalidHandle {
            index: 3,
            generation: 7,
        };
        assert_eq!(err.to_string(), "Invalid handle: index=3, generation=7");
    }

    #[test]
    fn result_alias_propagates_io_error() {
        fn read() -> KaadanResult<()> {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "nope"))?;
            Ok(())
        }
        assert!(matches!(read(), Err(KaadanError::Io(_))));
    }
}
