struct Uniforms {
    time: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@fragment
fn main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = pos.xy / uniforms.resolution;
    let t = uniforms.time;

    let r = sin(uv.x * 10.0 + t) * 0.5 + 0.5;
    let g = sin(uv.y * 10.0 + t * 0.8) * 0.5 + 0.5;
    let b = sin((uv.x + uv.y) * 8.0 + t * 0.6) * 0.5 + 0.5;

    return vec4(r, g, b, 1.0);
}
