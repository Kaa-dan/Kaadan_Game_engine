//! KaadanEngine visual editor — a desktop GUI for authoring scenes.

mod app;
mod commands;
mod components;
mod gizmo;
mod panels;
mod play;
mod scene_io;
mod state;
mod ui;
mod viewport;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kaadan_editor=info,wgpu_core=warn,wgpu_hal=warn".into()),
        )
        .init();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::EditorApp::default();
    event_loop.run_app(&mut app).expect("event loop error");
}
