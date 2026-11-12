struct VertexInput {
    @builtin(vertex_index) vertex_idx: u32,
    @location(0) pos: vec2<i32>,
    @location(1) dim: u32,
    @location(2) uv: u32,
    @location(3) color: u32,
    @location(4) content_type_with_srgb: u32,
    @location(5) depth: f32,
}

struct VertexOutput {
    @invariant @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) content_type: u32,
    @location(3) @interpolate(flat) srgb: u32,
    @location(4) @interpolate(flat) extent: f32,
};

struct Params {
    screen_resolution: vec2<u32>,
    _pad: vec2<u32>,
};

@group(0) @binding(0)
var color_atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var mask_atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

@group(1) @binding(0)
var<uniform> params: Params;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    } else {
        return pow((c + 0.055) / 1.055, 2.4);
    }
}

@vertex
fn vs_main(in_vert: VertexInput) -> VertexOutput {
    var pos = in_vert.pos;
    let width = in_vert.dim & 0xffffu;
    let height = (in_vert.dim & 0xffff0000u) >> 16u;
    let color = in_vert.color;
    var uv = vec2<u32>(in_vert.uv & 0xffffu, (in_vert.uv & 0xffff0000u) >> 16u);
    let v = in_vert.vertex_idx;

    let corner_position = vec2<u32>(
        in_vert.vertex_idx & 1u,
        (in_vert.vertex_idx >> 1u) & 1u,
    );

    let corner_offset = vec2<u32>(width, height) * corner_position;

    uv = uv + corner_offset;
    pos = pos + vec2<i32>(corner_offset);

    var vert_output: VertexOutput;

    vert_output.position = vec4<f32>(
        2.0 * vec2<f32>(pos) / vec2<f32>(params.screen_resolution) - 1.0,
        in_vert.depth,
        1.0,
    );

    vert_output.position.y *= -1.0;

    let content_type = in_vert.content_type_with_srgb & 0xffffu;
    let srgb = (in_vert.content_type_with_srgb & 0xffff0000u) >> 16u;

    switch srgb {
        case 0u: {
            vert_output.color = vec4<f32>(
                f32((color & 0x00ff0000u) >> 16u) / 255.0,
                f32((color & 0x0000ff00u) >> 8u) / 255.0,
                f32(color & 0x000000ffu) / 255.0,
                f32((color & 0xff000000u) >> 24u) / 255.0,
            );
        }
        case 1u: {
            vert_output.color = vec4<f32>(
                srgb_to_linear(f32((color & 0x00ff0000u) >> 16u) / 255.0),
                srgb_to_linear(f32((color & 0x0000ff00u) >> 8u) / 255.0),
                srgb_to_linear(f32(color & 0x000000ffu) / 255.0),
                f32((color & 0xff000000u) >> 24u) / 255.0,
            );
        }
        default: {}
    }

    var dim: vec2<u32> = vec2(0u);
    switch content_type {
        case 0u: {
            dim = textureDimensions(color_atlas_texture);
            break;
        }
        case 1u: {
            dim = textureDimensions(mask_atlas_texture);
            break;
        }
        default: {}
    }

    vert_output.content_type = content_type;
    vert_output.srgb = srgb;
    vert_output.extent = f32(max(width, height));

    vert_output.uv = vec2<f32>(uv) / vec2<f32>(dim);

    return vert_output;
}

@fragment
fn fs_main(in_frag: VertexOutput) -> @location(0) vec4<f32> {
    switch in_frag.content_type {
        case 0u: {
            return textureSampleLevel(color_atlas_texture, atlas_sampler, in_frag.uv, 0.0);
        }
        case 1u: {
            var coverage = textureSampleLevel(mask_atlas_texture, atlas_sampler, in_frag.uv, 0.0).x;
            // Folia patch (PATCH.md): match Chromium grayscale contrast for both text polarities.
            if in_frag.srgb == 0u {
                let luminance = dot(in_frag.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
                let polarity = smoothstep(0.1, 0.9, luminance);
                let small = mix(
                    1.0 - smoothstep(18.0, 26.0, in_frag.extent),
                    1.0 - smoothstep(16.0, 20.0, in_frag.extent),
                    polarity,
                );
                // Limit hinting's dense small-stroke edges while preserving isolated thin strokes.
                if small > 0.0 {
                    let texel = 1.0 / vec2<f32>(textureDimensions(mask_atlas_texture));
                    let horizontal = textureSampleLevel(mask_atlas_texture, atlas_sampler,
                        in_frag.uv + vec2<f32>(texel.x, 0.0), 0.0).x
                        + textureSampleLevel(mask_atlas_texture, atlas_sampler,
                        in_frag.uv - vec2<f32>(texel.x, 0.0), 0.0).x;
                    let vertical = textureSampleLevel(mask_atlas_texture, atlas_sampler,
                        in_frag.uv + vec2<f32>(0.0, texel.y), 0.0).x
                        + textureSampleLevel(mask_atlas_texture, atlas_sampler,
                        in_frag.uv - vec2<f32>(0.0, texel.y), 0.0).x;
                    let support = min(horizontal, vertical);
                    let edge = smoothstep(0.4, 0.9, support) * (1.0 - smoothstep(1.4, 1.9, support))
                        * smoothstep(0.65, 0.95, coverage) * small;
                    coverage = max(0.0, coverage - mix(0.14, 0.35, polarity) * edge);
                }
                coverage = pow(coverage, mix(1.05, 0.4, polarity));
            }
            return vec4<f32>(in_frag.color.rgb, in_frag.color.a * coverage);
        }
        default: {
            return vec4<f32>(0.0);
        }
    }
}
