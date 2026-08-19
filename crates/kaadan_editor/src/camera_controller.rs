use kaadan_renderer::Camera3D;

const ORBIT_SPEED: f32 = 0.008;
const PAN_SPEED: f32 = 0.0015;
const ZOOM_SPEED: f32 = 0.0015;

/// Apply one frame of camera navigation from the viewport image `response`.
pub fn handle(ui: &egui::Ui, response: &egui::Response, camera: &mut Camera3D) {
    let dist = camera.distance().max(0.05);

    // Orbit — right mouse button.
    if response.dragged_by(egui::PointerButton::Secondary) {
        let d = response.drag_delta();
        camera.orbit(-d.x * ORBIT_SPEED, d.y * ORBIT_SPEED);
    }

    // Pan — middle mouse button. Drag right → scene moves right (eye moves left).
    if response.dragged_by(egui::PointerButton::Middle) {
        let d = response.drag_delta();
        let scale = dist * PAN_SPEED;
        camera.pan(-d.x * scale, d.y * scale);
    }

    // Zoom — scroll wheel, only while the pointer is over the viewport.
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON {
            camera.dolly(scroll * dist * ZOOM_SPEED);
        }
    }
}
