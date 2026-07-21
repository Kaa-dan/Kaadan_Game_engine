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

/// An audio clip holding raw, still-encoded bytes (WAV/OGG/MP3/FLAC). Decoding
/// is deferred to `kaadan_audio`, which consumes these bytes directly.
pub struct AudioClip {
    pub bytes: Vec<u8>,
}

/// Loads audio files by storing their raw encoded bytes verbatim.
pub struct AudioLoader;

impl AssetLoader for AudioLoader {
    type Output = AudioClip;

    fn load(&self, bytes: &[u8], _path: &str) -> Result<Self::Output, kaadan_core::KaadanError> {
        Ok(AudioClip {
            bytes: bytes.to_vec(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["wav", "ogg", "mp3", "flac"]
    }
}
