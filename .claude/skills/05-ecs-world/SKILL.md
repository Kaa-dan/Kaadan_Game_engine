# 05 — ECS World

## Description
Implement the Entity Component System at the engine's heart. Wraps `hecs` for archetype-based component storage. Defines `World`, entity lifecycle, query iteration, system scheduling, and the central `App` struct that wires ECS to the platform event loop.

## Phase
3 — ECS & Sprites

## Prerequisites
- Skill 02 (`02-math-and-core-types`) — `Transform`, `Vec3` used as components

## Complexity
Medium — ECS concepts are well-documented; hecs does the heavy lifting

## Architecture Decisions

### Why hecs (not custom ECS)?
- Minimal, well-tested archetype ECS — ~2000 lines of code
- No proc macros, no global state, no runtime registration
- Perfect for wrapping: we add scheduling, `App`, and plugins on top
- If we outgrow it, replacing the storage layer is feasible since the API wraps it

### Why archetype ECS over other patterns?
- **Archetype ECS**: Entities with the same component set are stored together in contiguous arrays → cache-friendly iteration
- **Sparse set ECS** (like EnTT): Better for frequent component add/remove but worse cache locality for queries
- For a mobile game engine, iteration speed matters more than component churn

### App struct design
- `App` is the top-level orchestrator: owns `World`, `Resources`, system schedule
- `Resources` are shared singleton data (not per-entity): `Time`, `InputState`, `AssetServer`
- Systems are functions that receive `&World` and `&Resources`, run each frame in order
- Plugins group related systems and resources for modularity

## Step-by-Step Implementation

### 1. Crate Setup

```toml
# crates/kaadan_ecs/Cargo.toml
[package]
name = "kaadan_ecs"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
hecs = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
rayon = { workspace = true }
```

### 2. World Wrapper

```rust
// crates/kaadan_ecs/src/world.rs
pub use hecs::{Entity, Component, Query, QueryBorrow, Ref, RefMut};

/// Engine's world — wraps hecs::World with convenience methods.
pub struct World {
    inner: hecs::World,
}

impl World {
    pub fn new() -> Self {
        Self { inner: hecs::World::new() }
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
    pub fn get_mut<T: Component>(&self, entity: Entity) -> Result<RefMut<'_, T>, hecs::ComponentError> {
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
```

### 3. Resources (Typed Singleton Storage)

```rust
// crates/kaadan_ecs/src/resources.rs
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Type-erased storage for singleton resources (Time, InputState, etc.)
pub struct Resources {
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Resources {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn insert<T: 'static>(&mut self, resource: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(resource));
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map.remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast().ok())
            .map(|b| *b)
    }
}
```

### 4. System Scheduling

```rust
// crates/kaadan_ecs/src/schedule.rs

/// A system is a function that operates on the world and resources.
pub type SystemFn = Box<dyn FnMut(&mut World, &mut Resources)>;

/// Named system for debugging and ordering.
struct SystemEntry {
    name: String,
    system: SystemFn,
}

/// Ordered list of systems run each frame.
pub struct Schedule {
    systems: Vec<SystemEntry>,
}

impl Schedule {
    pub fn new() -> Self {
        Self { systems: Vec::new() }
    }

    /// Add a system with a name.
    pub fn add_system(&mut self, name: impl Into<String>, system: impl FnMut(&mut World, &mut Resources) + 'static) {
        self.systems.push(SystemEntry {
            name: name.into(),
            system: Box::new(system),
        });
    }

    /// Run all systems in order.
    pub fn run(&mut self, world: &mut World, resources: &mut Resources) {
        for entry in &mut self.systems {
            tracing::trace!("Running system: {}", entry.name);
            (entry.system)(world, resources);
        }
    }
}
```

### 5. Time Resource

```rust
// crates/kaadan_ecs/src/time.rs
use std::time::{Duration, Instant};

/// Frame timing resource — inserted by App, updated each frame.
pub struct Time {
    startup: Instant,
    last_frame: Instant,
    delta: Duration,
    frame_count: u64,
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            startup: now,
            last_frame: now,
            delta: Duration::ZERO,
            frame_count: 0,
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last_frame;
        self.last_frame = now;
        self.frame_count += 1;
    }

    /// Delta time in seconds (f32 for game math).
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn elapsed(&self) -> Duration {
        self.last_frame - self.startup
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}
```

### 6. App Struct

```rust
// crates/kaadan_ecs/src/app.rs
use crate::{World, Resources, Schedule, Time};

/// Plugin trait for modular system registration.
pub trait Plugin {
    fn build(&self, app: &mut App);
}

/// Central engine orchestrator.
pub struct App {
    pub world: World,
    pub resources: Resources,
    pub schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
}

impl App {
    pub fn new() -> Self {
        let mut resources = Resources::new();
        resources.insert(Time::new());

        Self {
            world: World::new(),
            resources,
            schedule: Schedule::new(),
            plugins: Vec::new(),
        }
    }

    pub fn add_system(&mut self, name: impl Into<String>, system: impl FnMut(&mut World, &mut Resources) + 'static) -> &mut Self {
        self.schedule.add_system(name, system);
        self
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin + 'static) -> &mut Self {
        plugin.build(self);
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn insert_resource<T: 'static>(&mut self, resource: T) -> &mut Self {
        self.resources.insert(resource);
        self
    }

    /// Run one tick: update time, run all systems.
    pub fn tick(&mut self) {
        if let Some(time) = self.resources.get_mut::<Time>() {
            time.update();
        }
        self.schedule.run(&mut self.world, &mut self.resources);
    }
}
```

### 7. Example: Bouncing Entities

```rust
// examples/ecs_demo.rs
use kaadan_ecs::*;
use kaadan_math::{Transform, Vec3};

struct Velocity(Vec3);

fn movement_system(world: &mut World, resources: &mut Resources) {
    let dt = resources.get::<Time>().unwrap().delta_seconds();
    for (_entity, (transform, vel)) in world.query::<(&mut Transform, &Velocity)>().iter() {
        transform.position += vel.0 * dt;
    }
}

fn main() {
    let mut app = App::new();
    app.add_system("movement", movement_system);

    // Spawn 10,000 entities with Transform + Velocity
    for i in 0..10_000 {
        app.world.spawn((
            Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)),
            Velocity(Vec3::new(0.0, 1.0, 0.0)),
        ));
    }

    // Simulate 60 frames
    for _ in 0..60 {
        app.tick();
    }

    println!("Entities: {}", app.world.len());
}
```

## Deliverables Checklist

- [ ] `World` wrapper with `spawn()`, `despawn()`, `get::<T>()`, `query::<(&A, &mut B)>()`
- [ ] `Resources` typed singleton storage
- [ ] `Schedule` with ordered system execution
- [ ] `Time` resource tracking delta time and frame count
- [ ] `App` struct with `add_system()`, `add_plugin()`, `tick()`
- [ ] `Plugin` trait for modular registration
- [ ] Example: query iteration over 10,000 entities at interactive frame rates
- [ ] Unit tests for World operations, Resource storage, Handle integration
- [ ] `cargo bench` or timing test showing query performance

## Common Pitfalls

1. **hecs query syntax** — `world.query::<(&A, &mut B)>()` returns a `QueryBorrow`. You must call `.iter()` on it and iterate in a `for` loop. Don't hold the `QueryBorrow` across mutation points.

2. **Borrow checker with World** — You can't query `&mut A` while also holding a `Ref<B>` from `world.get()`. Use queries that request all needed components at once.

3. **System ordering matters** — Systems run in the order added. Movement before collision detection, collision before rendering. Document the expected order.

4. **Resources are not components** — Don't store per-entity data in Resources. Resources are for singletons (Time, InputState, AssetServer). Per-entity data goes in components.

5. **Don't over-split components** — Each unique combination of components creates an archetype. Having 20 tiny components leads to archetype explosion. Group related data.

6. **Entity is just an ID** — `hecs::Entity` is a lightweight handle (u64). You can store it, pass it around, compare it. But it might be despawned — always check with `is_alive()` or handle the error.

## References

- [hecs docs](https://docs.rs/hecs/latest/hecs/)
- [hecs examples](https://github.com/Ralith/hecs/tree/main/examples)
- [ECS FAQ](https://github.com/SanderMertens/ecs-faq)
- [Bevy ECS architecture](https://bevyengine.org/learn/quick-start/getting-started/ecs/) (inspiration, different implementation)
