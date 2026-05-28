use crate::resources::Resources;
use crate::world::World;

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

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Add a system with a name.
    pub fn add_system(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) {
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
