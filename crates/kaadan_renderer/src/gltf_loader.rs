use kaadan_core::KaadanError;
use kaadan_math::Color;

use crate::material::PbrMaterial;
use crate::vertex3d::Vertex3D;

/// A loaded glTF model: render-ready meshes plus their materials.
pub struct GltfModel {
    pub meshes: Vec<LoadedMesh>,
    pub materials: Vec<PbrMaterial>,
}

/// One mesh primitive extracted from a glTF file.
pub struct LoadedMesh {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
}

/// Parse a `.gltf`/`.glb` byte buffer into ready-to-upload meshes + materials.
/// Textures are not resolved here (material texture handles are left `None`).
pub fn load_gltf(bytes: &[u8], path: &str) -> Result<GltfModel, KaadanError> {
    let (document, buffers, _images) =
        gltf::import_slice(bytes).map_err(|e| KaadanError::AssetLoad {
            path: path.to_string(),
            reason: e.to_string(),
        })?;

    let mut meshes = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions: Vec<[f32; 3]> = match reader.read_positions() {
                Some(iter) => iter.collect(),
                None => continue,
            };
            let count = positions.len();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|i| i.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; count]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; count]);
            let tangents: Vec<[f32; 4]> = reader
                .read_tangents()
                .map(|i| i.collect())
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; count]);

            let vertices: Vec<Vertex3D> = (0..count)
                .map(|i| Vertex3D {
                    position: positions[i],
                    normal: *normals.get(i).unwrap_or(&[0.0, 1.0, 0.0]),
                    uv: *uvs.get(i).unwrap_or(&[0.0, 0.0]),
                    tangent: *tangents.get(i).unwrap_or(&[1.0, 0.0, 0.0, 1.0]),
                })
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..count as u32).collect());

            meshes.push(LoadedMesh {
                vertices,
                indices,
                material_index: primitive.material().index(),
            });
        }
    }

    let mut materials = Vec::new();
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        let bc = pbr.base_color_factor();
        let em = material.emissive_factor();
        materials.push(PbrMaterial {
            base_color: Color::new(bc[0], bc[1], bc[2], bc[3]),
            base_color_texture: None,
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            metallic_roughness_texture: None,
            normal_texture: None,
            emissive: Color::new(em[0], em[1], em[2], 1.0),
        });
    }

    Ok(GltfModel { meshes, materials })
}
