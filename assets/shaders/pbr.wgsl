// Physically-based (Cook-Torrance) metallic-roughness shader.
//
// Lighting model:
//   - GGX/Trowbridge-Reitz normal distribution (D)
//   - Smith geometry with Schlick-GGX (G)
//   - Fresnel-Schlick (F)
//   - Metallic-roughness workflow (glTF): F0 = mix(0.04, albedo, metallic),
//     diffuse = albedo * (1 - metallic), energy-conserving kS/kD split.
//   - One directional light + up to 4 point lights (inverse-square attenuation).
//   - Optional albedo / metallic-roughness / normal / emissive textures, gated
//     by the `has_*` flags in the material uniform.
//
// Output is linear; the *Srgb surface format performs the final encode. A
// simple Reinhard tonemap tames bright highlights before that encode.

const PI: f32 = 3.14159265359;

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    position: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model: ModelUniform;

// std140 / vec4-aligned layout. `flags` packs the per-texture presence bits as
// floats: x = albedo, y = metallic-roughness, z = normal, w = emissive.
struct MaterialUniform {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    flags: vec4<f32>,
};
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var t_albedo: texture_2d<f32>;
@group(2) @binding(2) var s_albedo: sampler;
@group(2) @binding(3) var t_metallic_roughness: texture_2d<f32>;
@group(2) @binding(4) var s_metallic_roughness: sampler;
@group(2) @binding(5) var t_normal: texture_2d<f32>;
@group(2) @binding(6) var s_normal: sampler;
@group(2) @binding(7) var t_emissive: texture_2d<f32>;
@group(2) @binding(8) var s_emissive: sampler;

struct PointLight {
    position: vec4<f32>, // xyz position, w = range
    color: vec4<f32>,    // rgb color, w = intensity
};
struct LightUniform {
    direction: vec4<f32>,       // xyz direction (toward scene), w unused
    dir_color: vec4<f32>,       // rgb color, w = intensity
    camera_position: vec4<f32>, // xyz, w unused
    point_lights: array<PointLight, 4>,
    num_point_lights: u32,
};
@group(3) @binding(0) var<uniform> lights: LightUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) tangent_sign: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world.xyz;
    let nm = mat3x3<f32>(
        model.normal_matrix[0].xyz,
        model.normal_matrix[1].xyz,
        model.normal_matrix[2].xyz,
    );
    out.world_normal = normalize(nm * in.normal);
    out.world_tangent = normalize(nm * in.tangent.xyz);
    out.tangent_sign = in.tangent.w;
    out.uv = in.uv;
    out.clip_position = camera.view_projection * world;
    return out;
}

// GGX / Trowbridge-Reitz normal distribution.
fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    let denom = ndoth2 * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, 0.0001);
}

// Schlick-GGX geometry term for a single direction.
fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return ndotv / (ndotv * (1.0 - k) + k);
}

// Smith geometry term (view + light occlusion).
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    return geometry_schlick_ggx(ndotv, roughness) * geometry_schlick_ggx(ndotl, roughness);
}

// Fresnel-Schlick approximation.
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Evaluate the Cook-Torrance BRDF for one light and return its outgoing
// radiance contribution.
fn brdf(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    radiance: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    f0: vec3<f32>,
) -> vec3<f32> {
    let h = normalize(v + l);
    let ndotl = max(dot(n, l), 0.0);

    let d = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let numerator = d * g * f;
    let denom = 4.0 * max(dot(n, v), 0.0) * ndotl + 0.0001;
    let specular = numerator / denom;

    let ks = f;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * radiance * ndotl;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // --- Sample material inputs, falling back to factors when absent. ---
    var albedo = material.base_color;
    if (material.flags.x > 0.5) {
        albedo = albedo * textureSample(t_albedo, s_albedo, in.uv);
    }

    var metallic = material.metallic;
    var roughness = material.roughness;
    if (material.flags.y > 0.5) {
        // glTF packs roughness in G and metallic in B.
        let mr = textureSample(t_metallic_roughness, s_metallic_roughness, in.uv);
        roughness = roughness * mr.g;
        metallic = metallic * mr.b;
    }
    roughness = clamp(roughness, 0.04, 1.0);
    metallic = clamp(metallic, 0.0, 1.0);

    // --- Build the shading normal (tangent-space normal map if present). ---
    var n = normalize(in.world_normal);
    if (material.flags.z > 0.5) {
        let t = normalize(in.world_tangent - n * dot(n, in.world_tangent));
        let b = cross(n, t) * in.tangent_sign;
        let tbn = mat3x3<f32>(t, b, n);
        let sampled = textureSample(t_normal, s_normal, in.uv).xyz * 2.0 - 1.0;
        n = normalize(tbn * sampled);
    }

    let v = normalize(camera.position.xyz - in.world_position);
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);

    var lo = vec3<f32>(0.0);

    // Directional light.
    let dir_l = normalize(-lights.direction.xyz);
    let dir_radiance = lights.dir_color.rgb * lights.dir_color.w;
    lo = lo + brdf(n, v, dir_l, dir_radiance, albedo.rgb, metallic, roughness, f0);

    // Point lights with physical inverse-square attenuation, smoothly faded to
    // zero at the configured range.
    for (var i = 0u; i < lights.num_point_lights; i = i + 1u) {
        let pl = lights.point_lights[i];
        let to_light = pl.position.xyz - in.world_position;
        let dist = length(to_light);
        let l = to_light / max(dist, 0.0001);
        let inv_sq = 1.0 / max(dist * dist, 0.0001);
        let range = max(pl.position.w, 0.0001);
        let window = clamp(1.0 - pow(dist / range, 4.0), 0.0, 1.0);
        let attenuation = inv_sq * window * window;
        let radiance = pl.color.rgb * pl.color.w * attenuation;
        lo = lo + brdf(n, v, l, radiance, albedo.rgb, metallic, roughness, f0);
    }

    // Small ambient term (cheap stand-in for image-based lighting).
    let ambient = albedo.rgb * 0.03;
    var color = ambient + lo;

    // Emissive added after lighting.
    var emissive = material.emissive.rgb;
    if (material.flags.w > 0.5) {
        emissive = emissive * textureSample(t_emissive, s_emissive, in.uv).rgb;
    }
    color = color + emissive;

    // Reinhard tonemap; the sRGB surface handles the linear->sRGB encode.
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, albedo.a);
}
