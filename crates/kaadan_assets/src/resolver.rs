use std::path::PathBuf;

/// How to locate and read raw bytes from the platform's asset storage.
pub trait AssetResolver: Send + Sync {
    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError>;
    fn exists(&self, path: &str) -> bool;
}

/// Desktop filesystem resolver — reads from an assets directory.
pub struct FilesystemResolver {
    root: PathBuf,
}

impl FilesystemResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetResolver for FilesystemResolver {
    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError> {
        let full_path = self.root.join(path);
        std::fs::read(&full_path).map_err(|e| kaadan_core::KaadanError::AssetLoad {
            path: full_path.display().to_string(),
            reason: e.to_string(),
        })
    }

    fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

impl FilesystemResolver {
    /// Find the assets directory by walking up from the executable.
    pub fn find_assets_dir() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            let candidate = dir.join("assets");
            if candidate.is_dir() {
                return Some(candidate);
            }
            if !dir.pop() {
                break;
            }
        }
        None
    }
}
