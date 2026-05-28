//! Scene graph, hierarchy management, and scene serialization via [`ron`].
//!
//! Provides scene loading, saving, and runtime scene tree operations.

mod hierarchy;
mod scene;

pub use hierarchy::{transform_propagation_system, Children, GlobalTransform, Parent};
pub use scene::{EntityDesc, Scene, TransformDesc};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_ron_roundtrip() {
        let scene = Scene {
            name: "test".to_string(),
            entities: vec![EntityDesc {
                name: Some("player".to_string()),
                transform: Some(TransformDesc {
                    position: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                }),
                children: vec![],
                tags: vec!["player".to_string()],
            }],
        };

        let ron_str = scene.to_ron().unwrap();
        let loaded = Scene::from_ron(&ron_str).unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.entities[0].name.as_deref(), Some("player"));
    }
}
