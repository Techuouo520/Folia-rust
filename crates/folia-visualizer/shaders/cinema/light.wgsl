// The projected surface supplies three averaged light colors directly on the GPU.

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(1) @binding(0) var<uniform> scene: Cinema;
@group(1) @binding(1) var<storage, read_write> light: Light;

fn to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color + 0.055) / 1.055, vec3(2.4)), color / 12.92, color <= vec3(0.04045));
}

fn to_srgb(color: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(max(color, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, color * 12.92, color <= vec3(0.0031308));
}

fn third_color(third: u32) -> vec3<f32> {
    var average = vec3(0.0);
    for (var y = 0u; y < 8u; y += 1u) {
        for (var x = 0u; x < 8u; x += 1u) {
            let uv = vec2((f32(third * 8u + x) + 0.5) / 24.0, (f32(y) + 0.5) / 8.0);
            var color = textureSampleLevel(picture, picture_sampler, uv, 0.0).rgb;
            if scene.output.z > 0.5 { color = to_srgb(color); }
            average += color;
        }
    }
    let color = to_linear(average / 64.0);
    let luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
    var vivid = max(vec3(0.0), luminance + (color - luminance) * 1.3);
    if luminance < 0.05 { vivid *= sqrt(0.05 / max(luminance, 0.004)); }
    return vivid;
}

@compute @workgroup_size(1)
fn sample_screen() {
    let day = scene.lamp.w;
    let shutter = scene.atmosphere.y;
    var average = vec3(0.0);
    var projected = vec3(0.0);
    let now = scene.eye_time.w;
    let sampling = scene.output.y > 0.5 && (light.state.x < 0.5 || now - light.state.y >= 0.25);
    if scene.reel.w > 0.5 { light.state.w = 0.0; }
    if sampling {
        for (var index = 0u; index < 3u; index += 1u) {
            light.sampled[index] = vec4(third_color(index), 1.0);
            if light.state.z == 0.0 { light.raw[index] = light.sampled[index]; }
        }
        light.state.x = 1.0;
        light.state.y = now;
        light.state.w = 1.0;
    }
    light.state.z = mix(light.state.z, light.state.w, 1.0 - exp(-2.0 * scene.output.x));
    if light.state.w == 0.0 && light.state.z < 0.001 { light.state.z = 0.0; }
    let cross = cinema_smooth(0.42, 0.58, scene.reel.z);
    for (var index = 0u; index < 3u; index += 1u) {
        light.raw[index] = mix(light.raw[index], light.sampled[index], 1.0 - exp(-3.0 * scene.output.x));
        let weight = select(0.32, 1.0, index == 1u);
        let reel = mix(scene.reel_light_b[index].xyz, scene.reel_light_a[index].xyz, cross) * weight;
        let mixed = mix(reel, light.raw[index].xyz, light.state.z) * (1.0 - day) * shutter + vec3(day * 0.32);
        let brightness = mixed * scene.light_cover.w;
        projected += brightness;
        light.screen[index] = vec4(brightness * scene.light_cover[index], 0.0);
        average += light.screen[index].xyz;
    }
    average /= 3.0;
    light.ambient = vec4(average * 0.012 + vec3(0.0018, 0.002, 0.0027) * (1.0 - day), 0.0);
    light.haze = vec4(average * 0.004 * (1.0 - day) + vec3(0.2, 0.15, 0.1) * day * 0.1, 0.0);
    let luminance = dot(projected, vec3(0.2126, 0.7152, 0.0722)) / (3.0 * max(scene.light_cover.w, 0.001));
    let beam = mix(0.06, 0.006, day) * (1.0 + 0.18 * scene.motion.y) * min(1.6, 0.16 / max(luminance, 0.02)) * shutter;
    light.projected = vec4(projected, beam);
}
