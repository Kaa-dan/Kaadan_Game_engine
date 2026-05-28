//! KaadanEngine math primitives.

mod color;
mod handle;
mod rect;
mod transform;

pub use color::Color;
pub use handle::{Handle, HandleAllocator};
pub use rect::{Rect, AABB};
pub use transform::Transform;

// Re-export glam types so downstream never imports glam directly
pub use glam::{
    Affine2, Affine3A, EulerRot, IVec2, IVec3, IVec4, Mat2, Mat3, Mat4, Quat, UVec2, UVec3, UVec4,
    Vec2, Vec3, Vec3A, Vec4,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_identity() {
        let t = Transform::IDENTITY;
        assert_eq!(t.to_matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn transform_composition() {
        let parent = Transform::from_position(Vec3::new(10.0, 0.0, 0.0));
        let child = Transform::from_position(Vec3::new(0.0, 5.0, 0.0));
        let result = parent.mul_transform(&child);
        assert_eq!(result.position, Vec3::new(10.0, 5.0, 0.0));
    }

    #[test]
    fn color_srgb_roundtrip() {
        let c = Color::from_srgb(0.5, 0.5, 0.5, 1.0);
        assert!(c.r < 0.5); // linear value is less than sRGB for mid-tones
        assert!(c.r > 0.2);
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let b = Rect::new(Vec2::new(5.0, 5.0), Vec2::new(15.0, 15.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn rect_no_intersection() {
        let a = Rect::new(Vec2::ZERO, Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(15.0, 15.0));
        assert!(!a.intersects(&b));
    }

    #[test]
    fn aabb_intersection() {
        let a = AABB::new(Vec3::ZERO, Vec3::splat(10.0));
        let b = AABB::new(Vec3::splat(5.0), Vec3::splat(15.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn handle_generational_safety() {
        let mut alloc = HandleAllocator::<u32>::new();
        let h1 = alloc.allocate();
        assert!(alloc.is_valid(h1));

        alloc.free(h1);
        assert!(!alloc.is_valid(h1));

        let h2 = alloc.allocate();
        assert!(alloc.is_valid(h2));
        assert!(!alloc.is_valid(h1)); // Old handle still invalid — generation bumped
        assert_eq!(h2.index(), h1.index()); // Same slot reused
        assert_ne!(h2.generation(), h1.generation()); // Different generation
    }

    #[test]
    fn handle_allocator_live_count() {
        let mut alloc = HandleAllocator::<u32>::new();
        let h1 = alloc.allocate();
        let h2 = alloc.allocate();
        assert_eq!(alloc.live_count(), 2);
        alloc.free(h1);
        assert_eq!(alloc.live_count(), 1);
        alloc.free(h2);
        assert_eq!(alloc.live_count(), 0);
    }
}
