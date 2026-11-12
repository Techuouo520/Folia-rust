// CSS cover layout, half_pane-pane mask, drift and readability overlays for the processed Monet poster.

struct Uniforms {
    size: vec4<f32>,
    drift: vec4<f32>,
    background: vec4<f32>,
    primary: vec4<f32>,
    accent: vec4<f32>,
    placement: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var poster: texture_2d<f32>;
@group(0) @binding(2) var poster_sampler: sampler;

struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex
fn vs(@builtin(vertex_index) index: u32) -> Vertex {
    var corners = array<vec2<f32>, 3>(vec2<f32>(-1., -1.), vec2<f32>(3., -1.), vec2<f32>(-1., 3.));
    let p = corners[index];
    return Vertex(vec4<f32>(p, 0., 1.), vec2<f32>(p.x * .5 + .5, .5 - p.y * .5));
}
fn over(base: vec3<f32>, ink: vec4<f32>) -> vec3<f32> { return mix(base, ink.rgb, ink.a); }
fn lerp_ink(a: vec4<f32>, b: vec4<f32>, t: f32) -> vec4<f32> {
    let alpha = mix(a.a, b.a, t);
    return vec4<f32>(mix(a.rgb * a.a, b.rgb * b.a, t) / max(alpha, .00001), alpha);
}
fn fallback(uv: vec2<f32>) -> vec3<f32> {
    let span = u.size.x + u.size.y;
    let t = (uv.x * u.size.x + uv.y * u.size.y) / span;
    let a = vec4<f32>(u.accent.rgb, .22);
    let b = vec4<f32>(u.background.rgb, .96);
    let c = vec4<f32>(u.primary.rgb, .18);
    let color = select(lerp_ink(b, c, (t - .5) * 2.), lerp_ink(a, b, t * 2.), t < .5);
    return over(u.background.rgb, color);
}
fn stop4(x: f32, positions: vec4<f32>, values: vec4<f32>) -> f32 {
    if (x < positions.y) { return mix(values.x, values.y, clamp((x - positions.x) / (positions.y - positions.x), 0., 1.)); }
    if (x < positions.z) { return mix(values.y, values.z, (x - positions.y) / (positions.z - positions.y)); }
    return mix(values.z, values.w, clamp((x - positions.z) / (positions.w - positions.z), 0., 1.));
}
@fragment
fn fs(in: Vertex) -> @location(0) vec4<f32> {
    let half_pane = u.placement.x > .5;
    let pane = select(1., select(.72, select(.68, .60, u.size.x >= 1024.), u.size.x >= 640.), half_pane);
    var uv = in.uv;
    uv.x /= pane;
    uv = (uv - vec2<f32>(.5) - u.drift.xy) / u.drift.z + vec2<f32>(.5);
    let box_aspect = u.size.x * pane / u.size.y;
    let aspect = u.size.z / max(u.size.w, 1.);
    let crop = vec2<f32>(min(1., box_aspect / aspect), min(1., aspect / box_aspect));
    let alignment = vec2<f32>(select(.5, clamp(.5 + u.placement.y * .01, .1, .9), half_pane), .5);
    let sample_uv = uv * crop + (vec2<f32>(1.) - crop) * alignment;
    let image = textureSample(poster, poster_sampler, sample_uv);
    var rgb = fallback(in.uv);
    if (u.placement.w > .5) {
        var opacity = image.a;
        if (half_pane) {
            opacity *= (select(.3, .7, u.placement.z > .5) + u.drift.w * .16);
            opacity *= stop4(in.uv.x / pane, vec4<f32>(0., .48, .74, 1.), vec4<f32>(1., .94, .46, 0.));
        }
        rgb = mix(rgb, image.rgb, opacity);
    }
    if (half_pane) {
        let mask = stop4(in.uv.x, vec4<f32>(0., .28, .64, 1.), vec4<f32>(.14, .28, .62, .86));
        rgb = mix(rgb, u.background.rgb, mask);
        let delta = (in.uv - vec2<f32>(.18, .34)) * u.size.xy;
        let radius = length(u.size.xy) * .36;
        rgb = mix(rgb, u.accent.rgb, .18 * clamp(1. - length(delta) / radius, 0., 1.));
    }
    let read = stop4(in.uv.x, vec4<f32>(0., .34, .7, 1.), vec4<f32>(.18, .32, .66, .82));
    return vec4<f32>(mix(rgb, u.background.rgb, read), 1.);
}
