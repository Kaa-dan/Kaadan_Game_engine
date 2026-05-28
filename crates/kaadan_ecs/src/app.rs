use crate::{Resources, Schedule, Time, World};

/// Plugin trait for modular system registration.
pub trait Plugin {
    fn build(&self, app: &mut App);
}

/// Central engine orchestrator.
pub struct App {
    pub world: World,
    pub resources: Resources,
    pub schedule: Schedule,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut resources = Resources::new();
        resources.insert(Time::new());

        Self {
            world: World::new(),
            resources,
            schedule: Schedule::new(),
        }
    }

    pub fn add_system(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> &mut Self {
        self.schedule.add_system(name, system);
        self
    }

    pub fn add_plugin(&mut self, plugin: &dyn Plugin) -> &mut Self {
        plugin.build(self);
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
