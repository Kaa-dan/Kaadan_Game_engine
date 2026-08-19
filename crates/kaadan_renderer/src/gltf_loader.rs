use kaadan_core::KaadanError;
use kaadan_math::Color;

use crate::material::PbrMaterial;
use crate::vertex3d::Vertex3D;

/// A loaded glTF model: render-ready meshes, their materials, and decoded
/// images (RGBA8) that base-color textures reference by index.
pub struct GltfModel {
    pub meshes: Vec<LoadedMesh>,
    pub materials: Vec<LoadedMaterial>,
    pub images: Vec<GltfImageData>,
}

/// One mesh primitive extracted from a glTF file.
pub struct LoadedMesh {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
}

/// A material's factors plus the index of its base-color image (if any) into
/// [`GltfModel::images`]. Texture GPU handles are resolved by the caller.
pub struct LoadedMaterial {
    pub material: PbrMaterial,
    pub base_color_image: Option<usize>,
}

/// A decoded image, always stored as tightly-packed RGBA8.
pub struct GltfImageData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn to_rgba8(image: &gltf::image::Data) -> GltfImageData {
    use gltf::image::Format;
    let px = &image.pixels;
    let n = (image.width * image.height) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    match image.format {
        Format::R8G8B8A8 => rgba.extend_from_slice(px),
        Format::R8G8B8 => {
            for c in px.chunks_exact(3) {
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        Format::R8G8 => {
            for c in px.chunks_exact(2) {
                rgba.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
            }
        }
        Format::R8 => {
            for &v in px {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        // Uncommon (16/32-bit) formats: fall back to opaque white rather than
        // misinterpreting the bytes.
        _ => rgba.extend(std::iter::repeat(255).take(n * 4)),
    }
    GltfImageData {
        rgba,
        width: image.width,
        height: image.height,
    }
}

/// Parse a `.gltf`/`.glb` byte buffer into ready-to-upload meshes + materials.
/// Textures are not resolved here (material texture handles are left `None`).
pub fn load_gltf(bytes: &[u8], path: &str) -> Result<GltfModel, KaadanError> {
    let (document, buffers, images) =
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
        let base_color_image = pbr
            .base_color_texture()
            .map(|info| info.texture().source().index());
        materials.push(LoadedMaterial {
            material: PbrMaterial {
                base_color: Color::new(bc[0], bc[1], bc[2], bc[3]),
                base_color_texture: None,
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                metallic_roughness_texture: None,
                normal_texture: None,
                emissive: Color::new(em[0], em[1], em[2], 1.0),
                emissive_texture: None,
            },
            base_color_image,
        });
    }

    let images = images.iter().map(to_rgba8).collect();

    Ok(GltfModel {
        meshes,
        materials,
        images,
    })
}
