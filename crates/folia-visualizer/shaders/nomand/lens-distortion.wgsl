// Paper Shaders 0.0.80, Copyright 2026 Paper, Apache-2.0. GLSL translated using Naga 27.0.3.
// Source https://github.com/paper-design/shaders; license and notice in assets/paper-shaders.
struct Params {
    data: array<vec4<f32>, 72>,
}

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

@group(0) @binding(0) 
var<uniform> params: Params;
@group(0) @binding(1) 
var cover_texture: texture_2d<f32>;
@group(0) @binding(2) 
var cover_sampler: sampler;
@group(0) @binding(3) 
var noise_texture: texture_2d<f32>;
@group(0) @binding(4) 
var noise_sampler: sampler;
var<private> v_imageUV_1: vec2<f32>;
var<private> fragColor: vec4<f32>;

fn hash21_(p: vec2<f32>) -> f32 {
    var p_1: vec2<f32>;

    p_1 = p;
    let _e11: vec2<f32> = p_1;
    p_1 = (fract((_e11 * vec2<f32>(0.3183099f, 0.3678794f))) + vec2(0.1f));
    let _e20: vec2<f32> = p_1;
    let _e21: vec2<f32> = p_1;
    let _e22: vec2<f32> = p_1;
    p_1 = (_e20 + vec2(dot(_e21, (_e22 + vec2(19.19f)))));
    let _e29: vec2<f32> = p_1;
    let _e31: vec2<f32> = p_1;
    return fract((_e29.x * _e31.y));
}

fn rotate(uv: vec2<f32>, th: f32) -> vec2<f32> {
    var uv_1: vec2<f32>;
    var th_1: f32;

    uv_1 = uv;
    th_1 = th;
    let _e13: f32 = th_1;
    let _e15: f32 = th_1;
    let _e17: f32 = th_1;
    let _e20: f32 = th_1;
    let _e25: vec2<f32> = uv_1;
    return (mat2x2<f32>(vec2<f32>(cos(_e13), sin(_e15)), vec2<f32>(-(sin(_e17)), cos(_e20))) * _e25);
}

fn valueNoise(st: vec2<f32>) -> f32 {
    var st_1: vec2<f32>;
    var i: vec2<f32>;
    var f: vec2<f32>;
    var a: f32;
    var b: f32;
    var c: f32;
    var d: f32;
    var u: vec2<f32>;

    st_1 = st;
    let _e11: vec2<f32> = st_1;
    i = floor(_e11);
    let _e14: vec2<f32> = st_1;
    f = fract(_e14);
    let _e17: vec2<f32> = i;
    let _e18: f32 = hash21_(_e17);
    a = _e18;
    let _e20: vec2<f32> = i;
    let _e25: f32 = hash21_((_e20 + vec2<f32>(1f, 0f)));
    b = _e25;
    let _e27: vec2<f32> = i;
    let _e32: f32 = hash21_((_e27 + vec2<f32>(0f, 1f)));
    c = _e32;
    let _e34: vec2<f32> = i;
    let _e39: f32 = hash21_((_e34 + vec2<f32>(1f, 1f)));
    d = _e39;
    let _e41: vec2<f32> = f;
    let _e42: vec2<f32> = f;
    let _e46: vec2<f32> = f;
    u = ((_e41 * _e42) * (vec2(3f) - (2f * _e46)));
    let _e52: f32 = a;
    let _e53: f32 = b;
    let _e54: vec2<f32> = u;
    let _e57: f32 = c;
    let _e58: f32 = d;
    let _e59: vec2<f32> = u;
    let _e62: vec2<f32> = u;
    return mix(mix(_e52, _e53, _e54.x), mix(_e57, _e58, _e59.x), _e62.y);
}

fn getUvFrame(uv_2: vec2<f32>) -> f32 {
    var uv_3: vec2<f32>;
    var invAA: vec2<f32>;
    var lo: vec2<f32>;
    var hi: vec2<f32>;

    uv_3 = uv_2;
    let _e12: vec2<f32> = uv_3;
    let _e13: vec2<f32> = fwidth(_e12);
    invAA = (vec2(1f) / clamp(_e13, vec2(0.00001f), vec2(0.02f)));
    let _e22: vec2<f32> = uv_3;
    let _e23: vec2<f32> = invAA;
    lo = clamp(((_e22 * _e23) + vec2(0.5f)), vec2(0f), vec2(1f));
    let _e35: vec2<f32> = uv_3;
    let _e38: vec2<f32> = invAA;
    hi = clamp((((vec2(1f) - _e35) * _e38) + vec2(0.5f)), vec2(0f), vec2(1f));
    let _e49: vec2<f32> = lo;
    let _e51: vec2<f32> = hi;
    let _e54: vec2<f32> = lo;
    let _e57: vec2<f32> = hi;
    return (((_e49.x * _e51.x) * _e54.y) * _e57.y);
}

fn sampleOverWhite(uv_4: vec2<f32>) -> vec4<f32> {
    var uv_5: vec2<f32>;
    var img: vec4<f32>;
    var cover: f32;
    var colorOverWhite: vec3<f32>;

    uv_5 = uv_4;
    let _e11: vec2<f32> = uv_5;
    let _e12: vec4<f32> = textureSample(cover_texture, cover_sampler, _e11);
    img = _e12;
    let _e14: vec4<f32> = img;
    let _e16: vec2<f32> = uv_5;
    let _e17: f32 = getUvFrame(_e16);
    cover = (_e14.w * _e17);
    let _e22: vec4<f32> = img;
    let _e24: f32 = cover;
    colorOverWhite = mix(vec3(1f), _e22.xyz, vec3(_e24));
    let _e28: vec3<f32> = colorOverWhite;
    let _e29: f32 = cover;
    return vec4<f32>(_e28.x, _e28.y, _e28.z, _e29);
}

fn hueColor(hue: f32) -> vec3<f32> {
    var hue_1: f32;
    var rgb: vec3<f32>;

    hue_1 = hue;
    let _e11: f32 = hue_1;
    let _e19: vec3<f32> = (vec3((_e11 * 6f)) + vec3<f32>(0f, 4f, 2f));
    let _e21: vec3<f32> = vec3(6f);
    rgb = clamp((abs(((_e19 - (floor((_e19 / _e21)) * _e21)) - vec3(3f))) - vec3(1f)), vec3(0f), vec3(1f));
    let _e39: vec3<f32> = rgb;
    let _e40: vec3<f32> = rgb;
    let _e44: vec3<f32> = rgb;
    rgb = ((_e39 * _e40) * (vec3(3f) - (2f * _e44)));
    let _e49: vec3<f32> = rgb;
    return _e49;
}

fn boxInradius() -> f32 {
    let _e13: vec4<f32> = params.data[14];
    return (0.5f * min(_e13.x, 1f));
}

fn boxOutradius() -> f32 {
    let _e13: vec4<f32> = params.data[14];
    return (0.5f * length(vec2<f32>(_e13.x, 1f)));
}

fn spreadReach() -> f32 {
    let _e13: vec4<f32> = params.data[53];
    let _e20: vec4<f32> = params.data[53];
    return (0.7f * pow(_e13.x, (1.3f + (2.7f * _e20.x))));
}

fn innerCircleMask(radius: f32, inradius: f32) -> f32 {
    var radius_1: f32;
    var inradius_1: f32;

    radius_1 = radius;
    inradius_1 = inradius;
    let _e15: f32 = inradius_1;
    let _e18: f32 = inradius_1;
    let _e20: f32 = radius_1;
    return (1f - smoothstep((0.5f * _e15), (1.1f * _e18), _e20));
}

fn dispersionCurve(t: f32, biasPow: f32) -> f32 {
    var t_1: f32;
    var biasPow_1: f32;
    var local: f32;
    var mirroredT: f32;
    var curved: f32;
    var local_1: f32;

    t_1 = t;
    biasPow_1 = biasPow;
    let _e16: vec4<f32> = params.data[54];
    if (_e16.x < 0f) {
        let _e21: f32 = t_1;
        local = (1f - _e21);
    } else {
        let _e23: f32 = t_1;
        local = _e23;
    }
    let _e25: f32 = local;
    mirroredT = _e25;
    let _e27: f32 = mirroredT;
    let _e28: f32 = biasPow_1;
    curved = pow(_e27, _e28);
    let _e34: vec4<f32> = params.data[54];
    if (_e34.x < 0f) {
        let _e39: f32 = curved;
        local_1 = (1f - _e39);
    } else {
        let _e41: f32 = curved;
        local_1 = _e41;
    }
    let _e43: f32 = local_1;
    return _e43;
}

fn getSpread(fromCenter: vec2<f32>, radius_2: f32, warpedUV: vec2<f32>, edgeAA: f32, inradius_2: f32, outradius: f32, reach: f32, outStrength: ptr<function, f32>) -> vec2<f32> {
    var fromCenter_1: vec2<f32>;
    var radius_3: f32;
    var warpedUV_1: vec2<f32>;
    var edgeAA_1: f32;
    var inradius_3: f32;
    var outradius_1: f32;
    var reach_1: f32;
    var angleRad: f32;
    var uniformDir: vec2<f32>;
    var invInradius: f32;
    var radialDir: vec2<f32>;
    var maxLen: f32;
    var radialLen: f32;
    var spreadDir: vec2<f32>;
    var bandProximity: f32;
    var lensCircleMaxing: f32;
    var warpedAbs: vec2<f32>;
    var inner: f32;
    var boxDist: f32;
    var outer: f32;
    var strength: f32;
    var outside: vec2<f32>;
    var margin: f32;
    var axis: vec2<f32>;
    var turn: f32;
    var cs: f32;
    var sn: f32;

    fromCenter_1 = fromCenter;
    radius_3 = radius_2;
    warpedUV_1 = warpedUV;
    edgeAA_1 = edgeAA;
    inradius_3 = inradius_2;
    outradius_1 = outradius;
    reach_1 = reach;
    let _e27: vec4<f32> = params.data[23];
    angleRad = radians(_e27.x);
    let _e31: f32 = angleRad;
    let _e33: f32 = angleRad;
    uniformDir = vec2<f32>(cos(_e31), sin(_e33));
    let _e38: f32 = inradius_3;
    invInradius = (1f / _e38);
    let _e41: vec2<f32> = fromCenter_1;
    let _e42: f32 = invInradius;
    radialDir = (_e41 * _e42);
    let _e45: f32 = outradius_1;
    let _e46: f32 = reach_1;
    let _e48: f32 = invInradius;
    maxLen = ((_e45 + _e46) * _e48);
    let _e51: f32 = radius_3;
    let _e52: f32 = invInradius;
    radialLen = (_e51 * _e52);
    let _e55: f32 = radialLen;
    let _e56: f32 = maxLen;
    if (_e55 > _e56) {
        let _e58: vec2<f32> = radialDir;
        let _e59: f32 = maxLen;
        let _e60: f32 = radialLen;
        radialDir = (_e58 * (_e59 / _e60));
    }
    let _e63: vec2<f32> = uniformDir;
    let _e64: vec2<f32> = radialDir;
    let _e68: vec4<f32> = params.data[55];
    spreadDir = mix(_e63, _e64, vec2(_e68.x));
    let _e73: f32 = inradius_3;
    let _e76: f32 = inradius_3;
    let _e77: f32 = radius_3;
    bandProximity = smoothstep((_e73 * 0.8f), _e76, _e77);
    let _e80: f32 = bandProximity;
    let _e84: vec4<f32> = params.data[67];
    let _e90: vec4<f32> = params.data[67];
    let _e96: vec4<f32> = params.data[67];
    lensCircleMaxing = (((_e80 * _e84.x) * _e90.x) * _e96.x);
    let _e100: vec2<f32> = spreadDir;
    let _e101: vec2<f32> = radialDir;
    let _e102: f32 = lensCircleMaxing;
    spreadDir = mix(_e100, _e101, vec2(_e102));
    let _e105: vec2<f32> = warpedUV_1;
    warpedAbs = abs((_e105 - vec2(0.5f)));
    let _e113: f32 = inradius_3;
    let _e114: f32 = outradius_1;
    let _e118: vec4<f32> = params.data[60];
    let _e121: f32 = radius_3;
    let _e126: vec4<f32> = params.data[60];
    inner = mix(1f, smoothstep(0f, mix(_e113, _e114, _e118.x), _e121), _e126.x);
    let _e130: vec2<f32> = warpedAbs;
    let _e132: vec2<f32> = warpedAbs;
    boxDist = (max(_e130.x, _e132.y) * 2f);
    let _e140: f32 = boxDist;
    let _e147: vec4<f32> = params.data[61];
    outer = mix(1f, (1f - min(_e140, 1f)), _e147.x);
    let _e151: f32 = inner;
    let _e152: f32 = outer;
    strength = (_e151 * _e152);
    let _e155: f32 = strength;
    let _e162: vec4<f32> = params.data[66];
    let _e168: f32 = lensCircleMaxing;
    strength = (_e155 * mix(1f, mix(0.15f, 0.03f, max(-(_e162.x), 0f)), _e168));
    let _e171: vec2<f32> = warpedAbs;
    outside = max((_e171 - vec2(0.5f)), vec2(0f));
    let _e179: f32 = reach_1;
    let _e180: vec2<f32> = spreadDir;
    let _e187: vec4<f32> = params.data[67];
    let _e191: f32 = edgeAA_1;
    margin = max(((_e179 * length(_e180)) * (1f - _e187.x)), _e191);
    let _e194: f32 = strength;
    let _e197: f32 = margin;
    let _e198: vec2<f32> = outside;
    strength = (_e194 * (1f - smoothstep(0f, _e197, length(_e198))));
    let _e203: f32 = strength;
    (*outStrength) = _e203;
    let _e204: vec2<f32> = spreadDir;
    let _e205: f32 = reach_1;
    let _e206: f32 = strength;
    axis = (_e204 * (_e205 * _e206));
    let _e213: vec4<f32> = params.data[63];
    if (_e213.x > 0f) {
        {
            let _e217: vec2<f32> = fromCenter_1;
            let _e221: vec4<f32> = params.data[64];
            let _e229: vec4<f32> = params.data[65];
            let _e235: f32 = valueNoise((((_e217 * _e221.x) * 18f) + vec2((_e229.x * 30f))));
            let _e243: vec4<f32> = params.data[63];
            turn = (((_e235 - 0.5f) * 2f) * _e243.x);
            let _e247: f32 = turn;
            cs = cos(_e247);
            let _e250: f32 = turn;
            sn = sin(_e250);
            let _e253: f32 = cs;
            let _e254: f32 = sn;
            let _e256: f32 = sn;
            let _e257: f32 = cs;
            let _e261: vec2<f32> = axis;
            axis = (mat2x2<f32>(vec2<f32>(_e253, -(_e254)), vec2<f32>(_e256, _e257)) * _e261);
        }
    }
    let _e264: vec2<f32> = axis;
    let _e269: vec4<f32> = params.data[14];
    axis.x = (_e264.x / _e269.x);
    let _e272: vec2<f32> = axis;
    return _e272;
}

fn lensWarp(fromCenter_2: vec2<f32>, radius_4: f32, inradius_4: f32, bulgeFade: ptr<function, f32>) -> vec2<f32> {
    var fromCenter_3: vec2<f32>;
    var radius_5: f32;
    var inradius_5: f32;
    var r: f32;
    var rn: f32;
    var local_2: f32;
    var bulge: f32;
    var local_3: f32;
    var map: f32;
    var bulgeScale: f32;
    var dir: vec2<f32>;
    var halfBox: vec2<f32>;
    var rBox: f32;
    var band: f32;
    var innerEdge: f32;
    var over: f32;
    var g: f32;

    fromCenter_3 = fromCenter_2;
    radius_5 = radius_4;
    inradius_5 = inradius_4;
    (*bulgeFade) = 1f;
    let _e20: vec4<f32> = params.data[66];
    let _e27: vec4<f32> = params.data[67];
    if ((_e20.x == 0f) && (_e27.x <= 0f)) {
        let _e32: vec2<f32> = fromCenter_3;
        return _e32;
    }
    let _e33: f32 = radius_5;
    if (_e33 < 0.00001f) {
        let _e36: vec2<f32> = fromCenter_3;
        return _e36;
    }
    let _e37: f32 = radius_5;
    r = _e37;
    let _e42: vec4<f32> = params.data[66];
    if (_e42.x != 0f) {
        {
            let _e46: f32 = radius_5;
            let _e47: f32 = inradius_5;
            rn = (_e46 / _e47);
            let _e53: vec4<f32> = params.data[66];
            let _e59: vec4<f32> = params.data[66];
            if (_e59.x > 0f) {
                local_2 = 1.4f;
            } else {
                local_2 = 1.2f;
            }
            let _e66: f32 = local_2;
            bulge = (abs(_e53.x) * _e66);
            let _e72: vec4<f32> = params.data[66];
            if (_e72.x > 0f) {
                let _e79: f32 = rn;
                let _e80: f32 = bulge;
                (*bulgeFade) = (1f - smoothstep(1.45f, 1.53f, (_e79 * _e80)));
            }
            let _e87: vec4<f32> = params.data[66];
            if (_e87.x > 0f) {
                let _e91: f32 = rn;
                let _e92: f32 = bulge;
                let _e97: f32 = bulge;
                local_3 = (tan(min((_e91 * _e92), 1.53f)) / tan(_e97));
            } else {
                let _e100: f32 = rn;
                let _e101: f32 = bulge;
                let _e105: f32 = bulge;
                local_3 = (atan((_e100 * tan(_e101))) / _e105);
            }
            let _e108: f32 = local_3;
            map = _e108;
            let _e110: f32 = map;
            let _e111: f32 = rn;
            bulgeScale = (_e110 / _e111);
            let _e114: vec2<f32> = fromCenter_3;
            let _e115: f32 = bulgeScale;
            fromCenter_3 = (_e114 * _e115);
            let _e117: f32 = r;
            let _e118: f32 = bulgeScale;
            r = (_e117 * _e118);
        }
    }
    let _e123: vec4<f32> = params.data[67];
    if (_e123.x > 0f) {
        {
            let _e127: vec2<f32> = fromCenter_3;
            let _e128: f32 = r;
            dir = (_e127 / vec2(max(_e128, 0.00001f)));
            let _e137: vec4<f32> = params.data[14];
            halfBox = (vec2<f32>(_e137.x, 1f) * 0.5f);
            let _e144: vec2<f32> = halfBox;
            let _e146: vec2<f32> = dir;
            let _e152: vec2<f32> = halfBox;
            let _e154: vec2<f32> = dir;
            rBox = min((_e144.x / max(abs(_e146.x), 0.0001f)), (_e152.y / max(abs(_e154.y), 0.0001f)));
            let _e162: f32 = inradius_5;
            let _e169: vec4<f32> = params.data[66];
            band = (_e162 * mix(0.03f, 0.2f, (0.5f * (_e169.x + 1f))));
            let _e177: f32 = inradius_5;
            let _e178: f32 = band;
            innerEdge = (_e177 - _e178);
            let _e183: f32 = r;
            let _e184: f32 = innerEdge;
            let _e186: f32 = band;
            over = smoothstep(0f, 1f, ((_e183 - _e184) / _e186));
            let _e190: f32 = r;
            let _e191: f32 = rBox;
            let _e192: f32 = inradius_5;
            let _e194: f32 = over;
            g = (_e190 + ((_e191 - _e192) * pow(_e194, 14f)));
            let _e200: vec2<f32> = dir;
            let _e201: f32 = r;
            let _e202: f32 = g;
            let _e206: vec4<f32> = params.data[67];
            fromCenter_3 = (_e200 * mix(_e201, _e202, _e206.x));
        }
    }
    let _e210: vec2<f32> = fromCenter_3;
    return _e210;
}

fn main_1() {
    var uv_6: vec2<f32>;
    var invAspect: f32;
    var inradius_6: f32;
    var outradius_2: f32;
    var reach_2: f32;
    var fromCenter_4: vec2<f32>;
    var radius_6: f32;
    var bulgeFade_1: f32;
    var warpedFromCenter: vec2<f32>;
    var baseUV: vec2<f32>;
    var baseDerivative: vec2<f32>;
    var edgeAA_2: f32;
    var spreadStrength: f32;
    var spreadAxis: vec2<f32>;
    var grainUV: vec2<f32> = vec2(0f);
    var dudx: vec2<f32>;
    var dudy: vec2<f32>;
    var grainSeed: f32;
    var jit: vec2<f32>;
    var count: f32;
    var countInteger: i32;
    var invCount: f32;
    var invSpan: f32;
    var hueBase: f32;
    var biasPower: f32;
    var tapBias: vec2<f32>;
    var centerAmount: f32;
    var edgesAmount: f32;
    var dispersion: f32;
    var dispersionPower: f32;
    var warpedRadius: f32;
    var swirlAngle: f32;
    var colorSum: vec3<f32> = vec3(0f);
    var weightSum: vec3<f32> = vec3(0f);
    var coverSum: f32 = 0f;
    var i_1: i32 = 0i;
    var layer: f32;
    var hue_2: f32;
    var spread: f32;
    var fanPos: f32;
    var tapUV: vec2<f32>;
    var tap: vec4<f32>;
    var weight: vec3<f32>;
    var color: vec3<f32>;
    var coverAvg: f32;
    var ground: f32;
    var alpha: f32;
    var premult: vec3<f32>;
    var grain: f32;
    var grainV: f32;
    var grainStrength: f32;

    let _e9: vec2<f32> = v_imageUV_1;
    uv_6 = _e9;
    let _e15: vec4<f32> = params.data[14];
    invAspect = (1f / _e15.x);
    let _e19: f32 = boxInradius();
    inradius_6 = _e19;
    let _e21: f32 = boxOutradius();
    outradius_2 = _e21;
    let _e23: f32 = spreadReach();
    reach_2 = _e23;
    let _e25: vec2<f32> = uv_6;
    fromCenter_4 = (_e25 - vec2(0.5f));
    let _e31: vec2<f32> = fromCenter_4;
    let _e36: vec4<f32> = params.data[14];
    fromCenter_4.x = (_e31.x * _e36.x);
    let _e39: vec2<f32> = fromCenter_4;
    radius_6 = length(_e39);
    let _e43: vec2<f32> = fromCenter_4;
    let _e44: f32 = radius_6;
    let _e45: f32 = inradius_6;
    let _e48: vec2<f32> = lensWarp(_e43, _e44, _e45, (&bulgeFade_1));
    warpedFromCenter = _e48;
    let _e50: vec2<f32> = warpedFromCenter;
    let _e52: f32 = invAspect;
    let _e54: vec2<f32> = warpedFromCenter;
    baseUV = (vec2<f32>((_e50.x * _e52), _e54.y) + vec2(0.5f));
    let _e61: vec2<f32> = baseUV;
    let _e62: vec2<f32> = fwidth(_e61);
    baseDerivative = _e62;
    let _e65: vec2<f32> = baseDerivative;
    let _e67: vec2<f32> = baseDerivative;
    edgeAA_2 = clamp((2f * max(_e65.x, _e67.y)), 0.001f, 0.02f);
    let _e76: vec2<f32> = fromCenter_4;
    let _e77: f32 = radius_6;
    let _e78: vec2<f32> = baseUV;
    let _e79: f32 = edgeAA_2;
    let _e80: f32 = inradius_6;
    let _e81: f32 = outradius_2;
    let _e82: f32 = reach_2;
    let _e85: vec2<f32> = getSpread(_e76, _e77, _e78, _e79, _e80, _e81, _e82, (&spreadStrength));
    spreadAxis = _e85;
    let _e93: vec4<f32> = params.data[36];
    let _e100: vec4<f32> = params.data[37];
    if ((_e93.x > 0f) || (_e100.x > 0f)) {
        {
            let _e105: vec2<f32> = v_imageUV_1;
            let _e106: vec2<f32> = dpdx(_e105);
            dudx = _e106;
            let _e108: vec2<f32> = v_imageUV_1;
            let _e109: vec2<f32> = dpdy(_e108);
            dudy = _e109;
            let _e111: vec2<f32> = v_imageUV_1;
            let _e116: vec2<f32> = dudx;
            let _e118: vec2<f32> = dudy;
            grainUV = (((_e111 - vec2(0.5f)) * (vec2(0.8f) / vec2<f32>(length(_e116), length(_e118)))) + vec2(0.5f));
        }
    }
    let _e130: vec4<f32> = params.data[36];
    if (_e130.x > 0f) {
        {
            let _e134: vec2<f32> = grainUV;
            let _e135: f32 = valueNoise(_e134);
            grainSeed = _e135;
            let _e137: f32 = grainSeed;
            jit = ((fract((_e137 * vec2<f32>(157.31f, 113.57f))) * 2f) - vec2(1f));
            let _e149: vec2<f32> = spreadAxis;
            let _e150: vec2<f32> = jit;
            let _e156: vec4<f32> = params.data[36];
            let _e159: vec2<f32> = spreadAxis;
            spreadAxis = (_e149 + (((_e150 * 0.3f) * _e156.x) * length(_e159)));
        }
    }
    let _e166: vec4<f32> = params.data[56];
    count = floor(_e166.x);
    let _e170: f32 = count;
    countInteger = i32(_e170);
    let _e174: f32 = count;
    invCount = (1f / _e174);
    let _e178: f32 = count;
    invSpan = (1f / max((_e178 - 1f), 1f));
    let _e188: vec4<f32> = params.data[59];
    hueBase = (_e188.x + 0.5f);
    let _e198: vec4<f32> = params.data[54];
    biasPower = (1f + (2f * abs(_e198.x)));
    let _e208: vec4<f32> = params.data[68];
    let _e213: vec4<f32> = params.data[69];
    tapBias = (vec2(0.5f) - vec2<f32>(_e208.x, _e213.x));
    let _e223: vec4<f32> = params.data[58];
    centerAmount = clamp((1f - _e223.x), 0f, 1f);
    let _e234: vec4<f32> = params.data[58];
    edgesAmount = clamp((1f + _e234.x), 0f, 1f);
    let _e244: vec4<f32> = params.data[57];
    let _e246: f32 = edgesAmount;
    let _e247: f32 = centerAmount;
    let _e248: f32 = radius_6;
    let _e249: f32 = inradius_6;
    let _e250: f32 = innerCircleMask(_e248, _e249);
    dispersion = (_e244.x * mix(_e246, _e247, _e250));
    let _e254: f32 = dispersion;
    dispersionPower = pow(_e254, 0.8f);
    let _e258: vec2<f32> = warpedFromCenter;
    warpedRadius = length(_e258);
    let _e264: vec4<f32> = params.data[62];
    let _e270: f32 = spreadStrength;
    let _e275: vec4<f32> = params.data[53];
    let _e279: f32 = inradius_6;
    let _e280: f32 = warpedRadius;
    swirlAngle = (((((_e264.x * 0.4f) * 3.1415927f) * _e270) * _e275.x) * min(1f, (_e279 / max(_e280, 0.0001f))));
    loop {
        let _e297: i32 = i_1;
        if !((_e297 < 50i)) {
            break;
        }
        {
            let _e304: i32 = i_1;
            let _e305: i32 = countInteger;
            if (_e304 >= _e305) {
                break;
            }
            let _e307: i32 = i_1;
            layer = f32(_e307);
            let _e310: f32 = hueBase;
            let _e311: f32 = layer;
            let _e312: f32 = invCount;
            hue_2 = (_e310 + (_e311 * _e312));
            let _e316: f32 = layer;
            let _e317: f32 = invSpan;
            spread = (_e316 * _e317);
            let _e323: vec4<f32> = params.data[54];
            if (_e323.x != 0f) {
                let _e327: f32 = spread;
                let _e328: f32 = biasPower;
                let _e329: f32 = dispersionCurve(_e327, _e328);
                spread = _e329;
            }
            let _e332: f32 = spread;
            fanPos = (1f - (2f * _e332));
            let _e336: vec2<f32> = baseUV;
            let _e337: vec2<f32> = spreadAxis;
            let _e338: f32 = fanPos;
            tapUV = ((_e336 + (_e337 * _e338)) - vec2(0.5f));
            let _e348: vec4<f32> = params.data[62];
            if (_e348.x != 0f) {
                {
                    let _e353: vec2<f32> = tapUV;
                    let _e358: vec4<f32> = params.data[14];
                    tapUV.x = (_e353.x * _e358.x);
                    let _e361: vec2<f32> = tapUV;
                    let _e362: f32 = swirlAngle;
                    let _e363: f32 = fanPos;
                    let _e365: vec2<f32> = rotate(_e361, (_e362 * _e363));
                    tapUV = _e365;
                    let _e367: vec2<f32> = tapUV;
                    let _e369: f32 = invAspect;
                    tapUV.x = (_e367.x * _e369);
                }
            }
            let _e371: vec2<f32> = tapUV;
            let _e372: vec2<f32> = tapBias;
            let _e374: vec4<f32> = sampleOverWhite((_e371 + _e372));
            tap = _e374;
            let _e377: f32 = dispersionPower;
            let _e378: f32 = hue_2;
            let _e379: vec3<f32> = hueColor(_e378);
            weight = (vec3(1f) - (_e377 * _e379));
            let _e384: vec3<f32> = colorSum;
            let _e385: vec4<f32> = tap;
            let _e387: vec3<f32> = weight;
            colorSum = (_e384 + (_e385.xyz * _e387));
            let _e390: vec3<f32> = weightSum;
            let _e391: vec3<f32> = weight;
            weightSum = (_e390 + _e391);
            let _e393: f32 = coverSum;
            let _e394: vec4<f32> = tap;
            coverSum = (_e393 + _e394.w);
        }
        continuing {
            let _e301: i32 = i_1;
            i_1 = (_e301 + 1i);
        }
    }
    let _e397: vec3<f32> = colorSum;
    let _e398: vec3<f32> = weightSum;
    color = (_e397 / max(_e398, vec3(0.0001f)));
    let _e404: f32 = coverSum;
    let _e405: f32 = invCount;
    coverAvg = (_e404 * _e405);
    let _e408: vec3<f32> = color;
    let _e410: vec3<f32> = color;
    let _e412: vec3<f32> = color;
    ground = min(_e408.x, min(_e410.y, _e412.z));
    let _e417: f32 = coverAvg;
    let _e419: f32 = ground;
    alpha = max(_e417, (1f - _e419));
    let _e423: vec3<f32> = color;
    let _e425: f32 = alpha;
    premult = max((_e423 - vec3((1f - _e425))), vec3(0f));
    let _e433: vec3<f32> = premult;
    let _e434: f32 = alpha;
    let _e439: f32 = bulgeFade_1;
    fragColor = (vec4<f32>(_e433.x, _e433.y, _e433.z, _e434) * _e439);
    let _e444: vec4<f32> = params.data[37];
    if (_e444.x > 0f) {
        {
            let _e448: vec2<f32> = grainUV;
            let _e450: vec2<f32> = rotate(_e448, 1f);
            let _e454: f32 = valueNoise((_e450 + vec2(3f)));
            grain = _e454;
            let _e456: f32 = grain;
            let _e457: vec2<f32> = grainUV;
            let _e459: vec2<f32> = rotate(_e457, 2f);
            let _e464: f32 = valueNoise((_e459 + vec2(-1f)));
            grain = mix(_e456, _e464, 0.5f);
            let _e467: f32 = grain;
            grain = pow(_e467, 1.3f);
            let _e470: f32 = grain;
            grainV = ((_e470 * 2f) - 1f);
            let _e479: vec4<f32> = params.data[37];
            let _e481: f32 = grainV;
            let _e486: vec4<f32> = fragColor;
            grainStrength = (pow((_e479.x * abs(_e481)), 0.8f) * _e486.w);
            let _e490: vec4<f32> = fragColor;
            let _e492: vec4<f32> = fragColor;
            let _e495: f32 = grainV;
            let _e498: vec4<f32> = fragColor;
            let _e502: f32 = grainStrength;
            let _e505: vec3<f32> = mix(_e492.xyz, (vec3(step(0f, _e495)) * _e498.w), vec3((0.35f * _e502)));
            fragColor.x = _e505.x;
            fragColor.y = _e505.y;
            fragColor.z = _e505.z;
            return;
        }
    } else {
        return;
    }
}

@fragment 
fn main(@location(0) v_imageUV: vec2<f32>) -> FragmentOutput {
    v_imageUV_1 = v_imageUV;
    main_1();
    let _e18: vec4<f32> = fragColor;
    let paper = _e18;
    let background = params.data[12].rgb;
    let covered = paper.rgb + background * (1.0 - paper.a);
    let visible = select(background, covered, params.data[71].x > 0.5);
    return FragmentOutput(vec4<f32>(mix(visible, background, params.data[71].y), 1.0));
}

struct ImageVertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) index: u32) -> ImageVertex {
    var corners = array<vec2<f32>, 6>(vec2<f32>(-1.,-1.),vec2<f32>(1.,-1.),vec2<f32>(-1.,1.),vec2<f32>(-1.,1.),vec2<f32>(1.,-1.),vec2<f32>(1.,1.));
    let p = corners[index];
    let resolution = params.data[0].xy;
    let aspect = params.data[14].x;
    let height = max(resolution.x / aspect, resolution.y);
    let image_box = vec2<f32>(height * aspect, height);
    let box_scale = resolution / image_box;
    var uv = p * .5;
    uv *= box_scale;
    uv += vec2<f32>(.5-params.data[2].x,params.data[3].x-.5) * (box_scale-vec2<f32>(1.));
    uv += vec2<f32>(-params.data[9].x,params.data[10].x);
    uv /= params.data[7].x;
    uv.x *= aspect;
    let angle = params.data[8].x * 3.14159265358979323846 / 180.;
    uv = mat2x2<f32>(vec2<f32>(cos(angle),sin(angle)),vec2<f32>(-sin(angle),cos(angle))) * uv;
    uv.x /= aspect;
    uv += vec2<f32>(.5);
    uv.y = 1. - uv.y;
    return ImageVertex(vec4<f32>(p,0.,1.),uv);
}
