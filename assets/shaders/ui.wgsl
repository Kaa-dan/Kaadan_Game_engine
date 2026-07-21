// Screen-space UI quad shader.
//
// group(0) = projection uniform (pixel -> clip ortho)
// group(1) = texture + sampler

struct ProjectionUniform {
    projection: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> proj: ProjectionUniform;

@group(1) @binding(0) var t_ui: texture_2d<f32>;
@group(1) @binding(1) var s_ui: sampler;

struct VertexInput {
    // Position in pixels (top-left origin, y-down).
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = proj.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_ui, s_ui, in.uv);
    return tex_color * in.color;
}
