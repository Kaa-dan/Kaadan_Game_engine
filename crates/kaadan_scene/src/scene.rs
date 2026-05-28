use serde::{Deserialize, Serialize};

/// A serializable scene definition.
#[derive(Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub entities: Vec<EntityDesc>,
}

/// Description of an entity for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityDesc {
    pub name: Option<String>,
    pub transform: Option<TransformDesc>,
    pub children: Vec<EntityDesc>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Serializable transform.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransformDesc {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default = "default_rotation")]
    pub rotation: [f32; 4],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

fn default_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

impl Scene {
    /// Serialize to RON string.
    pub fn to_ron(&self) -> Result<String, kaadan_core::KaadanError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| kaadan_core::KaadanError::Other(e.to_string()))
    }

    /// Deserialize from RON string.
    pub fn from_ron(s: &str) -> Result<Self, kaadan_core::KaadanError> {
        ron::from_str(s).map_err(|e| kaadan_core::KaadanError::Other(e.to_string()))
    }
}
