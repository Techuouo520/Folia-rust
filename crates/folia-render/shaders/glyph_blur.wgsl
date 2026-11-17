// Separable Gaussian filtering downsamples only after each corresponding axis has been filtered.

@group(0) @binding(0) var<uniform> view: vec4<f32>;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;
@group(0) @binding(3) var<storage, read> taps: array<vec2<f32>>;

struct Job {
    @location(0) source: vec4<f32>,
    @location(1) scratch: vec4<f32>,
    @location(2) destination: vec4<f32>,
    @location(3) values: vec4<f32>,
}

struct Pixel {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) source: vec4<f32>,
    @location(1) @interpolate(flat) scratch: vec4<f32>,
    @location(2) @interpolate(flat) destination: vec4<f32>,
    @location(3) @interpolate(flat) values: vec4<f32>,
}

@vertex
fn vs(job: Job, @builtin(vertex_index) index: u32) -> Pixel {
    let corner = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0))[index];
    let rect = select(job.scratch, job.destination, view.z > 0.5);
    let point = rect.xy + corner * rect.zw;
    return Pixel(vec4(point.x / view.x * 2.0 - 1.0, 1.0 - point.y / view.y * 2.0, 0.0, 1.0), job.source, job.scratch, job.destination, job.values);
}

fn coverage(point: vec2<f32>, bounds: vec4<f32>) -> f32 {
    if any(point < bounds.xy) || any(point >= bounds.xy + bounds.zw) { return 0.0; }
    let texel = clamp(point, bounds.xy + 0.5, bounds.xy + bounds.zw - 0.5);
    return textureSampleLevel(image, image_sampler, texel / vec2<f32>(textureDimensions(image)), 0.0).r;
}

@fragment
fn fs(input: Pixel) -> @location(0) vec4<f32> {
    let vertical = view.z > 0.5;
    let local = input.position.xy - select(input.scratch.xy, input.destination.xy, vertical);
    let point = select(input.source.xy + vec2(local.x * input.values.z, local.y) - input.values.y,
        input.scratch.xy + vec2(local.x, local.y * input.values.z), vertical);
    let bounds = select(input.source, input.scratch, vertical);
    let axis = select(vec2(1.0, 0.0), vec2(0.0, 1.0), vertical);
    if (!vertical && (point.y < bounds.y || point.y >= bounds.y + bounds.w))
        || (vertical && (point.x < bounds.x || point.x >= bounds.x + bounds.z)) {
        return vec4(0.0);
    }
    let first = u32(input.values.w);
    let count = u32(input.values.x);
    var sum = coverage(point, bounds) * taps[first].y;
    for (var index = 1u; index < count; index += 1u) {
        let tap = taps[first + index];
        sum += (coverage(point + axis * tap.x, bounds) + coverage(point - axis * tap.x, bounds)) * tap.y;
    }
    return vec4(sum, 0.0, 0.0, 1.0);
}
