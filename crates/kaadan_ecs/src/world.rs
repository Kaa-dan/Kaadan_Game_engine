pub use hecs::{Component, Entity, Query, QueryBorrow, Ref, RefMut};

/// Engine's world — wraps hecs::World with convenience methods.
pub struct World {
    inner: hecs::World,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            inner: hecs::World::new(),
        }
    }

    /// Spawn an entity with components. Uses hecs tuple bundle syntax.
    pub fn spawn(&mut self, components: impl hecs::DynamicBundle) -> Entity {
        self.inner.spawn(components)
    }

    /// Despawn an entity, removing all its components.
    pub fn despawn(&mut self, entity: Entity) -> Result<(), hecs::NoSuchEntity> {
        self.inner.despawn(entity)
    }

    /// Get a component reference.
    pub fn get<T: Component>(&self, entity: Entity) -> Result<Ref<'_, T>, hecs::ComponentError> {
        self.inner.get::<&T>(entity)
    }

    /// Get a mutable component reference.
    pub fn get_mut<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<RefMut<'_, T>, hecs::ComponentError> {
        self.inner.get::<&mut T>(entity)
    }

    /// Query entities with matching component sets.
    /// Usage: `world.query::<(&Position, &mut Velocity)>()`
    pub fn query<Q: hecs::Query>(&self) -> QueryBorrow<'_, Q> {
        self.inner.query::<Q>()
    }

    /// Check if an entity is alive.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.inner.contains(entity)
    }

    /// Number of live entities.
    pub fn len(&self) -> u32 {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Access the inner hecs::World for advanced operations.
    pub fn inner(&self) -> &hecs::World {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut hecs::World {
        &mut self.inner
    }
}
