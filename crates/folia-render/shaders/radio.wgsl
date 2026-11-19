// CSS rotateY reveal and pointer tilt project the complete face with perspective-correct sampling.
struct Plane { rect:vec4<f32>, rotation:vec4<f32>, effect:vec4<f32>, color:vec4<f32> }
struct Parameters { size:vec4<f32>, planes:array<Plane> }
@group(0) @binding(0) var face:texture_2d<f32>;
@group(0) @binding(1) var linear_sampler:sampler;
@group(1) @binding(0) var<storage,read> parameters:Parameters;
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) alpha:f32, @location(2) color:vec3<f32> }
@vertex fn vs(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->Vertex {
    let corners=array<vec2<f32>,6>(vec2<f32>(0,0),vec2<f32>(1,0),vec2<f32>(0,1),vec2<f32>(0,1),vec2<f32>(1,0),vec2<f32>(1,1));
    let p=parameters.planes[instance];
    let uv=corners[vertex];
    let local=(uv-0.5)*p.rect.zw;
    let angle=p.effect.w;
    var v=vec3<f32>(local.x*cos(angle),local.y,-local.x*sin(angle));
    let inner_w=max(0.1,1.0-v.z/1400.0);
    v=vec3<f32>(v.xy/inner_w,0.0)*p.rotation.w;
    let x=p.rotation.x;
    let y=p.rotation.y;
    v=vec3<f32>(v.x*cos(y)+v.z*sin(y),v.y,-v.x*sin(y)+v.z*cos(y));
    v=vec3<f32>(v.x,v.y*cos(x)-v.z*sin(x),v.y*sin(x)+v.z*cos(x));
    let w=max(0.1,1.0-v.z/p.effect.z);
    let z=p.rotation.z;
    let rotated=vec2<f32>(v.x*cos(z)-v.y*sin(z),v.x*sin(z)+v.y*cos(z));
    let center=p.rect.xy+p.rect.zw*0.5+vec2<f32>(0.0,p.effect.x);
    let screen=rotated+center*w;
    var out:Vertex;
    out.position=vec4<f32>(screen/parameters.size.xy*vec2<f32>(2,-2)+vec2<f32>(-1,1)*w,0.0,w)*inner_w;
    let face_uv=vec2<f32>(select(uv.x,1.0-uv.x,cos(angle)<0.0),uv.y);
    out.uv=(p.rect.xy+face_uv*p.rect.zw)/parameters.size.xy;
    out.alpha=p.effect.y;
    out.color=p.color.xyz;
    return out;
}
@fragment fn fs(v:Vertex)->@location(0) vec4<f32> {
    let pixel=textureSample(face,linear_sampler,v.uv);
    let rgb=pixel.rgb/max(pixel.a,0.00001);
    let luma=dot(rgb,vec3<f32>(0.2126,0.7152,0.0722));
    let grey=mix(rgb,vec3<f32>(luma),v.color.x);
    let saturated=mix(vec3<f32>(luma),grey,v.color.z)*v.color.y;
    return vec4<f32>(clamp(saturated,vec3<f32>(0),vec3<f32>(1))*pixel.a,pixel.a)*v.alpha;
}
