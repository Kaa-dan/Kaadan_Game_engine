//! Asynchronous asset loading, caching, and hot-reload pipeline.
//!
//! Supports images, audio, scenes, and custom asset types.

mod loader;
mod resolver;
mod server;
mod storage;

pub use loader::AssetLoader;
pub use resolver::{AssetResolver, FilesystemResolver};
pub use server::AssetServer;
pub use storage::AssetStorage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_insert_get() {
        let mut storage = AssetStorage::<String>::new();
        let handle = storage.insert("hello".to_string());
        assert_eq!(storage.get(handle).unwrap(), "hello");
        assert_eq!(storage.len(), 1);
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
}
