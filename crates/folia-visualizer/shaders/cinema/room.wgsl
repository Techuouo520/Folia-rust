// Cinema material and lighting ported from Folia 8006b40f6, AGPL-3.0-only.







struct FragmentOutput {
    @location(0) fragmentColor: vec4<f32>,
}

const HALL_HALF_WIDTH: f32 = 9.5f;
const HALL_CEILING: f32 = 10.6f;
const SCREEN_W: f32 = 14f;
const SCREEN_H: f32 = 7.57f;
const SCREEN_CY: f32 = 4.885f;
const PROSC_Z: f32 = 0.4f;
const PROSC_HALF_WIDTH: f32 = 8.2f;
const PILASTER_X: f32 = 9.05f;
const ROW_FIRST: f32 = 5.6f;
const ROW_PITCH: f32 = 1.05f;
const ROW_COUNT: f32 = 14f;
const ROW_FLAT: f32 = 3f;
const ROW_RISE: f32 = 0.32f;
const ROW_RADIUS: f32 = 40f;
const ROW_TREAD_FRONT: f32 = 0.62f;
const AISLE_INNER: f32 = 3.92f;
const AISLE_OUTER: f32 = 5.08f;
const SIDE_EDGE: f32 = 8.44f;
const BAY_FIRST: f32 = 2.6f;
const BAY_PITCH: f32 = 3.4f;
const BAY_COUNT: f32 = 8f;
const SCONCE_Y: f32 = 4.6f;
const EXIT_Z: f32 = 3.4f;
const EXIT_SIGN_Y: f32 = 2.78f;
const FOOT_Y: f32 = 1.02f;
const FOOT_Z: f32 = 1.92f;
const FOOT_HALF_WIDTH: f32 = 7.5f;
const PROJ_Y: f32 = 8.9f;
const PROJ_Z: f32 = 31f;
const BOX_LAMP_X: f32 = 8.6833f;
const BOX_LAMP_Y: f32 = 5.42f;
const BOX_LAMP_Z0_: f32 = 2.242f;
const BOX_LAMP_Z1_: f32 = 4.258f;
const M_CARPET: i32 = 0i;
const M_WALL: i32 = 1i;
const M_PILASTER: i32 = 2i;
const M_PILASTER_FACE: i32 = 3i;
const M_CEILING: i32 = 4i;
const M_BEAM: i32 = 5i;
const M_COVE: i32 = 6i;
const M_PROSC_WALL: i32 = 7i;
const M_GOLD: i32 = 8i;
const M_MASKING: i32 = 9i;
const M_CURTAIN: i32 = 10i;
const M_FRINGE: i32 = 11i;
const M_STAGE_FRONT: i32 = 12i;
const M_STAGE_TOP: i32 = 13i;
const M_SEAT_FABRIC: i32 = 14i;
const M_SEAT_SHELL: i32 = 15i;
const M_SEAT_ARM: i32 = 16i;
const M_DOOR: i32 = 17i;
const M_EXIT_SIGN: i32 = 18i;
const M_EXIT_HOUSING: i32 = 19i;
const M_SCONCE_GLASS: i32 = 20i;
const M_BRASS: i32 = 21i;
const M_AUDIENCE: i32 = 22i;
const M_BOX_FRONT: i32 = 23i;
const M_BOX_LAMP: i32 = 24i;
const WARM: vec3<f32> = vec3<f32>(1f, 0.62f, 0.3f);
const AMBER: vec3<f32> = vec3<f32>(1f, 0.5f, 0.16f);
const EXIT_GREEN: vec3<f32> = vec3<f32>(0.12f, 1f, 0.38f);
const GOLD: vec3<f32> = vec3<f32>(0.78f, 0.55f, 0.22f);
const WALNUT: vec3<f32> = vec3<f32>(0.075f, 0.042f, 0.024f);

@group(1) @binding(0)
var<uniform> scene: Cinema;
@group(1) @binding(1)
var<storage> light: Light;
var<private> fragmentColor: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;
var<private> vPos_1: vec3<f32>;
var<private> vNormal_1: vec3<f32>;
var<private> vUv_1: vec2<f32>;
var<private> vMat_1: f32;
var<private> vShade_1: f32;
var<private> gl_FrontFacing_1: bool;

fn hash12_(p: vec2<f32>) -> f32 {
    var p3_: vec3<f32>;
    p3_ = fract((vec3<f32>(p.xyx) * 0.1031f));
    p3_ = (p3_ + vec3(dot(p3_, (p3_.yzx + vec3(33.33f)))));
    return fract(((p3_.x + p3_.y) * p3_.z));
}

fn acesFilm(x: vec3<f32>) -> vec3<f32> {
    return clamp(((x * ((2.51f * x) + vec3(0.03f))) / ((x * ((2.43f * x) + vec3(0.59f))) + vec3(0.14f))), vec3(0f), vec3(1f));
}

fn linearToSrgb(c: vec3<f32>) -> vec3<f32> {
    return mix((c * 12.92f), ((1.055f * pow(c, vec3(0.41666666f))) - vec3(0.055f)), step(vec3(0.0031308f), c));
}

fn finishColor(hdr: vec3<f32>) -> vec3<f32> {
    var c: vec3<f32>;
    var l: f32;
    c = (acesFilm((hdr * scene.atmosphere.x)));
    l = dot(c, vec3<f32>(0.2126f, 0.7152f, 0.0722f));
    c = (c * mix(vec3<f32>(0.86f, 0.98f, 1.12f), vec3<f32>(1.06f, 1f, 0.9f), vec3(cinema_smooth(0f, 0.3f, l))));
    c = (linearToSrgb(c));
    c = (c + vec3((((hash12_((vec2<f32>(gl_FragCoord_1.x, (scene.render.y - gl_FragCoord_1.y)) + vec2((fract((scene.eye_time.w * 3.7f)) * 311f))))) - 0.5f) / 255f)));
    return ((c * 0.992f) + vec3<f32>(0.002f, 0.003f, 0.005f));
}

fn aaSin(x: f32) -> f32 {
    return (sin(x) * (1f - cinema_smooth(0.6f, 1.6f, (fwidth(x)))));
}

fn projectorUv(p: vec3<f32>) -> vec2<f32> {
    var o: vec3<f32> = vec3<f32>(0f, 8.9f, 31f);
    var s: vec3<f32>;
    var q: vec3<f32>;
    s = (p - o);
    q = (o + (s * (-(o.z) / min(s.z, -0.0001f))));
    return vec2<f32>(((q.x + 7f) / SCREEN_W), ((q.y - 1.1000001f) / SCREEN_H));
}

fn rowRadius(r: f32) -> f32 {
    return (ROW_RADIUS + (r * ROW_PITCH));
}

fn rowZ(r: f32, x: f32) -> f32 {
    var R: f32;
    R = (rowRadius(r));
    return (-34.4f + sqrt(((R * R) - (x * x))));
}

fn rowFloor(r: f32) -> f32 {
    return (max(0f, ((r - ROW_FLAT) + 1f)) * ROW_RISE);
}

fn rowAt(p: vec3<f32>) -> f32 {
    var R: f32;
    var off: f32;
    R = (rowRadius(5f));
    off = (R - sqrt(((R * R) - (p.x * p.x))));
    return floor(((((p.z + off) - ROW_FIRST) + ROW_TREAD_FRONT) / ROW_PITCH));
}

fn edgeTerm(a: vec3<f32>, b: vec3<f32>, n: vec3<f32>) -> f32 {
    var c: vec3<f32>;
    c = cross(a, b);
    return ((acos(clamp(dot(a, b), -0.9999f, 0.9999f)) * dot(c, n)) / max(length(c), 0.00001f));
}

fn rectFormFactor(p: vec3<f32>, n: vec3<f32>, x0_: f32, x1_: f32) -> f32 {
    var y0_: f32 = 1.1000001f;
    var y1_: f32 = 8.67f;
    var a: vec3<f32>;
    var b: vec3<f32>;
    var c: vec3<f32>;
    var d: vec3<f32>;
    a = normalize((vec3<f32>(x0_, y0_, 0f) - p));
    b = normalize((vec3<f32>(x1_, y0_, 0f) - p));
    c = normalize((vec3<f32>(x1_, y1_, 0f) - p));
    d = normalize((vec3<f32>(x0_, y1_, 0f) - p));
    return (max(0f, ((((edgeTerm(a, b, n)) + (edgeTerm(b, c, n))) + (edgeTerm(c, d, n))) + (edgeTerm(d, a, n)))) / 6.2831855f);
}

fn screenLight(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var h: f32 = 2.3333333f;
    if (p.z < 0.05f) {
        return vec3(0f);
    }
    return ((((light.screen[0]).xyz * (rectFormFactor(p, n, -7f, -(h)))) + ((light.screen[1]).xyz * (rectFormFactor(p, n, -(h), h)))) + ((light.screen[2]).xyz * (rectFormFactor(p, n, h, 7f))));
}

fn pointLight(lp: vec3<f32>, color: vec3<f32>, p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var d: vec3<f32>;
    var d2_: f32;
    var l: vec3<f32>;
    var ndl: f32;
    var e: vec3<f32>;
    d = (lp - p);
    d2_ = max(dot(d, d), 0.03f);
    l = (d * inverseSqrt(d2_));
    ndl = max(dot(n, l), 0f);
    e = ((color * ndl) / vec3(d2_));
    (*spec) = (((*spec)) + (e * pow(max(dot(n, normalize((l + v))), 0f), gloss)));
    return e;
}

fn sconceLight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var local: f32;
    var side: f32;
    var k0_: f32;
    var e: vec3<f32> = vec3(0f);
    var i: i32 = -1i;
    var k: f32;
    var lp: vec3<f32>;
    var toP: vec3<f32>;
    var lobe: f32;
    if (p.x < 0f) {
        local = -1f;
    } else {
        local = 1f;
    }
    side = local;
    k0_ = floor((((p.z - BAY_FIRST) / BAY_PITCH) + 0.5f));
    loop {
        if !((i <= 1i)) {
            break;
        }
        k = (k0_ + f32(i));
        if ((k < 1f) || (k > 7f)) {
            continue;
        }
        lp = vec3<f32>((side * 9.08f), 4.65f, (BAY_FIRST + (k * BAY_PITCH)));
        toP = normalize((p - lp));
        lobe = (0.1f + (2.6f * cinema_smooth(-0.1f, 0.9f, toP.y)));
        e = (e + (pointLight(lp, ((WARM * scene.lamp.y) * lobe), p, n, v, gloss, spec)));
        continuing {
            i = (i + 1i);
        }
    }
    return e;
}

fn stepLight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var ax: f32;
    var local: f32;
    var side: f32;
    var r0_: f32;
    var e: vec3<f32> = vec3(0f);
    var i: i32 = 0i;
    var r: f32;
    var y: f32;
    var edges: array<f32, 3>;
    var j: i32;
    var x: f32;
    var lp: vec3<f32>;
    ax = abs(p.x);
    if ((ax < 3.1200001f) || (p.y > 6f)) {
        return vec3(0f);
    }
    if (p.x < 0f) {
        local = -1f;
    } else {
        local = 1f;
    }
    side = local;
    r0_ = (rowAt(p));
    loop {
        if !((i <= 1i)) {
            break;
        }
        r = (r0_ + f32(i));
        if ((r < 0f) || (r > ROW_COUNT)) {
            continue;
        }
        y = ((rowFloor(r)) + 0.2f);
        edges[0i] = 3.95f;
        edges[1i] = 5.0499997f;
        edges[2i] = 8.469999f;
        j = 0i;
        loop {
            if !((j < 3i)) {
                break;
            }
            x = (edges[j]);
            if (abs((ax - x)) > 2.2f) {
                continue;
            }
            lp = vec3<f32>((side * x), y, ((rowZ(r, x)) - 0.3f));
            e = (e + (pointLight(lp, vec3<f32>(0.05f, 0.025f, 0.008f), p, n, v, gloss, spec)));
            continuing {
                j = (j + 1i);
            }
        }
        continuing {
            i = (i + 1i);
        }
    }
    return e;
}

fn footLight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var c: vec3<f32>;
    var d: vec3<f32>;
    var dist: f32;
    var l: vec3<f32>;
    var aim: f32;
    var e: vec3<f32>;
    c = vec3<f32>(clamp(p.x, -7.5f, FOOT_HALF_WIDTH), FOOT_Y, FOOT_Z);
    d = (c - p);
    dist = max(length(d), 0.05f);
    l = (d / vec3(dist));
    aim = (cinema_smooth(-0.05f, 0.35f, -(l.y)) * cinema_smooth(0.25f, -0.35f, -(l.z)));
    e = ((((vec3<f32>(1f, 0.68f, 0.38f) * scene.lamp.z) * aim) * max(dot(n, l), 0f)) / vec3(((0.25f + (dist * 0.55f)) + ((dist * dist) * 0.12f))));
    (*spec) = (((*spec)) + (e * pow(max(dot(n, normalize((l + v))), 0f), gloss)));
    return e;
}

fn uplight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var local: f32;
    var lp: vec3<f32>;
    var toP: vec3<f32>;
    var cone: f32;
    if (p.x < 0f) {
        local = -9.05f;
    } else {
        local = PILASTER_X;
    }
    lp = vec3<f32>(local, 0.32f, 0.9f);
    toP = normalize((p - lp));
    cone = pow(max(toP.y, 0f), 10f);
    return (pointLight(lp, (((WARM * scene.lamp.z) * 1.6f) * cone), p, n, v, gloss, spec));
}

fn coveLight(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var k: f32;
    var c: vec3<f32>;
    var d: vec3<f32>;
    var dist: f32;
    var l: vec3<f32>;
    if (p.y < 7.6000004f) {
        return vec3(0f);
    }
    k = floor((((p.z - BAY_FIRST) - 0.4f) / BAY_PITCH));
    if ((k < 0f) || (k > 7f)) {
        return vec3(0f);
    }
    c = vec3<f32>(p.x, 9.940001f, ((BAY_FIRST + (k * BAY_PITCH)) + 0.45f));
    d = (c - p);
    dist = max(length(d), 0.05f);
    l = (d / vec3(dist));
    return (((scene.cove.xyz * max(dot(n, l), 0f)) * cinema_smooth(0.1f, -0.2f, l.y)) / vec3((0.15f + ((dist * dist) * 0.5f))));
}

fn boxLight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var local: f32;
    var side: f32;
    var e: vec3<f32> = vec3(0f);
    var i: i32 = 0i;
    var local_4: f32;
    var lp: vec3<f32>;
    if (((p.z > 8f) || (p.y < 2.5f)) || (abs(p.x) < 6.7f)) {
        return vec3(0f);
    }
    if (p.x < 0f) {
        local = -1f;
    } else {
        local = 1f;
    }
    side = local;
    loop {
        if !((i < 2i)) {
            break;
        }
        let _e154: f32 = side;
        if (i == 0i) {
            local_4 = BOX_LAMP_Z0_;
        } else {
            local_4 = BOX_LAMP_Z1_;
        }
        lp = vec3<f32>((_e154 * BOX_LAMP_X), BOX_LAMP_Y, local_4);
        e = (e + (pointLight(lp, (vec3<f32>(0.2f, 0.124000005f, 0.060000002f) * scene.cove.w), p, n, v, gloss, spec)));
        continuing {
            i = (i + 1i);
        }
    }
    return e;
}

fn exitLight(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32, spec: ptr<function, vec3<f32>>) -> vec3<f32> {
    var local: f32;
    var lp: vec3<f32>;
    if (p.x < 0f) {
        local = -1f;
    } else {
        local = 1f;
    }
    lp = vec3<f32>((local * 9.3f), EXIT_SIGN_Y, EXIT_Z);
    return (pointLight(lp, vec3<f32>(0.006f, 0.05f, 0.019f), p, n, v, gloss, spec));
}

fn houseLight(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var sky: vec3<f32> = vec3<f32>(1f, 0.88f, 0.72f);
    var ground: vec3<f32> = vec3<f32>(0.4f, 0.17f, 0.12f);
    var e: vec3<f32>;
    e = mix(ground, sky, vec3((0.5f + (0.5f * n.y))));
    return (((e * scene.lamp.x) * (0.42f + (0.58f * cinema_smooth(0.5f, 10f, p.y)))) * (0.55f + (0.45f * cinema_smooth(22f, 6f, p.z))));
}

fn mirrored(p: vec3<f32>, r: vec3<f32>) -> vec3<f32> {
    var e: vec3<f32>;
    var toScreen: f32;
    e = ((light.ambient.xyz * 2.5f) + ((houseLight(p, r)) * 0.6f));
    toScreen = cinema_smooth(0.75f, 0.98f, dot(r, normalize((vec3<f32>(0f, 4.885f, 0f) - p))));
    e = (e + (((((light.screen[0]).xyz + (light.screen[1]).xyz) + (light.screen[2]).xyz) * 0.04f) * toScreen));
    e = (e + (((vec3<f32>(1f, 0.68f, 0.38f) * scene.lamp.z) * 0.08f) * cinema_smooth(0.1f, -0.5f, r.y)));
    e = (e + (((WARM * scene.lamp.y) * 0.05f) * cinema_smooth(0.3f, 0.9f, abs(r.x))));
    return e;
}

fn sconceScallop(p: vec3<f32>) -> vec3<f32> {
    var k: f32;
    var d: vec2<f32>;
    var up: f32;
    var spread: f32;
    var fan: f32;
    k = floor((((p.z - BAY_FIRST) / BAY_PITCH) + 0.5f));
    if ((k < 1f) || (k > 7f)) {
        return vec3(0f);
    }
    d = vec2<f32>((p.z - (BAY_FIRST + (k * BAY_PITCH))), (p.y - SCONCE_Y));
    up = max(d.y, 0f);
    spread = (0.16f + (0.42f * up));
    fan = ((exp((-((d.x * d.x)) / (spread * spread))) * exp((-(up) / 1.8f))) * cinema_smooth(-0.45f, 0.05f, d.y));
    return (((WARM * fan) * scene.lamp.y) * 0.16f);
}

fn lineMask(d: f32, width: f32) -> f32 {
    var w: f32;
    w = ((fwidth(d)) + 0.0001f);
    return (1f - cinema_smooth(width, (width + w), d));
}

fn main_1() {
    var matId: i32;
    var p: vec3<f32>;
    var n: vec3<f32>;
    var v: vec3<f32>;
    var albedo: vec3<f32> = vec3(0.1f);
    var emissive: vec3<f32> = vec3(0f);
    var ao: f32;
    var specAmount: f32 = 0.04f;
    var gloss: f32 = 12f;
    var velvet: f32 = 0f;
    var metal: f32 = 0f;
    var ax: f32;
    var row: f32;
    var open: f32;
    var cell: vec2<f32>;
    var edge: f32;
    var motif: f32;
    var nose: f32;
    var row_1: f32;
    var local: f32;
    var floorY: f32;
    var h: f32;
    var rail: f32;
    var cell_1: vec2<f32>;
    var seam: f32;
    var fine: f32;
    var pillow: f32;
    var button: f32;
    var s: f32;
    var fillet: f32;
    var band: f32;
    var strand: f32;
    var fade: f32;
    var panel: f32;
    var variation: f32;
    var seam_1: f32;
    var panel_1: f32;
    var bar: f32;
    var q: vec2<f32>;
    var arrowShaft: f32;
    var arrowHead: f32;
    var doorOutline: f32;
    var glyph: f32;
    var rib: f32;
    var panel_2: f32;
    var dn: vec3<f32>;
    var spec: vec3<f32> = vec3(0f);
    var e: vec3<f32>;
    var facing: f32;
    var color: vec3<f32>;
    var sheen: vec3<f32>;
    var dist: f32;
    var air: vec3<f32>;

    matId = i32((vMat_1 + 0.5f));
    p = vPos_1;
    n = normalize(vNormal_1);
    if !(gl_FrontFacing_1) {
        n = -(n);
    }
    v = normalize((scene.eye_time.xyz - p));
    ao = vShade_1;
    if (matId == M_CARPET) {
        ax = abs(p.x);
        row = (rowAt(p));
        open = max((cinema_smooth(3.88f, 3.96f, ax) * (1f - cinema_smooth(5.04f, 5.12f, ax))), cinema_smooth(8.4f, 8.48f, ax));
        if (row < 0f) {
            open = 1f;
        }
        cell = (fract((vec2<f32>((p.x + p.z), (p.x - p.z)) * 1.1f)) - vec2(0.5f));
        edge = (0.5f - max(abs(cell.x), abs(cell.y)));
        motif = ((lineMask(edge, 0.03f)) + (0.6f * (lineMask(length(cell), 0.07f))));
        motif = mix(clamp(motif, 0f, 1f), 0.18f, cinema_smooth(0.08f, 0.25f, ((fwidth((p.x + p.z))) * 1.1f)));
        albedo = mix(vec3<f32>(0.11f, 0.016f, 0.022f), vec3<f32>(0.32f, 0.19f, 0.07f), vec3((motif * 0.6f)));
        nose = (p.z - ((rowZ(max(row, 0f), ax)) - ROW_TREAD_FRONT));
        if (((row >= 0f) && (open > 0.5f)) && (nose < 0.045f)) {
            albedo = vec3<f32>(0.5f, 0.33f, 0.14f);
            specAmount = 0.5f;
            gloss = 40f;
        }
        ao = (0.12f + (0.88f * open));
    } else if (matId == M_WALL) {
        row_1 = (rowAt(vec3<f32>(p.x, 0f, p.z)));
        if (row_1 < 0f) {
            local = 0f;
        } else {
            local = (rowFloor(row_1));
        }
        floorY = local;
        h = (p.y - floorY);
        if (p.y > 7.4f) {
            albedo = (vec3<f32>(0.07f, 0.05f, 0.04f) * (0.85f + (0.15f * (aaSin((p.z * 31.4f))))));
        } else if (h < 1.25f) {
            rail = (lineMask(abs((h - 1.2f)), 0.035f));
            albedo = mix((WALNUT * (0.8f + (0.2f * (aaSin((p.y * 60f)))))), vec3<f32>(0.54599994f, 0.385f, 0.154f), vec3(rail));
            specAmount = mix(0.15f, 0.6f, rail);
            gloss = 30f;
        } else {
            cell_1 = (fract((vec2<f32>((p.z + p.y), (p.z - p.y)) * 1.25f)) - vec2(0.5f));
            seam = (0.5f - max(abs(cell_1.x), abs(cell_1.y)));
            fine = cinema_smooth(0.06f, 0.2f, ((fwidth((p.z + p.y))) * 1.25f));
            pillow = mix(cinema_smooth(0f, 0.25f, seam), 0.75f, fine);
            albedo = (vec3<f32>(0.2f, 0.135f, 0.085f) * (0.6f + (0.4f * pillow)));
            button = ((lineMask(length((abs(cell_1) - vec2<f32>(0.5f, 0f))), 0.035f)) + (lineMask(length((abs(cell_1) - vec2<f32>(0f, 0.5f))), 0.035f)));
            albedo = mix(albedo, vec3<f32>(0.468f, 0.33f, 0.132f), vec3(mix(clamp(button, 0f, 1f), 0.03f, fine)));
            velvet = 0.5f;
            emissive = (((sconceScallop(p)) * albedo) * 4f);
        }
        ao = (ao * (0.55f + (0.45f * cinema_smooth(0f, 1.2f, h))));
    } else if ((matId == M_PILASTER) || (matId == M_PILASTER_FACE)) {
        albedo = WALNUT;
        specAmount = 0.2f;
        gloss = 30f;
        if (matId == M_PILASTER_FACE) {
            s = fract((vUv_1.x * 5f));
            fillet = mix((lineMask(min(s, (1f - s)), 0.06f)), 0.14f, cinema_smooth(0.04f, 0.12f, (fwidth((vUv_1.x * 5f)))));
            albedo = mix((WALNUT * (0.55f + (0.45f * sin((s * 3.14159f))))), vec3<f32>(0.585f, 0.41250002f, 0.16499999f), vec3(fillet));
            specAmount = mix(0.25f, 0.8f, fillet);
            gloss = mix(30f, 60f, fillet);
            emissive = (((sconceScallop(p)) * albedo) * 6f);
        }
    } else if (matId == M_CEILING) {
        albedo = vec3<f32>(0.02f, 0.03f, 0.06f);
    } else if (matId == M_BEAM) {
        albedo = vec3<f32>(0.42f, 0.36f, 0.29f);
        band = (lineMask(abs((p.y - 10.1f)), 0.02f));
        albedo = mix(albedo, GOLD, vec3(band));
    } else if (matId == M_COVE) {
        albedo = vec3(0f);
        emissive = (scene.cove.xyz * 2.6f);
    } else if (matId == M_PROSC_WALL) {
        albedo = (vec3<f32>(0.1f, 0.065f, 0.05f) * (0.82f + (0.18f * (aaSin((p.x * 28f))))));
    } else if (matId == M_GOLD) {
        albedo = GOLD;
        metal = 1f;
        specAmount = 1f;
        gloss = 24f;
    } else if (matId == M_MASKING) {
        albedo = vec3(0.004f);
    } else if (matId == M_CURTAIN) {
        albedo = vec3<f32>(0.36f, 0.022f, 0.03f);
        velvet = 1f;
    } else if (matId == M_FRINGE) {
        strand = (0.5f + (0.5f * sin((vUv_1.x * 5800f))));
        fade = (1f - cinema_smooth(0.3f, 0.8f, (fwidth((vUv_1.x * 5800f)))));
        albedo = ((GOLD * mix(1f, (0.45f + (0.55f * strand)), fade)) * (0.5f + (0.5f * vUv_1.y)));
        metal = 1f;
        specAmount = 0.8f;
        gloss = 18f;
    } else if (matId == M_STAGE_FRONT) {
        panel = (lineMask(((abs((fract((p.x / 1.27f)) - 0.5f)) * 1.27f) - 0.6f), 0.02f));
        albedo = ((WALNUT * (0.75f + (0.25f * (aaSin(((p.x * 9f) + (p.y * 3f))))))) * (1f - (0.5f * panel)));
        specAmount = 0.15f;
    } else if (matId == M_STAGE_TOP) {
        albedo = (vec3<f32>(0.04f, 0.032f, 0.027f) * (0.8f + (0.2f * (0.5f + (0.5f * (aaSin((p.z * 37.7f))))))));
        specAmount = 0.12f;
        gloss = 20f;
    } else if (matId == M_SEAT_FABRIC) {
        variation = (hash12_(floor(((p.xz * 1.6f) + vec2(0.5f)))));
        seam_1 = (lineMask(min(min(vUv_1.x, (1f - vUv_1.x)), min(vUv_1.y, (1f - vUv_1.y))), 0.035f));
        albedo = ((vec3<f32>(0.19f, 0.028f, 0.038f) * (0.9f + (0.2f * variation))) * (1f - (0.45f * seam_1)));
        velvet = 0.4f;
    } else if (matId == M_SEAT_SHELL) {
        albedo = vec3<f32>(0.035f, 0.024f, 0.018f);
        specAmount = 0.35f;
        gloss = 40f;
    } else if (matId == M_SEAT_ARM) {
        albedo = vec3<f32>(0.09f, 0.05f, 0.028f);
        specAmount = 0.5f;
        gloss = 60f;
    } else if (matId == M_DOOR) {
        panel_1 = (lineMask(abs((vUv_1.x - 0.5f)), 0.012f));
        bar = (lineMask(abs((p.y - 1.05f)), 0.025f));
        albedo = mix((vec3<f32>(0.035f, 0.026f, 0.02f) * (1f - (0.6f * panel_1))), vec3<f32>(0.5f, 0.35f, 0.15f), vec3(bar));
        specAmount = mix(0.1f, 0.6f, bar);
    } else if (matId == M_EXIT_SIGN) {
        q = vUv_1;
        arrowShaft = ((step(abs((q.y - 0.5f)), 0.07f) * step(0.2f, q.x)) * step(q.x, 0.48f));
        arrowHead = ((step(abs((q.y - 0.5f)), (0.2f * (1f - ((q.x - 0.44f) / 0.14f)))) * step(0.44f, q.x)) * step(q.x, 0.58f));
        doorOutline = ((((step(0.64f, q.x) * step(q.x, 0.84f)) * step(0.16f, q.y)) * step(q.y, 0.84f)) * (1f - (((step(0.68f, q.x) * step(q.x, 0.8f)) * step(0.24f, q.y)) * step(q.y, 0.84f))));
        glyph = clamp(((arrowShaft + arrowHead) + doorOutline), 0f, 1f);
        albedo = vec3(0f);
        emissive = (mix(vec3<f32>(0.066f, 0.55f, 0.209f), vec3<f32>(0.75f, 1f, 0.85f), vec3(glyph)) * 1.4f);
    } else if (matId == M_EXIT_HOUSING) {
        albedo = vec3(0.02f);
    } else if (matId == M_SCONCE_GLASS) {
        rib = (lineMask(abs((fract((vUv_1.x * 8f)) - 0.5f)), 0.06f));
        albedo = vec3(0f);
        emissive = (((WARM * (0.35f + (0.65f * vUv_1.y))) * (1f - (0.55f * rib))) * ((1.2f * scene.lamp.y) + (0.4f * scene.lamp.w)));
    } else if (matId == M_BRASS) {
        albedo = vec3<f32>(0.5f, 0.33f, 0.14f);
        metal = 1f;
        specAmount = 0.9f;
        gloss = 36f;
    } else if (matId == M_AUDIENCE) {
        albedo = vec3<f32>(0.018f, 0.014f, 0.013f);
        velvet = 1.6f;
    } else if (matId == M_BOX_FRONT) {
        panel_2 = ((lineMask(((abs((fract((vUv_1.x * 6f)) - 0.5f)) * 2f) - 0.86f), 0.02f)) + (lineMask(((abs((vUv_1.y - 0.5f)) * 2f) - 0.8f), 0.03f)));
        albedo = mix((WALNUT * (0.8f + (0.2f * (aaSin((p.y * 40f)))))), vec3<f32>(0.624f, 0.44000003f, 0.176f), vec3(clamp(panel_2, 0f, 1f)));
        specAmount = mix(0.2f, 0.8f, clamp(panel_2, 0f, 1f));
        gloss = 30f;
    } else if (matId == M_BOX_LAMP) {
        albedo = vec3(0f);
        emissive = (WARM * ((1.4f * scene.cove.w) + (0.3f * scene.lamp.w)));
    }
    dn = (fwidth(n));
    gloss = (gloss / (1f + ((gloss * dot(dn, dn)) * 2f)));
    e = ((((((((((screenLight(p, n)) + (boxLight(p, n, v, gloss, (&spec)))) + (sconceLight(p, n, v, gloss, (&spec)))) + (stepLight(p, n, v, gloss, (&spec)))) + (footLight(p, n, v, gloss, (&spec)))) + (uplight(p, n, v, gloss, (&spec)))) + (exitLight(p, n, v, gloss, (&spec)))) + (coveLight(p, n))) + (houseLight(p, n))) + light.ambient.xyz);
    facing = max(dot(n, v), 0f);
    if (metal > 0.5f) {
        color = (((albedo * ((e * 0.3f) + (mirrored(p, reflect(-(v), n))))) * ao) + ((albedo * spec) * specAmount));
    } else {
        color = (((albedo * e) * ao) * (1f - (0.15f * velvet)));
        sheen = mix((albedo * 2.5f), vec3(0.05f), vec3(0.5f));
        color = (color + (((((velvet * e) * ao) * sheen) * 1.6f) * pow((1f - facing), 3f)));
        color = (color + (((spec * specAmount) * 0.25f) * ao));
    }
    color = (color + emissive);
    dist = length((scene.eye_time.xyz - p));
    air = (light.haze.xyz + ((((((light.screen[0]).xyz + (light.screen[1]).xyz) + (light.screen[2]).xyz) * 0.0008f) * cinema_smooth(14f, 2f, p.z)) * (1f - scene.lamp.w)));
    color = mix(color, air, vec3((1f - exp((-(dist) * 0.024f)))));
    fragmentColor = vec4<f32>((finishColor(color)), 1f);
    return;
}

@fragment
fn main(@builtin(position) gl_FragCoord: vec4<f32>, @location(0) vPos: vec3<f32>, @location(1) vNormal: vec3<f32>, @location(2) vUv: vec2<f32>, @location(3) vMat: f32, @location(4) vShade: f32, @builtin(front_facing) gl_FrontFacing: bool) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    vPos_1 = vPos;
    vNormal_1 = vNormal;
    vUv_1 = vUv;
    vMat_1 = vMat;
    vShade_1 = vShade;
    gl_FrontFacing_1 = gl_FrontFacing;
    main_1();
    return FragmentOutput(fragmentColor);
}
