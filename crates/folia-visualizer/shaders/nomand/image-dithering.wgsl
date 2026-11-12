// Paper Shaders 0.0.80, Copyright 2026 Paper, Apache-2.0. GLSL translated using Naga 27.0.3.
// Source https://github.com/paper-design/shaders; license and notice in assets/paper-shaders.
struct Params {
    data: array<vec4<f32>, 72>,
}

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

const bayer2x2_: array<i32, 4> = array<i32, 4>(0i, 2i, 3i, 1i);
const bayer4x4_: array<i32, 16> = array<i32, 16>(0i, 8i, 2i, 10i, 12i, 4i, 14i, 6i, 3i, 11i, 1i, 9i, 15i, 7i, 13i, 5i);
const bayer8x8_: array<i32, 64> = array<i32, 64>(0i, 32i, 8i, 40i, 2i, 34i, 10i, 42i, 48i, 16i, 56i, 24i, 50i, 18i, 58i, 26i, 12i, 44i, 4i, 36i, 14i, 46i, 6i, 38i, 60i, 28i, 52i, 20i, 62i, 30i, 54i, 22i, 3i, 35i, 11i, 43i, 1i, 33i, 9i, 41i, 51i, 19i, 59i, 27i, 49i, 17i, 57i, 25i, 15i, 47i, 7i, 39i, 13i, 45i, 5i, 37i, 63i, 31i, 55i, 23i, 61i, 29i, 53i, 21i);

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
var<private> fragColor: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

fn hash21_(p: vec2<f32>) -> f32 {
    var p_1: vec2<f32>;

    p_1 = p;
    let _e10: vec2<f32> = p_1;
    p_1 = (fract((_e10 * vec2<f32>(0.3183099f, 0.3678794f))) + vec2(0.1f));
    let _e19: vec2<f32> = p_1;
    let _e20: vec2<f32> = p_1;
    let _e21: vec2<f32> = p_1;
    p_1 = (_e19 + vec2(dot(_e20, (_e21 + vec2(19.19f)))));
    let _e28: vec2<f32> = p_1;
    let _e30: vec2<f32> = p_1;
    return fract((_e28.x * _e30.y));
}

fn getUvFrame(uv: vec2<f32>, pad: vec2<f32>) -> f32 {
    var uv_1: vec2<f32>;
    var pad_1: vec2<f32>;
    var aa: f32 = 0.0001f;
    var left: f32;
    var right: f32;
    var bottom: f32;
    var top: f32;

    uv_1 = uv;
    pad_1 = pad;
    let _e14: vec2<f32> = pad_1;
    let _e17: vec2<f32> = pad_1;
    let _e20: f32 = aa;
    let _e22: vec2<f32> = uv_1;
    left = smoothstep(-(_e14.x), (-(_e17.x) + _e20), _e22.x);
    let _e27: vec2<f32> = pad_1;
    let _e31: vec2<f32> = pad_1;
    let _e34: f32 = aa;
    let _e36: vec2<f32> = uv_1;
    right = smoothstep((1f + _e27.x), ((1f + _e31.x) - _e34), _e36.x);
    let _e40: vec2<f32> = pad_1;
    let _e43: vec2<f32> = pad_1;
    let _e46: f32 = aa;
    let _e48: vec2<f32> = uv_1;
    bottom = smoothstep(-(_e40.y), (-(_e43.y) + _e46), _e48.y);
    let _e53: vec2<f32> = pad_1;
    let _e57: vec2<f32> = pad_1;
    let _e60: f32 = aa;
    let _e62: vec2<f32> = uv_1;
    top = smoothstep((1f + _e53.y), ((1f + _e57.y) - _e60), _e62.y);
    let _e66: f32 = left;
    let _e67: f32 = right;
    let _e69: f32 = bottom;
    let _e71: f32 = top;
    return (((_e66 * _e67) * _e69) * _e71);
}

fn getImageUV(uv_2: vec2<f32>) -> vec2<f32> {
    var uv_3: vec2<f32>;
    var boxOrigin: vec2<f32>;
    var r: f32;
    var graphicRotation: mat2x2<f32>;
    var graphicOffset: vec2<f32>;
    var imageBoxSize: vec2<f32>;
    var imageBoxScale: vec2<f32>;
    var imageUV: vec2<f32>;

    uv_3 = uv_2;
    let _e14: vec4<f32> = params.data[2];
    let _e20: vec4<f32> = params.data[3];
    boxOrigin = vec2<f32>((0.5f - _e14.x), (_e20.x - 0.5f));
    let _e29: vec4<f32> = params.data[8];
    r = ((_e29.x * 3.1415927f) / 180f);
    let _e36: f32 = r;
    let _e38: f32 = r;
    let _e40: f32 = r;
    let _e43: f32 = r;
    graphicRotation = mat2x2<f32>(vec2<f32>(cos(_e36), sin(_e38)), vec2<f32>(-(sin(_e40)), cos(_e43)));
    let _e52: vec4<f32> = params.data[9];
    let _e58: vec4<f32> = params.data[10];
    graphicOffset = vec2<f32>(-(_e52.x), _e58.x);
    let _e66: vec4<f32> = params.data[6];
    if (_e66.x == 1f) {
        {
            let _e74: vec4<f32> = params.data[0];
            let _e80: vec4<f32> = params.data[14];
            let _e86: vec4<f32> = params.data[0];
            let _e93: vec4<f32> = params.data[14];
            imageBoxSize.x = (min((_e74.x / _e80.x), _e86.y) * _e93.x);
        }
    } else {
        let _e99: vec4<f32> = params.data[6];
        if (_e99.x == 2f) {
            {
                let _e107: vec4<f32> = params.data[0];
                let _e113: vec4<f32> = params.data[14];
                let _e119: vec4<f32> = params.data[0];
                let _e126: vec4<f32> = params.data[14];
                imageBoxSize.x = (max((_e107.x / _e113.x), _e119.y) * _e126.x);
            }
        } else {
            {
                let _e135: vec4<f32> = params.data[14];
                let _e141: vec4<f32> = params.data[14];
                imageBoxSize.x = min(10f, ((10f / _e135.x) * _e141.x));
            }
        }
    }
    let _e146: vec2<f32> = imageBoxSize;
    let _e151: vec4<f32> = params.data[14];
    imageBoxSize.y = (_e146.x / _e151.x);
    let _e157: vec4<f32> = params.data[0];
    let _e160: vec2<f32> = imageBoxSize;
    imageBoxScale = (_e157.xy / _e160);
    let _e163: vec2<f32> = uv_3;
    imageUV = _e163;
    let _e165: vec2<f32> = imageUV;
    let _e166: vec2<f32> = imageBoxScale;
    imageUV = (_e165 * _e166);
    let _e168: vec2<f32> = imageUV;
    let _e169: vec2<f32> = boxOrigin;
    let _e170: vec2<f32> = imageBoxScale;
    imageUV = (_e168 + (_e169 * (_e170 - vec2(1f))));
    let _e176: vec2<f32> = imageUV;
    let _e177: vec2<f32> = graphicOffset;
    imageUV = (_e176 + _e177);
    let _e179: vec2<f32> = imageUV;
    let _e183: vec4<f32> = params.data[7];
    imageUV = (_e179 / vec2(_e183.x));
    let _e188: vec2<f32> = imageUV;
    let _e193: vec4<f32> = params.data[14];
    imageUV.x = (_e188.x * _e193.x);
    let _e196: mat2x2<f32> = graphicRotation;
    let _e197: vec2<f32> = imageUV;
    imageUV = (_e196 * _e197);
    let _e200: vec2<f32> = imageUV;
    let _e205: vec4<f32> = params.data[14];
    imageUV.x = (_e200.x / _e205.x);
    let _e208: vec2<f32> = imageUV;
    imageUV = (_e208 + vec2(0.5f));
    let _e214: vec2<f32> = imageUV;
    imageUV.y = (1f - _e214.y);
    let _e217: vec2<f32> = imageUV;
    return _e217;
}

fn getBayerValue(uv_4: vec2<f32>, size: i32) -> f32 {
    var uv_5: vec2<f32>;
    var size_1: i32;
    var pos: vec2<i32>;
    var index: i32;
    var local: array<i32, 4> = bayer2x2_;
    var local_1: array<i32, 16> = bayer4x4_;
    var local_2: array<i32, 64> = bayer8x8_;

    uv_5 = uv_4;
    size_1 = size;
    let _e15: vec2<f32> = uv_5;
    let _e16: i32 = size_1;
    let _e21: i32 = size_1;
    pos = vec2<i32>((fract((_e15 / vec2(f32(_e16)))) * f32(_e21)));
    let _e26: vec2<i32> = pos;
    let _e28: i32 = size_1;
    let _e30: vec2<i32> = pos;
    index = ((_e26.y * _e28) + _e30.x);
    let _e34: i32 = size_1;
    if (_e34 == 2i) {
        {
            let _e37: i32 = index;
            let _e41: i32 = local[_e37];
            return (f32(_e41) / 4f);
        }
    } else {
        let _e45: i32 = size_1;
        if (_e45 == 4i) {
            {
                let _e48: i32 = index;
                let _e52: i32 = local_1[_e48];
                return (f32(_e52) / 16f);
            }
        } else {
            let _e56: i32 = size_1;
            if (_e56 == 8i) {
                {
                    let _e59: i32 = index;
                    let _e63: i32 = local_2[_e59];
                    return (f32(_e63) / 64f);
                }
            }
        }
    }
    return 0f;
}

fn main_1() {
    var pxSize: f32;
    var pxSizeUV: vec2<f32>;
    var canvasPixelizedUV: vec2<f32>;
    var normalizedUV: vec2<f32>;
    var imageUV_1: vec2<f32>;
    var ditheringNoiseUV: vec2<f32>;
    var image: vec4<f32>;
    var frame: f32;
    var type_45: i32;
    var dithering: f32 = 0f;
    var lum: f32;
    var local_3: f32;
    var colorSteps: f32;
    var color: vec3<f32> = vec3(0f);
    var opacity: f32 = 1f;
    var brightness: f32;
    var quantLum: f32;
    var normColor: vec3<f32>;
    var quantAlpha: f32;
    var fgColor: vec3<f32>;
    var fgOpacity: f32;
    var bgColor: vec3<f32>;
    var bgOpacity: f32;
    var hlColor: vec3<f32>;
    var hlOpacity: f32;

    let _e14: vec4<f32> = params.data[16];
    let _e19: vec4<f32> = params.data[1];
    pxSize = (_e14.x * _e19.x);
    let _e24: vec4<f32> = gl_FragCoord_1;
    let _e29: vec4<f32> = params.data[0];
    let _e32: vec4<f32> = gl_FragCoord_1;
    let _e40: vec4<f32> = params.data[0];
    pxSizeUV = (vec2<f32>(_e24.x, (_e29.y - _e32.y)) - (0.5f * _e40.xy));
    let _e45: vec2<f32> = pxSizeUV;
    let _e46: f32 = pxSize;
    pxSizeUV = (_e45 / vec2(_e46));
    let _e49: vec2<f32> = pxSizeUV;
    let _e54: f32 = pxSize;
    canvasPixelizedUV = ((floor(_e49) + vec2(0.5f)) * _e54);
    let _e57: vec2<f32> = canvasPixelizedUV;
    let _e61: vec4<f32> = params.data[0];
    normalizedUV = (_e57 / _e61.xy);
    let _e65: vec2<f32> = normalizedUV;
    let _e66: vec2<f32> = getImageUV(_e65);
    imageUV_1 = _e66;
    let _e68: vec2<f32> = canvasPixelizedUV;
    ditheringNoiseUV = _e68;
    let _e70: vec2<f32> = imageUV_1;
    let _e71: vec4<f32> = textureSample(cover_texture, cover_sampler, _e70);
    image = _e71;
    let _e73: vec2<f32> = imageUV_1;
    let _e74: f32 = pxSize;
    let _e78: vec4<f32> = params.data[0];
    let _e82: f32 = getUvFrame(_e73, (vec2(_e74) / _e78.xy));
    frame = _e82;
    let _e87: vec4<f32> = params.data[15];
    type_45 = i32(floor(_e87.x));
    let _e98: vec4<f32> = image;
    lum = dot(vec3<f32>(0.2126f, 0.7152f, 0.0722f), _e98.xyz);
    let _e105: vec4<f32> = params.data[18];
    if (_e105.x != 0f) {
        let _e110: f32 = lum;
        local_3 = (1f - _e110);
    } else {
        let _e112: f32 = lum;
        local_3 = _e112;
    }
    let _e114: f32 = local_3;
    lum = _e114;
    let _e115: i32 = type_45;
    switch _e115 {
        case 1: {
            {
                let _e116: vec2<f32> = ditheringNoiseUV;
                let _e117: f32 = hash21_(_e116);
                let _e118: f32 = lum;
                dithering = step(_e117, _e118);
            }
        }
        case 2: {
            let _e120: vec2<f32> = pxSizeUV;
            let _e122: f32 = getBayerValue(_e120, 2i);
            dithering = _e122;
        }
        case 3: {
            let _e123: vec2<f32> = pxSizeUV;
            let _e125: f32 = getBayerValue(_e123, 4i);
            dithering = _e125;
        }
        default: {
            let _e126: vec2<f32> = pxSizeUV;
            let _e128: f32 = getBayerValue(_e126, 8i);
            dithering = _e128;
        }
    }
    let _e132: vec4<f32> = params.data[19];
    colorSteps = max(floor(_e132.x), 1f);
    let _e143: f32 = dithering;
    dithering = (_e143 - 0.5f);
    let _e146: f32 = lum;
    let _e147: f32 = dithering;
    let _e148: f32 = colorSteps;
    brightness = clamp((_e146 + (_e147 / _e148)), 0f, 1f);
    let _e156: f32 = brightness;
    let _e157: f32 = frame;
    brightness = mix(0f, _e156, _e157);
    let _e160: f32 = brightness;
    let _e161: vec4<f32> = image;
    brightness = mix(0f, _e160, _e161.w);
    let _e164: f32 = brightness;
    let _e165: f32 = colorSteps;
    let _e170: f32 = colorSteps;
    quantLum = (floor(((_e164 * _e165) + 0.5f)) / _e170);
    let _e174: f32 = quantLum;
    let _e175: f32 = frame;
    quantLum = mix(0f, _e174, _e175);
    let _e180: vec4<f32> = params.data[17];
    if ((_e180.x != 0f) == true) {
        {
            let _e186: vec4<f32> = image;
            let _e188: f32 = lum;
            normColor = (_e186.xyz / vec3(max(_e188, 0.001f)));
            let _e194: vec3<f32> = normColor;
            let _e195: f32 = quantLum;
            color = (_e194 * _e195);
            let _e197: vec4<f32> = image;
            let _e199: f32 = colorSteps;
            let _e204: f32 = colorSteps;
            quantAlpha = (floor(((_e197.w * _e199) + 0.5f)) / _e204);
            let _e207: f32 = quantLum;
            let _e209: f32 = quantAlpha;
            opacity = mix(_e207, 1f, _e209);
        }
    } else {
        {
            let _e214: vec4<f32> = params.data[11];
            let _e219: vec4<f32> = params.data[11];
            fgColor = (_e214.xyz * _e219.w);
            let _e226: vec4<f32> = params.data[11];
            fgOpacity = _e226.w;
            let _e232: vec4<f32> = params.data[12];
            let _e237: vec4<f32> = params.data[12];
            bgColor = (_e232.xyz * _e237.w);
            let _e244: vec4<f32> = params.data[12];
            bgOpacity = _e244.w;
            let _e250: vec4<f32> = params.data[13];
            let _e255: vec4<f32> = params.data[13];
            hlColor = (_e250.xyz * _e255.w);
            let _e262: vec4<f32> = params.data[13];
            hlOpacity = _e262.w;
            let _e265: vec3<f32> = fgColor;
            let _e266: vec3<f32> = hlColor;
            let _e272: vec4<f32> = params.data[19];
            let _e276: f32 = brightness;
            fgColor = mix(_e265, _e266, vec3(step((1.02f - (0.02f * _e272.x)), _e276)));
            let _e280: f32 = fgOpacity;
            let _e281: f32 = hlOpacity;
            let _e287: vec4<f32> = params.data[19];
            let _e291: f32 = brightness;
            fgOpacity = mix(_e280, _e281, step((1.02f - (0.02f * _e287.x)), _e291));
            let _e294: vec3<f32> = fgColor;
            let _e295: f32 = quantLum;
            color = (_e294 * _e295);
            let _e297: f32 = fgOpacity;
            let _e298: f32 = quantLum;
            opacity = (_e297 * _e298);
            let _e300: vec3<f32> = color;
            let _e301: vec3<f32> = bgColor;
            let _e303: f32 = opacity;
            color = (_e300 + (_e301 * (1f - _e303)));
            let _e307: f32 = opacity;
            let _e308: f32 = bgOpacity;
            let _e310: f32 = opacity;
            opacity = (_e307 + (_e308 * (1f - _e310)));
        }
    }
    let _e314: vec3<f32> = color;
    let _e315: f32 = opacity;
    fragColor = vec4<f32>(_e314.x, _e314.y, _e314.z, _e315);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
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
