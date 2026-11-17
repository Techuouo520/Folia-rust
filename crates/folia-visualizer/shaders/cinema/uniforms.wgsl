// Cinema uniform layout shared by the material, lighting and lens passes.

diagnostic(off, derivative_uniformity);

struct Cinema {
    eye_time: vec4<f32>,
    right_tan: vec4<f32>,
    up_tan: vec4<f32>,
    forward_near: vec4<f32>,
    render: vec4<f32>,
    lamp: vec4<f32>,
    atmosphere: vec4<f32>,
    cove: vec4<f32>,
    motion: vec4<f32>,
    curtains: vec4<f32>,
    post: vec4<f32>,
    light_cover: vec4<f32>,
    reel: vec4<f32>,
    output: vec4<f32>,
    reel_light_a: array<vec4<f32>, 3>,
    reel_light_b: array<vec4<f32>, 3>,
}

struct Light {
    screen: array<vec4<f32>, 3>,
    ambient: vec4<f32>,
    haze: vec4<f32>,
    projected: vec4<f32>,
    raw: array<vec4<f32>, 3>,
    sampled: array<vec4<f32>, 3>,
    state: vec4<f32>,
}

fn cinema_smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
    let amount = clamp((value - edge0) / (edge1 - edge0), 0.0, 1.0);
    return amount * amount * (3.0 - 2.0 * amount);
}
