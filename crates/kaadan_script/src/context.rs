use kaadan_ecs::{App, Resources, Stage, World};

use crate::behaviour::{
    behaviour_driver_system, Behaviour, BehaviourFactory, BehaviourRegistry,
    BEHAVIOUR_DRIVER_SYSTEM,
};

/// The safe facade handed to gameplay code at registration time.
///
/// A gameplay plugin's `build` function receives a `&mut ScriptContext` and uses
/// it to register systems and resources. The context records every system name
/// it registers so the [`ScriptHost`](crate::ScriptHost) can later unregister
/// exactly those systems on reload.
///
/// `ScriptContext` deliberately exposes a narrow API: it borrows the [`App`]
/// mutably but only forwards the operations gameplay should perform during
/// registration. This keeps the host/plugin ABI surface small and stable.
pub struct ScriptContext<'a> {
    app: &'a mut App,
    registered: Vec<String>,
    behaviours_enabled: bool,
}

impl<'a> ScriptContext<'a> {
    /// Wrap an [`App`] for the duration of a plugin's registration call.
    pub fn new(app: &'a mut App) -> Self {
        Self {
            app,
            registered: Vec::new(),
            behaviours_enabled: false,
        }
    }

    /// Register a system on the default [`Stage::Update`] stage.
    ///
    /// The system name is recorded so it can be removed by name on reload.
    pub fn add_system(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> &mut Self {
        self.add_system_to_stage(Stage::Update, name, system)
    }

    /// Register a system on a specific [`Stage`].
    ///
    /// The system name is recorded so it can be removed by name on reload.
    pub fn add_system_to_stage(
        &mut self,
        stage: Stage,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> &mut Self {
        let name = name.into();
        self.registered.push(name.clone());
        self.app.add_system_to_stage(stage, name, system);
        self
    }

    /// Enable Unity-style [`Behaviour`](crate::Behaviour)s by registering the
    /// driver system that runs their `start`/`update` lifecycle.
    ///
    /// Idempotent within a single registration call. Registering the driver
    /// through the context records its name so the host removes and re-adds it
    /// cleanly on reload (like any other plugin system). Called automatically by
    /// [`register_behaviour`](Self::register_behaviour); call it directly if you
    /// attach behaviours without registering named factories.
    pub fn enable_behaviours(&mut self) -> &mut Self {
        if !self.behaviours_enabled {
            self.add_system(BEHAVIOUR_DRIVER_SYSTEM, behaviour_driver_system);
            self.behaviours_enabled = true;
        }
        self
    }

    /// Register a named [`Behaviour`](crate::Behaviour) factory so tooling (the
    /// editor's "Add Behaviour" menu, scene deserialization) can construct it by
    /// name, and ensure the driver system is running.
    ///
    /// The factory is stored in a [`BehaviourRegistry`] resource, created on
    /// first use.
    pub fn register_behaviour(
        &mut self,
        name: impl Into<String>,
        factory: BehaviourFactory,
    ) -> &mut Self {
        self.enable_behaviours();
        let name = name.into();
        if self.app.resources.get::<BehaviourRegistry>().is_none() {
            self.app.resources.insert(BehaviourRegistry::new());
        }
        if let Some(registry) = self.app.resources.get_mut::<BehaviourRegistry>() {
            registry.register(name, factory);
        }
        self
    }

    /// Convenience: register a `Default`-constructible behaviour type `B` under
    /// `name`, wiring up a factory automatically.
    pub fn register_behaviour_default<B>(&mut self, name: impl Into<String>) -> &mut Self
    where
        B: Behaviour + Default,
    {
        self.register_behaviour(name, || Box::new(B::default()))
    }

    /// Mutable access to the world (e.g. to spawn initial entities).
    pub fn world(&mut self) -> &mut World {
        &mut self.app.world
    }

    /// Mutable access to resources.
    pub fn resources(&mut self) -> &mut Resources {
        &mut self.app.resources
    }

    /// Insert (or replace) a resource of type `T`.
    pub fn insert_resource<T: 'static>(&mut self, resource: T) -> &mut Self {
        self.app.insert_resource(resource);
        self
    }

    /// Names of the systems registered through this context, in order.
    pub fn registered(&self) -> &[String] {
        &self.registered
    }

    /// Consume the context and return the registered system names.
    ///
    /// The [`ScriptHost`](crate::ScriptHost) stores these so it can remove the
    /// exact systems this plugin added when the library is reloaded.
    pub fn take_registered(self) -> Vec<String> {
        self.registered
    }
}
