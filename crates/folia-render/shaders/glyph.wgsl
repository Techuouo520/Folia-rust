// Glyph sprites: a quad per glyph from the atlas, placed by an anchor and a 2×2 map, blended with straight alpha.

struct Screen {
    size: vec4<f32>,
}

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var blurred_atlas: texture_2d_array<f32>;

struct SpriteIn {
    @location(0) rect: vec4<f32>,
    @location(1) axes: vec4<f32>,
    @location(2) anchor: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) uv: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) blur: vec2<f32>,
    @location(3) @interpolate(flat) uv_bounds: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32, sprite: SpriteIn) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    let local = sprite.rect.xy + corner * sprite.rect.zw;
    let p = sprite.anchor.xy + sprite.axes.xy * local.x + sprite.axes.zw * local.y;
    var out: VsOut;
    out.pos = vec4<f32>(p.x / screen.size.x * 2.0 - 1.0, 1.0 - p.y / screen.size.y * 2.0, 0.0, 1.0);
    out.uv = mix(sprite.uv.xy, sprite.uv.zw, corner);
    out.color = sprite.color;
    out.blur = sprite.anchor.zw;
    out.uv_bounds = sprite.uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var coverage = 0.0;
    if in.blur.x > 0.5 {
        let inset = vec2<f32>(0.5) / vec2<f32>(textureDimensions(blurred_atlas));
        let uv = clamp(in.uv, in.uv_bounds.xy + inset, in.uv_bounds.zw - inset);
        coverage = textureSampleLevel(blurred_atlas, atlas_sampler, uv, i32(in.blur.y), 0.0).r;
    } else {
        coverage = textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r;
    }
    if screen.size.z > 0.5 {
        let luminance = dot(in.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        coverage = pow(coverage, mix(1.8, 1.0, smoothstep(0.1, 0.9, luminance)));
    }
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
