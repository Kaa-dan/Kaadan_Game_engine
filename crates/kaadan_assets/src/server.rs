use std::sync::Arc;

use crate::resolver::AssetResolver;

/// Central asset manager — resolves paths, deduplicates, tracks load state.
pub struct AssetServer {
    resolver: Arc<dyn AssetResolver>,
}

impl AssetServer {
    pub fn new(resolver: impl AssetResolver + 'static) -> Self {
        Self {
            resolver: Arc::new(resolver),
        }
    }

    /// Load raw bytes from the asset resolver.
    pub fn load_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError> {
        self.resolver.read_bytes(path)
    }

    /// Check if an asset path exists.
    pub fn exists(&self, path: &str) -> bool {
        self.resolver.exists(path)
    }

    /// Access the resolver.
    pub fn resolver(&self) -> &dyn AssetResolver {
        self.resolver.as_ref()
    }
}
