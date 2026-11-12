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

fn valueNoise(st: vec2<f32>) -> f32 {
    var st_1: vec2<f32>;
    var i: vec2<f32>;
    var f: vec2<f32>;
    var a: f32;
    var b: f32;
    var c: f32;
    var d: f32;
    var u: vec2<f32>;
    var x1_: f32;
    var x2_: f32;

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
    x1_ = mix(_e52, _e53, _e54.x);
    let _e58: f32 = c;
    let _e59: f32 = d;
    let _e60: vec2<f32> = u;
    x2_ = mix(_e58, _e59, _e60.x);
    let _e64: f32 = x1_;
    let _e65: f32 = x2_;
    let _e66: vec2<f32> = u;
    return mix(_e64, _e65, _e66.y);
}

fn lst(edge0_: f32, edge1_: f32, x: f32) -> f32 {
    var edge0_1: f32;
    var edge1_1: f32;
    var x_1: f32;

    edge0_1 = edge0_;
    edge1_1 = edge1_;
    x_1 = x;
    let _e15: f32 = x_1;
    let _e16: f32 = edge0_1;
    let _e18: f32 = edge1_1;
    let _e19: f32 = edge0_1;
    return clamp(((_e15 - _e16) / (_e18 - _e19)), 0f, 1f);
}

fn sst(edge0_2: f32, edge1_2: f32, x_2: f32) -> f32 {
    var edge0_3: f32;
    var edge1_3: f32;
    var x_3: f32;

    edge0_3 = edge0_2;
    edge1_3 = edge1_2;
    x_3 = x_2;
    let _e15: f32 = edge0_3;
    let _e16: f32 = edge1_3;
    let _e17: f32 = x_3;
    return smoothstep(_e15, _e16, _e17);
}

fn getCircle(uv_2: vec2<f32>, r: f32, baseR: f32) -> f32 {
    var uv_3: vec2<f32>;
    var r_1: f32;
    var baseR_1: f32;
    var d_1: f32;
    var aa: f32;

    uv_3 = uv_2;
    r_1 = r;
    baseR_1 = baseR;
    let _e16: f32 = baseR_1;
    let _e19: f32 = r_1;
    r_1 = mix((0.25f * _e16), 0f, _e19);
    let _e21: vec2<f32> = uv_3;
    d_1 = length((_e21 - vec2(0.5f)));
    let _e27: f32 = d_1;
    let _e28: f32 = gl_width(_e27);
    aa = _e28;
    let _e31: f32 = r_1;
    let _e32: f32 = aa;
    let _e34: f32 = r_1;
    let _e35: f32 = aa;
    let _e37: f32 = d_1;
    return (1f - smoothstep((_e31 - _e32), (_e34 + _e35), _e37));
}

fn getCell(uv_4: vec2<f32>) -> f32 {
    var uv_5: vec2<f32>;
    var insideX: f32;
    var insideY: f32;

    uv_5 = uv_4;
    let _e12: vec2<f32> = uv_5;
    let _e17: vec2<f32> = uv_5;
    insideX = (step(0f, _e12.x) * (1f - step(1f, _e17.x)));
    let _e24: vec2<f32> = uv_5;
    let _e29: vec2<f32> = uv_5;
    insideY = (step(0f, _e24.y) * (1f - step(1f, _e29.y)));
    let _e35: f32 = insideX;
    let _e36: f32 = insideY;
    return (_e35 * _e36);
}

fn getCircleWithHole(uv_6: vec2<f32>, r_2: f32, baseR_2: f32) -> f32 {
    var uv_7: vec2<f32>;
    var r_3: f32;
    var baseR_3: f32;
    var cell: f32;
    var rMod: f32;
    var d_2: f32;
    var aa_1: f32;
    var circle: f32;

    uv_7 = uv_6;
    r_3 = r_2;
    baseR_3 = baseR_2;
    let _e15: vec2<f32> = uv_7;
    let _e16: f32 = getCell(_e15);
    cell = _e16;
    let _e19: f32 = baseR_3;
    let _e22: f32 = r_3;
    r_3 = mix((0.75f * _e19), 0f, _e22);
    let _e24: f32 = r_3;
    rMod = (_e24 - (floor((_e24 / 0.5f)) * 0.5f));
    let _e31: vec2<f32> = uv_7;
    d_2 = length((_e31 - vec2(0.5f)));
    let _e37: f32 = d_2;
    let _e38: f32 = gl_width(_e37);
    aa_1 = _e38;
    let _e41: f32 = rMod;
    let _e42: f32 = aa_1;
    let _e44: f32 = rMod;
    let _e45: f32 = aa_1;
    let _e47: f32 = d_2;
    circle = (1f - smoothstep((_e41 - _e42), (_e44 + _e45), _e47));
    let _e51: f32 = r_3;
    if (_e51 < 0.5f) {
        {
            let _e54: f32 = circle;
            return _e54;
        }
    } else {
        {
            let _e55: f32 = cell;
            let _e56: f32 = circle;
            return (_e55 - _e56);
        }
    }
}

fn getGooeyBall(uv_8: vec2<f32>, r_4: f32, baseR_4: f32) -> f32 {
    var uv_9: vec2<f32>;
    var r_5: f32;
    var baseR_5: f32;
    var d_3: f32;
    var sizeRadius: f32 = 0.3f;

    uv_9 = uv_8;
    r_5 = r_4;
    baseR_5 = baseR_4;
    let _e15: vec2<f32> = uv_9;
    d_3 = length((_e15 - vec2(0.5f)));
    let _e26: vec4<f32> = params.data[52];
    if (_e26.x == 1f) {
        {
            sizeRadius = 0.42f;
        }
    }
    let _e31: f32 = sizeRadius;
    let _e32: f32 = baseR_5;
    let _e35: f32 = r_5;
    sizeRadius = mix((_e31 * _e32), 0f, _e35);
    let _e39: f32 = sizeRadius;
    let _e40: f32 = d_3;
    let _e41: f32 = sst(0f, _e39, _e40);
    d_3 = (1f - _e41);
    let _e43: f32 = d_3;
    let _e45: f32 = baseR_5;
    d_3 = pow(_e43, (2f + _e45));
    let _e48: f32 = d_3;
    return _e48;
}

fn getSoftBall(uv_10: vec2<f32>, r_6: f32, baseR_6: f32) -> f32 {
    var uv_11: vec2<f32>;
    var r_7: f32;
    var baseR_7: f32;
    var d_4: f32;
    var sizeRadius_1: f32;
    var powRadius: f32;

    uv_11 = uv_10;
    r_7 = r_6;
    baseR_7 = baseR_6;
    let _e15: vec2<f32> = uv_11;
    d_4 = length((_e15 - vec2(0.5f)));
    let _e21: f32 = baseR_7;
    sizeRadius_1 = clamp(_e21, 0f, 1f);
    let _e27: f32 = sizeRadius_1;
    let _e30: f32 = r_7;
    sizeRadius_1 = mix((0.5f * _e27), 0f, _e30);
    let _e34: f32 = sizeRadius_1;
    let _e35: f32 = d_4;
    let _e36: f32 = lst(0f, _e34, _e35);
    d_4 = (1f - _e36);
    let _e41: f32 = baseR_7;
    let _e42: f32 = lst(0f, 2f, _e41);
    powRadius = (1f - _e42);
    let _e45: f32 = d_4;
    let _e48: f32 = powRadius;
    d_4 = pow(_e45, (4f + (3f * _e48)));
    let _e52: f32 = d_4;
    return _e52;
}

fn getUvFrame(uv_12: vec2<f32>, pad: vec2<f32>) -> f32 {
    var uv_13: vec2<f32>;
    var pad_1: vec2<f32>;
    var aa_2: f32 = 0.0001f;
    var left: f32;
    var right: f32;
    var bottom: f32;
    var top: f32;

    uv_13 = uv_12;
    pad_1 = pad;
    let _e15: vec2<f32> = pad_1;
    let _e18: vec2<f32> = pad_1;
    let _e21: f32 = aa_2;
    let _e23: vec2<f32> = uv_13;
    left = smoothstep(-(_e15.x), (-(_e18.x) + _e21), _e23.x);
    let _e28: vec2<f32> = pad_1;
    let _e32: vec2<f32> = pad_1;
    let _e35: f32 = aa_2;
    let _e37: vec2<f32> = uv_13;
    right = smoothstep((1f + _e28.x), ((1f + _e32.x) - _e35), _e37.x);
    let _e41: vec2<f32> = pad_1;
    let _e44: vec2<f32> = pad_1;
    let _e47: f32 = aa_2;
    let _e49: vec2<f32> = uv_13;
    bottom = smoothstep(-(_e41.y), (-(_e44.y) + _e47), _e49.y);
    let _e54: vec2<f32> = pad_1;
    let _e58: vec2<f32> = pad_1;
    let _e61: f32 = aa_2;
    let _e63: vec2<f32> = uv_13;
    top = smoothstep((1f + _e54.y), ((1f + _e58.y) - _e61), _e63.y);
    let _e67: f32 = left;
    let _e68: f32 = right;
    let _e70: f32 = bottom;
    let _e72: f32 = top;
    return (((_e67 * _e68) * _e70) * _e72);
}

fn sigmoid(x_4: f32, k: f32) -> f32 {
    var x_5: f32;
    var k_1: f32;

    x_5 = x_4;
    k_1 = k;
    let _e15: f32 = k_1;
    let _e17: f32 = x_5;
    return (1f / (1f + exp((-(_e15) * (_e17 - 0.5f)))));
}

fn getLumAtPx(uv_14: vec2<f32>, contrast: f32) -> f32 {
    var uv_15: vec2<f32>;
    var contrast_1: f32;
    var tex: vec4<f32>;
    var color: vec3<f32>;
    var lum: f32;
    var local: f32;

    uv_15 = uv_14;
    contrast_1 = contrast;
    let _e13: vec2<f32> = uv_15;
    let _e14: vec4<f32> = textureSample(cover_texture, cover_sampler, _e13);
    tex = _e14;
    let _e16: vec4<f32> = tex;
    let _e18: f32 = contrast_1;
    let _e19: f32 = sigmoid(_e16.x, _e18);
    let _e20: vec4<f32> = tex;
    let _e22: f32 = contrast_1;
    let _e23: f32 = sigmoid(_e20.y, _e22);
    let _e24: vec4<f32> = tex;
    let _e26: f32 = contrast_1;
    let _e27: f32 = sigmoid(_e24.z, _e26);
    color = vec3<f32>(_e19, _e23, _e27);
    let _e34: vec3<f32> = color;
    lum = dot(vec3<f32>(0.2126f, 0.7152f, 0.0722f), _e34);
    let _e38: f32 = lum;
    let _e39: vec4<f32> = tex;
    lum = mix(1f, _e38, _e39.w);
    let _e45: vec4<f32> = params.data[18];
    if (_e45.x != 0f) {
        let _e50: f32 = lum;
        local = (1f - _e50);
    } else {
        let _e52: f32 = lum;
        local = _e52;
    }
    let _e54: f32 = local;
    lum = _e54;
    let _e55: f32 = lum;
    return _e55;
}

fn getLumBall(p_2: vec2<f32>, pad_2: vec2<f32>, inCellOffset: vec2<f32>, contrast_2: f32, baseR_8: f32, stepSize: f32, ballColor: ptr<function, vec4<f32>>) -> f32 {
    var p_3: vec2<f32>;
    var pad_3: vec2<f32>;
    var inCellOffset_1: vec2<f32>;
    var contrast_3: f32;
    var baseR_9: f32;
    var stepSize_1: f32;
    var uv_i: vec2<f32>;
    var uv_f: vec2<f32>;
    var samplingUV: vec2<f32>;
    var outOfFrame: f32;
    var lum_1: f32;
    var ball: f32 = 0f;

    p_3 = p_2;
    pad_3 = pad_2;
    inCellOffset_1 = inCellOffset;
    contrast_3 = contrast_2;
    baseR_9 = baseR_8;
    stepSize_1 = stepSize;
    let _e22: vec2<f32> = p_3;
    let _e23: vec2<f32> = inCellOffset_1;
    p_3 = (_e22 + _e23);
    let _e25: vec2<f32> = p_3;
    uv_i = floor(_e25);
    let _e28: vec2<f32> = p_3;
    uv_f = fract(_e28);
    let _e31: vec2<f32> = uv_i;
    let _e35: vec2<f32> = inCellOffset_1;
    let _e37: vec2<f32> = pad_3;
    samplingUV = ((((_e31 + vec2(0.5f)) - _e35) * _e37) + vec2(0.5f));
    let _e43: vec2<f32> = samplingUV;
    let _e44: vec2<f32> = pad_3;
    let _e45: f32 = stepSize_1;
    let _e47: f32 = getUvFrame(_e43, (_e44 * _e45));
    outOfFrame = _e47;
    let _e49: vec2<f32> = samplingUV;
    let _e50: f32 = contrast_3;
    let _e51: f32 = getLumAtPx(_e49, _e50);
    lum_1 = _e51;
    let _e53: vec2<f32> = samplingUV;
    let _e54: vec4<f32> = textureSample(cover_texture, cover_sampler, _e53);
    (*ballColor) = _e54;
    let _e55: vec4<f32> = (*ballColor);
    let _e57: vec4<f32> = (*ballColor);
    let _e59: vec4<f32> = (*ballColor);
    let _e61: vec3<f32> = (_e57.xyz * _e59.w);
    (*ballColor).x = _e61.x;
    (*ballColor).y = _e61.y;
    (*ballColor).z = _e61.z;
    let _e68: vec4<f32> = (*ballColor);
    let _e69: f32 = outOfFrame;
    (*ballColor) = (_e68 * _e69);
    let _e76: vec4<f32> = params.data[15];
    if (_e76.x == 0f) {
        {
            let _e80: vec2<f32> = uv_f;
            let _e81: f32 = lum_1;
            let _e82: f32 = baseR_9;
            let _e83: f32 = getCircle(_e80, _e81, _e82);
            ball = _e83;
        }
    } else {
        let _e87: vec4<f32> = params.data[15];
        if (_e87.x == 1f) {
            {
                let _e91: vec2<f32> = uv_f;
                let _e92: f32 = lum_1;
                let _e93: f32 = baseR_9;
                let _e94: f32 = getGooeyBall(_e91, _e92, _e93);
                ball = _e94;
            }
        } else {
            let _e98: vec4<f32> = params.data[15];
            if (_e98.x == 2f) {
                {
                    let _e102: vec2<f32> = uv_f;
                    let _e103: f32 = lum_1;
                    let _e104: f32 = baseR_9;
                    let _e105: f32 = getCircleWithHole(_e102, _e103, _e104);
                    ball = _e105;
                }
            } else {
                let _e109: vec4<f32> = params.data[15];
                if (_e109.x == 3f) {
                    {
                        let _e113: vec2<f32> = uv_f;
                        let _e114: f32 = lum_1;
                        let _e115: f32 = baseR_9;
                        let _e116: f32 = getSoftBall(_e113, _e114, _e115);
                        ball = _e116;
                    }
                }
            }
        }
    }
    let _e117: f32 = ball;
    let _e118: f32 = outOfFrame;
    return (_e117 * _e118);
}

fn main_1() {
    var stepMultiplier: f32 = 1f;
    var cellsPerSide: f32;
    var cellSizeY: f32;
    var pad_4: vec2<f32>;
    var uv_16: vec2<f32>;
    var contrast_4: f32;
    var baseRadius: f32;
    var totalShape: f32 = 0f;
    var totalColor: vec3<f32> = vec3(0f);
    var totalOpacity: f32 = 0f;
    var ballColor_1: vec4<f32>;
    var shape: f32;
    var stepSize_2: f32;
    var x_6: f32 = -0.5f;
    var y: f32;
    var offset: vec2<f32>;
    var rowIndex: f32;
    var colIndex: f32;
    var eps: f32 = 0.0001f;
    var finalShape: f32 = 0f;
    var aa_3: f32;
    var th_2: f32 = 0.5f;
    var grainSize: vec2<f32>;
    var grainUV: vec2<f32>;
    var grain: f32;
    var color_1: vec3<f32> = vec3(0f);
    var opacity: f32 = 0f;
    var bgColor: vec3<f32>;
    var fgColor: vec3<f32>;
    var fgOpacity: f32;
    var bgColor_1: vec3<f32>;
    var bgOpacity: f32;
    var grainOverlay: f32;
    var grainOverlayV: f32;
    var grainOverlayColor: vec3<f32>;
    var grainOverlayStrength: f32;

    let _e14: vec4<f32> = params.data[15];
    if (_e14.x == 0f) {
        {
            stepMultiplier = 2f;
        }
    } else {
        let _e22: vec4<f32> = params.data[15];
        let _e29: vec4<f32> = params.data[15];
        if ((_e22.x == 1f) || (_e29.x == 3f)) {
            {
                stepMultiplier = 6f;
            }
        }
    }
    let _e40: vec4<f32> = params.data[21];
    cellsPerSide = mix(300f, 7f, pow(_e40.x, 0.7f));
    let _e46: f32 = cellsPerSide;
    let _e47: f32 = stepMultiplier;
    cellsPerSide = (_e46 / _e47);
    let _e50: f32 = cellsPerSide;
    cellSizeY = (1f / _e50);
    let _e53: f32 = cellSizeY;
    let _e58: vec4<f32> = params.data[14];
    pad_4 = (_e53 * vec2<f32>((1f / _e58.x), 1f));
    let _e68: vec4<f32> = params.data[15];
    let _e75: vec4<f32> = params.data[52];
    if ((_e68.x == 1f) && (_e75.x == 1f)) {
        {
            let _e80: vec2<f32> = pad_4;
            pad_4 = (_e80 * 0.7f);
        }
    }
    let _e83: vec2<f32> = v_imageUV_1;
    uv_16 = _e83;
    let _e85: vec2<f32> = uv_16;
    uv_16 = (_e85 - vec2(0.5f));
    let _e89: vec2<f32> = uv_16;
    let _e90: vec2<f32> = pad_4;
    uv_16 = (_e89 / _e90);
    let _e97: vec4<f32> = params.data[38];
    contrast_4 = mix(0f, 15f, pow(_e97.x, 1.5f));
    let _e106: vec4<f32> = params.data[50];
    baseRadius = _e106.x;
    let _e112: vec4<f32> = params.data[17];
    if ((_e112.x != 0f) == true) {
        {
            let _e123: vec4<f32> = params.data[38];
            contrast_4 = mix(0.1f, 4f, pow(_e123.x, 2f));
            let _e133: vec4<f32> = params.data[50];
            baseRadius = (2f * pow((0.5f * _e133.x), 0.3f));
        }
    }
    let _e149: f32 = stepMultiplier;
    stepSize_2 = (1f / _e149);
    loop {
        let _e155: f32 = x_6;
        if !((_e155 < 0.5f)) {
            break;
        }
        {
            y = -0.5f;
            loop {
                let _e165: f32 = y;
                if !((_e165 < 0.5f)) {
                    break;
                }
                {
                    let _e172: f32 = x_6;
                    let _e173: f32 = y;
                    offset = vec2<f32>(_e172, _e173);
                    let _e179: vec4<f32> = params.data[52];
                    if (_e179.x == 1f) {
                        {
                            let _e183: f32 = y;
                            let _e186: f32 = stepSize_2;
                            rowIndex = floor(((_e183 + 0.5f) / _e186));
                            let _e190: f32 = x_6;
                            let _e193: f32 = stepSize_2;
                            colIndex = floor(((_e190 + 0.5f) / _e193));
                            let _e197: f32 = stepSize_2;
                            if (_e197 == 1f) {
                                {
                                    let _e200: vec2<f32> = uv_16;
                                    let _e202: f32 = y;
                                    rowIndex = floor(((_e200.y + _e202) + 1f));
                                    let _e210: vec4<f32> = params.data[15];
                                    if (_e210.x == 1f) {
                                        {
                                            let _e214: vec2<f32> = uv_16;
                                            let _e216: f32 = x_6;
                                            colIndex = floor(((_e214.x + _e216) + 1f));
                                        }
                                    }
                                }
                            }
                            let _e224: vec4<f32> = params.data[15];
                            if (_e224.x == 1f) {
                                {
                                    let _e228: f32 = rowIndex;
                                    let _e229: f32 = colIndex;
                                    let _e230: f32 = (_e228 + _e229);
                                    if ((_e230 - (floor((_e230 / 2f)) * 2f)) == 1f) {
                                        {
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                {
                                    let _e238: f32 = rowIndex;
                                    if ((_e238 - (floor((_e238 / 2f)) * 2f)) == 1f) {
                                        {
                                            let _e247: vec2<f32> = offset;
                                            let _e250: f32 = stepSize_2;
                                            offset.x = (_e247.x + (0.5f * _e250));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let _e253: vec2<f32> = uv_16;
                    let _e254: vec2<f32> = pad_4;
                    let _e255: vec2<f32> = offset;
                    let _e256: f32 = contrast_4;
                    let _e257: f32 = baseRadius;
                    let _e258: f32 = stepSize_2;
                    let _e261: f32 = getLumBall(_e253, _e254, _e255, _e256, _e257, _e258, (&ballColor_1));
                    shape = _e261;
                    let _e262: vec3<f32> = totalColor;
                    let _e263: vec4<f32> = ballColor_1;
                    let _e265: f32 = shape;
                    totalColor = (_e262 + (_e263.xyz * _e265));
                    let _e268: f32 = totalShape;
                    let _e269: f32 = shape;
                    totalShape = (_e268 + _e269);
                    let _e271: f32 = totalOpacity;
                    let _e272: f32 = shape;
                    totalOpacity = (_e271 + _e272);
                }
                continuing {
                    let _e169: f32 = y;
                    let _e170: f32 = stepSize_2;
                    y = (_e169 + _e170);
                }
            }
        }
        continuing {
            let _e159: f32 = x_6;
            let _e160: f32 = stepSize_2;
            x_6 = (_e159 + _e160);
        }
    }
    let _e276: vec3<f32> = totalColor;
    let _e277: f32 = totalShape;
    let _e278: f32 = eps;
    totalColor = (_e276 / vec3(max(_e277, _e278)));
    let _e282: f32 = totalOpacity;
    let _e283: f32 = totalShape;
    let _e284: f32 = eps;
    totalOpacity = (_e282 / max(_e283, _e284));
    let _e292: vec4<f32> = params.data[15];
    if (_e292.x == 0f) {
        {
            let _e297: f32 = totalShape;
            finalShape = min(1f, _e297);
        }
    } else {
        let _e302: vec4<f32> = params.data[15];
        if (_e302.x == 1f) {
            {
                let _e306: f32 = totalShape;
                let _e307: f32 = gl_width(_e306);
                aa_3 = _e307;
                let _e311: f32 = th_2;
                let _e312: f32 = aa_3;
                let _e314: f32 = th_2;
                let _e315: f32 = aa_3;
                let _e317: f32 = totalShape;
                finalShape = smoothstep((_e311 - _e312), (_e314 + _e315), _e317);
            }
        } else {
            let _e322: vec4<f32> = params.data[15];
            if (_e322.x == 2f) {
                {
                    let _e327: f32 = totalShape;
                    finalShape = min(1f, _e327);
                }
            } else {
                let _e332: vec4<f32> = params.data[15];
                if (_e332.x == 3f) {
                    {
                        let _e336: f32 = totalShape;
                        finalShape = _e336;
                    }
                }
            }
        }
    }
    let _e342: vec4<f32> = params.data[51];
    let _e350: vec4<f32> = params.data[14];
    grainSize = (mix(2000f, 200f, _e342.x) * vec2<f32>(1f, (1f / _e350.x)));
    let _e356: vec2<f32> = v_imageUV_1;
    grainUV = (_e356 - vec2(0.5f));
    let _e361: vec2<f32> = grainUV;
    let _e362: vec2<f32> = grainSize;
    grainUV = (_e361 * _e362);
    let _e364: vec2<f32> = grainUV;
    grainUV = (_e364 + vec2(0.5f));
    let _e368: vec2<f32> = grainUV;
    let _e369: f32 = valueNoise(_e368);
    grain = _e369;
    let _e377: vec4<f32> = params.data[36];
    let _e381: f32 = grain;
    grain = smoothstep(0.55f, (0.7f + (0.2f * _e377.x)), _e381);
    let _e383: f32 = grain;
    let _e387: vec4<f32> = params.data[36];
    grain = (_e383 * _e387.x);
    let _e390: f32 = finalShape;
    let _e392: f32 = grain;
    finalShape = mix(_e390, 0f, _e392);
    let _e402: vec4<f32> = params.data[17];
    if ((_e402.x != 0f) == true) {
        {
            let _e408: vec3<f32> = totalColor;
            let _e409: f32 = finalShape;
            color_1 = (_e408 * _e409);
            let _e411: f32 = totalOpacity;
            let _e412: f32 = finalShape;
            opacity = (_e411 * _e412);
            let _e417: vec4<f32> = params.data[12];
            let _e422: vec4<f32> = params.data[12];
            bgColor = (_e417.xyz * _e422.w);
            let _e426: vec3<f32> = color_1;
            let _e427: vec3<f32> = bgColor;
            let _e429: f32 = opacity;
            color_1 = (_e426 + (_e427 * (1f - _e429)));
            let _e433: f32 = opacity;
            let _e437: vec4<f32> = params.data[12];
            let _e440: f32 = opacity;
            opacity = (_e433 + (_e437.w * (1f - _e440)));
        }
    } else {
        {
            let _e447: vec4<f32> = params.data[11];
            let _e452: vec4<f32> = params.data[11];
            fgColor = (_e447.xyz * _e452.w);
            let _e459: vec4<f32> = params.data[11];
            fgOpacity = _e459.w;
            let _e465: vec4<f32> = params.data[12];
            let _e470: vec4<f32> = params.data[12];
            bgColor_1 = (_e465.xyz * _e470.w);
            let _e477: vec4<f32> = params.data[12];
            bgOpacity = _e477.w;
            let _e480: vec3<f32> = fgColor;
            let _e481: f32 = finalShape;
            color_1 = (_e480 * _e481);
            let _e483: f32 = fgOpacity;
            let _e484: f32 = finalShape;
            opacity = (_e483 * _e484);
            let _e486: vec3<f32> = color_1;
            let _e487: vec3<f32> = bgColor_1;
            let _e489: f32 = opacity;
            color_1 = (_e486 + (_e487 * (1f - _e489)));
            let _e493: f32 = opacity;
            let _e494: f32 = bgOpacity;
            let _e496: f32 = opacity;
            opacity = (_e493 + (_e494 * (1f - _e496)));
        }
    }
    let _e500: vec2<f32> = grainUV;
    let _e502: vec2<f32> = rotate(_e500, 1f);
    let _e506: f32 = valueNoise((_e502 + vec2(3f)));
    grainOverlay = _e506;
    let _e508: f32 = grainOverlay;
    let _e509: vec2<f32> = grainUV;
    let _e511: vec2<f32> = rotate(_e509, 2f);
    let _e516: f32 = valueNoise((_e511 + vec2(-1f)));
    grainOverlay = mix(_e508, _e516, 0.5f);
    let _e519: f32 = grainOverlay;
    grainOverlay = pow(_e519, 1.3f);
    let _e522: f32 = grainOverlay;
    grainOverlayV = ((_e522 * 2f) - 1f);
    let _e529: f32 = grainOverlayV;
    grainOverlayColor = vec3(step(0f, _e529));
    let _e536: vec4<f32> = params.data[37];
    let _e538: f32 = grainOverlayV;
    grainOverlayStrength = (_e536.x * abs(_e538));
    let _e542: f32 = grainOverlayStrength;
    grainOverlayStrength = pow(_e542, 0.8f);
    let _e545: vec3<f32> = color_1;
    let _e546: vec3<f32> = grainOverlayColor;
    let _e548: f32 = grainOverlayStrength;
    color_1 = mix(_e545, _e546, vec3((0.5f * _e548)));
    let _e552: f32 = opacity;
    let _e554: f32 = grainOverlayStrength;
    opacity = (_e552 + (0.5f * _e554));
    let _e557: f32 = opacity;
    opacity = clamp(_e557, 0f, 1f);
    let _e561: vec3<f32> = color_1;
    let _e562: f32 = opacity;
    fragColor = vec4<f32>(_e561.x, _e561.y, _e561.z, _e562);
    return;
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

fn gl_width(value: f32) -> f32 {
    return abs(dpdxCoarse(value) + dpdyCoarse(dpdxFine(value))) + abs(dpdyCoarse(value));
}
