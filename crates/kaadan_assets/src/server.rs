use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use kaadan_math::Handle;

use crate::async_io::AssetWorker;
use crate::loader::AssetLoader;
use crate::resolver::AssetResolver;
use crate::storage::AssetStorage;
use crate::LoadState;

/// Re-runs a loader for a previously-loaded path, replacing the stored asset at
/// the same handle. Captured at load time; invoked by [`AssetServer::reload_path`].
type Reloader = Box<dyn FnMut(&mut AssetServer) -> Result<(), kaadan_core::KaadanError> + Send>;

/// Central asset manager — resolves paths, deduplicates, runs type-specific
/// loaders, and tracks per-asset load state in type-erased storages.
///
/// Supports synchronous loading ([`AssetServer::load`]), non-blocking background
/// loading ([`AssetServer::load_async`] + [`AssetServer::poll`]), and in-place
/// hot-reload ([`AssetServer::reload_path`]).
pub struct AssetServer {
    resolver: Arc<dyn AssetResolver>,
    storages: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    worker: AssetWorker,
    /// Reload closures keyed by path, recorded when a path is first loaded.
    reloaders: HashMap<String, Reloader>,
    /// Monotonic reload counter per path, bumped on each successful reload so
    /// consumers can detect that an asset changed underneath them.
    reload_versions: HashMap<String, u64>,
}

impl AssetServer {
    pub fn new(resolver: impl AssetResolver + 'static) -> Self {
        Self {
            resolver: Arc::new(resolver),
            storages: HashMap::new(),
            worker: AssetWorker::new(),
            reloaders: HashMap::new(),
            reload_versions: HashMap::new(),
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

    /// Begin loading an asset on a background thread and return immediately.
    ///
    /// The returned handle starts in [`LoadState::Queued`]; the worker thread
    /// reads the bytes and runs the loader, and a later [`AssetServer::poll`]
    /// transitions it to [`LoadState::Loaded`] or [`LoadState::Failed`].
    /// Re-loading a previously requested path returns the existing handle.
    ///
    /// The loader is consumed (it must be `Send + 'static`) and shared with a
    /// reload closure so the same path can later be hot-reloaded.
    pub fn load_async<T, L>(&mut self, path: &str, loader: L) -> Handle<T>
    where
        T: Send + Sync + 'static,
        L: AssetLoader<Output = T> + Send + Sync + 'static,
    {
        if let Some(existing) = self.storage::<T>().and_then(|s| s.handle_for_path(path)) {
            return existing;
        }

        let handle = self.storage_mut::<T>().reserve(path);
        let resolver = Arc::clone(&self.resolver);
        let loader = Arc::new(loader);
        let path_owned = path.to_string();

        // Record a reloader so this async-loaded path can be hot-reloaded later.
        self.record_reloader::<T, L>(path, Arc::clone(&loader));

        // Build the background job. It owns everything it needs and produces a
        // type-erased closure that routes the result back into typed storage.
        let job_resolver = resolver;
        let job_loader = loader;
        let job_path = path_owned;
        let job: crate::async_io::Job = Box::new(move || {
            let result = job_resolver
                .read_bytes(&job_path)
                .and_then(|bytes| job_loader.load(&bytes, &job_path));
            let completed: crate::async_io::Completed =
                Box::new(move |server: &mut AssetServer| {
                    let storage = server.storage_mut::<T>();
                    match result {
                        Ok(asset) => {
                            storage.fulfill(handle, asset);
                        }
                        Err(err) => {
                            tracing::error!("async load of '{job_path}' failed: {err}");
                            storage.set_state(handle, LoadState::Failed);
                        }
                    }
                });
            completed
        });

        // State stays Queued until `poll` applies the worker's result, which
        // transitions it to Loaded/Failed. (With a single result channel there
        // is no race-free way to observe a transient Loading on the main thread,
        // so we keep the handle Queued in-flight, as the trait permits.)
        self.worker.submit(job);
        handle
    }

    /// Drain finished background jobs and apply their results to storage.
    /// Call once per frame. Returns the number of jobs applied.
    pub fn poll(&mut self) -> usize {
        let completed = self.worker.drain_completed();
        let count = completed.len();
        for apply in completed {
            apply(self);
        }
        count
    }

    /// Alias for [`AssetServer::poll`], for callers that prefer `update`.
    pub fn update(&mut self) -> usize {
        self.poll()
    }

    /// Re-run the loader recorded for `path` and replace the stored asset in
    /// place (same handle), bumping its reload version. Returns true if a
    /// reloader was registered for the path and the reload succeeded.
    pub fn reload_path(&mut self, path: &str) -> bool {
        // Temporarily take the reloader out so we can pass `&mut self` to it.
        let Some(mut reloader) = self.reloaders.remove(path) else {
            return false;
        };
        let result = reloader(self);
        // Put it back for the next reload.
        self.reloaders.insert(path.to_string(), reloader);

        match result {
            Ok(()) => {
                *self.reload_versions.entry(path.to_string()).or_insert(0) += 1;
                true
            }
            Err(err) => {
                tracing::error!("reload of '{path}' failed: {err}");
                false
            }
        }
    }

    /// Number of times `path` has been successfully reloaded (0 if never).
    pub fn reload_version(&self, path: &str) -> u64 {
        self.reload_versions.get(path).copied().unwrap_or(0)
    }

    /// True if a reloader is registered for `path` (i.e. it can be hot-reloaded).
    pub fn is_reloadable(&self, path: &str) -> bool {
        self.reloaders.contains_key(path)
    }

    /// Apply file changes reported by a [`crate::HotReloader`], reloading any
    /// changed path that has a registered reloader. Returns the count reloaded.
    #[cfg(feature = "hot-reload")]
    pub fn process_changes(&mut self, reloader: &mut crate::HotReloader) -> usize {
        let mut reloaded = 0;
        for path in reloader.poll_changes() {
            if self.reload_path(&path) {
                reloaded += 1;
            }
        }
        reloaded
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

    /// Record (or replace) the reloader for a path. The closure clones the
    /// shared loader and resolver and re-fulfils the existing handle.
    fn record_reloader<T, L>(&mut self, path: &str, loader: Arc<L>)
    where
        T: Send + Sync + 'static,
        L: AssetLoader<Output = T> + Send + Sync + 'static,
    {
        let path_owned = path.to_string();
        let reloader: Reloader = Box::new(move |server: &mut AssetServer| {
            let handle = server
                .storage::<T>()
                .and_then(|s| s.handle_for_path(&path_owned))
                .ok_or_else(|| kaadan_core::KaadanError::AssetNotFound(path_owned.clone()))?;
            let resolver = Arc::clone(&server.resolver);
            let bytes = resolver.read_bytes(&path_owned)?;
            let asset = loader.load(&bytes, &path_owned)?;
            server.storage_mut::<T>().fulfill(handle, asset);
            Ok(())
        });
        self.reloaders.insert(path.to_string(), reloader);
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
