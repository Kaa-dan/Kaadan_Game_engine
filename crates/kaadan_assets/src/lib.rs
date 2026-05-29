//! Asset loading, caching, and (optional) hot-reload pipeline.
//!
//! Supports images and custom asset types via the [`AssetLoader`] trait.

mod group;
mod loader;
mod loaders;
mod resolver;
mod server;
mod storage;

#[cfg(feature = "hot-reload")]
mod hot_reload;

pub use group::AssetGroup;
pub use loader::AssetLoader;
pub use loaders::{ImageAsset, ImageLoader};
pub use resolver::{AssetResolver, FilesystemResolver};
pub use server::AssetServer;
pub use storage::AssetStorage;

#[cfg(feature = "hot-reload")]
pub use hot_reload::HotReloader;

/// Lifecycle state of an asset tracked by the [`AssetServer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Queued,
    Loading,
    Loaded,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_insert_get() {
        let mut storage = AssetStorage::<String>::new();
        let handle = storage.insert("hello".to_string());
        assert_eq!(storage.get(handle).unwrap(), "hello");
        assert_eq!(storage.len(), 1);
        assert_eq!(storage.state(handle), Some(LoadState::Loaded));
    }

    #[test]
    fn storage_path_dedup() {
        let mut storage = AssetStorage::<u32>::new();
        let h1 = storage.insert_with_path("test.png", 42);
        let h2 = storage.handle_for_path("test.png");
        assert_eq!(h2, Some(h1));
    }

    #[test]
    fn storage_remove() {
        let mut storage = AssetStorage::<u32>::new();
        let h = storage.insert(99);
        assert_eq!(storage.remove(h), Some(99));
        assert!(storage.get(h).is_none());
        assert_eq!(storage.len(), 0);
    }

    /// In-memory resolver returning a single PNG for any path.
    struct PngResolver {
        bytes: Vec<u8>,
    }

    impl AssetResolver for PngResolver {
        fn read_bytes(&self, _path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError> {
            Ok(self.bytes.clone())
        }
        fn exists(&self, _path: &str) -> bool {
            true
        }
    }

    fn make_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn server_loads_image_and_dedups() {
        let mut server = AssetServer::new(PngResolver {
            bytes: make_png(4, 2),
        });
        let handle = server.load("sprite.png", &ImageLoader);
        assert!(server.is_loaded(handle));
        let img = server.get(handle).expect("image present");
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(img.pixels.len(), 4 * 2 * 4);

        // Re-loading the same path returns the same handle.
        let again = server.load("sprite.png", &ImageLoader);
        assert!(handle == again);
    }

    #[test]
    fn server_reports_failed_load() {
        struct FailResolver;
        impl AssetResolver for FailResolver {
            fn read_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError> {
                Err(kaadan_core::KaadanError::AssetNotFound(path.to_string()))
            }
            fn exists(&self, _path: &str) -> bool {
                false
            }
        }
        let mut server = AssetServer::new(FailResolver);
        let handle = server.load("missing.png", &ImageLoader);
        assert_eq!(server.load_state(handle), Some(LoadState::Failed));
        assert!(!server.is_loaded(handle));
    }

    #[test]
    fn asset_group_progress() {
        let mut server = AssetServer::new(PngResolver {
            bytes: make_png(2, 2),
        });
        let good = server.load("a.png", &ImageLoader);
        let bad = server.load("b.png", &FailLoader);

        let mut group = AssetGroup::new();
        group.add(good);
        group.add(bad);
        assert_eq!(group.progress(&server), 0.5);
        assert!(!group.is_complete(&server));
    }

    // A loader whose output type matches ImageAsset but always fails, used to
    // exercise the Failed path within the same storage type.
    struct FailLoader;
    impl AssetLoader for FailLoader {
        type Output = ImageAsset;
        fn load(&self, _bytes: &[u8], path: &str) -> Result<ImageAsset, kaadan_core::KaadanError> {
            Err(kaadan_core::KaadanError::AssetLoad {
                path: path.to_string(),
                reason: "intentional".into(),
            })
        }
        fn extensions(&self) -> &[&str] {
            &["png"]
        }
    }
}
