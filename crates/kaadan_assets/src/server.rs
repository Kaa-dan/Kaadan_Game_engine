use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use kaadan_math::Handle;

use crate::loader::AssetLoader;
use crate::resolver::AssetResolver;
use crate::storage::AssetStorage;
use crate::LoadState;

/// Central asset manager — resolves paths, deduplicates, runs type-specific
/// loaders, and tracks per-asset load state in type-erased storages.
pub struct AssetServer {
    resolver: Arc<dyn AssetResolver>,
    storages: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl AssetServer {
    pub fn new(resolver: impl AssetResolver + 'static) -> Self {
        Self {
            resolver: Arc::new(resolver),
            storages: HashMap::new(),
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

    /// Load an asset of the loader's output type. Returns a handle that is
    /// always valid; query [`AssetServer::load_state`] to see if it succeeded.
    /// Re-loading a previously requested path returns the existing handle.
    pub fn load<L: AssetLoader>(&mut self, path: &str, loader: &L) -> Handle<L::Output> {
        if let Some(existing) = self
            .storage::<L::Output>()
            .and_then(|s| s.handle_for_path(path))
        {
            return existing;
        }

        let resolver = Arc::clone(&self.resolver);
        let result = resolver
            .read_bytes(path)
            .and_then(|bytes| loader.load(&bytes, path));

        let storage = self.storage_mut::<L::Output>();
        match result {
            Ok(asset) => storage.insert_with_path(path, asset),
            Err(err) => {
                tracing::error!("failed to load asset '{path}': {err}");
                storage.insert_failed(path)
            }
        }
    }

    /// Get a loaded asset by handle.
    pub fn get<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> Option<&T> {
        self.storage::<T>()?.get(handle)
    }

    /// Current load state for a handle.
    pub fn load_state<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> Option<LoadState> {
        self.storage::<T>()?.state(handle)
    }

    /// True once the asset behind a handle is fully loaded.
    pub fn is_loaded<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> bool {
        self.load_state::<T>(handle) == Some(LoadState::Loaded)
    }

    fn storage<T: Send + Sync + 'static>(&self) -> Option<&AssetStorage<T>> {
        self.storages
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<AssetStorage<T>>())
    }

    fn storage_mut<T: Send + Sync + 'static>(&mut self) -> &mut AssetStorage<T> {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(AssetStorage::<T>::new()))
            .downcast_mut::<AssetStorage<T>>()
            .expect("AssetStorage type mismatch for TypeId")
    }
}
