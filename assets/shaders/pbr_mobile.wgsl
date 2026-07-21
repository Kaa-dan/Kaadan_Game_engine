// Mobile / low-end PBR variant.
//
// Same Cook-Torrance metallic-roughness model as pbr.wgsl, but cheaper:
//   - skips tangent-space normal mapping (geometric normal only),
//   - clamps to a single directional light + at most 2 point lights,
//   - uses a combined approximate Smith term to cut ALU.
//
// It shares the exact bind-group / uniform layout of pbr.wgsl so the Rust side
// (PbrRenderer) can swap the shader source without any other changes.

const PI: f32 = 3.14159265359;
const MAX_POINT_LIGHTS_MOBILE: u32 = 2u;

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
    position: vec4<f32>,
    color: vec4<f32>,
};
struct LightUniform {
    direction: vec4<f32>,
    dir_color: vec4<f32>,
    camera_position: vec4<f32>,
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
    out.uv = in.uv;
    out.clip_position = camera.view_projection * world;
    return out;
}

fn distribution_ggx(ndoth: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = ndoth * ndoth * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, 0.0001);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

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
    let ndotv = max(dot(n, v), 0.0);
    let ndoth = max(dot(n, h), 0.0);

    let d = distribution_ggx(ndoth, roughness);
    // Cheap combined visibility approximation (Kelemen-style).
    let vis = 0.25 / max(ndotl * ndotv, 0.0001);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let specular = d * vis * f;
    let kd = (vec3<f32>(1.0) - f) * (1.0 - metallic);
    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * radiance * ndotl;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var albedo = material.base_color;
    if (material.flags.x > 0.5) {
        albedo = albedo * textureSample(t_albedo, s_albedo, in.uv);
    }

    var metallic = material.metallic;
    var roughness = material.roughness;
    if (material.flags.y > 0.5) {
        let mr = textureSample(t_metallic_roughness, s_metallic_roughness, in.uv);
        roughness = roughness * mr.g;
        metallic = metallic * mr.b;
    }
    roughness = clamp(roughness, 0.04, 1.0);
    metallic = clamp(metallic, 0.0, 1.0);

    // No normal mapping on mobile; use the geometric normal.
    let n = normalize(in.world_normal);
    let v = normalize(camera.position.xyz - in.world_position);
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);

    var lo = vec3<f32>(0.0);

    let dir_l = normalize(-lights.direction.xyz);
    let dir_radiance = lights.dir_color.rgb * lights.dir_color.w;
    lo = lo + brdf(n, v, dir_l, dir_radiance, albedo.rgb, metallic, roughness, f0);

    let count = min(lights.num_point_lights, MAX_POINT_LIGHTS_MOBILE);
    for (var i = 0u; i < count; i = i + 1u) {
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

    let ambient = albedo.rgb * 0.03;
    var color = ambient + lo;

    var emissive = material.emissive.rgb;
    if (material.flags.w > 0.5) {
        emissive = emissive * textureSample(t_emissive, s_emissive, in.uv).rgb;
    }
    color = color + emissive;

    color = color / (color + vec3<f32>(1.0));
    return vec4<f32>(color, albedo.a);
}
