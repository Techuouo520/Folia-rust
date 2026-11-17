// Linear-light mip generation for the projector's cover texture.

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

struct Pixel {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Pixel {
    let point = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0))[index];
    return Pixel(vec4(point, 0.0, 1.0), vec2(point.x * 0.5 + 0.5, 0.5 - point.y * 0.5));
}

@fragment
fn fs(pixel: Pixel) -> @location(0) vec4<f32> {
    return textureSampleLevel(image, image_sampler, pixel.uv, 0.0);
}
