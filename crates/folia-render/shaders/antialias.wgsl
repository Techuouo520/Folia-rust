// Edge-adaptive filtering shared by flat lyrics and projected scene surfaces.

fn fxaa_sample(source: texture_2d<f32>, source_sampler: sampler, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source, source_sampler, uv, 0.0);
}

fn fxaa_luma(color: vec4<f32>) -> f32 {
    return dot(color.rgb, vec3(0.299, 0.587, 0.114)) + color.a * 0.25;
}

fn fxaa(source: texture_2d<f32>, source_sampler: sampler, uv: vec2<f32>) -> vec4<f32> {
    let step = 1.0 / vec2<f32>(textureDimensions(source));
    let middle = fxaa_sample(source, source_sampler, uv);
    let nw = fxaa_luma(fxaa_sample(source, source_sampler, uv + vec2(-1.0, -1.0) * step));
    let ne = fxaa_luma(fxaa_sample(source, source_sampler, uv + vec2(1.0, -1.0) * step));
    let sw = fxaa_luma(fxaa_sample(source, source_sampler, uv + vec2(-1.0, 1.0) * step));
    let se = fxaa_luma(fxaa_sample(source, source_sampler, uv + vec2(1.0, 1.0) * step));
    let center = fxaa_luma(middle);
    let low = min(center, min(min(nw, ne), min(sw, se)));
    let high = max(center, max(max(nw, ne), max(sw, se)));
    if high - low < max(0.0312, high * 0.125) {
        return middle;
    }
    var direction = vec2(-((nw + ne) - (sw + se)), (nw + sw) - (ne + se));
    let reduce = max((nw + ne + sw + se) * 0.03125, 0.0078125);
    direction = clamp(direction / (min(abs(direction.x), abs(direction.y)) + reduce), vec2(-8.0), vec2(8.0)) * step;
    let a = 0.5 * (fxaa_sample(source, source_sampler, uv + direction * (-1.0 / 6.0)) + fxaa_sample(source, source_sampler, uv + direction * (1.0 / 6.0)));
    let b = a * 0.5 + 0.25 * (fxaa_sample(source, source_sampler, uv - direction * 0.5) + fxaa_sample(source, source_sampler, uv + direction * 0.5));
    return select(b, a, fxaa_luma(b) < low || fxaa_luma(b) > high);
}
