// The wall.css hover filter, applied in sRGB to the poster after compositing.
fn poster_color(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.213, 0.715, 0.072));
    let saturated = mix(vec3<f32>(luma), color, 1.0 + 0.08 * amount);
    return clamp(saturated * (1.0 + 0.05 * amount), vec3<f32>(0.0), vec3<f32>(1.0));
}
