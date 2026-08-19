use kaadan_ecs::Entity;
use kaadan_math::{Color, Transform, Vec3};
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight};

use crate::commands::{Command, EntitySnapshot, UndoStack};
use crate::components::Name;
use crate::scene_io::MeshSource;
use crate::viewport::Viewport;

/// One entry in the GameObject menu.
#[derive(Clone, Copy)]
pub enum SpawnKind {
    Empty,
    Cube,
    Sphere,
    Plane,
    Cylinder,
    Capsule,
    DirectionalLight,
    PointLight,
}

impl SpawnKind {
    fn mesh_source(self) -> Option<MeshSource> {
        Some(match self {
            SpawnKind::Cube => MeshSource::Cube { half_extent: 0.5 },
            SpawnKind::Sphere => MeshSource::Sphere { radius: 0.5 },
            SpawnKind::Plane => MeshSource::Plane { half_extent: 2.0 },
            SpawnKind::Cylinder => MeshSource::Cylinder {
                radius: 0.5,
                half_height: 0.5,
            },
            SpawnKind::Capsule => MeshSource::Capsule {
                radius: 0.5,
                half_height: 0.5,
            },
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            SpawnKind::Empty => "Entity",
            SpawnKind::Cube => "Cube",
            SpawnKind::Sphere => "Sphere",
            SpawnKind::Plane => "Plane",
            SpawnKind::Cylinder => "Cylinder",
            SpawnKind::Capsule => "Capsule",
            SpawnKind::DirectionalLight => "Directional Light",
            SpawnKind::PointLight => "Point Light",
        }
    }
}

/// Build the entity template for `kind` (resolving mesh geometry on the GPU) and
/// run it through the undo stack so the spawn is a single undoable step.
pub fn perform(
    kind: SpawnKind,
    viewport: &mut Viewport,
    commands: &mut UndoStack,
    selection: &mut Option<Entity>,
    device: &wgpu::Device,
) {
    let mut snap = EntitySnapshot {
        name: Some(Name::new(kind.name())),
        transform: Some(Transform::IDENTITY),
        ..Default::default()
    };

    if let Some(source) = kind.mesh_source() {
        let handle = viewport.get_or_create_mesh(device, &source);
        snap.mesh = Some(Mesh3D::new(handle));
        snap.material = Some(PbrMaterial {
            base_color: Color::from_hex(0x9AA0AA),
            metallic: 0.0,
            roughness: 0.6,
            ..Default::default()
        });
    }

    match kind {
        SpawnKind::DirectionalLight => {
            snap.dir_light = Some(DirectionalLight {
                direction: Vec3::new(-0.4, -1.0, -0.6).normalize(),
                color: Color::WHITE,
                intensity: 1.2,
            });
        }
        SpawnKind::PointLight => {
            snap.point_light = Some(PointLight {
                color: Color::WHITE,
                intensity: 1.0,
                range: 10.0,
            });
        }
        _ => {}
    }

    commands.run(&mut viewport.world, selection, Command::spawn(snap));
}
