//! Hand-rolled viewport gizmos + click picking.
//!
//! No published `transform-gizmo` release targets egui 0.30 (which we're pinned
//! to via wgpu 23), so this draws axis handles directly with egui's painter and
//! does the screen<->world math itself. The pure math (projection, ray/sphere,
//! sprite AABB) is unit-tested so behaviour is verifiable without a display.

use egui::{Color32, Pos2, Stroke, Vec2 as EVec2};
use kaadan_ecs::{Entity, World};
use kaadan_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use kaadan_renderer::{Camera2D, Camera3D, Mesh3D, Sprite};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy)]
pub enum DragTarget {
    Axis(Axis),
    Free,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn vec(self) -> Vec3 {
        match self {
            Axis::X => Vec3::X,
            Axis::Y => Vec3::Y,
            Axis::Z => Vec3::Z,
        }
    }

    fn color(self) -> Color32 {
        match self {
            Axis::X => Color32::from_rgb(230, 80, 80),
            Axis::Y => Color32::from_rgb(90, 210, 90),
            Axis::Z => Color32::from_rgb(90, 130, 230),
        }
    }
}

const HANDLE_PX: f32 = 70.0;
const GRAB_PX: f32 = 8.0;

/// Inputs the gizmo needs from the selected entity's relevant camera.
struct View {
    view_projection: Mat4,
    is_3d: bool,
}

/// Drive picking + gizmo for one frame. `response` is the viewport image's
/// response; `rect` its screen rectangle.
pub fn handle(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    world: &mut World,
    selected: &mut Option<Entity>,
    mode: GizmoMode,
    drag: &mut Option<DragTarget>,
    cam2d: &Camera2D,
    cam3d: &Camera3D,
) {
    // Resolve the active view for the current selection (mesh -> 3D, else 2D).
    let view = selected
        .filter(|&e| world.is_alive(e))
        .map(|e| active_view(world, e, cam2d, cam3d));

    if let (Some(entity), Some(view)) = (selected.filter(|&e| world.is_alive(e)), &view) {
        let origin = world
            .get::<Transform>(entity)
            .map(|t| t.position)
            .unwrap_or(Vec3::ZERO);

        // Begin a drag: pick an axis handle (translate/scale) or free (rotate).
        if response.drag_started() {
            if let Some(p) = response.interact_pointer_pos() {
                *drag = match mode {
                    GizmoMode::Rotate => Some(DragTarget::Free),
                    _ => nearest_axis(p, rect, view, origin).map(DragTarget::Axis),
                };
            }
        }
        if response.dragged() {
            if let Some(target) = *drag {
                apply_drag(
                    world,
                    entity,
                    target,
                    response.drag_delta(),
                    mode,
                    view,
                    rect,
                );
            }
        }
        if response.drag_stopped() {
            *drag = None;
        }

        draw(ui, rect, view, origin, mode);
    }

    // A plain click (no drag) picks whatever is under the cursor.
    if response.clicked() && drag.is_none() {
        if let Some(p) = response.interact_pointer_pos() {
            *selected = pick(world, cam2d, cam3d, rect, p);
        }
    }
}

fn active_view(world: &World, entity: Entity, cam2d: &Camera2D, cam3d: &Camera3D) -> View {
    if world.get::<Mesh3D>(entity).is_ok() {
        View {
            view_projection: cam3d.view_projection(),
            is_3d: true,
        }
    } else {
        View {
            view_projection: cam2d.view_projection(),
            is_3d: false,
        }
    }
}

fn axes(is_3d: bool) -> &'static [Axis] {
    if is_3d {
        &[Axis::X, Axis::Y, Axis::Z]
    } else {
        &[Axis::X, Axis::Y]
    }
}

fn nearest_axis(pointer: Pos2, rect: egui::Rect, view: &View, origin: Vec3) -> Option<Axis> {
    let s0 = world_to_screen(view.view_projection, rect, origin)?;
    let mut best: Option<(Axis, f32)> = None;
    for &axis in axes(view.is_3d) {
        let Some(dir) = axis_screen_dir(view.view_projection, rect, origin, axis) else {
            continue;
        };
        let end = s0 + dir * HANDLE_PX;
        let d = dist_point_segment(pointer, s0, end);
        if d < GRAB_PX && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((axis, d));
        }
    }
    best.map(|(a, _)| a)
}

fn apply_drag(
    world: &mut World,
    entity: Entity,
    target: DragTarget,
    delta: EVec2,
    mode: GizmoMode,
    view: &View,
    rect: egui::Rect,
) {
    let Ok(mut t) = world.get_mut::<Transform>(entity) else {
        return;
    };
    match target {
        DragTarget::Free => {
            let yaw = delta.x * 0.01;
            let pitch = delta.y * 0.01;
            t.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * t.rotation;
        }
        DragTarget::Axis(axis) => {
            let origin = t.position;
            let Some(dir) = axis_screen_dir(view.view_projection, rect, origin, axis) else {
                return;
            };
            let len = dir.length();
            if len < 1e-4 {
                return;
            }
            let unit = dir / len;
            let along = delta.x * unit.x + delta.y * unit.y; // screen px along the axis
            match mode {
                GizmoMode::Translate => {
                    // `len` screen px == 1 world unit along this axis.
                    t.position += axis.vec() * (along / len);
                }
                GizmoMode::Scale => {
                    let new = (axis_component(t.scale, axis) + along * 0.01).max(0.01);
                    set_axis_component(&mut t.scale, axis, new);
                }
                GizmoMode::Rotate => {}
            }
        }
    }
}

fn draw(ui: &egui::Ui, rect: egui::Rect, view: &View, origin: Vec3, mode: GizmoMode) {
    let painter = ui.painter_at(rect);
    let Some(s0) = world_to_screen(view.view_projection, rect, origin) else {
        return;
    };

    if matches!(mode, GizmoMode::Rotate) {
        painter.circle_stroke(s0, 36.0, Stroke::new(2.0, Color32::from_gray(220)));
        return;
    }

    for &axis in axes(view.is_3d) {
        let Some(dir) = axis_screen_dir(view.view_projection, rect, origin, axis) else {
            continue;
        };
        if dir.length() < 1e-4 {
            continue;
        }
        let end = s0 + dir.normalized() * HANDLE_PX;
        painter.line_segment([s0, end], Stroke::new(2.5, axis.color()));
        match mode {
            GizmoMode::Scale => {
                let b = egui::Rect::from_center_size(end, EVec2::splat(8.0));
                painter.rect_filled(b, 1.0, axis.color());
            }
            _ => {
                painter.circle_filled(end, 4.0, axis.color());
            }
        }
    }
}

// --- Pure math (unit-tested) ---------------------------------------------

/// Project a world point into the egui screen rect. `None` if behind the camera.
fn world_to_screen(view_projection: Mat4, rect: egui::Rect, world: Vec3) -> Option<Pos2> {
    let clip = view_projection * Vec4::new(world.x, world.y, world.z, 1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
    let y = rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height();
    Some(Pos2::new(x, y))
}

/// Screen-space vector for 1 world unit along `axis` from `origin`.
fn axis_screen_dir(
    view_projection: Mat4,
    rect: egui::Rect,
    origin: Vec3,
    axis: Axis,
) -> Option<EVec2> {
    let s0 = world_to_screen(view_projection, rect, origin)?;
    let s1 = world_to_screen(view_projection, rect, origin + axis.vec())?;
    Some(s1 - s0)
}

fn dist_point_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}

fn axis_component(v: Vec3, axis: Axis) -> f32 {
    match axis {
        Axis::X => v.x,
        Axis::Y => v.y,
        Axis::Z => v.z,
    }
}

fn set_axis_component(v: &mut Vec3, axis: Axis, value: f32) {
    match axis {
        Axis::X => v.x = value,
        Axis::Y => v.y = value,
        Axis::Z => v.z = value,
    }
}

/// Pick the entity under `screen`: topmost 2D sprite, else nearest 3D mesh.
pub fn pick(
    world: &World,
    cam2d: &Camera2D,
    cam3d: &Camera3D,
    rect: egui::Rect,
    screen: Pos2,
) -> Option<Entity> {
    let world2d = screen_to_world_2d(cam2d, rect, screen);
    let mut best_sprite: Option<(Entity, i32)> = None;
    for (e, (sprite, transform)) in world.query::<(&Sprite, &Transform)>().iter() {
        let (min, max) = sprite_aabb(transform, sprite);
        let inside =
            world2d.x >= min.x && world2d.x <= max.x && world2d.y >= min.y && world2d.y <= max.y;
        if inside && best_sprite.map_or(true, |(_, z)| sprite.z_order >= z) {
            best_sprite = Some((e, sprite.z_order));
        }
    }
    if let Some((e, _)) = best_sprite {
        return Some(e);
    }

    let inv = cam3d.view_projection().inverse();
    let (origin, dir) = screen_ray(inv, rect, screen);
    let mut best: Option<(Entity, f32)> = None;
    for (e, (_mesh, transform)) in world.query::<(&Mesh3D, &Transform)>().iter() {
        let radius = transform.scale.max_element().max(0.001);
        if let Some(t) = ray_sphere(origin, dir, transform.position, radius) {
            if best.map_or(true, |(_, bt)| t < bt) {
                best = Some((e, t));
            }
        }
    }
    best.map(|(e, _)| e)
}

fn screen_to_world_2d(cam: &Camera2D, rect: egui::Rect, screen: Pos2) -> Vec2 {
    let nx = ((screen.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ny = 1.0 - ((screen.y - rect.min.y) / rect.height()) * 2.0;
    let half = cam.viewport_size / (2.0 * cam.zoom);
    cam.position + Vec2::new(nx * half.x, ny * half.y)
}

fn screen_ray(inv_view_projection: Mat4, rect: egui::Rect, screen: Pos2) -> (Vec3, Vec3) {
    let nx = ((screen.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ny = 1.0 - ((screen.y - rect.min.y) / rect.height()) * 2.0;
    let near = inv_view_projection.project_point3(Vec3::new(nx, ny, 0.0));
    let far = inv_view_projection.project_point3(Vec3::new(nx, ny, 1.0));
    (near, (far - near).normalize())
}

fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_d = disc.sqrt();
    let t0 = -b - sqrt_d;
    if t0 >= 0.0 {
        return Some(t0);
    }
    let t1 = -b + sqrt_d;
    (t1 >= 0.0).then_some(t1)
}

fn sprite_aabb(transform: &Transform, sprite: &Sprite) -> (Vec2, Vec2) {
    let size =
        sprite.size.unwrap_or(Vec2::splat(32.0)) * Vec2::new(transform.scale.x, transform.scale.y);
    let pos = Vec2::new(transform.position.x, transform.position.y);
    let min = pos - size * sprite.anchor;
    (min, min + size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_math::HandleAllocator;

    #[test]
    fn ray_sphere_hits_and_misses() {
        let origin = Vec3::new(0.0, 0.0, 5.0);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        assert!(ray_sphere(origin, dir, Vec3::ZERO, 1.0).is_some());
        // Pointing away from the sphere.
        assert!(ray_sphere(origin, Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, 1.0).is_none());
        // Off to the side, missing.
        assert!(ray_sphere(origin, dir, Vec3::new(5.0, 0.0, 0.0), 1.0).is_none());
    }

    #[test]
    fn world_to_screen_centers_origin() {
        // Identity VP maps world (0,0,0) to NDC (0,0) -> rect center.
        let rect = egui::Rect::from_min_size(Pos2::ZERO, EVec2::new(800.0, 600.0));
        let s = world_to_screen(Mat4::IDENTITY, rect, Vec3::ZERO).unwrap();
        assert!((s.x - 400.0).abs() < 0.01);
        assert!((s.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn pick_selects_topmost_sprite() {
        let mut alloc = HandleAllocator::<kaadan_renderer::Texture>::new();
        let tex = alloc.allocate();
        let mut world = World::new();

        let mut low = Sprite::new(tex);
        low.size = Some(Vec2::splat(100.0));
        low.z_order = 0;
        let low_e = world.spawn((low, Transform::from_position_2d(0.0, 0.0)));

        let mut high = Sprite::new(tex);
        high.size = Some(Vec2::splat(100.0));
        high.z_order = 5;
        let high_e = world.spawn((high, Transform::from_position_2d(0.0, 0.0)));

        let cam2d = Camera2D::new(800.0, 600.0);
        let cam3d = Camera3D::new(800.0 / 600.0);
        let rect = egui::Rect::from_min_size(Pos2::ZERO, EVec2::new(800.0, 600.0));
        // Click dead center -> world (0,0), inside both overlapping sprites.
        let hit = pick(&world, &cam2d, &cam3d, rect, Pos2::new(400.0, 300.0));
        assert_eq!(hit, Some(high_e));
        assert_ne!(hit, Some(low_e));
    }

    #[test]
    fn pick_returns_none_on_empty_space() {
        let mut alloc = HandleAllocator::<kaadan_renderer::Texture>::new();
        let tex = alloc.allocate();
        let mut world = World::new();
        let mut s = Sprite::new(tex);
        s.size = Some(Vec2::splat(20.0));
        world.spawn((s, Transform::from_position_2d(0.0, 0.0)));

        let cam2d = Camera2D::new(800.0, 600.0);
        let cam3d = Camera3D::new(800.0 / 600.0);
        let rect = egui::Rect::from_min_size(Pos2::ZERO, EVec2::new(800.0, 600.0));
        // Top-left corner -> far from the small centered sprite, no 3D meshes.
        let hit = pick(&world, &cam2d, &cam3d, rect, Pos2::new(5.0, 5.0));
        assert_eq!(hit, None);
    }
}
