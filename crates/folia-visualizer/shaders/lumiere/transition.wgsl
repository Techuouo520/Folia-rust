// Premultiplied scene filtering and centered paragraph composition, matching Pixi's five-tap blur.

struct Parameters {
    value: vec4<f32>,
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(1) @binding(0) var<uniform> parameters: Parameters;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}

fn sample_clamped(uv: vec2<f32>) -> vec4<f32> {
    let edge = vec2<f32>(0.5) / vec2<f32>(textureDimensions(image));
    return textureSample(image, image_sampler, clamp(uv, edge, vec2<f32>(1.0) - edge));
}

@fragment
fn blur(input: Vertex) -> @location(0) vec4<f32> {
    let offset = parameters.value.xy;
    return sample_clamped(input.uv - 2.0 * offset) * 0.153388
        + sample_clamped(input.uv - offset) * 0.221461
        + sample_clamped(input.uv) * 0.250301
        + sample_clamped(input.uv + offset) * 0.221461
        + sample_clamped(input.uv + 2.0 * offset) * 0.153388;
}

@fragment
fn compose(input: Vertex) -> @location(0) vec4<f32> {
    let uv = (input.uv - vec2<f32>(0.5)) * parameters.value.xy / parameters.value.w + vec2<f32>(0.5);
    return sample_clamped(uv) * parameters.value.z;
}
