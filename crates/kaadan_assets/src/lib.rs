//! Asset loading, caching, and (optional) hot-reload pipeline.
//!
//! Supports images and custom asset types via the [`AssetLoader`] trait.

mod async_io;
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
pub use loaders::{AudioClip, AudioLoader, ImageAsset, ImageLoader};
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

    // ----- Test helpers for async / reload / audio -----

    /// Trivial loader that returns the bytes as a UTF-8 string, so reload tests
    /// can assert exact content with no decode fuzz.
    struct TextLoader;
    impl AssetLoader for TextLoader {
        type Output = String;
        fn load(&self, bytes: &[u8], path: &str) -> Result<String, kaadan_core::KaadanError> {
            String::from_utf8(bytes.to_vec()).map_err(|e| kaadan_core::KaadanError::AssetLoad {
                path: path.to_string(),
                reason: e.to_string(),
            })
        }
        fn extensions(&self) -> &[&str] {
            &["txt"]
        }
    }

    /// Create a unique temp directory for a test (avoids the `tempfile` dep).
    fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("kaadan_assets_{tag}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Spin `poll()` until the handle leaves Queued, with bounded retries.
    fn poll_until_settled<T: Send + Sync + 'static>(
        server: &mut AssetServer,
        handle: kaadan_math::Handle<T>,
    ) -> LoadState {
        for _ in 0..200 {
            server.poll();
            match server.load_state(handle) {
                Some(LoadState::Loaded) => return LoadState::Loaded,
                Some(LoadState::Failed) => return LoadState::Failed,
                _ => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        }
        panic!("async load did not settle within retry budget");
    }

    #[test]
    fn load_async_image_succeeds() {
        let dir = unique_temp_dir("async_ok");
        std::fs::write(dir.join("sprite.png"), make_png(8, 4)).unwrap();
        let mut server = AssetServer::new(FilesystemResolver::new(&dir));

        let handle = server.load_async("sprite.png", ImageLoader);
        // Returns immediately, still Queued (not yet polled).
        assert_eq!(server.load_state(handle), Some(LoadState::Queued));
        assert!(!server.is_loaded(handle));

        assert_eq!(poll_until_settled(&mut server, handle), LoadState::Loaded);
        let img = server.get(handle).expect("image present after poll");
        assert_eq!((img.width, img.height), (8, 4));

        // Re-requesting the same path returns the same handle.
        let again = server.load_async("sprite.png", ImageLoader);
        assert!(again == handle);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_async_missing_file_fails() {
        let dir = unique_temp_dir("async_fail");
        let mut server = AssetServer::new(FilesystemResolver::new(&dir));

        let handle = server.load_async("nope.png", ImageLoader);
        assert_eq!(server.load_state(handle), Some(LoadState::Queued));
        assert_eq!(poll_until_settled(&mut server, handle), LoadState::Failed);
        assert!(server.get(handle).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reload_path_replaces_asset() {
        let dir = unique_temp_dir("reload");
        let file = dir.join("note.txt");
        std::fs::write(&file, b"original").unwrap();
        let mut server = AssetServer::new(FilesystemResolver::new(&dir));

        // Load via async (which registers a reloader) and wait for it.
        let handle = server.load_async::<String, _>("note.txt", TextLoader);
        assert_eq!(poll_until_settled(&mut server, handle), LoadState::Loaded);
        assert_eq!(server.get(handle).map(String::as_str), Some("original"));
        assert!(server.is_reloadable("note.txt"));
        assert_eq!(server.reload_version("note.txt"), 0);

        // Change the file on disk and reload in place.
        std::fs::write(&file, b"updated contents").unwrap();
        assert!(server.reload_path("note.txt"));

        // Same handle, new contents, bumped version.
        assert_eq!(
            server.get(handle).map(String::as_str),
            Some("updated contents")
        );
        assert!(server.is_loaded(handle));
        assert_eq!(server.reload_version("note.txt"), 1);

        // Reloading an unknown path is a no-op returning false.
        assert!(!server.reload_path("ghost.txt"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn audio_loader_returns_bytes() {
        let raw = vec![1u8, 2, 3, 4, 5, 250];

        // Direct loader use.
        let clip = AudioLoader.load(&raw, "boom.wav").unwrap();
        assert_eq!(clip.bytes, raw);

        // Through the server.
        let mut server = AssetServer::new(PngResolver { bytes: raw.clone() });
        let handle = server.load("boom.wav", &AudioLoader);
        assert!(server.is_loaded(handle));
        assert_eq!(server.get(handle).unwrap().bytes, raw);

        // Extensions registered.
        assert!(AudioLoader.extensions().contains(&"ogg"));
        assert!(AudioLoader.extensions().contains(&"mp3"));
    }
}
