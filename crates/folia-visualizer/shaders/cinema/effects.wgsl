// Instanced lamp glows and the original 520 drifting projector motes.

struct GlowInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) size_kind: vec2<f32>,
}

struct LightSprite {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) blur: f32,
}

fn sprite_vertex(world: vec3<f32>, pixels: f32, color: vec3<f32>, blur: f32, index: u32) -> LightSprite {
    let uv = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0))[index];
    var position = project_point(world);
    position.x += (uv.x * 2.0 - 1.0) * pixels / scene.render.x * position.w;
    position.y += (uv.y * 2.0 - 1.0) * pixels / scene.render.y * position.w;
    return LightSprite(position, uv, color, blur);
}

@vertex
fn vs_glow(input: GlowInput, @builtin(vertex_index) index: u32) -> LightSprite {
    let depth = max(dot(input.position - scene.eye_time.xyz, scene.forward_near.xyz), 0.1);
    let pixels = input.size_kind.x * scene.render.y / (2.0 * scene.up_tan.w * depth);
    var color = input.color;
    if input.size_kind.y > 1.5 {
        color = scene.cove.xyz * input.color.r;
    } else if input.size_kind.y > 0.5 {
        let seed = fract(sin(dot(input.position.xz, vec2(12.9898, 78.233))) * 43758.5453);
        color *= (0.55 + 0.45 * sin(scene.eye_time.w * (0.6 + seed * 1.4) + seed * 40.0)) * (1.0 - scene.lamp.w);
    }
    color *= min(1.0, pixels * pixels / 4.0);
    return sprite_vertex(input.position, clamp(pixels, 2.0, 220.0), color, 0.0, index);
}

@fragment
fn fs_glow(input: LightSprite) -> @location(0) vec4<f32> {
    let radius = length(input.uv - 0.5) * 2.0;
    if radius > 1.0 { discard; }
    let glow = exp(-radius * radius * 6.0) * (1.0 - radius) + 0.7 * exp(-radius * radius * 60.0);
    return vec4(input.color * glow, 1.0);
}

@vertex
fn vs_dust(@location(0) seed: vec4<f32>, @builtin(vertex_index) index: u32) -> LightSprite {
    let time = scene.atmosphere.w;
    let drift = vec3(sin(time * 0.11 + seed.w * 40.0) * 0.012, time * (0.0018 + 0.003 * seed.w), cos(time * 0.07 + seed.x * 30.0) * 0.006);
    var position = vec3(-7.9, 3.0, 3.0) + fract(seed.xyz + drift) * vec3(15.8, 7.4, 22.0);
    position += vec3(sin(time * 0.6 + seed.y * 50.0), sin(time * 0.47 + seed.z * 40.0), cos(time * 0.53 + seed.w * 20.0)) * 0.05;
    let uv = projectorUv(position);
    let inside = cinema_smooth(0.0, 0.05, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
    let color = select(mix(light.screen[1].xyz, light.screen[2].xyz, uv.x * 2.0 - 1.0), mix(light.screen[0].xyz, light.screen[1].xyz, uv.x * 2.0), uv.x < 0.5);
    let glint = 0.25 + 0.75 * pow(0.5 + 0.5 * sin(time * (1.2 + seed.w * 2.6) + seed.x * 60.0), 4.0);
    let depth = max(dot(position - scene.eye_time.xyz, scene.forward_near.xyz), 0.1);
    let size = (0.014 + 0.03 * seed.w) * scene.render.y / (2.0 * scene.up_tan.w * depth);
    let blur = cinema_smooth(5.0, 1.5, depth);
    let pixels = mix(max(size, 1.5), min(size * 3.0 + 6.0, 44.0), blur);
    let energy = min(1.0, size * size / (pixels * pixels)) * mix(1.0, 3.0, blur);
    return sprite_vertex(position, pixels, color * inside * glint * energy * scene.motion.w, blur, index);
}

@fragment
fn fs_dust(input: LightSprite) -> @location(0) vec4<f32> {
    let radius = length(input.uv - 0.5) * 2.0;
    if radius > 1.0 { discard; }
    let spot = exp(-radius * radius * 4.0);
    let disc = (1.0 - cinema_smooth(0.82, 1.0, radius)) * (0.55 + 0.45 * radius);
    return vec4(input.color * mix(spot, disc, input.blur), 1.0);
}
