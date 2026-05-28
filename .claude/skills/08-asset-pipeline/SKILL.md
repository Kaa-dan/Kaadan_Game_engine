# 08 — Asset Pipeline

## Description
Async asset loading with format-agnostic handles, caching, hot-reload (dev builds), and platform-aware resolvers. Unified `AssetServer` handles textures, audio, fonts, shaders across filesystem (desktop), Android `AssetManager`, and iOS bundles.

## Phase
4 — Content Pipeline

## Prerequisites
- Skill 03 (`03-platform-abstraction`) — platform-aware file access
- Skill 05 (`05-ecs-world`) — ECS Resources, Handle<T> integration
- Skill 06 (`06-2d-sprite-rendering`) — Texture type as a loaded asset

## Complexity
High — async loading, platform-specific storage, cache management

## Architecture Decisions

### Why async loading?
- Synchronous file I/O blocks the render thread → frame hitches
- Mobile storage is slow (eMMC/UFS) compared to desktop NVMe
- Loading a texture takes 5-50ms; at 60fps you have 16ms per frame
- Async loading returns a `Handle<T>` immediately; the asset appears when ready

### Platform-aware resolvers
- **Desktop:** Read from filesystem (`assets/` directory)
- **Android:** Assets are inside the APK (a zip). Must use `ndk::asset::AssetManager`, not `std::fs`
- **iOS:** Assets are in the app bundle. Use `NSBundle::mainBundle::pathForResource`
- The `AssetResolver` trait abstracts this; `AssetServer` doesn't know which platform it's on

### Handle-based architecture
- `load::<Texture>("sprites/player.png")` returns `Handle<Texture>` immediately
- The handle is valid even before loading finishes
- Systems check `asset_server.is_loaded(handle)` or use the asset if ready
- Allows loading screens: track a group of handles, show progress

### Hot-reload (dev only)
- `notify` crate watches the `assets/` directory for file changes
- On change, the asset is re-loaded and existing handles automatically point to the new version
- Only enabled in dev builds (`#[cfg(debug_assertions)]`)
- Critical for iteration speed: change a texture, see it update instantly

## Step-by-Step Implementation

### 1. Crate Setup

```toml
# crates/kaadan_assets/Cargo.toml
[package]
name = "kaadan_assets"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
tracing = { workspace = true }
tokio = { workspace = true }

[target.'cfg(not(target_os = "android"))'.dependencies]
notify = { workspace = true, optional = true }

[target.'cfg(target_os = "android")'.dependencies]
ndk = { workspace = true }

[features]
default = []
hot-reload = ["notify"]
```

### 2. AssetResolver Trait

```rust
// crates/kaadan_assets/src/resolver.rs

/// Platform-specific asset byte loading.
pub trait AssetResolver: Send + Sync {
    /// Read raw bytes from an asset path.
    /// Path is relative to the assets root (e.g., "textures/player.png").
    fn load_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError>;

    /// Check if an asset exists.
    fn exists(&self, path: &str) -> bool;
}

/// Desktop resolver — reads from filesystem.
pub struct FilesystemResolver {
    root: std::path::PathBuf,
}

impl FilesystemResolver {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetResolver for FilesystemResolver {
    fn load_bytes(&self, path: &str) -> Result<Vec<u8>, kaadan_core::KaadanError> {
        let full_path = self.root.join(path);
        std::fs::read(&full_path).map_err(|e| kaadan_core::KaadanError::AssetLoad {
            path: path.to_string(),
            reason: e.to_string(),
        })
    }

    fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

/// Android resolver — reads from APK AssetManager.
#[cfg(target_os = "android")]
pub struct AndroidResolver {
    // Holds reference to ndk AssetManager
    // Initialized from android_activity::AndroidApp
}

/// iOS resolver — reads from NSBundle.
#[cfg(target_os = "ios")]
pub struct BundleResolver {
    bundle_path: std::path::PathBuf,
}
```

### 3. Asset Loader Trait

```rust
// crates/kaadan_assets/src/loader.rs

/// Type-specific asset loading. Converts raw bytes into the final asset type.
pub trait AssetLoader: Send + Sync {
    type Asset: Send + Sync + 'static;

    /// File extensions this loader handles.
    fn extensions(&self) -> &[&str];

    /// Load an asset from raw bytes.
    fn load(&self, bytes: &[u8], path: &str) -> Result<Self::Asset, kaadan_core::KaadanError>;
}

// Example: TextureLoader (implemented in kaadan_renderer, registered with AssetServer)
// Example: AudioLoader (implemented in kaadan_audio)
// Example: FontLoader (implemented in kaadan_ui)
```

### 4. AssetServer

```rust
// crates/kaadan_assets/src/server.rs
use kaadan_math::{Handle, HandleAllocator};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Load state of an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// Load requested but not started
    Queued,
    /// Currently loading (async)
    Loading,
    /// Successfully loaded
    Loaded,
    /// Load failed
    Failed,
}

/// Central asset management. Inserted as an ECS Resource.
pub struct AssetServer {
    resolver: Arc<dyn AssetResolver>,
    /// Type-erased storage: TypeId → AssetStorage<T>
    storages: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

/// Typed storage for one asset type.
pub struct AssetStorage<T: Send + Sync + 'static> {
    allocator: HandleAllocator<T>,
    assets: HashMap<u32, AssetEntry<T>>,
    path_to_handle: HashMap<String, Handle<T>>,
}

struct AssetEntry<T> {
    asset: Option<T>,
    state: LoadState,
    ref_count: u32,
    path: String,
}

impl AssetServer {
    pub fn new(resolver: impl AssetResolver + 'static) -> Self {
        Self {
            resolver: Arc::new(resolver),
            storages: HashMap::new(),
        }
    }

    /// Request an asset to be loaded. Returns a handle immediately.
    /// The asset may not be ready yet — check with `is_loaded()`.
    pub fn load<T: Send + Sync + 'static>(
        &mut self,
        path: &str,
        loader: &dyn AssetLoader<Asset = T>,
    ) -> Handle<T> {
        let storage = self.storage_mut::<T>();

        // Check if already loaded/loading
        if let Some(handle) = storage.path_to_handle.get(path) {
            if let Some(entry) = storage.assets.get_mut(&handle.index()) {
                entry.ref_count += 1;
            }
            return *handle;
        }

        // Allocate handle, queue load
        let handle = storage.allocator.allocate();
        storage.path_to_handle.insert(path.to_string(), handle);
        storage.assets.insert(handle.index(), AssetEntry {
            asset: None,
            state: LoadState::Queued,
            ref_count: 1,
            path: path.to_string(),
        });

        // Synchronous load for now; async in production
        match self.resolver.load_bytes(path) {
            Ok(bytes) => {
                match loader.load(&bytes, path) {
                    Ok(asset) => {
                        let storage = self.storage_mut::<T>();
                        if let Some(entry) = storage.assets.get_mut(&handle.index()) {
                            entry.asset = Some(asset);
                            entry.state = LoadState::Loaded;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load asset '{}': {}", path, e);
                        let storage = self.storage_mut::<T>();
                        if let Some(entry) = storage.assets.get_mut(&handle.index()) {
                            entry.state = LoadState::Failed;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to read asset '{}': {}", path, e);
                let storage = self.storage_mut::<T>();
                if let Some(entry) = storage.assets.get_mut(&handle.index()) {
                    entry.state = LoadState::Failed;
                }
            }
        }

        handle
    }

    /// Get a loaded asset by handle. Returns None if not yet loaded.
    pub fn get<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> Option<&T> {
        self.storage::<T>()
            .and_then(|s| s.assets.get(&handle.index()))
            .and_then(|e| e.asset.as_ref())
    }

    /// Check load state.
    pub fn load_state<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> LoadState {
        self.storage::<T>()
            .and_then(|s| s.assets.get(&handle.index()))
            .map_or(LoadState::Failed, |e| e.state)
    }

    /// Check if an asset is loaded and ready.
    pub fn is_loaded<T: Send + Sync + 'static>(&self, handle: Handle<T>) -> bool {
        self.load_state(handle) == LoadState::Loaded
    }

    fn storage<T: Send + Sync + 'static>(&self) -> Option<&AssetStorage<T>> {
        self.storages.get(&TypeId::of::<T>())?.downcast_ref()
    }

    fn storage_mut<T: Send + Sync + 'static>(&mut self) -> &mut AssetStorage<T> {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(AssetStorage::<T> {
                allocator: HandleAllocator::new(),
                assets: HashMap::new(),
                path_to_handle: HashMap::new(),
            }))
            .downcast_mut()
            .unwrap()
    }
}
```

### 5. Asset Groups (Loading Screens)

```rust
// crates/kaadan_assets/src/group.rs

/// Tracks loading progress for a group of assets.
pub struct AssetGroup {
    handles: Vec<(std::any::TypeId, u32)>, // (type_id, handle_index)
    total: usize,
}

impl AssetGroup {
    pub fn new() -> Self {
        Self { handles: Vec::new(), total: 0 }
    }

    pub fn add<T: Send + Sync + 'static>(&mut self, handle: Handle<T>) {
        self.handles.push((TypeId::of::<T>(), handle.index()));
        self.total += 1;
    }

    /// Returns progress as 0.0–1.0.
    pub fn progress(&self, server: &AssetServer) -> f32 {
        if self.total == 0 { return 1.0; }
        let loaded = self.handles.iter()
            .filter(|(type_id, index)| {
                // Check if loaded (simplified — real impl checks type-erased storage)
                true
            })
            .count();
        loaded as f32 / self.total as f32
    }

    pub fn is_complete(&self, server: &AssetServer) -> bool {
        self.progress(server) >= 1.0
    }
}
```

### 6. Hot-Reload (Dev Builds)

```rust
// crates/kaadan_assets/src/hot_reload.rs
#[cfg(feature = "hot-reload")]
mod watcher {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
    use std::sync::mpsc;

    pub struct HotReloader {
        _watcher: RecommendedWatcher,
        rx: mpsc::Receiver<notify::Result<Event>>,
    }

    impl HotReloader {
        pub fn new(watch_path: &std::path::Path) -> Result<Self, notify::Error> {
            let (tx, rx) = mpsc::channel();
            let mut watcher = notify::recommended_watcher(move |res| {
                let _ = tx.send(res);
            })?;
            watcher.watch(watch_path, RecursiveMode::Recursive)?;
            Ok(Self { _watcher: watcher, rx })
        }

        /// Poll for changed files. Call once per frame.
        pub fn poll_changes(&self) -> Vec<String> {
            let mut changed = Vec::new();
            while let Ok(Ok(event)) = self.rx.try_recv() {
                for path in event.paths {
                    if let Some(s) = path.to_str() {
                        changed.push(s.to_string());
                    }
                }
            }
            changed
        }
    }
}
```

## Deliverables Checklist

- [ ] `AssetServer` with `load::<T>(path)` returning `Handle<T>`
- [ ] `AssetResolver` trait with `FilesystemResolver` (desktop)
- [ ] Android `AssetManager` resolver scaffold
- [ ] iOS `NSBundle` resolver scaffold
- [ ] `AssetLoader` trait for type-specific loading
- [ ] `AssetStorage<T>` with reference counting
- [ ] `LoadState` tracking: Queued → Loading → Loaded/Failed
- [ ] `AssetGroup` for tracking loading progress
- [ ] Hot-reload via `notify` (desktop dev builds, feature-gated)
- [ ] Integration test: load a PNG texture through the pipeline

## Common Pitfalls

1. **Android `AssetManager` is NOT thread-safe** — Use it only from the main thread, or wrap access behind a channel/mutex. Loading on a background thread requires reading bytes on the main thread first.

2. **Don't use `std::fs` on Android** — Files inside the APK are not on the filesystem. This is the #1 mobile porting mistake.

3. **Reference counting vs. garbage collection** — Explicitly track ref counts. When ref count hits 0, the asset can be unloaded. On mobile, memory is precious — don't hold assets forever.

4. **Hot-reload must not crash** — If a file is saved mid-write, the bytes may be corrupt. Always handle decode errors gracefully during hot-reload.

5. **Asset path normalization** — Normalize paths (forward slashes, no leading slash, lowercase) to avoid duplicates: `"Textures/Player.PNG"` and `"textures/player.png"` should be the same asset.

6. **Large asset loading blocks the main thread** — Even with async, the final "insert into storage" step happens on the main thread. For large assets (3D models), stream the processing across multiple frames.

## References

- [Android AssetManager NDK](https://developer.android.com/ndk/reference/group/asset)
- [notify crate docs](https://docs.rs/notify/latest/notify/)
- [Bevy asset system](https://bevyengine.org/learn/quick-start/getting-started/assets/) (inspiration)
- [tokio docs](https://docs.rs/tokio/latest/tokio/)
