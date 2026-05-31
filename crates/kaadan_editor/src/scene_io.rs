//! Editor scene file format (RON). Captures the editor-known components plus
//! *asset sources* so procedural assets (a generated checkerboard, a code-built
//! cube) round-trip without needing files on disk. `File`/`Gltf` variants are the
//! Unity-style asset references for when a real import pipeline lands.

use serde::{Deserialize, Serialize};

use kaadan_math::{Color, Quat, Transform, Vec2, Vec3};
use kaadan_renderer::{DirectionalLight, PointLight, Sprite};

/// What an editor I/O menu action wants to do (resolved by the app loop, which
/// owns the GPU device needed to rebuild assets).
#[derive(Clone, Copy)]
pub enum IoRequest {
    Save,
    Load,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum TextureSource {
    Checker { size: u32, cells: u32 },
    Solid { rgba: [u8; 4] },
    File { path: String },
}

impl TextureSource {
    pub fn key(&self) -> String {
        match self {
            TextureSource::Checker { size, cells } => format!("checker:{size}:{cells}"),
            TextureSource::Solid { rgba } => format!("solid:{rgba:?}"),
            TextureSource::File { path } => format!("file:{path}"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MeshSource {
    Cube { half_extent: f32 },
    Gltf { path: String },
}

impl MeshSource {
    pub fn key(&self) -> String {
        match self {
            MeshSource::Cube { half_extent } => format!("cube:{half_extent}"),
            MeshSource::Gltf { path } => format!("gltf:{path}"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransformDesc {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl From<&Transform> for TransformDesc {
    fn from(t: &Transform) -> Self {
        Self {
            position: [t.position.x, t.position.y, t.position.z],
            rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
            scale: [t.scale.x, t.scale.y, t.scale.z],
        }
    }
}

impl From<&TransformDesc> for Transform {
    fn from(d: &TransformDesc) -> Self {
        Transform {
            position: Vec3::new(d.position[0], d.position[1], d.position[2]),
            rotation: Quat::from_xyzw(d.rotation[0], d.rotation[1], d.rotation[2], d.rotation[3]),
            scale: Vec3::new(d.scale[0], d.scale[1], d.scale[2]),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpriteDesc {
    pub texture: TextureSource,
    pub color: [f32; 4],
    pub size: Option<[f32; 2]>,
    pub anchor: [f32; 2],
    pub z_order: i32,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl SpriteDesc {
    /// Build a runtime `Sprite` once the texture handle has been resolved.
    pub fn to_sprite(&self, texture: kaadan_math::Handle<kaadan_renderer::Texture>) -> Sprite {
        let mut sprite = Sprite::new(texture);
        sprite.color = Color::new(self.color[0], self.color[1], self.color[2], self.color[3]);
        sprite.size = self.size.map(|s| Vec2::new(s[0], s[1]));
        sprite.anchor = Vec2::new(self.anchor[0], self.anchor[1]);
        sprite.z_order = self.z_order;
        sprite.flip_x = self.flip_x;
        sprite.flip_y = self.flip_y;
        sprite
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MaterialDesc {
    pub base_color: [f32; 4],
    pub base_color_texture: Option<TextureSource>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirLightDesc {
    pub direction: [f32; 3],
    pub color: [f32; 4],
    pub intensity: f32,
}

impl From<&DirectionalLight> for DirLightDesc {
    fn from(l: &DirectionalLight) -> Self {
        Self {
            direction: [l.direction.x, l.direction.y, l.direction.z],
            color: l.color.to_array(),
            intensity: l.intensity,
        }
    }
}

impl From<&DirLightDesc> for DirectionalLight {
    fn from(d: &DirLightDesc) -> Self {
        DirectionalLight {
            direction: Vec3::new(d.direction[0], d.direction[1], d.direction[2]),
            color: Color::new(d.color[0], d.color[1], d.color[2], d.color[3]),
            intensity: d.intensity,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PointLightDesc {
    pub color: [f32; 4],
    pub intensity: f32,
    pub range: f32,
}

impl From<&PointLight> for PointLightDesc {
    fn from(l: &PointLight) -> Self {
        Self {
            color: l.color.to_array(),
            intensity: l.intensity,
            range: l.range,
        }
    }
}

impl From<&PointLightDesc> for PointLight {
    fn from(d: &PointLightDesc) -> Self {
        PointLight {
            color: Color::new(d.color[0], d.color[1], d.color[2], d.color[3]),
            intensity: d.intensity,
            range: d.range,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EntityDesc {
    pub name: Option<String>,
    pub transform: Option<TransformDesc>,
    pub sprite: Option<SpriteDesc>,
    pub mesh: Option<MeshSource>,
    pub material: Option<MaterialDesc>,
    pub dir_light: Option<DirLightDesc>,
    pub point_light: Option<PointLightDesc>,
    #[serde(default)]
    pub children: Vec<EntityDesc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorScene {
    pub name: String,
    pub entities: Vec<EntityDesc>,
}

impl EditorScene {
    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())
    }

    pub fn from_ron(s: &str) -> Result<Self, String> {
        ron::from_str(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_ron_roundtrips() {
        let scene = EditorScene {
            name: "test".to_string(),
            entities: vec![EntityDesc {
                name: Some("Cube".to_string()),
                transform: Some(TransformDesc {
                    position: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                }),
                mesh: Some(MeshSource::Cube { half_extent: 1.0 }),
                material: Some(MaterialDesc {
                    base_color: [0.2, 0.4, 1.0, 1.0],
                    base_color_texture: None,
                    metallic: 0.1,
                    roughness: 0.35,
                    emissive: [0.0, 0.0, 0.0],
                }),
                children: vec![EntityDesc {
                    name: Some("Tile".to_string()),
                    sprite: Some(SpriteDesc {
                        texture: TextureSource::Checker { size: 64, cells: 8 },
                        color: [1.0, 1.0, 1.0, 1.0],
                        size: Some([48.0, 48.0]),
                        anchor: [0.5, 0.5],
                        z_order: 0,
                        flip_x: false,
                        flip_y: false,
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let ron = scene.to_ron().expect("serialize");
        let loaded = EditorScene::from_ron(&ron).expect("deserialize");
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.entities[0].name.as_deref(), Some("Cube"));
        assert_eq!(loaded.entities[0].children.len(), 1);
        assert_eq!(
            loaded.entities[0].mesh,
            Some(MeshSource::Cube { half_extent: 1.0 })
        );
        assert_eq!(
            loaded.entities[0].children[0]
                .sprite
                .as_ref()
                .unwrap()
                .texture,
            TextureSource::Checker { size: 64, cells: 8 }
        );
    }
}
