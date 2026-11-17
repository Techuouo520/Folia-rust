// Original cinema depth blur, bright-pass bloom, volumetric projector and film composite.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(0) @binding(2) var depth: texture_depth_multisampled_2d;
@group(0) @binding(3) var soft: texture_2d<f32>;
@group(0) @binding(4) var softer: texture_2d<f32>;
@group(0) @binding(5) var bloom: texture_2d<f32>;
@group(0) @binding(6) var volume: texture_2d<f32>;
@group(0) @binding(7) var<uniform> parameters: vec4<f32>;
@group(0) @binding(8) var noise: texture_3d<f32>;
@group(0) @binding(9) var noise_sampler: sampler;
@group(0) @binding(10) var film_a: texture_2d<f32>;
@group(0) @binding(11) var film_b: texture_2d<f32>;
@group(1) @binding(0) var<uniform> scene: Cinema;
@group(1) @binding(1) var<storage, read> light: Light;

struct Pixel {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Pixel {
    let point = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0))[index];
    return Pixel(vec4(point, 0.0, 1.0), vec2(point.x * 0.5 + 0.5, 0.5 - point.y * 0.5));
}

fn ray(uv: vec2<f32>) -> vec3<f32> {
    return vec3((uv.x * 2.0 - 1.0) * scene.right_tan.w, (1.0 - uv.y * 2.0) * scene.up_tan.w, 1.0);
}

fn distance_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    let position = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2(0), size - 1);
    let value = textureLoad(depth, position, 0);
    let near = scene.forward_near.w;
    let far = scene.render.z;
    return near * far / max(far - value * (far - near), 0.0001) * length(ray(uv));
}

fn coc_at(uv: vec2<f32>) -> f32 {
    return clamp(scene.post.w * (scene.render.w / max(distance_at(uv), 0.05) - 1.0), 0.0, 1.0);
}

@fragment
fn down(input: Pixel) -> @location(0) vec4<f32> {
    let step = parameters.xy;
    let uv0 = input.uv + step * vec2(-0.5, -0.5);
    let uv1 = input.uv + step * vec2(0.5, -0.5);
    let uv2 = input.uv + step * vec2(-0.5, 0.5);
    let uv3 = input.uv + step * vec2(0.5, 0.5);
    var color = (textureSampleLevel(source, image_sampler, uv0, 0.0) + textureSampleLevel(source, image_sampler, uv1, 0.0)
        + textureSampleLevel(source, image_sampler, uv2, 0.0) + textureSampleLevel(source, image_sampler, uv3, 0.0)) * 0.25;
    if parameters.w > 0.5 {
        color.a = (coc_at(uv0) + coc_at(uv1) + coc_at(uv2) + coc_at(uv3)) * 0.25;
    }
    if parameters.z > 0.5 {
        let peak = max(color.r, max(color.g, color.b));
        let knee = cinema_smooth(0.55, 1.0, peak);
        color = vec4(color.rgb * knee * knee, 1.0);
    }
    return color;
}

@fragment
fn blur(input: Pixel) -> @location(0) vec4<f32> {
    let step = parameters.xy;
    var color = textureSampleLevel(source, image_sampler, input.uv, 0.0) * 0.2042;
    let weights = array<f32, 4>(0.1802, 0.1238, 0.0663, 0.0276);
    for (var index = 0u; index < 4u; index += 1u) {
        let offset = step * f32(index + 1u);
        color += (textureSampleLevel(source, image_sampler, input.uv + offset, 0.0)
            + textureSampleLevel(source, image_sampler, input.uv - offset, 0.0)) * weights[index];
    }
    return color;
}

fn hash12(point: vec2<f32>) -> f32 {
    var p = fract(point.xyx * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

fn to_srgb(color: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(max(color, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, color * 12.92, color <= vec3(0.0031308));
}

fn to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color + 0.055) / 1.055, vec3(2.4)), color / 12.92, color <= vec3(0.04045));
}

fn projected_image(image: texture_2d<f32>, aspect: f32, uv: vec2<f32>) -> vec3<f32> {
    let background_uv = vec2(uv.x, 0.5 + (uv.y - 0.5) * 7.57 * aspect / 14.0);
    let background = textureSampleLevel(image, image_sampler, vec2(background_uv.x, 1.0 - background_uv.y), 6.0).rgb;
    let width = aspect * 7.57 / 14.0 * 0.86;
    let centered = vec2((uv.x - 0.5) / width, (uv.y - 0.5) / 0.86);
    let delta = abs(centered) - 0.5;
    let distance = length(max(delta, vec2(0.0))) + min(max(delta.x, delta.y), 0.0);
    let inside = 1.0 - cinema_smooth(-0.01, 0.01, distance);
    let shadow = 0.5 + 0.5 * cinema_smooth(0.0, 0.12, distance);
    let cover_uv = clamp(centered + 0.5, vec2(0.0), vec2(1.0));
    let sharp = textureSampleLevel(image, image_sampler, vec2(cover_uv.x, 1.0 - cover_uv.y), 4.0).rgb;
    let grey = dot(background, vec3(0.2126, 0.7152, 0.0722));
    return mix(mix(vec3(grey), background, 0.7) * 0.26 * shadow, sharp, inside);
}

@fragment
fn beam(input: Pixel) -> @location(0) vec4<f32> {
    let origin = scene.eye_time.xyz;
    let relative = ray(input.uv);
    let direction = normalize(scene.right_tan.xyz * relative.x + scene.up_tan.xyz * relative.y + scene.forward_near.xyz);
    var near = 0.0;
    var far = select(1000.0, -origin.z / direction.z, direction.z < -0.0001);
    far = min(far, distance_at(input.uv));
    let planes = array<vec4<f32>, 4>(
        vec4(0.0, 0.96977335, -0.24400748, -1.0667506),
        vec4(-0.97544098, 0.0, -0.22026087, 6.828087),
        vec4(0.0, -0.99997246, 0.007419151, 8.669762),
        vec4(0.97544098, 0.0, -0.22026087, 6.828087));
    for (var index = 0u; index < 4u; index += 1u) {
        let plane = planes[index];
        let projection = dot(plane.xyz, direction);
        let distance = dot(plane.xyz, origin) + plane.w;
        if abs(projection) < 0.000001 {
            if distance < 0.0 { return vec4(0.0); }
            continue;
        }
        let step = -distance / projection;
        if projection > 0.0 { near = max(near, step); } else { far = min(far, step); }
    }
    if far <= near { return vec4(0.0); }
    let lens = vec3(0.0, 8.9, 31.0);
    let throw_distance = distance(vec3(0.0, 4.885, 0.0), lens);
    let step = (far - near) / 14.0;
    let pixel = vec2(input.position.x, scene.render.y * 0.5 - input.position.y);
    let jitter = fract(52.9829189 * fract(dot(pixel, vec2(0.06711056, 0.00583715))));
    var sum = vec3(0.0);
    for (var index = 0u; index < 14u; index += 1u) {
        let position = origin + direction * (near + (f32(index) + jitter) * step);
        let toward = position - lens;
        let on_screen = lens + toward * (-lens.z / min(toward.z, -0.0001));
        let uv = vec2((on_screen.x + 7.0) / 14.0, (on_screen.y - 1.1) / 7.57);
        let image_uv = clamp(uv, vec2(0.0), vec2(1.0));
        var color = projected_image(film_a, scene.reel.x, image_uv);
        if scene.reel.z < 1.0 {
            color = mix(projected_image(film_b, scene.reel.y, image_uv), color, cinema_smooth(0.42, 0.58, scene.reel.z));
        }
        color /= 1.0 + 0.8 * color;
        let distance = length(toward);
        let spread = min(throw_distance * throw_distance / (distance * distance), 1.6);
        let drift = vec3(scene.atmosphere.z * 0.013, -scene.atmosphere.z * 0.02, scene.atmosphere.z * 0.008);
        let smoke = textureSampleLevel(noise, noise_sampler, position * 0.085 + drift, 0.0).r * 0.62
            + textureSampleLevel(noise, noise_sampler, position * 0.23 - drift * 1.7 + 0.37, 0.0).r * 0.38;
        let density = 0.3 + 1.3 * cinema_smooth(0.35, 0.9, smoke);
        let edge = cinema_smooth(0.0, 0.025, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
        sum += color * density * spread * edge;
    }
    sum *= step * light.projected.w;
    return vec4(to_srgb(min(sum, vec3(1.0))) * 0.55, 1.0);
}

@fragment
fn composite(input: Pixel) -> @location(0) vec4<f32> {
    let sharp = textureSampleLevel(source, image_sampler, input.uv, 0.0);
    let first = textureSampleLevel(soft, image_sampler, input.uv, 0.0);
    let second = textureSampleLevel(softer, image_sampler, input.uv, 0.0);
    let coc = max(coc_at(input.uv), max(first.a, second.a * 0.9));
    var color = mix(sharp.rgb, first.rgb, cinema_smooth(0.02, 0.3, coc));
    color = mix(color, second.rgb, cinema_smooth(0.3, 0.85, coc));
    let offset = 2.0 / scene.render.xy;
    let glow = (textureSampleLevel(bloom, image_sampler, input.uv + offset, 0.0)
        + textureSampleLevel(bloom, image_sampler, input.uv - offset, 0.0)
        + textureSampleLevel(bloom, image_sampler, input.uv + vec2(offset.x, -offset.y), 0.0)
        + textureSampleLevel(bloom, image_sampler, input.uv + vec2(-offset.x, offset.y), 0.0)).rgb * 0.25 * scene.post.z;
    color += glow * (1.0 - color * 0.5);
    color += textureSampleLevel(volume, image_sampler, input.uv, 0.0).rgb;
    let radial = (input.uv - 0.5) * vec2(scene.render.x / scene.render.y, 1.0);
    color *= 1.0 - scene.post.y * cinema_smooth(0.3, 1.15, length(radial));
    let pixels = vec2(input.uv.x, 1.0 - input.uv.y) * scene.render.xy;
    let grain = hash12(floor(pixels / 1.5) + fract(scene.eye_time.w * 7.13) * 917.0) - 0.5;
    color += grain * scene.post.x * (0.35 + 0.65 * (1.0 - color)) * sharp.a;
    color = min(color, vec3(sharp.a));
    return vec4(color, sharp.a);
}

@fragment
fn blit(input: Pixel) -> @location(0) vec4<f32> {
    var color = textureSampleLevel(source, image_sampler, input.uv, 0.0);
    if scene.output.z > 0.5 {
        color = vec4(to_linear(max(color.rgb, vec3(0.0)) / max(color.a, 0.00001)) * color.a, color.a);
    }
    return color;
}
