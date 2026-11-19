// Composites a premultiplied lyric target with optional edge-adaptive antialiasing.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Vertex {
    let position = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0))[index];
    var out: Vertex;
    out.position = vec4(position, 0.0, 1.0);
    out.uv = vec2(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
    return out;
}

@fragment
fn plain(in: Vertex) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, in.uv);
}

@fragment
fn antialias(in: Vertex) -> @location(0) vec4<f32> {
    return fxaa(source, source_sampler, in.uv);
}

struct PosterRegion {
    center: vec2<f32>,
    x_axis: vec2<f32>,
    y_axis: vec2<f32>,
    amount: f32,
    padding: f32,
}

struct PosterFilters {
    count: u32,
    srgb_target: u32,
    padding: vec2<u32>,
    faces: array<PosterRegion>,
}

@group(1) @binding(0) var<storage,read> filters: PosterFilters;

@fragment
fn poster_filter(in: Vertex) -> @location(0) vec4<f32> {
    let pixel=textureSample(source,source_sampler,in.uv);
    if pixel.a <= 0.0 {return pixel;}
    for (var i=filters.count; i>0u; i--) {
        let face=filters.faces[i-1u];
        let d=in.uv-face.center;
        if max(abs(dot(d,face.x_axis)),abs(dot(d,face.y_axis)))>1.0 {continue;}
        if face.amount<=0.0 {break;}
        var color=clamp(pixel.rgb/pixel.a,vec3<f32>(0.0),vec3<f32>(1.0));
        if filters.srgb_target!=0u {
            color=select(1.055*pow(color,vec3<f32>(1.0/2.4))-0.055,12.92*color,color<=vec3<f32>(0.0031308));
        }
        color = poster_color(color, face.amount);
        if filters.srgb_target!=0u {
            color=select(pow((color+0.055)/1.055,vec3<f32>(2.4)),color/12.92,color<=vec3<f32>(0.04045));
        }
        return vec4<f32>(color*pixel.a,pixel.a);
    }
    return pixel;
}
