struct Resolution {
    width: f32,
    height: f32,
}

@group(0) @binding(0) var<uniform> res: Resolution;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

fn to_clip(pos: vec2<f32>) -> vec4<f32> {
    let x = (pos.x / res.width) * 2.0 - 1.0;
    let y = 1.0 - (pos.y / res.height) * 2.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = to_clip(in.position);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

@vertex
fn vs_cursor(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = to_clip(in.position);
    out.color = in.color;
    return out;
}

@fragment
fn fs_cursor(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.5, 1.0, 1.0);
}