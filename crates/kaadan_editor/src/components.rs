//! Editor-side ECS components layered onto the engine's runtime components.

/// Human-readable name shown in the hierarchy. Maps to `EntityDesc.name` when
/// a scene is saved.
#[derive(Clone)]
pub struct Name(pub String);

impl Name {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Names of the gameplay behaviours attached to this entity. Serialized to
/// `EntityDesc.scripts`; on Play, each name is resolved against the loaded
/// plugin's behaviour registry and attached as a `ScriptComponent`.
#[derive(Clone, Default)]
pub struct Scripts(pub Vec<String>);
