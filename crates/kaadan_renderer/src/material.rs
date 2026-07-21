use kaadan_math::{Color, Handle};

use crate::texture::Texture;

/// PBR metallic-roughness material.
#[derive(Clone)]
pub struct PbrMaterial {
    pub base_color: Color,
    pub base_color_texture: Option<Handle<Texture>>,
    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: Option<Handle<Texture>>,
    pub normal_texture: Option<Handle<Texture>>,
    pub emissive: Color,
    pub emissive_texture: Option<Handle<Texture>>,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            base_color_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            metallic_roughness_texture: None,
            normal_texture: None,
            emissive: Color::BLACK,
            emissive_texture: None,
        }
    }
}

/// Component: directional light (sun-like).
#[derive(Clone)]
pub struct DirectionalLight {
    pub direction: kaadan_math::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: kaadan_math::Vec3::new(0.0, -1.0, -1.0).normalize(),
            color: Color::WHITE,
            intensity: 1.0,
        }
    }
}

/// Component: point light.
#[derive(Clone)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            intensity: 1.0,
            range: 10.0,
        }
    }
}
