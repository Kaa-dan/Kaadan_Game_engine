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
