use kaadan_math::{Mat4, Vec3};

/// 3D perspective camera.
pub struct Camera3D {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera3D {
    pub fn new(aspect_ratio: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: 45.0_f32.to_radians(),
            aspect_ratio,
            near: 0.1,
            far: 1000.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect_ratio, self.near, self.far)
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Orbit the camera around the target.
    pub fn orbit(&mut self, yaw: f32, pitch: f32) {
        let offset = self.position - self.target;
        let radius = offset.length();
        let current_yaw = offset.z.atan2(offset.x);
        let current_pitch = (offset.y / radius).asin();

        let new_yaw = current_yaw + yaw;
        let new_pitch = (current_pitch + pitch).clamp(-1.5, 1.5);

        self.position = self.target
            + Vec3::new(
                radius * new_pitch.cos() * new_yaw.cos(),
                radius * new_pitch.sin(),
                radius * new_pitch.cos() * new_yaw.sin(),
            );
    }

    /// Distance from the eye to the target.
    pub fn distance(&self) -> f32 {
        (self.position - self.target).length()
    }

    /// Slide both eye and target within the camera's view plane (screen-space
    /// pan). `right`/`up` are in world units along the camera's right and up axes.
    pub fn pan(&mut self, right: f32, up: f32) {
        let forward = (self.target - self.position).normalize_or_zero();
        let right_axis = forward.cross(self.up).normalize_or_zero();
        let up_axis = right_axis.cross(forward).normalize_or_zero();
        let delta = right_axis * right + up_axis * up;
        self.position += delta;
        self.target += delta;
    }

    /// Move the eye toward (positive) or away from (negative) the target,
    /// keeping the target fixed. Distance is clamped so it can't cross the target.
    pub fn dolly(&mut self, amount: f32) {
        let offset = self.position - self.target;
        let dist = offset.length();
        let new_dist = (dist - amount).max(0.05);
        self.position = self.target + offset.normalize_or_zero() * new_dist;
    }

    /// Recenter the target on `point`, preserving the current view direction and
    /// distance (frame-selected behaviour).
    pub fn focus_on(&mut self, point: Vec3) {
        let offset = self.position - self.target;
        self.target = point;
        self.position = point + offset;
    }
}
