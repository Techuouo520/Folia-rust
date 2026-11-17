// Perspective meshes, moving velvet and the high-resolution projected content surface.

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;

struct MeshVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_shade: vec2<f32>,
    @location(4) matrix0: vec4<f32>,
    @location(5) matrix1: vec4<f32>,
    @location(6) matrix2: vec4<f32>,
    @location(7) matrix3: vec4<f32>,
}

struct HallVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: f32,
    @location(4) shade: f32,
}

fn project_point(point: vec3<f32>) -> vec4<f32> {
    let relative = point - scene.eye_time.xyz;
    let depth = dot(relative, scene.forward_near.xyz);
    let near = scene.forward_near.w;
    let far = scene.render.z;
    return vec4<f32>(dot(relative, scene.right_tan.xyz) / scene.right_tan.w,
        dot(relative, scene.up_tan.xyz) / scene.up_tan.w,
        (far * depth - near * far) / (far - near), depth);
}

fn mesh_vertex(input: MeshVertex) -> HallVertex {
    let matrix = mat4x4<f32>(input.matrix0, input.matrix1, input.matrix2, input.matrix3);
    let world = matrix * vec4<f32>(input.position, 1.0);
    let normal = normalize(mat3x3<f32>(input.matrix0.xyz, input.matrix1.xyz, input.matrix2.xyz) * input.normal);
    return HallVertex(project_point(world.xyz), world.xyz, normal, input.uv, input.material_shade.x, input.material_shade.y);
}

@vertex
fn vs_room(input: MeshVertex) -> HallVertex {
    return mesh_vertex(input);
}

fn curtain_point(side: f32, pose: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let y = mix(0.95, 9.8, uv.y);
    let closure = min(1.0, pose.x - (pose.x - pose.y) * pow(1.0 - uv.y, 1.5));
    let drawn = max(0.0, closure);
    let distance = (y - 3.1) / 1.9;
    let gather = exp(-distance * distance) * (1.0 - drawn);
    let inner = 7.12 + 0.62 * gather + 0.12 * cinema_smooth(3.1, 9.7, y);
    let x = mix(mix(inner, -0.12, closure), 8.4, uv.x);
    let depth = (0.07 + 0.07 * gather) * (1.0 - 0.45 * drawn);
    let fine = 0.05 * drawn;
    let wave = depth * (0.62 * sin(uv.x * 6.28318530718 * 7.3 + 1.3)
        + 0.28 * sin(uv.x * 6.28318530718 * 15.7 + 0.4)
        + 0.1 * sin(uv.x * 6.28318530718 * 3.1 + 2.2))
        + fine * sin(uv.x * 6.28318530718 * 31.0 + 0.9);
    let under = cinema_smooth(8.2, 8.8, y);
    let lip = select(0.0, 0.1 * drawn * (1.0 - cinema_smooth(0.02, 0.05, uv.x)) * (1.0 - 0.5 * under), side < 0.0);
    let z = mix(0.2, 0.1, under) + wave * (1.0 - 0.55 * under) + lip + 0.05 * gather;
    return vec4<f32>(side * x, y, z, 0.4 + 0.6 * cinema_smooth(-depth - fine, depth + fine, wave));
}

@vertex
fn vs_curtain(input: MeshVertex) -> HallVertex {
    let side = select(1.0, -1.0, input.position.x < 0.0);
    let pose = select(scene.curtains.zw, scene.curtains.xy, side < 0.0);
    if max(abs(pose.x), abs(pose.y)) < 0.0001 {
        return mesh_vertex(input);
    }
    let point = curtain_point(side, pose, input.uv);
    let du = curtain_point(side, pose, clamp(input.uv + vec2(0.001, 0.0), vec2(0.0), vec2(1.0))).xyz
        - curtain_point(side, pose, clamp(input.uv - vec2(0.001, 0.0), vec2(0.0), vec2(1.0))).xyz;
    let dv = curtain_point(side, pose, clamp(input.uv + vec2(0.0, 0.001), vec2(0.0), vec2(1.0))).xyz
        - curtain_point(side, pose, clamp(input.uv - vec2(0.0, 0.001), vec2(0.0), vec2(1.0))).xyz;
    let normal = normalize(cross(du, dv)) * side;
    return HallVertex(project_point(point.xyz), point.xyz, normal, input.uv, 10.0, point.w);
}

fn screen_vertex(index: u32, margin: f32) -> HallVertex {
    let uv = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
        vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0))[index];
    let world = vec3((uv.x - 0.5) * (14.0 + margin * 2.0), 4.885 + (uv.y - 0.5) * (7.57 + margin * 2.0), 0.02);
    return HallVertex(project_point(world), world, vec3(0.0, 0.0, 1.0), uv, 9.0, 1.0);
}

@vertex
fn vs_screen(@builtin(vertex_index) index: u32) -> HallVertex {
    return screen_vertex(index, 0.32);
}

@vertex
fn vs_picture(@builtin(vertex_index) index: u32) -> HallVertex {
    return screen_vertex(index, 0.0);
}

@fragment
fn fs_picture(input: HallVertex) -> @location(0) vec4<f32> {
    let uv = vec2(input.uv.x, 1.0 - input.uv.y);
    if scene.output.w > 0.5 { return fxaa(picture, picture_sampler, uv); }
    return textureSample(picture, picture_sampler, uv);
}

@fragment
fn fs_screen(input: HallVertex) -> @location(0) vec4<f32> {
    gl_FragCoord_1 = input.position;
    let uv = (input.uv - 0.5) * (vec2(1.0) + 2.0 * vec2(0.32 / 14.0, 0.32 / 7.57)) + 0.5;
    let thrown = (light.screen[0].xyz + light.screen[1].xyz + light.screen[2].xyz) * (1.0 - scene.lamp.w);
    let outside = max(abs(uv - 0.5) - 0.5, vec2(0.0)) * vec2(14.0, 7.57);
    let distance = length(outside);
    if distance > 0.0 {
        return vec4(finishColor(vec3(0.004) + thrown * 0.0035 * exp(-distance * 16.0)), 1.0);
    }
    let bare = 1.0 - scene.atmosphere.y;
    return vec4(finishColor(vec3(0.5, 0.48, 0.45) * scene.lamp.w) * bare, bare);
}
