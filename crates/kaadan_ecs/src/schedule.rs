use crate::resources::Resources;
use crate::world::World;

/// A system is a function that operates on the world and resources.
pub type SystemFn = Box<dyn FnMut(&mut World, &mut Resources)>;

/// Execution stages, run in this order every frame.
///
/// `FixedUpdate` is special: the [`App`](crate::App) main loop runs it zero or
/// more times per frame based on the accumulated [`Time`](crate::Time) so that
/// simulation (e.g. physics) advances at a fixed rate independent of framerate.
/// All other stages run exactly once per frame.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(usize)]
pub enum Stage {
    First = 0,
    PreUpdate = 1,
    FixedUpdate = 2,
    Update = 3,
    PostUpdate = 4,
    Render = 5,
}

impl Stage {
    pub const COUNT: usize = 6;
    /// Stages in execution order.
    pub const ORDER: [Stage; Self::COUNT] = [
        Stage::First,
        Stage::PreUpdate,
        Stage::FixedUpdate,
        Stage::Update,
        Stage::PostUpdate,
        Stage::Render,
    ];
}

/// Named system for debugging and ordering.
struct SystemEntry {
    name: String,
    system: SystemFn,
}

/// Systems grouped by [`Stage`]; within a stage they run in insertion order.
///
/// Execution is single-threaded and deterministic.
pub struct Schedule {
    stages: [Vec<SystemEntry>; Stage::COUNT],
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            stages: std::array::from_fn(|_| Vec::new()),
        }
    }

    /// Add a named system to a specific stage.
    pub fn add_system(
        &mut self,
        stage: Stage,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) {
        self.stages[stage as usize].push(SystemEntry {
            name: name.into(),
            system: Box::new(system),
        });
    }

    /// Run all systems registered for a single stage, in insertion order.
    pub fn run_stage(&mut self, stage: Stage, world: &mut World, resources: &mut Resources) {
        for entry in &mut self.stages[stage as usize] {
            tracing::trace!("Running system: {} ({:?})", entry.name, stage);
            (entry.system)(world, resources);
        }
    }

    /// Run every stage once, in [`Stage::ORDER`].
    ///
    /// Note: this runs `FixedUpdate` exactly once. The [`App`](crate::App) loop
    /// drives `FixedUpdate` separately to honor the fixed-timestep accumulator.
    pub fn run(&mut self, world: &mut World, resources: &mut Resources) {
        for stage in Stage::ORDER {
            self.run_stage(stage, world, resources);
        }
    }
}
