use kaadan_math::{Handle, HandleAllocator};
use std::collections::HashMap;

/// Stores loaded assets of a specific type, indexed by Handle.
pub struct AssetStorage<T> {
    allocator: HandleAllocator<T>,
    assets: HashMap<u32, T>,
    path_to_handle: HashMap<String, Handle<T>>,
}

impl<T> Default for AssetStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AssetStorage<T> {
    pub fn new() -> Self {
        Self {
            allocator: HandleAllocator::new(),
            assets: HashMap::new(),
            path_to_handle: HashMap::new(),
        }
    }

    /// Insert an asset and return its handle.
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        let handle = self.allocator.allocate();
        self.assets.insert(handle.index(), asset);
        handle
    }

    /// Insert with a path for deduplication.
    pub fn insert_with_path(&mut self, path: impl Into<String>, asset: T) -> Handle<T> {
        let path = path.into();
        let handle = self.insert(asset);
        self.path_to_handle.insert(path, handle);
        handle
    }

    /// Get by handle.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        if self.allocator.is_valid(handle) {
            self.assets.get(&handle.index())
        } else {
            None
        }
    }

    /// Get mutable by handle.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        if self.allocator.is_valid(handle) {
            self.assets.get_mut(&handle.index())
        } else {
            None
        }
    }

    /// Look up a handle by path.
    pub fn handle_for_path(&self, path: &str) -> Option<Handle<T>> {
        self.path_to_handle.get(path).copied()
    }

    /// Remove an asset.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        if self.allocator.free(handle) {
            self.assets.remove(&handle.index())
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.allocator.live_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
