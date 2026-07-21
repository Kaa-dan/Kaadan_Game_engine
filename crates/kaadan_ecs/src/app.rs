use crate::{Resources, Schedule, Stage, Time, World};

/// Upper bound on `FixedUpdate` runs per frame, a second guard against the
/// spiral of death (the first being delta clamping in [`Time`]).
const MAX_FIXED_STEPS: u32 = 8;

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

    /// Add a system to the default [`Stage::Update`] stage.
    pub fn add_system(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> &mut Self {
        self.schedule.add_system(Stage::Update, name, system);
        self
    }

    /// Add a system to a specific [`Stage`].
    pub fn add_system_to_stage(
        &mut self,
        stage: Stage,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> &mut Self {
        self.schedule.add_system(stage, name, system);
        self
    }

    pub fn add_plugin(&mut self, plugin: &dyn Plugin) -> &mut Self {
        plugin.build(self);
        self
    }

    /// Remove all systems with the given name (across every stage). Used by the
    /// scripting host to unregister a reloaded plugin's systems.
    pub fn remove_system(&mut self, name: &str) -> &mut Self {
        self.schedule.remove_system(name);
        self
    }

    pub fn insert_resource<T: 'static>(&mut self, resource: T) -> &mut Self {
        self.resources.insert(resource);
        self
    }

    /// Run one frame: update time, then run each stage. `FixedUpdate` runs zero
    /// or more times based on the accumulated time (clamped to
    /// [`MAX_FIXED_STEPS`]); all other stages run once.
    pub fn tick(&mut self) {
        if let Some(time) = self.resources.get_mut::<Time>() {
            time.update();
        }

        self.schedule
            .run_stage(Stage::First, &mut self.world, &mut self.resources);
        self.schedule
            .run_stage(Stage::PreUpdate, &mut self.world, &mut self.resources);

        let mut steps = 0;
        while steps < MAX_FIXED_STEPS {
            let run = self
                .resources
                .get_mut::<Time>()
                .map(|t| t.expend_fixed_step())
                .unwrap_or(false);
            if !run {
                break;
            }
            self.schedule
                .run_stage(Stage::FixedUpdate, &mut self.world, &mut self.resources);
            steps += 1;
        }

        self.schedule
            .run_stage(Stage::Update, &mut self.world, &mut self.resources);
        self.schedule
            .run_stage(Stage::PostUpdate, &mut self.world, &mut self.resources);
        self.schedule
            .run_stage(Stage::Render, &mut self.world, &mut self.resources);
    }
}
