use crate::loader::AssetLoader;

/// A decoded image in RGBA8, ready for GPU upload.
pub struct ImageAsset {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Loads PNG/JPEG bytes into an [`ImageAsset`] via the `image` crate.
pub struct ImageLoader;

impl AssetLoader for ImageLoader {
    type Output = ImageAsset;

    fn load(&self, bytes: &[u8], path: &str) -> Result<Self::Output, kaadan_core::KaadanError> {
        let img =
            image::load_from_memory(bytes).map_err(|e| kaadan_core::KaadanError::AssetLoad {
                path: path.to_string(),
                reason: e.to_string(),
            })?;
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        Ok(ImageAsset {
            pixels: rgba.into_raw(),
            width,
            height,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg"]
    }
}
