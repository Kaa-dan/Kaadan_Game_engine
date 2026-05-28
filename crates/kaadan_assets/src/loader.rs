/// Trait for type-specific asset loading.
/// Implementations know how to parse raw bytes into a concrete type.
pub trait AssetLoader: Send + Sync + 'static {
    type Output: Send + Sync + 'static;

    /// Parse raw bytes into the target asset type.
    fn load(&self, bytes: &[u8], path: &str) -> Result<Self::Output, kaadan_core::KaadanError>;

    /// File extensions this loader handles (e.g., &["png", "jpg"]).
    fn extensions(&self) -> &[&str];
}
