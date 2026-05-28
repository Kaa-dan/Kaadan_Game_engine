use thiserror::Error;

#[derive(Error, Debug)]
pub enum KaadanError {
    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Renderer error: {0}")]
    Renderer(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Asset load error: {path}: {reason}")]
    AssetLoad { path: String, reason: String },

    #[error("Invalid handle: index={index}, generation={generation}")]
    InvalidHandle { index: u32, generation: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type KaadanResult<T> = Result<T, KaadanError>;
