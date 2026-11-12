// Paper Shaders 0.0.80, Copyright 2026 Paper, Apache-2.0. GLSL translated using Naga 27.0.3.
// Source https://github.com/paper-design/shaders; license and notice in assets/paper-shaders.
struct Params {
    data: array<vec4<f32>, 72>,
}

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

const MAX_RADIUS: i32 = 50i;

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
var<private> gl_FragCoord_1: vec4<f32>;

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

fn getUvFrame(uv_2: vec2<f32>, softness: f32) -> f32 {
    var uv_3: vec2<f32>;
    var softness_1: f32;
    var aax: f32;
    var aay: f32;
    var left: f32;
    var right: f32;
    var bottom: f32;
    var top: f32;

    uv_3 = uv_2;
    softness_1 = softness;
    let _e14: vec2<f32> = uv_3;
    let _e16: f32 = fwidth(_e14.x);
    aax = (2f * _e16);
    let _e20: vec2<f32> = uv_3;
    let _e22: f32 = fwidth(_e20.y);
    aay = (2f * _e22);
    let _e26: f32 = aax;
    let _e27: f32 = softness_1;
    let _e29: vec2<f32> = uv_3;
    left = smoothstep(0f, (_e26 + _e27), _e29.x);
    let _e35: f32 = softness_1;
    let _e37: f32 = aax;
    let _e40: vec2<f32> = uv_3;
    right = (1f - smoothstep(((1f - _e35) - _e37), 1f, _e40.x));
    let _e46: f32 = aay;
    let _e47: f32 = softness_1;
    let _e49: vec2<f32> = uv_3;
    bottom = smoothstep(0f, (_e46 + _e47), _e49.y);
    let _e55: f32 = softness_1;
    let _e57: f32 = aay;
    let _e60: vec2<f32> = uv_3;
    top = (1f - smoothstep(((1f - _e55) - _e57), 1f, _e60.y));
    let _e65: f32 = left;
    let _e66: f32 = right;
    let _e68: f32 = bottom;
    let _e70: f32 = top;
    return (((_e65 * _e66) * _e68) * _e70);
}

fn samplePremultiplied(uv_4: vec2<f32>) -> vec4<f32> {
    var uv_5: vec2<f32>;
    var c_1: vec4<f32>;

    uv_5 = uv_4;
    let _e12: vec2<f32> = uv_5;
    let _e13: vec4<f32> = textureSample(cover_texture, cover_sampler, _e12);
    c_1 = _e13;
    let _e15: vec4<f32> = c_1;
    let _e17: vec4<f32> = c_1;
    let _e19: vec4<f32> = c_1;
    let _e21: vec3<f32> = (_e17.xyz * _e19.w);
    c_1.x = _e21.x;
    c_1.y = _e21.y;
    c_1.z = _e21.z;
    let _e28: vec4<f32> = c_1;
    return _e28;
}

fn getBlur(uv_6: vec2<f32>, texelSize: vec2<f32>, dir: vec2<f32>, sigma: f32) -> vec4<f32> {
    var uv_7: vec2<f32>;
    var texelSize_1: vec2<f32>;
    var dir_1: vec2<f32>;
    var sigma_1: f32;
    var radius: i32;
    var twoSigma2_: f32;
    var gaussianNorm: f32;
    var sum: vec4<f32>;
    var weightSum: f32;
    var i_1: i32 = 1i;
    var x: f32;
    var w: f32;
    var offset: vec2<f32>;
    var s1_: vec4<f32>;
    var s2_: vec4<f32>;
    var result: vec4<f32>;

    uv_7 = uv_6;
    texelSize_1 = texelSize;
    dir_1 = dir;
    sigma_1 = sigma;
    let _e18: f32 = sigma_1;
    if (_e18 <= 0.5f) {
        let _e21: vec2<f32> = uv_7;
        let _e22: vec4<f32> = textureSample(cover_texture, cover_sampler, _e21);
        return _e22;
    }
    let _e26: f32 = sigma_1;
    radius = i32(min(50f, ceil((3f * _e26))));
    let _e33: f32 = sigma_1;
    let _e35: f32 = sigma_1;
    twoSigma2_ = ((2f * _e33) * _e35);
    let _e40: f32 = sigma_1;
    let _e42: f32 = sigma_1;
    gaussianNorm = (1f / sqrt(((6.2831855f * _e40) * _e42)));
    let _e47: vec2<f32> = uv_7;
    let _e48: vec4<f32> = samplePremultiplied(_e47);
    let _e49: f32 = gaussianNorm;
    sum = (_e48 * _e49);
    let _e52: f32 = gaussianNorm;
    weightSum = _e52;
    loop {
        let _e56: i32 = i_1;
        if !((_e56 <= MAX_RADIUS)) {
            break;
        }
        {
            let _e62: i32 = i_1;
            let _e63: i32 = radius;
            if (_e62 > _e63) {
                break;
            }
            let _e65: i32 = i_1;
            x = f32(_e65);
            let _e68: f32 = x;
            let _e69: f32 = x;
            let _e72: f32 = twoSigma2_;
            let _e75: f32 = gaussianNorm;
            w = (exp((-((_e68 * _e69)) / _e72)) * _e75);
            let _e78: vec2<f32> = dir_1;
            let _e79: vec2<f32> = texelSize_1;
            let _e81: f32 = x;
            offset = ((_e78 * _e79) * _e81);
            let _e84: vec2<f32> = uv_7;
            let _e85: vec2<f32> = offset;
            let _e87: vec4<f32> = samplePremultiplied((_e84 + _e85));
            s1_ = _e87;
            let _e89: vec2<f32> = uv_7;
            let _e90: vec2<f32> = offset;
            let _e92: vec4<f32> = samplePremultiplied((_e89 - _e90));
            s2_ = _e92;
            let _e94: vec4<f32> = sum;
            let _e95: vec4<f32> = s1_;
            let _e96: vec4<f32> = s2_;
            let _e98: f32 = w;
            sum = (_e94 + ((_e95 + _e96) * _e98));
            let _e101: f32 = weightSum;
            let _e103: f32 = w;
            weightSum = (_e101 + (2f * _e103));
        }
        continuing {
            let _e59: i32 = i_1;
            i_1 = (_e59 + 1i);
        }
    }
    let _e106: vec4<f32> = sum;
    let _e107: f32 = weightSum;
    result = (_e106 / vec4(_e107));
    let _e111: vec4<f32> = result;
    if (_e111.w > 0f) {
        {
            let _e115: vec4<f32> = result;
            let _e117: vec4<f32> = result;
            let _e119: vec4<f32> = result;
            let _e122: vec3<f32> = (_e117.xyz / vec3(_e119.w));
            result.x = _e122.x;
            result.y = _e122.y;
            result.z = _e122.z;
        }
    }
    let _e129: vec4<f32> = result;
    return _e129;
}

fn rotateAspect(p_2: vec2<f32>, a_1: f32, aspect: f32) -> vec2<f32> {
    var p_3: vec2<f32>;
    var a_2: f32;
    var aspect_1: f32;

    p_3 = p_2;
    a_2 = a_1;
    aspect_1 = aspect;
    let _e17: vec2<f32> = p_3;
    let _e19: f32 = aspect_1;
    p_3.x = (_e17.x * _e19);
    let _e21: vec2<f32> = p_3;
    let _e22: f32 = a_2;
    let _e23: vec2<f32> = rotate(_e21, _e22);
    p_3 = _e23;
    let _e25: vec2<f32> = p_3;
    let _e27: f32 = aspect_1;
    p_3.x = (_e25.x / _e27);
    let _e29: vec2<f32> = p_3;
    return _e29;
}

fn smoothFract(x_1: f32) -> f32 {
    var x_2: f32;
    var f_1: f32;
    var w_1: f32;
    var edge: f32;
    var band: f32;

    x_2 = x_1;
    let _e12: f32 = x_2;
    f_1 = fract(_e12);
    let _e15: f32 = x_2;
    let _e16: f32 = fwidth(_e15);
    w_1 = _e16;
    let _e18: f32 = f_1;
    edge = (abs((_e18 - 0.5f)) - 0.5f);
    let _e25: f32 = w_1;
    let _e27: f32 = w_1;
    let _e28: f32 = edge;
    band = smoothstep(-(_e25), _e27, _e28);
    let _e31: f32 = f_1;
    let _e33: f32 = f_1;
    let _e35: f32 = band;
    return mix(_e31, (1f - _e33), _e35);
}

fn main_1() {
    var patternRotation: f32;
    var patternSize: f32;
    var uv_8: vec2<f32>;
    var uvMask: vec2<f32>;
    var sw: vec2<f32> = vec2(0.005f);
    var margins: vec4<f32>;
    var mask: f32;
    var maskOuter: f32;
    var maskStroke: f32;
    var maskInner: f32;
    var maskStrokeInner: f32;
    var curve: f32 = 0f;
    var patternY: f32;
    var UvToFract: vec2<f32>;
    var fractOrigUV: vec2<f32>;
    var floorOrigUV: vec2<f32>;
    var x_3: f32;
    var xNonSmooth: f32;
    var highlightsWidth: f32;
    var highlights: f32;
    var shadows: f32;
    var distortion: f32 = 0f;
    var fadeX: f32 = 1f;
    var frameFade: f32 = 0f;
    var aa: f32;
    var dudx: vec2<f32>;
    var dudy: vec2<f32>;
    var grainUV: vec2<f32>;
    var grain: f32;
    var blur: f32;
    var edgeDistortion: f32;
    var frame: f32;
    var stretch: f32;
    var image: vec4<f32>;
    var backColor: vec4<f32>;
    var highlightColor: vec4<f32>;
    var shadowColor: vec4<f32>;
    var color: vec3<f32>;
    var opacity: f32;
    var grainOverlay: f32;
    var grainOverlayV: f32;
    var grainOverlayColor: vec3<f32>;
    var grainOverlayStrength: f32;

    let _e13: vec4<f32> = params.data[23];
    patternRotation = ((-(_e13.x) * 3.1415927f) / 180f);
    let _e26: vec4<f32> = params.data[21];
    patternSize = mix(200f, 5f, _e26.x);
    let _e30: vec2<f32> = v_imageUV_1;
    uv_8 = _e30;
    let _e33: vec4<f32> = gl_FragCoord_1;
    let _e38: vec4<f32> = params.data[0];
    let _e41: vec4<f32> = gl_FragCoord_1;
    let _e48: vec4<f32> = params.data[0];
    uvMask = (vec2<f32>(_e33.x, (_e38.y - _e41.y)) / _e48.xy);
    let _e59: vec4<f32> = params.data[32];
    let _e64: vec4<f32> = params.data[34];
    let _e69: vec4<f32> = params.data[33];
    let _e74: vec4<f32> = params.data[35];
    margins = vec4<f32>(_e59.x, _e64.x, _e69.x, _e74.x);
    let _e80: f32 = margins.x;
    let _e83: f32 = margins.x;
    let _e84: vec2<f32> = sw;
    let _e87: vec2<f32> = uvMask;
    let _e89: vec2<f32> = sw;
    let _e95: f32 = margins.z;
    let _e98: f32 = margins.z;
    let _e99: vec2<f32> = sw;
    let _e103: vec2<f32> = uvMask;
    let _e106: vec2<f32> = sw;
    let _e113: f32 = margins.y;
    let _e116: f32 = margins.y;
    let _e117: vec2<f32> = sw;
    let _e120: vec2<f32> = uvMask;
    let _e122: vec2<f32> = sw;
    let _e129: f32 = margins.w;
    let _e132: f32 = margins.w;
    let _e133: vec2<f32> = sw;
    let _e137: vec2<f32> = uvMask;
    let _e140: vec2<f32> = sw;
    mask = (((smoothstep(_e80, (_e83 + _e84.x), (_e87.x + _e89.x)) * smoothstep(_e95, (_e98 + _e99.x), ((1f - _e103.x) + _e106.x))) * smoothstep(_e113, (_e116 + _e117.y), (_e120.y + _e122.y))) * smoothstep(_e129, (_e132 + _e133.y), ((1f - _e137.y) + _e140.y)));
    let _e148: f32 = margins.x;
    let _e149: vec2<f32> = sw;
    let _e154: f32 = margins.x;
    let _e155: vec2<f32> = uvMask;
    let _e157: vec2<f32> = sw;
    let _e163: f32 = margins.z;
    let _e164: vec2<f32> = sw;
    let _e169: f32 = margins.z;
    let _e171: vec2<f32> = uvMask;
    let _e174: vec2<f32> = sw;
    let _e181: f32 = margins.y;
    let _e182: vec2<f32> = sw;
    let _e187: f32 = margins.y;
    let _e188: vec2<f32> = uvMask;
    let _e190: vec2<f32> = sw;
    let _e197: f32 = margins.w;
    let _e198: vec2<f32> = sw;
    let _e203: f32 = margins.w;
    let _e205: vec2<f32> = uvMask;
    let _e208: vec2<f32> = sw;
    maskOuter = (((smoothstep((_e148 - _e149.x), _e154, (_e155.x + _e157.x)) * smoothstep((_e163 - _e164.x), _e169, ((1f - _e171.x) + _e174.x))) * smoothstep((_e181 - _e182.y), _e187, (_e188.y + _e190.y))) * smoothstep((_e197 - _e198.y), _e203, ((1f - _e205.y) + _e208.y)));
    let _e214: f32 = maskOuter;
    let _e215: f32 = mask;
    maskStroke = (_e214 - _e215);
    let _e220: f32 = margins.x;
    let _e222: vec2<f32> = sw;
    let _e228: f32 = margins.x;
    let _e229: vec2<f32> = uvMask;
    let _e234: f32 = margins.z;
    let _e236: vec2<f32> = sw;
    let _e242: f32 = margins.z;
    let _e244: vec2<f32> = uvMask;
    let _e251: f32 = margins.y;
    let _e253: vec2<f32> = sw;
    let _e259: f32 = margins.y;
    let _e260: vec2<f32> = uvMask;
    let _e266: f32 = margins.w;
    let _e268: vec2<f32> = sw;
    let _e274: f32 = margins.w;
    let _e276: vec2<f32> = uvMask;
    maskInner = (((smoothstep((_e220 - (2f * _e222.x)), _e228, _e229.x) * smoothstep((_e234 - (2f * _e236.x)), _e242, (1f - _e244.x))) * smoothstep((_e251 - (2f * _e253.y)), _e259, _e260.y)) * smoothstep((_e266 - (2f * _e268.y)), _e274, (1f - _e276.y)));
    let _e282: f32 = maskInner;
    let _e283: f32 = mask;
    maskStrokeInner = (_e282 - _e283);
    let _e286: vec2<f32> = uv_8;
    uv_8 = (_e286 - vec2(0.5f));
    let _e290: vec2<f32> = uv_8;
    let _e291: f32 = patternSize;
    uv_8 = (_e290 * _e291);
    let _e293: vec2<f32> = uv_8;
    let _e294: f32 = patternRotation;
    let _e298: vec4<f32> = params.data[14];
    let _e300: vec2<f32> = rotateAspect(_e293, _e294, _e298.x);
    uv_8 = _e300;
    let _e303: vec2<f32> = uv_8;
    let _e308: vec4<f32> = params.data[14];
    patternY = (_e303.y / _e308.x);
    let _e315: vec4<f32> = params.data[25];
    if (_e315.x > 4.5f) {
        {
            let _e324: vec2<f32> = uv_8;
            let _e332: f32 = patternY;
            curve = (0.5f + ((0.5f * sin((1.5707964f * _e324.x))) * cos((1.5707964f * _e332))));
        }
    } else {
        let _e340: vec4<f32> = params.data[25];
        if (_e340.x > 3.5f) {
            {
                let _e346: f32 = patternY;
                curve = (10f * abs((fract((0.1f * _e346)) - 0.5f)));
            }
        } else {
            let _e356: vec4<f32> = params.data[25];
            if (_e356.x > 2.5f) {
                {
                    let _e362: f32 = patternY;
                    curve = (4f * sin((0.23f * _e362)));
                }
            } else {
                let _e369: vec4<f32> = params.data[25];
                if (_e369.x > 1.5f) {
                    {
                        let _e376: vec2<f32> = uv_8;
                        let _e382: vec2<f32> = uv_8;
                        curve = (0.5f + ((0.5f * sin((0.5f * _e376.x))) * sin((1.7f * _e382.x))));
                    }
                } else {
                    {
                    }
                }
            }
        }
    }
    let _e388: vec2<f32> = uv_8;
    let _e389: f32 = curve;
    UvToFract = (_e388 + vec2(_e389));
    let _e393: vec2<f32> = uv_8;
    fractOrigUV = fract(_e393);
    let _e396: vec2<f32> = uv_8;
    floorOrigUV = floor(_e396);
    let _e399: vec2<f32> = UvToFract;
    let _e401: f32 = smoothFract(_e399.x);
    x_3 = _e401;
    let _e403: vec2<f32> = UvToFract;
    xNonSmooth = (fract(_e403.x) + 0.0001f);
    let _e411: vec2<f32> = UvToFract;
    let _e413: f32 = fwidth(_e411.x);
    highlightsWidth = (2f * max(0.001f, _e413));
    let _e417: f32 = highlightsWidth;
    let _e419: f32 = maskStrokeInner;
    highlightsWidth = (_e417 + (2f * _e419));
    let _e423: f32 = highlightsWidth;
    let _e424: f32 = xNonSmooth;
    highlights = smoothstep(0f, _e423, _e424);
    let _e427: f32 = highlights;
    let _e430: f32 = highlightsWidth;
    let _e432: f32 = xNonSmooth;
    highlights = (_e427 * smoothstep(1f, (1f - _e430), _e432));
    let _e436: f32 = highlights;
    highlights = (1f - _e436);
    let _e438: f32 = highlights;
    let _e442: vec4<f32> = params.data[27];
    highlights = (_e438 * _e442.x);
    let _e445: f32 = highlights;
    highlights = clamp(_e445, 0f, 1f);
    let _e449: f32 = highlights;
    let _e450: f32 = mask;
    highlights = (_e449 * _e450);
    let _e452: f32 = x_3;
    shadows = pow(_e452, 1.3f);
    let _e462: f32 = xNonSmooth;
    let _e463: f32 = fwidth(_e462);
    aa = _e463;
    let _e465: f32 = aa;
    let _e466: vec2<f32> = uv_8;
    let _e468: f32 = fwidth(_e466.x);
    aa = max(_e465, _e468);
    let _e470: f32 = aa;
    let _e471: vec2<f32> = UvToFract;
    let _e473: f32 = fwidth(_e471.x);
    aa = max(_e470, _e473);
    let _e475: f32 = aa;
    aa = max(_e475, 0.0001f);
    let _e481: vec4<f32> = params.data[28];
    if (_e481.x == 1f) {
        {
            let _e486: f32 = x_3;
            distortion = -(pow((1.5f * _e486), 3f));
            let _e491: f32 = distortion;
            let _e496: vec4<f32> = params.data[29];
            distortion = (_e491 + (0.5f - _e496.x));
            let _e501: f32 = x_3;
            frameFade = pow((1.5f * _e501), 3f);
            let _e506: f32 = aa;
            aa = max(0.2f, _e506);
            let _e508: f32 = aa;
            let _e514: vec4<f32> = params.data[21];
            aa = (_e508 + mix(0.2f, 0f, _e514.x));
            let _e519: f32 = aa;
            let _e520: f32 = xNonSmooth;
            let _e524: f32 = aa;
            let _e526: f32 = xNonSmooth;
            fadeX = (smoothstep(0f, _e519, _e520) * smoothstep(1f, (1f - _e524), _e526));
            let _e530: f32 = distortion;
            let _e531: f32 = fadeX;
            distortion = mix(0.5f, _e530, _e531);
        }
    } else {
        let _e536: vec4<f32> = params.data[28];
        if (_e536.x == 2f) {
            {
                let _e541: f32 = x_3;
                distortion = (2f * pow(_e541, 2f));
                let _e545: f32 = distortion;
                let _e550: vec4<f32> = params.data[29];
                distortion = (_e545 - (0.5f + _e550.x));
                let _e554: f32 = x_3;
                frameFade = pow(abs((_e554 - 0.5f)), 4f);
                let _e561: f32 = aa;
                aa = max(0.2f, _e561);
                let _e563: f32 = aa;
                let _e569: vec4<f32> = params.data[21];
                aa = (_e563 + mix(0.2f, 0f, _e569.x));
                let _e574: f32 = aa;
                let _e575: f32 = xNonSmooth;
                let _e579: f32 = aa;
                let _e581: f32 = xNonSmooth;
                fadeX = (smoothstep(0f, _e574, _e575) * smoothstep(1f, (1f - _e579), _e581));
                let _e585: f32 = distortion;
                let _e586: f32 = fadeX;
                distortion = mix(0.5f, _e585, _e586);
                let _e589: f32 = frameFade;
                let _e591: f32 = fadeX;
                frameFade = mix(1f, _e589, (0.5f * _e591));
            }
        } else {
            let _e597: vec4<f32> = params.data[28];
            if (_e597.x == 3f) {
                {
                    let _e602: f32 = xNonSmooth;
                    distortion = pow((2f * (_e602 - 0.5f)), 6f);
                    let _e608: f32 = distortion;
                    distortion = (_e608 - 0.25f);
                    let _e611: f32 = distortion;
                    let _e615: vec4<f32> = params.data[29];
                    distortion = (_e611 - _e615.x);
                    let _e620: f32 = x_3;
                    frameFade = (1f - (2f * pow(abs((_e620 - 0.4f)), 2f)));
                    aa = 0.15f;
                    let _e629: f32 = aa;
                    let _e635: vec4<f32> = params.data[21];
                    aa = (_e629 + mix(0.1f, 0f, _e635.x));
                    let _e640: f32 = aa;
                    let _e641: f32 = xNonSmooth;
                    let _e645: f32 = aa;
                    let _e647: f32 = xNonSmooth;
                    fadeX = (smoothstep(0f, _e640, _e641) * smoothstep(1f, (1f - _e645), _e647));
                    let _e651: f32 = frameFade;
                    let _e652: f32 = fadeX;
                    frameFade = mix(1f, _e651, _e652);
                }
            } else {
                let _e657: vec4<f32> = params.data[28];
                if (_e657.x == 4f) {
                    {
                        let _e661: f32 = xNonSmooth;
                        x_3 = _e661;
                        let _e662: f32 = x_3;
                        distortion = sin(((_e662 + 0.25f) * 6.2831855f));
                        let _e670: f32 = distortion;
                        shadows = (0.5f + ((0.5f * asin(_e670)) / 1.5707964f));
                        let _e678: f32 = distortion;
                        distortion = (_e678 * 0.5f);
                        let _e681: f32 = distortion;
                        let _e685: vec4<f32> = params.data[29];
                        distortion = (_e681 - _e685.x);
                        let _e690: f32 = x_3;
                        frameFade = (0.5f + (0.5f * sin((_e690 * 6.2831855f))));
                    }
                } else {
                    let _e699: vec4<f32> = params.data[28];
                    if (_e699.x == 5f) {
                        {
                            let _e703: f32 = distortion;
                            let _e704: f32 = x_3;
                            let _e708: f32 = x_3;
                            distortion = (_e703 - (pow(abs(_e704), 0.2f) * _e708));
                            let _e711: f32 = distortion;
                            distortion = (_e711 + 0.33f);
                            let _e714: f32 = distortion;
                            let _e719: vec4<f32> = params.data[29];
                            distortion = (_e714 - (3f * _e719.x));
                            let _e723: f32 = distortion;
                            distortion = (_e723 * 0.33f);
                            let _e729: f32 = x_3;
                            frameFade = (0.3f * smoothstep(0f, 1f, _e729));
                            let _e732: f32 = x_3;
                            shadows = pow(_e732, 2.5f);
                            let _e736: f32 = aa;
                            aa = max(0.1f, _e736);
                            let _e738: f32 = aa;
                            let _e744: vec4<f32> = params.data[21];
                            aa = (_e738 + mix(0.1f, 0f, _e744.x));
                            let _e749: f32 = aa;
                            let _e750: f32 = xNonSmooth;
                            let _e754: f32 = aa;
                            let _e756: f32 = xNonSmooth;
                            fadeX = (smoothstep(0f, _e749, _e750) * smoothstep(1f, (1f - _e754), _e756));
                            let _e759: f32 = distortion;
                            let _e760: f32 = fadeX;
                            distortion = (_e759 * _e760);
                        }
                    }
                }
            }
        }
    }
    let _e762: vec2<f32> = v_imageUV_1;
    let _e763: vec2<f32> = dpdx(_e762);
    dudx = _e763;
    let _e765: vec2<f32> = v_imageUV_1;
    let _e766: vec2<f32> = dpdy(_e765);
    dudy = _e766;
    let _e768: vec2<f32> = v_imageUV_1;
    grainUV = (_e768 - vec2(0.5f));
    let _e773: vec2<f32> = grainUV;
    let _e775: vec2<f32> = dudx;
    let _e777: vec2<f32> = dudy;
    grainUV = (_e773 * (vec2(0.8f) / vec2<f32>(length(_e775), length(_e777))));
    let _e783: vec2<f32> = grainUV;
    grainUV = (_e783 + vec2(0.5f));
    let _e787: vec2<f32> = grainUV;
    let _e788: f32 = valueNoise(_e787);
    grain = _e788;
    let _e792: f32 = grain;
    grain = smoothstep(0.4f, 0.7f, _e792);
    let _e794: f32 = grain;
    let _e798: vec4<f32> = params.data[36];
    grain = (_e794 * _e798.x);
    let _e801: f32 = distortion;
    let _e803: f32 = grain;
    distortion = mix(_e801, 0f, _e803);
    let _e805: f32 = shadows;
    shadows = min(_e805, 1f);
    let _e808: f32 = shadows;
    let _e809: f32 = maskStrokeInner;
    shadows = (_e808 + _e809);
    let _e811: f32 = shadows;
    let _e812: f32 = mask;
    shadows = (_e811 * _e812);
    let _e814: f32 = shadows;
    shadows = min(_e814, 1f);
    let _e817: f32 = shadows;
    let _e821: vec4<f32> = params.data[22];
    shadows = (_e817 * pow(_e821.x, 2f));
    let _e826: f32 = shadows;
    shadows = clamp(_e826, 0f, 1f);
    let _e830: f32 = distortion;
    let _e835: vec4<f32> = params.data[26];
    distortion = (_e830 * (3f * _e835.x));
    let _e839: f32 = frameFade;
    let _e843: vec4<f32> = params.data[26];
    frameFade = (_e839 * _e843.x);
    let _e847: vec2<f32> = fractOrigUV;
    let _e849: f32 = distortion;
    fractOrigUV.x = (_e847.x + _e849);
    let _e851: vec2<f32> = floorOrigUV;
    let _e852: f32 = patternRotation;
    let _e857: vec4<f32> = params.data[14];
    let _e859: vec2<f32> = rotateAspect(_e851, -(_e852), _e857.x);
    floorOrigUV = _e859;
    let _e860: vec2<f32> = fractOrigUV;
    let _e861: f32 = patternRotation;
    let _e866: vec4<f32> = params.data[14];
    let _e868: vec2<f32> = rotateAspect(_e860, -(_e861), _e866.x);
    fractOrigUV = _e868;
    let _e869: vec2<f32> = floorOrigUV;
    let _e870: vec2<f32> = fractOrigUV;
    let _e872: f32 = patternSize;
    uv_8 = ((_e869 + _e870) / vec2(_e872));
    let _e875: vec2<f32> = uv_8;
    let _e876: f32 = maskStroke;
    uv_8 = (_e875 + vec2(pow(_e876, 4f)));
    let _e881: vec2<f32> = uv_8;
    uv_8 = (_e881 + vec2(0.5f));
    let _e885: vec2<f32> = v_imageUV_1;
    let _e886: vec2<f32> = uv_8;
    let _e889: f32 = mask;
    uv_8 = mix(_e885, _e886, vec2(smoothstep(0f, 0.7f, _e889)));
    let _e898: vec4<f32> = params.data[30];
    blur = mix(0f, 50f, _e898.x);
    let _e903: f32 = blur;
    let _e906: f32 = mask;
    blur = mix(0f, _e903, smoothstep(0.5f, 1f, _e906));
    let _e914: vec4<f32> = params.data[31];
    edgeDistortion = mix(0f, 0.04f, _e914.x);
    let _e918: f32 = edgeDistortion;
    let _e920: f32 = frameFade;
    let _e925: vec4<f32> = params.data[31];
    edgeDistortion = (_e918 + ((0.06f * _e920) * _e925.x));
    let _e929: f32 = edgeDistortion;
    let _e930: f32 = mask;
    edgeDistortion = (_e929 * _e930);
    let _e932: vec2<f32> = uv_8;
    let _e933: f32 = edgeDistortion;
    let _e934: f32 = getUvFrame(_e932, _e933);
    frame = _e934;
    let _e939: f32 = xNonSmooth;
    let _e945: f32 = xNonSmooth;
    stretch = (1f - (smoothstep(0f, 0.5f, _e939) * smoothstep(1f, 0.5f, _e945)));
    let _e950: f32 = stretch;
    stretch = pow(_e950, 2f);
    let _e953: f32 = stretch;
    let _e954: f32 = mask;
    stretch = (_e953 * _e954);
    let _e956: f32 = stretch;
    let _e957: vec2<f32> = uv_8;
    let _e960: f32 = mask;
    let _e962: f32 = frameFade;
    let _e965: f32 = getUvFrame(_e957, (0.1f + ((0.05f * _e960) * _e962)));
    stretch = (_e956 * _e965);
    let _e968: vec2<f32> = uv_8;
    let _e974: vec4<f32> = params.data[24];
    let _e976: f32 = stretch;
    uv_8.y = mix(_e968.y, 0.5f, (_e974.x * _e976));
    let _e979: vec2<f32> = uv_8;
    let _e984: vec4<f32> = params.data[0];
    let _e991: vec4<f32> = params.data[1];
    let _e998: f32 = blur;
    let _e999: vec4<f32> = getBlur(_e979, ((vec2(1f) / _e984.xy) / vec2(_e991.x)), vec2<f32>(0f, 1f), _e998);
    image = _e999;
    let _e1001: vec4<f32> = image;
    let _e1003: vec4<f32> = image;
    let _e1005: vec4<f32> = image;
    let _e1007: vec3<f32> = (_e1003.xyz * _e1005.w);
    image.x = _e1007.x;
    image.y = _e1007.y;
    image.z = _e1007.z;
    let _e1017: vec4<f32> = params.data[12];
    backColor = _e1017;
    let _e1019: vec4<f32> = backColor;
    let _e1021: vec4<f32> = backColor;
    let _e1023: vec4<f32> = backColor;
    let _e1025: vec3<f32> = (_e1021.xyz * _e1023.w);
    backColor.x = _e1025.x;
    backColor.y = _e1025.y;
    backColor.z = _e1025.z;
    let _e1035: vec4<f32> = params.data[13];
    highlightColor = _e1035;
    let _e1037: vec4<f32> = highlightColor;
    let _e1039: vec4<f32> = highlightColor;
    let _e1041: vec4<f32> = highlightColor;
    let _e1043: vec3<f32> = (_e1039.xyz * _e1041.w);
    highlightColor.x = _e1043.x;
    highlightColor.y = _e1043.y;
    highlightColor.z = _e1043.z;
    let _e1053: vec4<f32> = params.data[20];
    shadowColor = _e1053;
    let _e1055: vec4<f32> = highlightColor;
    let _e1057: f32 = highlights;
    color = (_e1055.xyz * _e1057);
    let _e1060: vec4<f32> = highlightColor;
    let _e1062: f32 = highlights;
    opacity = (_e1060.w * _e1062);
    let _e1065: f32 = shadows;
    let _e1066: vec4<f32> = shadowColor;
    let _e1070: f32 = highlights;
    shadows = mix((_e1065 * _e1066.w), 0f, _e1070);
    let _e1072: vec3<f32> = color;
    let _e1073: vec4<f32> = shadowColor;
    let _e1075: vec4<f32> = shadowColor;
    let _e1079: f32 = shadows;
    color = mix(_e1072, (_e1073.xyz * _e1075.w), vec3((0.5f * _e1079)));
    let _e1083: vec3<f32> = color;
    let _e1085: f32 = shadows;
    let _e1089: vec4<f32> = shadowColor;
    color = (_e1083 + ((0.5f * pow(_e1085, 0.5f)) * _e1089.xyz));
    let _e1093: f32 = opacity;
    let _e1094: f32 = shadows;
    opacity = (_e1093 + _e1094);
    let _e1096: vec3<f32> = color;
    color = clamp(_e1096, vec3(0f), vec3(1f));
    let _e1102: f32 = opacity;
    opacity = clamp(_e1102, 0f, 1f);
    let _e1106: vec3<f32> = color;
    let _e1107: vec4<f32> = image;
    let _e1110: f32 = opacity;
    let _e1113: f32 = frame;
    color = (_e1106 + ((_e1107.xyz * (1f - _e1110)) * _e1113));
    let _e1116: f32 = opacity;
    let _e1117: vec4<f32> = image;
    let _e1120: f32 = opacity;
    let _e1123: f32 = frame;
    opacity = (_e1116 + ((_e1117.w * (1f - _e1120)) * _e1123));
    let _e1126: vec3<f32> = color;
    let _e1127: vec4<f32> = backColor;
    let _e1130: f32 = opacity;
    color = (_e1126 + (_e1127.xyz * (1f - _e1130)));
    let _e1134: f32 = opacity;
    let _e1135: vec4<f32> = backColor;
    let _e1138: f32 = opacity;
    opacity = (_e1134 + (_e1135.w * (1f - _e1138)));
    let _e1142: vec2<f32> = grainUV;
    let _e1144: vec2<f32> = rotate(_e1142, 1f);
    let _e1148: f32 = valueNoise((_e1144 + vec2(3f)));
    grainOverlay = _e1148;
    let _e1150: f32 = grainOverlay;
    let _e1151: vec2<f32> = grainUV;
    let _e1153: vec2<f32> = rotate(_e1151, 2f);
    let _e1158: f32 = valueNoise((_e1153 + vec2(-1f)));
    grainOverlay = mix(_e1150, _e1158, 0.5f);
    let _e1161: f32 = grainOverlay;
    grainOverlay = pow(_e1161, 1.3f);
    let _e1164: f32 = grainOverlay;
    grainOverlayV = ((_e1164 * 2f) - 1f);
    let _e1171: f32 = grainOverlayV;
    grainOverlayColor = vec3(step(0f, _e1171));
    let _e1178: vec4<f32> = params.data[37];
    let _e1180: f32 = grainOverlayV;
    grainOverlayStrength = (_e1178.x * abs(_e1180));
    let _e1184: f32 = grainOverlayStrength;
    grainOverlayStrength = pow(_e1184, 0.8f);
    let _e1187: f32 = grainOverlayStrength;
    let _e1188: f32 = mask;
    grainOverlayStrength = (_e1187 * _e1188);
    let _e1190: vec3<f32> = color;
    let _e1191: vec3<f32> = grainOverlayColor;
    let _e1193: f32 = grainOverlayStrength;
    color = mix(_e1190, _e1191, vec3((0.35f * _e1193)));
    let _e1197: f32 = opacity;
    let _e1199: f32 = grainOverlayStrength;
    opacity = (_e1197 + (0.5f * _e1199));
    let _e1202: f32 = opacity;
    opacity = clamp(_e1202, 0f, 1f);
    let _e1206: vec3<f32> = color;
    let _e1207: f32 = opacity;
    fragColor = vec4<f32>(_e1206.x, _e1206.y, _e1206.z, _e1207);
    return;
}

@fragment 
fn main(@location(0) v_imageUV: vec2<f32>, @builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    v_imageUV_1 = v_imageUV;
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e22: vec4<f32> = fragColor;
    let paper = _e22;
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
