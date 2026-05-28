use glam::{Affine3A, Mat4, Quat, Vec3};

/// A 3D transform with position, rotation, and non-uniform scale.
/// Stored as separate components for easy manipulation;
/// composed into a matrix when needed for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Self::IDENTITY
        }
    }

    pub fn from_position_2d(x: f32, y: f32) -> Self {
        Self::from_position(Vec3::new(x, y, 0.0))
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Self::IDENTITY
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }

    pub fn from_scale_uniform(scale: f32) -> Self {
        Self::from_scale(Vec3::splat(scale))
    }

    /// Compose into a 4x4 matrix: Scale -> Rotate -> Translate
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Compose into an affine transform (more efficient than full Mat4)
    pub fn to_affine(&self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Apply parent transform to this child transform
    pub fn mul_transform(&self, child: &Transform) -> Transform {
        let position = self.rotation * (self.scale * child.position) + self.position;
        let rotation = self.rotation * child.rotation;
        let scale = self.scale * child.scale;
        Transform {
            position,
            rotation,
            scale,
        }
    }

    /// Local forward direction (negative Z in right-handed coords)
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Local right direction
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Local up direction
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Linearly interpolate between two transforms
    pub fn lerp(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            position: self.position.lerp(other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }
}
