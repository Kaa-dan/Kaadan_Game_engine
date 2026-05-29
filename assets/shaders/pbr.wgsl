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
    has_albedo_tex: f32,
    _pad: f32,
};
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var t_albedo: texture_2d<f32>;
@group(2) @binding(2) var s_albedo: sampler;

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
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world.xyz;
    out.world_normal = normalize((model.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.clip_position = camera.view_projection * world;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var albedo = material.base_color;
    if (material.has_albedo_tex > 0.5) {
        albedo = albedo * textureSample(t_albedo, s_albedo, in.uv);
    }

    let n = normalize(in.world_normal);
    let view_dir = normalize(camera.position.xyz - in.world_position);

    // Ambient term.
    var color = albedo.rgb * 0.05;

    // Directional light: Lambert diffuse + Blinn-Phong specular.
    let l = normalize(-lights.direction.xyz);
    let ndotl = max(dot(n, l), 0.0);
    let dir_radiance = lights.dir_color.rgb * lights.dir_color.w;
    let half_vec = normalize(l + view_dir);
    let shininess = mix(4.0, 64.0, 1.0 - material.roughness);
    let spec = pow(max(dot(n, half_vec), 0.0), shininess) * (1.0 - material.roughness);
    color = color + albedo.rgb * ndotl * dir_radiance + spec * dir_radiance;

    // Point lights with linear range attenuation.
    for (var i = 0u; i < lights.num_point_lights; i = i + 1u) {
        let pl = lights.point_lights[i];
        let to_light = pl.position.xyz - in.world_position;
        let dist = length(to_light);
        let ld = to_light / max(dist, 0.0001);
        let atten = clamp(1.0 - dist / max(pl.position.w, 0.0001), 0.0, 1.0);
        let pndotl = max(dot(n, ld), 0.0);
        color = color + albedo.rgb * pndotl * pl.color.rgb * pl.color.w * atten;
    }

    color = color + material.emissive.rgb;
    return vec4<f32>(color, albedo.a);
}
