use kaadan_ecs::{Component, Entity, World};

/// Type-erased component operations keyed by a stable string name.
///
/// This decouples editor / tooling code (which works in terms of component
/// *names*, e.g. from a serialized scene or a UI dropdown) from the concrete
/// generic component types. Each registered entry stores function pointers that
/// monomorphize the relevant `World` operation for one type `T`.
struct ComponentEntry {
    name: &'static str,
    has: fn(&World, Entity) -> bool,
    remove: fn(&mut World, Entity),
    insert_default: Option<fn(&mut World, Entity)>,
}

/// Registry mapping component names to type-erased component operations.
///
/// Unblocks the future editor: it can ask "does entity E have component named
/// `Mesh3D`?", remove it, or insert a default-constructed instance, all without
/// knowing the concrete type at the call site.
#[derive(Default)]
pub struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a component type `T` under `name` with `has`/`remove` support.
    pub fn register<T: Component>(&mut self, name: &'static str) {
        self.entries.push(ComponentEntry {
            name,
            has: |world, entity| world.get::<T>(entity).is_ok(),
            remove: |world, entity| {
                // Ignore the error: removing an absent component is a no-op.
                let _ = world.inner_mut().remove_one::<T>(entity);
            },
            insert_default: None,
        });
    }

    /// Register a component type `T: Default`, additionally enabling
    /// [`insert_default`](Self::insert_default).
    pub fn register_default<T: Component + Default>(&mut self, name: &'static str) {
        self.entries.push(ComponentEntry {
            name,
            has: |world, entity| world.get::<T>(entity).is_ok(),
            remove: |world, entity| {
                let _ = world.inner_mut().remove_one::<T>(entity);
            },
            insert_default: Some(|world, entity| {
                let _ = world.inner_mut().insert_one(entity, T::default());
            }),
        });
    }

    /// Iterate over the registered component names.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.name)
    }

    fn find(&self, name: &str) -> Option<&ComponentEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Whether `entity` has the named component.
    ///
    /// Returns `None` if `name` is not registered.
    pub fn has(&self, world: &World, entity: Entity, name: &str) -> Option<bool> {
        self.find(name).map(|e| (e.has)(world, entity))
    }

    /// Remove the named component from `entity`.
    ///
    /// Returns `true` if the name was registered (the removal ran; removing an
    /// absent component is a harmless no-op), `false` if `name` is unknown.
    pub fn remove(&self, world: &mut World, entity: Entity, name: &str) -> bool {
        match self.find(name) {
            Some(entry) => {
                (entry.remove)(world, entity);
                true
            }
            None => false,
        }
    }

    /// Insert a default-constructed instance of the named component onto
    /// `entity`.
    ///
    /// Returns `true` if the name was registered *and* was registered with
    /// [`register_default`](Self::register_default); `false` otherwise.
    pub fn insert_default(&self, world: &mut World, entity: Entity, name: &str) -> bool {
        match self.find(name).and_then(|e| e.insert_default) {
            Some(f) => {
                f(world, entity);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, PartialEq, Debug)]
    struct Health(u32);

    #[derive(Default)]
    struct Tag;

    #[test]
    fn register_has_remove_insert_default() {
        let mut registry = ComponentRegistry::new();
        registry.register_default::<Health>("Health");
        registry.register::<Tag>("Tag"); // no default registered

        let mut world = World::new();
        let e = world.spawn((Health(42),));

        // names() reports both registrations.
        let names: Vec<_> = registry.names().collect();
        assert!(names.contains(&"Health"));
        assert!(names.contains(&"Tag"));

        // has
        assert_eq!(registry.has(&world, e, "Health"), Some(true));
        assert_eq!(registry.has(&world, e, "Tag"), Some(false));
        assert_eq!(registry.has(&world, e, "Unknown"), None);

        // remove
        assert!(registry.remove(&mut world, e, "Health"));
        assert_eq!(registry.has(&world, e, "Health"), Some(false));
        // removing again / removing absent is a no-op that still returns true
        assert!(registry.remove(&mut world, e, "Health"));
        // unknown name -> false
        assert!(!registry.remove(&mut world, e, "Unknown"));

        // insert_default
        assert!(registry.insert_default(&mut world, e, "Health"));
        assert_eq!(registry.has(&world, e, "Health"), Some(true));
        assert_eq!(*world.get::<Health>(e).unwrap(), Health::default());
        // "Tag" was registered without a default ctor.
        assert!(!registry.insert_default(&mut world, e, "Tag"));
        // unknown name
        assert!(!registry.insert_default(&mut world, e, "Unknown"));
    }
}
