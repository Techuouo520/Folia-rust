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
var<private> gl_FragCoord_1: vec4<f32>;

fn getUvFrame(uv: vec2<f32>) -> f32 {
    var uv_1: vec2<f32>;
    var aax: f32;
    var aay: f32;
    var left: f32;
    var right: f32;
    var bottom: f32;
    var top: f32;

    uv_1 = uv;
    let _e12: vec2<f32> = uv_1;
    let _e14: f32 = fwidth(_e12.x);
    aax = (2f * _e14);
    let _e18: vec2<f32> = uv_1;
    let _e20: f32 = fwidth(_e18.y);
    aay = (2f * _e20);
    let _e24: f32 = aax;
    let _e25: vec2<f32> = uv_1;
    left = smoothstep(0f, _e24, _e25.x);
    let _e31: f32 = aax;
    let _e34: vec2<f32> = uv_1;
    right = (1f - smoothstep((1f - _e31), 1f, _e34.x));
    let _e40: f32 = aay;
    let _e41: vec2<f32> = uv_1;
    bottom = smoothstep(0f, _e40, _e41.y);
    let _e47: f32 = aay;
    let _e50: vec2<f32> = uv_1;
    top = (1f - smoothstep((1f - _e47), 1f, _e50.y));
    let _e55: f32 = left;
    let _e56: f32 = right;
    let _e58: f32 = bottom;
    let _e60: f32 = top;
    return (((_e55 * _e56) * _e58) * _e60);
}

fn rotate(uv_2: vec2<f32>, th: f32) -> vec2<f32> {
    var uv_3: vec2<f32>;
    var th_1: f32;

    uv_3 = uv_2;
    th_1 = th;
    let _e13: f32 = th_1;
    let _e15: f32 = th_1;
    let _e17: f32 = th_1;
    let _e20: f32 = th_1;
    let _e25: vec2<f32> = uv_3;
    return (mat2x2<f32>(vec2<f32>(cos(_e13), sin(_e15)), vec2<f32>(-(sin(_e17)), cos(_e20))) * _e25);
}

fn randomR(p: vec2<f32>) -> f32 {
    var p_1: vec2<f32>;
    var uv_4: vec2<f32>;

    p_1 = p;
    let _e11: vec2<f32> = p_1;
    uv_4 = ((floor(_e11) / vec2(100f)) + vec2(0.5f));
    let _e20: vec2<f32> = uv_4;
    let _e22: vec4<f32> = textureSample(noise_texture, noise_sampler, fract(_e20));
    return _e22.x;
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
    let _e18: f32 = randomR(_e17);
    a = _e18;
    let _e20: vec2<f32> = i;
    let _e25: f32 = randomR((_e20 + vec2<f32>(1f, 0f)));
    b = _e25;
    let _e27: vec2<f32> = i;
    let _e32: f32 = randomR((_e27 + vec2<f32>(0f, 1f)));
    c = _e32;
    let _e34: vec2<f32> = i;
    let _e39: f32 = randomR((_e34 + vec2<f32>(1f, 1f)));
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

fn fbm(n: vec2<f32>) -> f32 {
    var n_1: vec2<f32>;
    var total: f32 = 0f;
    var amplitude: f32 = 0.4f;
    var i_1: i32 = 0i;

    n_1 = n;
    loop {
        let _e17: i32 = i_1;
        if !((_e17 < 3i)) {
            break;
        }
        {
            let _e24: f32 = total;
            let _e25: vec2<f32> = n_1;
            let _e26: f32 = valueNoise(_e25);
            let _e27: f32 = amplitude;
            total = (_e24 + (_e26 * _e27));
            let _e30: vec2<f32> = n_1;
            n_1 = (_e30 * 1.99f);
            let _e33: f32 = amplitude;
            amplitude = (_e33 * 0.65f);
        }
        continuing {
            let _e21: i32 = i_1;
            i_1 = (_e21 + 1i);
        }
    }
    let _e36: f32 = total;
    return _e36;
}

fn randomG(p_2: vec2<f32>) -> f32 {
    var p_3: vec2<f32>;
    var uv_5: vec2<f32>;

    p_3 = p_2;
    let _e11: vec2<f32> = p_3;
    uv_5 = ((floor(_e11) / vec2(50f)) + vec2(0.5f));
    let _e20: vec2<f32> = uv_5;
    let _e22: vec4<f32> = textureSample(noise_texture, noise_sampler, fract(_e20));
    return _e22.y;
}

fn roughness(p_4: vec2<f32>) -> f32 {
    var p_5: vec2<f32>;
    var o: f32 = 0f;
    var i_2: f32 = 0f;
    var w: vec4<f32>;
    var f_1: vec2<f32>;

    p_5 = p_4;
    let _e11: vec2<f32> = p_5;
    p_5 = (_e11 * 0.1f);
    loop {
        let _e18: f32 = i_2;
        let _e20: f32 = (_e18 + 1f);
        i_2 = _e20;
        if !((_e20 < 4f)) {
            break;
        }
        {
            let _e27: vec2<f32> = p_5;
            let _e28: vec2<f32> = floor(_e27);
            let _e29: vec2<f32> = p_5;
            let _e30: vec2<f32> = ceil(_e29);
            w = vec4<f32>(_e28.x, _e28.y, _e30.x, _e30.y);
            let _e37: vec2<f32> = p_5;
            f_1 = fract(_e37);
            let _e40: f32 = o;
            let _e41: vec4<f32> = w;
            let _e43: f32 = randomG(_e41.xy);
            let _e44: vec4<f32> = w;
            let _e46: f32 = randomG(_e44.xw);
            let _e47: vec2<f32> = f_1;
            let _e50: vec4<f32> = w;
            let _e52: f32 = randomG(_e50.zy);
            let _e53: vec4<f32> = w;
            let _e55: f32 = randomG(_e53.zw);
            let _e56: vec2<f32> = f_1;
            let _e59: vec2<f32> = f_1;
            o = (_e40 + mix(mix(_e43, _e46, _e47.y), mix(_e52, _e55, _e56.y), _e59.x));
            let _e63: f32 = o;
            let _e67: vec2<f32> = p_5;
            let _e71: vec2<f32> = p_5;
            o = (_e63 + (0.2f / exp((2f * abs(sin(((0.2f * _e67.x) + (0.5f * _e71.y))))))));
        }
        continuing {
            let _e24: vec2<f32> = p_5;
            p_5 = (_e24 * 2.1f);
        }
    }
    let _e81: f32 = o;
    return (_e81 / 3f);
}

fn fiberRandom(p_6: vec2<f32>) -> f32 {
    var p_7: vec2<f32>;
    var uv_6: vec2<f32>;

    p_7 = p_6;
    let _e11: vec2<f32> = p_7;
    uv_6 = (floor(_e11) / vec2(100f));
    let _e17: vec2<f32> = uv_6;
    let _e19: vec4<f32> = textureSample(noise_texture, noise_sampler, fract(_e17));
    return _e19.z;
}

fn fiberValueNoise(st_2: vec2<f32>) -> f32 {
    var st_3: vec2<f32>;
    var i_3: vec2<f32>;
    var f_2: vec2<f32>;
    var a_1: f32;
    var b_1: f32;
    var c_1: f32;
    var d_1: f32;
    var u_1: vec2<f32>;
    var x1_1: f32;
    var x2_1: f32;

    st_3 = st_2;
    let _e11: vec2<f32> = st_3;
    i_3 = floor(_e11);
    let _e14: vec2<f32> = st_3;
    f_2 = fract(_e14);
    let _e17: vec2<f32> = i_3;
    let _e18: f32 = fiberRandom(_e17);
    a_1 = _e18;
    let _e20: vec2<f32> = i_3;
    let _e25: f32 = fiberRandom((_e20 + vec2<f32>(1f, 0f)));
    b_1 = _e25;
    let _e27: vec2<f32> = i_3;
    let _e32: f32 = fiberRandom((_e27 + vec2<f32>(0f, 1f)));
    c_1 = _e32;
    let _e34: vec2<f32> = i_3;
    let _e39: f32 = fiberRandom((_e34 + vec2<f32>(1f, 1f)));
    d_1 = _e39;
    let _e41: vec2<f32> = f_2;
    let _e42: vec2<f32> = f_2;
    let _e46: vec2<f32> = f_2;
    u_1 = ((_e41 * _e42) * (vec2(3f) - (2f * _e46)));
    let _e52: f32 = a_1;
    let _e53: f32 = b_1;
    let _e54: vec2<f32> = u_1;
    x1_1 = mix(_e52, _e53, _e54.x);
    let _e58: f32 = c_1;
    let _e59: f32 = d_1;
    let _e60: vec2<f32> = u_1;
    x2_1 = mix(_e58, _e59, _e60.x);
    let _e64: f32 = x1_1;
    let _e65: f32 = x2_1;
    let _e66: vec2<f32> = u_1;
    return mix(_e64, _e65, _e66.y);
}

fn fiberNoiseFbm(n_2: vec2<f32>, seedOffset: vec2<f32>) -> f32 {
    var n_3: vec2<f32>;
    var seedOffset_1: vec2<f32>;
    var total_1: f32 = 0f;
    var amplitude_1: f32 = 1f;
    var i_4: i32 = 0i;

    n_3 = n_2;
    seedOffset_1 = seedOffset;
    loop {
        let _e19: i32 = i_4;
        if !((_e19 < 4i)) {
            break;
        }
        {
            let _e26: vec2<f32> = n_3;
            let _e28: vec2<f32> = rotate(_e26, 0.7f);
            n_3 = _e28;
            let _e29: f32 = total_1;
            let _e30: vec2<f32> = n_3;
            let _e31: vec2<f32> = seedOffset_1;
            let _e33: f32 = fiberValueNoise((_e30 + _e31));
            let _e34: f32 = amplitude_1;
            total_1 = (_e29 + (_e33 * _e34));
            let _e37: vec2<f32> = n_3;
            n_3 = (_e37 * 2f);
            let _e40: f32 = amplitude_1;
            amplitude_1 = (_e40 * 0.6f);
        }
        continuing {
            let _e23: i32 = i_4;
            i_4 = (_e23 + 1i);
        }
    }
    let _e43: f32 = total_1;
    return _e43;
}

fn fiberNoise(uv_7: vec2<f32>, seedOffset_2: vec2<f32>) -> f32 {
    var uv_8: vec2<f32>;
    var seedOffset_3: vec2<f32>;
    var epsilon: f32 = 0.001f;
    var n1_: f32;
    var n2_: f32;
    var n3_: f32;
    var n4_: f32;

    uv_8 = uv_7;
    seedOffset_3 = seedOffset_2;
    let _e15: vec2<f32> = uv_8;
    let _e16: f32 = epsilon;
    let _e20: vec2<f32> = seedOffset_3;
    let _e21: f32 = fiberNoiseFbm((_e15 + vec2<f32>(_e16, 0f)), _e20);
    n1_ = _e21;
    let _e23: vec2<f32> = uv_8;
    let _e24: f32 = epsilon;
    let _e28: vec2<f32> = seedOffset_3;
    let _e29: f32 = fiberNoiseFbm((_e23 - vec2<f32>(_e24, 0f)), _e28);
    n2_ = _e29;
    let _e31: vec2<f32> = uv_8;
    let _e33: f32 = epsilon;
    let _e36: vec2<f32> = seedOffset_3;
    let _e37: f32 = fiberNoiseFbm((_e31 + vec2<f32>(0f, _e33)), _e36);
    n3_ = _e37;
    let _e39: vec2<f32> = uv_8;
    let _e41: f32 = epsilon;
    let _e44: vec2<f32> = seedOffset_3;
    let _e45: f32 = fiberNoiseFbm((_e39 - vec2<f32>(0f, _e41)), _e44);
    n4_ = _e45;
    let _e47: f32 = n1_;
    let _e48: f32 = n2_;
    let _e50: f32 = n3_;
    let _e51: f32 = n4_;
    let _e56: f32 = epsilon;
    return (length(vec2<f32>((_e47 - _e48), (_e50 - _e51))) / (2f * _e56));
}

fn randomGB(p_8: vec2<f32>) -> vec2<f32> {
    var p_9: vec2<f32>;
    var uv_9: vec2<f32>;

    p_9 = p_8;
    let _e11: vec2<f32> = p_9;
    uv_9 = ((floor(_e11) / vec2(50f)) + vec2(0.5f));
    let _e20: vec2<f32> = uv_9;
    let _e22: vec4<f32> = textureSample(noise_texture, noise_sampler, fract(_e20));
    return _e22.yz;
}

fn crumpledNoise(t: vec2<f32>, pw: f32) -> f32 {
    var t_1: vec2<f32>;
    var pw_1: f32;
    var p_10: vec2<f32>;
    var wsum: f32 = 0f;
    var cl: f32 = 0f;
    var y: i32 = -1i;
    var x: i32;
    var b_2: vec2<f32>;
    var q: vec2<f32>;
    var q2_: vec2<f32>;
    var c_2: vec2<f32>;
    var r: vec2<f32>;
    var w_1: f32;
    var local: f32;

    t_1 = t;
    pw_1 = pw;
    let _e13: vec2<f32> = t_1;
    p_10 = floor(_e13);
    loop {
        let _e23: i32 = y;
        if !((_e23 < 2i)) {
            break;
        }
        {
            x = -1i;
            loop {
                let _e33: i32 = x;
                if !((_e33 < 2i)) {
                    break;
                }
                {
                    let _e40: i32 = x;
                    let _e42: i32 = y;
                    b_2 = vec2<f32>(f32(_e40), f32(_e42));
                    let _e46: vec2<f32> = b_2;
                    let _e47: vec2<f32> = p_10;
                    q = (_e46 + _e47);
                    let _e50: vec2<f32> = q;
                    let _e51: vec2<f32> = q;
                    q2_ = (_e50 - (floor((_e51 / vec2(8f))) * 8f));
                    let _e60: vec2<f32> = q;
                    let _e61: vec2<f32> = q2_;
                    let _e62: vec2<f32> = randomGB(_e61);
                    c_2 = (_e60 + _e62);
                    let _e65: vec2<f32> = c_2;
                    let _e66: vec2<f32> = t_1;
                    r = (_e65 - _e66);
                    let _e72: vec2<f32> = r;
                    let _e77: f32 = pw_1;
                    let _e82: vec2<f32> = r;
                    let _e87: f32 = pw_1;
                    w_1 = (pow(smoothstep(0f, 1f, (1f - abs(_e72.x))), _e77) * pow(smoothstep(0f, 1f, (1f - abs(_e82.y))), _e87));
                    let _e91: f32 = cl;
                    let _e94: vec2<f32> = q2_;
                    let _e96: vec2<f32> = q2_;
                    let _e106: f32 = w_1;
                    cl = (_e91 + ((0.5f + (0.5f * sin(((_e94.x + (_e96.y * 5f)) * 8f)))) * _e106));
                    let _e109: f32 = wsum;
                    let _e110: f32 = w_1;
                    wsum = (_e109 + _e110);
                }
                continuing {
                    let _e37: i32 = x;
                    x = (_e37 + 1i);
                }
            }
        }
        continuing {
            let _e27: i32 = y;
            y = (_e27 + 1i);
        }
    }
    let _e112: f32 = wsum;
    if (_e112 != 0f) {
        let _e115: f32 = cl;
        let _e116: f32 = wsum;
        local = (_e115 / _e116);
    } else {
        local = 0f;
    }
    let _e120: f32 = local;
    return (pow(_e120, 0.5f) * 2f);
}

fn crumplesShape(uv_10: vec2<f32>) -> f32 {
    var uv_11: vec2<f32>;

    uv_11 = uv_10;
    let _e11: vec2<f32> = uv_11;
    let _e15: f32 = crumpledNoise((_e11 * 0.25f), 16f);
    let _e16: vec2<f32> = uv_11;
    let _e20: f32 = crumpledNoise((_e16 * 0.5f), 2f);
    return (_e15 * _e20);
}

fn folds(uv_12: vec2<f32>) -> vec2<f32> {
    var uv_13: vec2<f32>;
    var pp: vec3<f32> = vec3(0f);
    var l: f32 = 9f;
    var i_5: f32 = 0f;
    var rand: vec2<f32>;
    var an: f32;
    var p_11: vec2<f32>;
    var dist: f32;

    uv_13 = uv_12;
    loop {
        let _e18: f32 = i_5;
        if !((_e18 < 15f)) {
            break;
        }
        {
            let _e25: f32 = i_5;
            let _e29: vec4<f32> = params.data[45];
            if (_e25 >= _e29.x) {
                break;
            }
            let _e32: f32 = i_5;
            let _e33: f32 = i_5;
            let _e37: vec4<f32> = params.data[47];
            let _e41: vec2<f32> = randomGB(vec2<f32>(_e32, (_e33 * _e37.x)));
            rand = _e41;
            let _e43: vec2<f32> = rand;
            an = (_e43.x * 6.2831855f);
            let _e48: f32 = an;
            let _e50: f32 = an;
            let _e53: vec2<f32> = rand;
            p_11 = (vec2<f32>(cos(_e48), sin(_e50)) * _e53.y);
            let _e57: vec2<f32> = uv_13;
            let _e58: vec2<f32> = p_11;
            dist = distance(_e57, _e58);
            let _e61: f32 = l;
            let _e62: f32 = dist;
            l = min(_e61, _e62);
            let _e64: f32 = l;
            let _e65: f32 = dist;
            if (_e64 == _e65) {
                {
                    let _e67: vec3<f32> = pp;
                    let _e69: vec2<f32> = uv_13;
                    let _e70: vec2<f32> = p_11;
                    let _e72: vec2<f32> = (_e69 - _e70.xy);
                    pp.x = _e72.x;
                    pp.y = _e72.y;
                    let _e78: f32 = dist;
                    pp.z = _e78;
                }
            }
        }
        continuing {
            let _e22: f32 = i_5;
            i_5 = (_e22 + 1f);
        }
    }
    let _e79: vec3<f32> = pp;
    let _e83: vec3<f32> = pp;
    return mix(_e79.xy, vec2(0f), vec2(pow(_e83.z, 0.25f)));
}

fn drops(uv_14: vec2<f32>) -> f32 {
    var uv_15: vec2<f32>;
    var iDropsUV: vec2<f32>;
    var fDropsUV: vec2<f32>;
    var dropsMinDist: f32 = 1f;
    var j: i32 = -1i;
    var i_6: i32;
    var neighbor: vec2<f32>;
    var offset: vec2<f32>;
    var pos: vec2<f32>;
    var dist_1: f32;

    uv_15 = uv_14;
    let _e11: vec2<f32> = uv_15;
    iDropsUV = floor(_e11);
    let _e14: vec2<f32> = uv_15;
    fDropsUV = fract(_e14);
    loop {
        let _e22: i32 = j;
        if !((_e22 <= 1i)) {
            break;
        }
        {
            i_6 = -1i;
            loop {
                let _e32: i32 = i_6;
                if !((_e32 <= 1i)) {
                    break;
                }
                {
                    let _e39: i32 = i_6;
                    let _e41: i32 = j;
                    neighbor = vec2<f32>(f32(_e39), f32(_e41));
                    let _e45: vec2<f32> = iDropsUV;
                    let _e46: vec2<f32> = neighbor;
                    let _e48: vec2<f32> = randomGB((_e45 + _e46));
                    offset = _e48;
                    let _e56: vec4<f32> = params.data[47];
                    let _e60: vec2<f32> = offset;
                    offset = (vec2(0.5f) + (0.5f * sin((vec2((10f * _e56.x)) + (6.2831855f * _e60)))));
                    let _e68: vec2<f32> = neighbor;
                    let _e69: vec2<f32> = offset;
                    let _e71: vec2<f32> = fDropsUV;
                    pos = ((_e68 + _e69) - _e71);
                    let _e74: vec2<f32> = pos;
                    dist_1 = length(_e74);
                    let _e77: f32 = dropsMinDist;
                    let _e78: f32 = dropsMinDist;
                    let _e79: f32 = dist_1;
                    dropsMinDist = min(_e77, (_e78 * _e79));
                }
                continuing {
                    let _e36: i32 = i_6;
                    i_6 = (_e36 + 1i);
                }
            }
        }
        continuing {
            let _e26: i32 = j;
            j = (_e26 + 1i);
        }
    }
    let _e85: f32 = dropsMinDist;
    return (1f - smoothstep(0.05f, 0.09f, pow(_e85, 0.5f)));
}

fn lst(edge0_: f32, edge1_: f32, x_1: f32) -> f32 {
    var edge0_1: f32;
    var edge1_1: f32;
    var x_2: f32;

    edge0_1 = edge0_;
    edge1_1 = edge1_;
    x_2 = x_1;
    let _e15: f32 = x_2;
    let _e16: f32 = edge0_1;
    let _e18: f32 = edge1_1;
    let _e19: f32 = edge0_1;
    return clamp(((_e15 - _e16) / (_e18 - _e19)), 0f, 1f);
}

fn main_1() {
    var imageUV: vec2<f32>;
    var patternUV: vec2<f32>;
    var roughnessUv: vec2<f32>;
    var roughness_1: f32;
    var crumplesUV: vec2<f32>;
    var crumples: f32;
    var fiberUV: vec2<f32>;
    var fiber: f32;
    var normal: vec2<f32> = vec2(0f);
    var normalImage: vec2<f32> = vec2(0f);
    var foldsUV: vec2<f32>;
    var w_2: vec2<f32>;
    var w2_: vec2<f32>;
    var drops_1: f32;
    var fade: f32;
    var lightPos: vec3<f32> = vec3<f32>(1f, 2f, 1f);
    var res: f32;
    var fgColor: vec3<f32>;
    var fgOpacity: f32;
    var bgColor: vec3<f32>;
    var bgOpacity: f32;
    var frame: f32;
    var image: vec4<f32>;
    var color: vec3<f32>;
    var opacity: f32;

    let _e9: vec2<f32> = v_imageUV_1;
    imageUV = _e9;
    let _e11: vec2<f32> = v_imageUV_1;
    patternUV = (_e11 - vec2(0.5f));
    let _e17: vec2<f32> = patternUV;
    let _e21: vec4<f32> = params.data[14];
    patternUV = (5f * (_e17 * vec2<f32>(_e21.x, 1f)));
    let _e29: vec4<f32> = gl_FragCoord_1;
    let _e34: vec4<f32> = params.data[0];
    let _e37: vec4<f32> = gl_FragCoord_1;
    let _e45: vec4<f32> = params.data[0];
    let _e53: vec4<f32> = params.data[1];
    roughnessUv = ((1.5f * (vec2<f32>(_e29.x, (_e34.y - _e37.y)) - (0.5f * _e45.xy))) / vec2(_e53.x));
    let _e58: vec2<f32> = roughnessUv;
    let _e63: f32 = roughness((_e58 + vec2<f32>(1f, 0f)));
    let _e64: vec2<f32> = roughnessUv;
    let _e69: f32 = roughness((_e64 - vec2<f32>(1f, 0f)));
    roughness_1 = (_e63 - _e69);
    let _e72: vec2<f32> = patternUV;
    let _e78: vec4<f32> = params.data[43];
    let _e85: vec4<f32> = params.data[47];
    crumplesUV = (fract((((_e72 * 0.02f) / vec2(_e78.x)) - vec2(_e85.x))) * 32f);
    let _e96: vec4<f32> = params.data[42];
    let _e98: vec2<f32> = crumplesUV;
    let _e103: f32 = crumplesShape((_e98 + vec2<f32>(0.05f, 0f)));
    let _e104: vec2<f32> = crumplesUV;
    let _e105: f32 = crumplesShape(_e104);
    crumples = (_e96.x * (_e103 - _e105));
    let _e113: vec4<f32> = params.data[41];
    let _e116: vec2<f32> = patternUV;
    fiberUV = ((2f / _e113.x) * _e116);
    let _e119: vec2<f32> = fiberUV;
    let _e122: f32 = fiberNoise(_e119, vec2(0f));
    fiber = _e122;
    let _e128: vec4<f32> = params.data[40];
    let _e131: f32 = fiber;
    fiber = ((0.5f * _e128.x) * (_e131 - 1f));
    let _e141: vec2<f32> = patternUV;
    foldsUV = (_e141 * 0.12f);
    let _e145: vec2<f32> = foldsUV;
    let _e150: vec4<f32> = params.data[47];
    let _e153: vec2<f32> = rotate(_e145, (4f * _e150.x));
    foldsUV = _e153;
    let _e154: vec2<f32> = foldsUV;
    let _e155: vec2<f32> = folds(_e154);
    w_2 = _e155;
    let _e157: vec2<f32> = foldsUV;
    let _e162: vec4<f32> = params.data[47];
    let _e172: vec4<f32> = params.data[47];
    let _e176: vec2<f32> = rotate((_e157 + vec2((0.007f * cos(_e162.x)))), (0.01f * sin(_e172.x)));
    foldsUV = _e176;
    let _e177: vec2<f32> = foldsUV;
    let _e178: vec2<f32> = folds(_e177);
    w2_ = _e178;
    let _e183: vec4<f32> = params.data[46];
    let _e185: vec2<f32> = patternUV;
    let _e188: f32 = drops((_e185 * 2f));
    drops_1 = (_e183.x * _e188);
    let _e194: vec4<f32> = params.data[48];
    let _e197: vec2<f32> = patternUV;
    let _e203: vec4<f32> = params.data[47];
    let _e208: f32 = fbm(((0.17f * _e197) + vec2((10f * _e203.x))));
    fade = (_e194.x * _e208);
    let _e212: f32 = fade;
    let _e214: f32 = fade;
    let _e216: f32 = fade;
    fade = clamp((((8f * _e212) * _e214) * _e216), 0f, 1f);
    let _e221: vec2<f32> = w_2;
    let _e224: f32 = fade;
    w_2 = mix(_e221, vec2(0f), vec2(_e224));
    let _e227: vec2<f32> = w2_;
    let _e230: f32 = fade;
    w2_ = mix(_e227, vec2(0f), vec2(_e230));
    let _e233: f32 = crumples;
    let _e235: f32 = fade;
    crumples = mix(_e233, 0f, _e235);
    let _e237: f32 = drops_1;
    let _e239: f32 = fade;
    drops_1 = mix(_e237, 0f, _e239);
    let _e241: f32 = fiber;
    let _e244: f32 = fade;
    fiber = (_e241 * mix(1f, 0.5f, _e244));
    let _e247: f32 = roughness_1;
    let _e250: f32 = fade;
    roughness_1 = (_e247 * mix(1f, 0.5f, _e250));
    let _e253: vec2<f32> = normal;
    let _e255: vec2<f32> = normal;
    let _e260: vec4<f32> = params.data[44];
    let _e266: vec4<f32> = params.data[38];
    let _e276: vec2<f32> = w_2;
    let _e277: vec2<f32> = w2_;
    let _e281: vec2<f32> = (_e255.xy + (((_e260.x * min((5f * _e266.x), 1f)) * 4f) * max(vec2(0f), (_e276 + _e277))));
    normal.x = _e281.x;
    normal.y = _e281.y;
    let _e286: vec2<f32> = normalImage;
    let _e288: vec2<f32> = normalImage;
    let _e293: vec4<f32> = params.data[44];
    let _e297: vec2<f32> = w_2;
    let _e299: vec2<f32> = (_e288.xy + ((_e293.x * 2f) * _e297));
    normalImage.x = _e299.x;
    normalImage.y = _e299.y;
    let _e304: vec2<f32> = normal;
    let _e306: vec2<f32> = normal;
    let _e308: f32 = crumples;
    let _e310: vec2<f32> = (_e306.xy + vec2(_e308));
    normal.x = _e310.x;
    normal.y = _e310.y;
    let _e315: vec2<f32> = normalImage;
    let _e317: vec2<f32> = normalImage;
    let _e320: f32 = crumples;
    let _e323: vec2<f32> = (_e317.xy + vec2((1.5f * _e320)));
    normalImage.x = _e323.x;
    normalImage.y = _e323.y;
    let _e328: vec2<f32> = normal;
    let _e330: vec2<f32> = normal;
    let _e333: f32 = drops_1;
    let _e336: vec2<f32> = (_e330.xy + vec2((3f * _e333)));
    normal.x = _e336.x;
    normal.y = _e336.y;
    let _e341: vec2<f32> = normalImage;
    let _e343: vec2<f32> = normalImage;
    let _e346: f32 = drops_1;
    let _e349: vec2<f32> = (_e343.xy + vec2((0.2f * _e346)));
    normalImage.x = _e349.x;
    normalImage.y = _e349.y;
    let _e354: vec2<f32> = normal;
    let _e356: vec2<f32> = normal;
    let _e361: vec4<f32> = params.data[39];
    let _e365: f32 = roughness_1;
    let _e368: vec2<f32> = (_e356.xy + vec2(((_e361.x * 1.5f) * _e365)));
    normal.x = _e368.x;
    normal.y = _e368.y;
    let _e373: vec2<f32> = normal;
    let _e375: vec2<f32> = normal;
    let _e377: f32 = fiber;
    let _e379: vec2<f32> = (_e375.xy + vec2(_e377));
    normal.x = _e379.x;
    normal.y = _e379.y;
    let _e384: vec2<f32> = normalImage;
    let _e388: vec4<f32> = params.data[39];
    let _e392: f32 = roughness_1;
    normalImage = (_e384 + vec2(((_e388.x * 0.75f) * _e392)));
    let _e396: vec2<f32> = normalImage;
    let _e398: f32 = fiber;
    normalImage = (_e396 + vec2((0.2f * _e398)));
    let _e407: vec2<f32> = normal;
    let _e413: vec4<f32> = params.data[38];
    let _e423: vec3<f32> = lightPos;
    res = dot(normalize(vec3<f32>(_e407.x, _e407.y, (9.5f - (9f * pow(_e413.x, 0.1f))))), normalize(_e423));
    let _e430: vec4<f32> = params.data[11];
    let _e435: vec4<f32> = params.data[11];
    fgColor = (_e430.xyz * _e435.w);
    let _e442: vec4<f32> = params.data[11];
    fgOpacity = _e442.w;
    let _e448: vec4<f32> = params.data[12];
    let _e453: vec4<f32> = params.data[12];
    bgColor = (_e448.xyz * _e453.w);
    let _e460: vec4<f32> = params.data[12];
    bgOpacity = _e460.w;
    let _e463: vec2<f32> = imageUV;
    let _e465: vec2<f32> = normalImage;
    imageUV = (_e463 + (0.02f * _e465));
    let _e468: vec2<f32> = imageUV;
    let _e469: f32 = getUvFrame(_e468);
    frame = _e469;
    let _e471: vec2<f32> = imageUV;
    let _e472: vec4<f32> = textureSample(cover_texture, cover_sampler, _e471);
    image = _e472;
    let _e474: vec4<f32> = image;
    let _e476: vec4<f32> = image;
    let _e482: vec4<f32> = params.data[38];
    let _e487: f32 = res;
    let _e492: vec3<f32> = (_e476.xyz + vec3(((0.6f * pow(_e482.x, 0.4f)) * (_e487 - 0.7f))));
    image.x = _e492.x;
    image.y = _e492.y;
    image.z = _e492.z;
    let _e499: f32 = frame;
    let _e500: vec4<f32> = image;
    frame = (_e499 * _e500.w);
    let _e503: vec3<f32> = fgColor;
    let _e504: f32 = res;
    color = (_e503 * _e504);
    let _e507: f32 = fgOpacity;
    let _e508: f32 = res;
    opacity = (_e507 * _e508);
    let _e511: vec3<f32> = color;
    let _e512: vec3<f32> = bgColor;
    let _e514: f32 = opacity;
    color = (_e511 + (_e512 * (1f - _e514)));
    let _e518: f32 = opacity;
    let _e519: f32 = bgOpacity;
    let _e521: f32 = opacity;
    opacity = (_e518 + (_e519 * (1f - _e521)));
    let _e525: f32 = opacity;
    let _e527: f32 = frame;
    opacity = mix(_e525, 1f, _e527);
    let _e529: vec3<f32> = color;
    let _e531: f32 = drops_1;
    color = (_e529 - vec3((0.007f * _e531)));
    let _e535: vec3<f32> = color;
    let _e537: vec3<f32> = color;
    let _e538: vec4<f32> = image;
    let _e540: f32 = frame;
    let _e542: vec3<f32> = mix(_e537, _e538.xyz, vec3(_e540));
    color.x = _e542.x;
    color.y = _e542.y;
    color.z = _e542.z;
    let _e549: vec3<f32> = color;
    let _e550: f32 = opacity;
    fragColor = vec4<f32>(_e549.x, _e549.y, _e549.z, _e550);
    return;
}

@fragment 
fn main(@location(0) v_imageUV: vec2<f32>, @builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    v_imageUV_1 = v_imageUV;
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e20: vec4<f32> = fragColor;
    let paper = _e20;
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
