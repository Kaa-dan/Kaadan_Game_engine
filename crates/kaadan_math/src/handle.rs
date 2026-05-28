use std::marker::PhantomData;

/// A type-safe handle to a resource with generational index.
/// Prevents use-after-free: if the slot is reused, the generation won't match.
#[derive(Debug)]
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<T>,
}

// Manual impls to avoid requiring T: Clone/Copy/etc.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> Handle<T> {
    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}

/// Entry in the allocator's slot array.
struct Slot {
    generation: u32,
    is_live: bool,
}

/// Allocates and validates Handle<T> instances.
pub struct HandleAllocator<T> {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
    _marker: PhantomData<T>,
}

impl<T> Default for HandleAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HandleAllocator<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Allocate a new handle.
    pub fn allocate(&mut self) -> Handle<T> {
        if let Some(index) = self.free_list.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation += 1;
            slot.is_live = true;
            Handle {
                index,
                generation: slot.generation,
                _marker: PhantomData,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                is_live: true,
            });
            Handle {
                index,
                generation: 0,
                _marker: PhantomData,
            }
        }
    }

    /// Free a handle's slot for reuse. Returns true if the handle was valid.
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        if self.is_valid(handle) {
            self.slots[handle.index as usize].is_live = false;
            self.free_list.push(handle.index);
            true
        } else {
            false
        }
    }

    /// Check if a handle still refers to a live resource.
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.slots
            .get(handle.index as usize)
            .is_some_and(|slot| slot.is_live && slot.generation == handle.generation)
    }

    /// Number of currently live handles.
    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_live).count()
    }
}
