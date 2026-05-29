use kaadan_ecs::{App, Resources, Stage, World};

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
}

impl<'a> ScriptContext<'a> {
    /// Wrap an [`App`] for the duration of a plugin's registration call.
    pub fn new(app: &'a mut App) -> Self {
        Self {
            app,
            registered: Vec::new(),
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
