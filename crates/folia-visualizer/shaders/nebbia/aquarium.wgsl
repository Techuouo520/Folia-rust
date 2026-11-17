// Aquarium, jellyfish and bubble equations from the original nebbiaAquaGlsl/Spot/Jelly/Bubble modules.

fn aquarium_soft_box(d_2: vec2<f32>, size: vec2<f32>, soft: f32) -> f32 {
    var d_3: vec2<f32>;
    var size_1: vec2<f32>;
    var soft_1: f32;
    var q: vec2<f32>;
    var sd: f32;
    var k: f32;

    d_3 = d_2;
    size_1 = size;
    soft_1 = soft;
    q = (abs(d_3) - size_1);
    sd = (length(max(q, vec2(0f))) + min(max(q.x, q.y), 0f));
    k = min(size_1.x, size_1.y);
    return (((1f - smoothstep((-(soft_1) * 0.5f), soft_1, sd)) * k) / (k + (soft_1 * 0.6f)));
}

fn aquarium_spot_beam(q_3: vec2<f32>, b_6: f32) -> f32 {
    var q_4: vec2<f32>;
    var b_7: f32;

    q_4 = q_3;
    b_7 = b_6;
    return ((beam_from(vec2<f32>(u.spot_src.x, -0.04f), q_4, b_7)) + (beam_from(vec2<f32>(u.spot_src.y, -0.04f), q_4, b_7)));
}

fn aquarium_jellies(q_5: vec2<f32>, b_8: f32) -> vec3<f32> {
    var q_6: vec2<f32>;
    var b_9: f32;
    var c: vec3<f32> = vec3(0f);
    var sharp: f32;
    var fine: bool;
    var i_1: i32 = 0i;
    var J: vec4<f32>;
    var size_2: f32;
    var pulse: f32;
    var d_5: vec2<f32>;
    var jc: vec3<f32>;
    var ph: f32;
    var wx: f32;
    var hy: f32;
    var p_4: vec2<f32>;
    var soft_2: f32;
    var edge: f32;
    var r_4: f32;
    var fill: f32;
    var rr: f32;
    var rim: f32;
    var pc: vec2<f32>;
    var lobe: f32;
    var gon: f32;
    var we: f32;
    var w_1: f32;
    var k_2: f32;
    var len: f32;
    var s: f32;
    var tx: f32;
    var dx: f32;
    var s_1: f32;
    var wa: f32;
    var wae: f32;
    var dx_1: f32;

    q_6 = q_5;
    b_9 = b_8;
    sharp = (1f - clamp((b_9 * 25f), 0f, 1f));
    fine = (b_9 < 0.004f);
    loop {
        if !((i_1 < 24i)) {
            break;
        }
        {
            if (i_1 >= i32(u.aqua.x)) {
                break;
            }
            J = (u.jelly[i_1]);
            size_2 = J.z;
            pulse = J.w;
            d_5 = (q_6 - J.xy);
            if (((abs(d_5.x) > ((size_2 * 2.2f) + (b_9 * 3f))) || (d_5.y < ((-(size_2) * 1.6f) - (b_9 * 3f)))) || (d_5.y > ((size_2 * 5f) + (b_9 * 3f)))) {
                continue;
            }
            jc = ((u.jelly_c[i_1]).xyz * u.neon);
            ph = (u.jelly_c[i_1]).w;
            wx = (size_2 * (1f - (0.18f * pulse)));
            hy = ((size_2 * 0.72f) * (1f + (0.14f * pulse)));
            p_4 = vec2<f32>((d_5.x / wx), (d_5.y / hy));
            soft_2 = ((b_9 / size_2) + 0.04f);
            edge = ((0.05f + ((0.3f * p_4.x) * p_4.x)) + ((0.04f * cos(((p_4.x * 14f) + (ph * 0.5f)))) * sharp));
            r_4 = max(length(vec2<f32>(p_4.x, min(p_4.y, 0f))), (((p_4.y - edge) / (0.3f + soft_2)) + 1f));
            fill = smoothstep((1f + soft_2), (1f - soft_2), r_4);
            rr = ((r_4 - 0.9f) / (0.09f + soft_2));
            rim = exp((-(rr) * rr));
            pc = (p_4 + vec2<f32>(0f, 0.4f));
            lobe = ((length(pc) - 0.26f) - (0.07f * cos((4f * atan2(pc.y, pc.x)))));
            gon = (exp(((-(lobe) * lobe) / (0.004f + (soft_2 * soft_2)))) * (0.4f + (0.6f * sharp)));
            c = (c + (jc * ((fill * ((0.16f + (0.55f * rim)) + (0.45f * gon))) + ((glow_pt((d_5 + vec2<f32>(0f, (size_2 * 0.25f))), (size_2 * 0.9f), b_9, 0f)) * 0.18f))));
            if (d_5.y <= 0f) {
                continue;
            }
            we = sqrt((pow((wx * 0.55f), 2f) + (b_9 * b_9)));
            c = (c + ((((jc * exp(((-(d_5.x) * d_5.x) / (we * we)))) * pow(max((1f - (d_5.y / (size_2 * 3.5f))), 0f), 2f)) * 0.12f) * smoothstep(0f, (size_2 * 0.3f), d_5.y)));
            if !(fine) {
                continue;
            }
            w_1 = (0.0012f + (size_2 * 0.012f));
            we = sqrt(((w_1 * w_1) + (b_9 * b_9)));
            k_2 = 0f;
            loop {
                if !((k_2 < 6f)) {
                    break;
                }
                {
                    len = (size_2 * (2.6f + (1.6f * (h1(((f32(i_1) * 13f) + k_2))))));
                    s = (d_5.y / len);
                    if (s > 1.05f) {
                        continue;
                    }
                    tx = (((((k_2 / 5f) - 0.5f) * 1.5f) * wx) * (1f - (0.35f * min(s, 1f))));
                    dx = ((d_5.x - tx) - (((sin(((((d_5.y / size_2) * 2.6f) - (ph * 0.9f)) + (k_2 * 1.3f))) * size_2) * 0.28f) * min(s, 1f)));
                    c = (c + (((((jc * (w_1 / we)) * exp(((-(dx) * dx) / (we * we)))) * pow(max((1f - s), 0f), 1.5f)) * smoothstep((hy * 0.1f), (hy * 0.35f), d_5.y)) * 0.8f));
                }
                continuing {
                    k_2 = (k_2 + 1f);
                }
            }
            s_1 = (d_5.y / (size_2 * 1.7f));
            if (s_1 < 1f) {
                {
                    wa = ((size_2 * 0.14f) * (1f - (s_1 * 0.6f)));
                    wae = sqrt(((wa * wa) + (b_9 * b_9)));
                    dx_1 = (d_5.x - (((sin(((((d_5.y / size_2) * 3.5f) - ph) + 1f)) * size_2) * 0.12f) * s_1));
                    c = (c + (((((jc * (wa / wae)) * exp(((-(dx_1) * dx_1) / (wae * wae)))) * (1f - s_1)) * 0.35f) * smoothstep(0f, 0.01f, d_5.y)));
                }
            }
        }
        continuing {
            i_1 = (i_1 + 1i);
        }
    }
    return c;
}

fn aquarium_bubble(d_6: vec2<f32>, rad: f32, b_10: f32) -> vec4<f32> {
    var d_7: vec2<f32>;
    var rad_1: f32;
    var b_11: f32;
    var l: f32;
    var aa: f32;
    var keep: f32;
    var disc: f32;
    var up: f32;
    var fres: f32;
    var spec: f32;
    var spec2_: f32;
    var caus: f32;
    var big: vec4<f32>;
    var tiny: vec4<f32>;

    d_7 = d_6;
    rad_1 = rad;
    b_11 = b_10;
    l = length(d_7);
    aa = max(b_11, 0.0005f);
    keep = ((rad_1 * rad_1) / ((rad_1 * rad_1) + ((b_11 * b_11) * 4f)));
    disc = smoothstep((rad_1 + aa), (rad_1 - aa), l);
    up = (0.5f + (0.5f * clamp((-(d_7.y) / max(l, 0.00001f)), -1f, 1f)));
    fres = ((pow(clamp((l / rad_1), 0f, 1f), 5f) * disc) * (0.35f + (0.65f * up)));
    spec = (glow_pt((d_7 + vec2<f32>((rad_1 * 0.36f), (rad_1 * 0.4f))), (rad_1 * 0.15f), b_11, 0f));
    spec2_ = ((glow_pt((d_7 - vec2<f32>((rad_1 * 0.32f), (rad_1 * 0.42f))), (rad_1 * 0.1f), b_11, 0f)) * 0.3f);
    caus = ((glow_pt((d_7 - vec2<f32>(0f, (rad_1 * 0.5f))), (rad_1 * 0.32f), b_11, 0f)) * 0.14f);
    let rim_light: vec3<f32> = vec3((((((fres * 0.9f) * keep) + (spec * 1.3f)) + spec2_) + caus));
    big = vec4<f32>(rim_light.x, rim_light.y, rim_light.z, ((disc * 0.14f) * keep));
    let small_light: vec3<f32> = vec3((((glow_pt(d_7, (rad_1 * 0.6f), b_11, 0f)) * 1.3f) + (spec * 1.2f)));
    tiny = vec4<f32>(small_light.x, small_light.y, small_light.z, 0f);
    return mix(tiny, big, vec4(smoothstep(0.0018f, 0.0034f, rad_1)));
}

fn aquarium_bubbles(col: vec3<f32>, q_7: vec2<f32>, b_12: f32, bc: vec3<f32>) -> vec3<f32> {
    var col_1: vec3<f32>;
    var q_8: vec2<f32>;
    var b_13: f32;
    var bc_1: vec3<f32>;
    var t: f32;
    var local_2: f32;
    var drift: f32;
    var fade: f32;
    var qx: f32;
    var cw: f32 = 0.055f;
    var cx: f32;
    var k_3: f32 = -1f;
    var n_2: f32;
    var src_2: f32;
    var sp: f32;
    var ch: f32;
    var rise: f32;
    var sw: f32;
    var swe: f32;
    var sx: f32;
    var v: f32;
    var iy: f32;
    var j: f32;
    var m: f32;
    var hb: f32;
    var y: f32;
    var up_1: f32;
    var rad_2: f32;
    var c_1: vec2<f32>;
    var bub: vec4<f32>;
    var bw: f32 = 0.13f;
    var bx: f32;
    var k_4: f32 = -1f;
    var n_3: f32;
    var sp_1: f32;
    var ch_1: f32;
    var iy_1: f32;
    var j_1: f32;
    var m_1: f32;
    var rad_3: f32;
    var c_2: vec2<f32>;
    var d_8: vec2<f32>;
    var bubble_phase: vec2<f32>;
    var c2_: f32;
    var s2_1: f32;
    var c3_: f32;
    var s3_: f32;
    var f1_: f32;
    var f2_: f32;
    var shape: f32;
    var bub_1: vec4<f32>;
    var i_2: i32 = 0i;
    var B: vec4<f32>;
    var a: f32;
    var y0_: f32;
    var grow: f32;
    var k_5: f32;
    var h_2: f32;
    var h2_: f32;
    var rad_4: f32;
    var c_3: vec2<f32>;
    var d_9: vec2<f32>;
    var bub_2: vec4<f32>;

    col_1 = col;
    q_8 = q_7;
    b_13 = b_12;
    bc_1 = bc;
    t = u.time;
    if (scenery_on(32u)) {
        local_2 = 0.1f;
    } else {
        local_2 = 0f;
    }
    drift = local_2;
    fade = smoothstep(0.93f, 0.8f, q_8.y);
    qx = (q_8.x + ((0.92f - q_8.y) * drift));
    if (scenery_on(16u)) {
        {
            cx = floor((qx / cw));
            loop {
                if !((k_3 <= 1f)) {
                    break;
                }
                {
                    n_2 = (cx + k_3);
                    if ((h1(((n_2 * 2.3f) + 1f))) > (u.aqua.y * 0.9f)) {
                        continue;
                    }
                    src_2 = (((n_2 + 0.5f) * cw) + ((((h1((n_2 + 3f))) - 0.5f) * cw) * 0.5f));
                    sp = (0.14f + (0.1f * (h1((n_2 + 7f)))));
                    ch = (0.014f + (0.01f * (h1((n_2 + 5f)))));
                    rise = clamp(((0.92f - q_8.y) / 0.9f), 0f, 1f);
                    sw = (0.003f + (0.016f * rise));
                    swe = sqrt(((sw * sw) + (b_13 * b_13)));
                    sx = (qx - src_2);
                    col_1 = (col_1 + ((((bc_1 * exp(((-(sx) * sx) / (swe * swe)))) * 0.03f) * fade) * ((sw / swe) + 0.3f)));
                    if (b_13 > 0.02f) {
                        continue;
                    }
                    v = (q_8.y + (t * sp));
                    iy = floor((v / ch));
                    j = -1f;
                    loop {
                        if !((j <= 1f)) {
                            break;
                        }
                        {
                            m = (iy + j);
                            hb = (h1(((m * 1.31f) + (n_2 * 17f))));
                            if (hb > 0.85f) {
                                continue;
                            }
                            y = ((((m + 0.2f) + (0.6f * (h1(((m * 2.7f) + n_2))))) * ch) - (t * sp));
                            up_1 = clamp(((0.92f - y) / 0.9f), 0f, 1f);
                            rad_2 = ((0.0012f + (((0.003f * hb) * hb) * hb)) + (0.0012f * up_1));
                            c_1 = vec2<f32>(((src_2 + (sin(((m * 2.1f) + (t * 6f))) * (0.0015f + (0.006f * up_1)))) + (((h1((m + n_2))) - 0.5f) * (0.006f + (0.03f * up_1)))), y);
                            bub = (aquarium_bubble((vec2<f32>(qx, q_8.y) - c_1), rad_2, b_13));
                            col_1 = ((col_1 * (1f - (bub.w * fade))) + (((bc_1 * bub.xyz) * fade) * (0.6f + (0.4f * (h1(((m * 5f) + n_2)))))));
                        }
                        continuing {
                            j = (j + 1f);
                        }
                    }
                }
                continuing {
                    k_3 = (k_3 + 1f);
                }
            }
        }
    }
    if ((scenery_on(4u)) && (u.aqua.w < 0.5f)) {
        {
            bx = floor((qx / bw));
            loop {
                if !((k_4 <= 1f)) {
                    break;
                }
                {
                    n_3 = (bx + k_4);
                    sp_1 = (0.07f + (0.04f * (h1((n_3 + 41f)))));
                    ch_1 = 0.42f;
                    iy_1 = floor(((q_8.y + (t * sp_1)) / ch_1));
                    j_1 = -1f;
                    loop {
                        if !((j_1 <= 0f)) {
                            break;
                        }
                        {
                            m_1 = (iy_1 + j_1);
                            if ((h1(((m_1 * 3.7f) + (n_3 * 11f)))) > min((1.1f * u.aqua.y), 0.85f)) {
                                continue;
                            }
                            rad_3 = (0.008f + (0.016f * pow((h1((m_1 + (n_3 * 5f)))), 1.5f)));
                            c_2 = vec2<f32>(((((n_3 + 0.2f) + (0.6f * (h1(((m_1 * 7f) + n_3))))) * bw) + (sin(((t * 1.3f) + m_1)) * 0.02f)), (((m_1 + 0.5f) * ch_1) - (t * sp_1)));
                            d_8 = (vec2<f32>(qx, q_8.y) - c_2);
                            bubble_phase = (d_8 / vec2(max(length(d_8), 0.00001f)));
                            c2_ = ((bubble_phase.x * bubble_phase.x) - (bubble_phase.y * bubble_phase.y));
                            s2_1 = ((2f * bubble_phase.x) * bubble_phase.y);
                            c3_ = (bubble_phase.x * (((4f * bubble_phase.x) * bubble_phase.x) - 3f));
                            s3_ = (bubble_phase.y * (3f - ((4f * bubble_phase.y) * bubble_phase.y)));
                            f1_ = ((t * 4f) + m_1);
                            f2_ = (t * 3f);
                            shape = ((1f + (0.03f * ((s3_ * cos(f1_)) + (c3_ * sin(f1_))))) + (0.02f * ((s2_1 * cos(f2_)) - (c2_ * sin(f2_)))));
                            bub_1 = (aquarium_bubble((vec2<f32>(d_8.x, (d_8.y * 1.18f)) / vec2(shape)), rad_3, b_13));
                            col_1 = ((col_1 * (1f - (bub_1.w * fade))) + ((bc_1 * (bub_1.xyz + vec3(((glow_pt(d_8, (rad_3 * 1.4f), b_13, 0f)) * 0.04f)))) * fade));
                        }
                        continuing {
                            j_1 = (j_1 + 1f);
                        }
                    }
                }
                continuing {
                    k_4 = (k_4 + 1f);
                }
            }
        }
    }
    loop {
        if !((i_2 < 12i)) {
            break;
        }
        {
            if (i_2 >= i32(u.aqua.z)) {
                break;
            }
            B = (u.bursts[i_2]);
            a = (t - B.y);
            if ((B.z <= 0f) || (a < 0f)) {
                continue;
            }
            y0_ = ((0.9f - (0.2f * a)) - (0.28f * (1f - exp((-(a) * 3f)))));
            if (abs((q_8.y - y0_)) > (0.08f + (b_13 * 3f))) {
                continue;
            }
            grow = (smoothstep(0f, 0.15f, a) * (0.55f + (0.45f * B.z)));
            k_5 = 0f;
            loop {
                if !((k_5 < 6f)) {
                    break;
                }
                {
                    h_2 = (h1((B.w + (k_5 * 7.3f))));
                    h2_ = (h1(((B.w * 1.7f) + (k_5 * 3.1f))));
                    rad_4 = ((0.008f + (0.012f * h_2)) * grow);
                    c_3 = vec2<f32>((((B.x + ((k_5 - 2.5f) * 0.05f)) + ((h2_ - 0.5f) * 0.02f)) + ((sin(((a * 3f) + (k_5 * 1.7f))) * 0.004f) * a)), (y0_ + ((h_2 - 0.5f) * 0.04f)));
                    d_9 = (vec2<f32>(qx, q_8.y) - c_3);
                    bub_2 = (aquarium_bubble(vec2<f32>(d_9.x, (d_9.y * 1.18f)), rad_4, b_13));
                    col_1 = ((col_1 * (1f - (bub_2.w * fade))) + ((bc_1 * (bub_2.xyz + vec3(((glow_pt(d_9, ((rad_4 * 2f) + 0.004f), b_13, 0f)) * 0.22f)))) * fade));
                }
                continuing {
                    k_5 = (k_5 + 1f);
                }
            }
        }
        continuing {
            i_2 = (i_2 + 1i);
        }
    }
    return col_1;
}

fn aquarium_weed_offset(s_2: f32) -> f32 {
    var s_3: f32;
    var f_1: f32;
    var local_3: f32;
    var local_4: f32;

    s_3 = s_2;
    f_1 = (clamp(s_3, 0f, 1f) * 3f);
    if (f_1 < 1f) {
        local_4 = mix(u.sway.x, u.sway.y, f_1);
    } else {
        if (f_1 < 2f) {
            local_3 = mix(u.sway.y, u.sway.z, (f_1 - 1f));
        } else {
            local_3 = mix(u.sway.z, u.sway.w, (f_1 - 2f));
        }
        local_4 = local_3;
    }
    return local_4;
}

fn aquarium_scene(q_9: vec2<f32>, b_14: f32) -> vec3<f32> {
    var q_10: vec2<f32>;
    var b_15: f32;
    var t_1: f32;
    var sharp_1: f32;
    var deep: vec3<f32>;
    var col_2: vec3<f32>;
    var lightC: vec3<f32>;
    var rx: f32;
    var rays: f32;
    var floorY: f32;
    var caus_1: f32;
    var rockY: f32;
    var under: f32;
    var tex: f32;
    var rock: vec3<f32>;
    var rc: f32;
    var rw: f32 = 0.17f;
    var rcx: f32;
    var k_6: f32 = -1f;
    var n_4: f32;
    var h_3: f32;
    var rx_1: f32;
    var ry: f32;
    var c_4: vec2<f32>;
    var d_10: vec2<f32>;
    var edge_1: f32;
    var e: f32;
    var soft_3: f32;
    var cover: f32;
    var nrm: vec3<f32>;
    var lit: f32;
    var stone: vec3<f32>;
    var weeds: bool;
    var glow: bool;
    var cw_1: f32 = 0.06f;
    var cx_1: f32;
    var k_7: f32 = -1f;
    var n_5: f32;
    var h_4: f32;
    var top: f32;
    var base: f32;
    var len_1: f32;
    var sway: f32;
    var bend: f32;
    var s_4: f32;
    var x0_: f32;
    var w_2: f32;
    var we_1: f32;
    var dx_2: f32;
    var a_1: f32;
    var s_5: f32;
    var N: f32;
    var gap: f32;
    var xs: f32;
    var dx_3: f32;
    var fj: f32;
    var ja: f32;
    var pc_1: vec3<f32>;
    var line: f32;
    var r3_: f32;
    var local_5: f32;
    var local_6: f32;
    var r_5: f32;
    var local_7: f32;
    var local_8: f32;
    var wgt: f32;
    var s2_2: f32;
    var e_1: f32;
    var j0_: f32;
    var dj: f32;
    var j_2: f32;
    var sj: f32;
    var py: f32;
    var pp: vec2<f32>;
    var tw: f32;
    var pr: f32;
    var pc_2: vec3<f32>;
    var d_11: vec2<f32>;

    q_10 = q_9;
    b_15 = b_14;
    t_1 = u.time;
    sharp_1 = (1f - clamp((b_15 * 25f), 0f, 1f));
    deep = (u.sky0.xyz * 0.55f);
    col_2 = mix(((mix(u.sky1.xyz, u.la.xyz, vec3(0.3f)) * 1.25f) + vec3(0.02f)), deep, vec3(smoothstep(0f, 0.95f, q_10.y)));
    lightC = mix(u.la.xyz, vec3(1f), vec3(0.45f));
    rx = (q_10.x + (q_10.y * 0.35f));
    rays = (pow((0.5f + (0.5f * sin((((rx * 9f) + (t_1 * 0.3f)) + (sin(((rx * 3.1f) - (t_1 * 0.2f))) * 1.5f))))), 6f) + (0.6f * pow((0.5f + (0.5f * sin(((rx * 17f) - (t_1 * 0.45f))))), 8f)));
    col_2 = (col_2 + (((lightC * mix(0.25f, rays, ((sharp_1 * 0.6f) + 0.4f))) * (1f - smoothstep(0f, 0.85f, q_10.y))) * 0.13f));
    col_2 = (col_2 + (((lightC * 0.8f) * exp((-(q_10.y) * 14f))) * (0.25f + ((0.15f * sin((((q_10.x * 30f) + (t_1 * 1.3f)) + sin((q_10.x * 7f))))) * sharp_1))));
    if (scenery_on(8u)) {
        {
            floorY = (0.89f + (0.015f * sin((q_10.x * 3f))));
            caus_1 = pow(abs((sin((((q_10.x * 23f) + (sin(((q_10.y * 40f) + t_1)) * 1.2f)) + (t_1 * 0.6f))) * sin((((q_10.x * 17f) - (t_1 * 0.5f)) + (q_10.y * 30f))))), 3f);
            col_2 = mix(col_2, (((deep * 1.3f) + vec3(0.03f)) + (((lightC * caus_1) * 0.12f) * sharp_1)), vec3(smoothstep(((floorY - 0.01f) - b_15), ((floorY + 0.01f) + b_15), q_10.y)));
        }
    }
    if (scenery_on(1u)) {
        {
            rockY = ((0.8f + (0.07f * (vnoise(vec2<f32>((q_10.x * 3.5f), 1f))))) + (0.02f * (vnoise(vec2<f32>((q_10.x * 14f), 3f)))));
            under = max((q_10.y - rockY), 0f);
            tex = (0.75f + (0.5f * mix(0.5f, (((vnoise((q_10 * vec2<f32>(26f, 34f)))) * 0.65f) + ((vnoise((q_10 * vec2<f32>(70f, 90f)))) * 0.35f)), sharp_1)));
            rock = (mix(((deep * 1.6f) + (lightC * 0.07f)), (deep * 0.75f), vec3(smoothstep(0f, 0.09f, under))) * tex);
            rc = pow(abs((sin((((q_10.x * 31f) + (t_1 * 0.5f)) + sin(((q_10.y * 50f) - (t_1 * 0.7f))))) * sin((((q_10.x * 19f) - (t_1 * 0.4f)) + (q_10.y * 37f))))), 4f);
            rock = (rock + (lightC * ((0.16f * exp((-(under) * 70f))) + (((rc * 0.07f) * sharp_1) * (1f - smoothstep(0f, 0.05f, under))))));
            col_2 = mix(col_2, rock, vec3((smoothstep(((rockY - b_15) - 0.003f), ((rockY + b_15) + 0.003f), q_10.y) * 0.95f)));
            rcx = floor((q_10.x / rw));
            loop {
                if !((k_6 <= 1f)) {
                    break;
                }
                {
                    n_4 = (rcx + k_6);
                    h_3 = (h1(((n_4 * 4.1f) + 2f)));
                    if (h_3 > 0.7f) {
                        continue;
                    }
                    rx_1 = (0.06f + (0.05f * (h1((n_4 + 9f)))));
                    ry = (rx_1 * (0.55f + (0.3f * (h1((n_4 * 2.7f))))));
                    c_4 = vec2<f32>((((n_4 + 0.3f) + (0.4f * (h1((n_4 * 5.3f))))) * rw), 0.95f);
                    d_10 = ((q_10 - c_4) / vec2<f32>(rx_1, ry));
                    edge_1 = (0.12f * ((vnoise(vec2<f32>((atan2(d_10.y, d_10.x) * 2.5f), (n_4 * 3f)))) - 0.5f));
                    e = (length(d_10) + edge_1);
                    soft_3 = ((b_15 + 0.003f) / ry);
                    cover = smoothstep((1f + soft_3), (1f - soft_3), e);
                    if (cover <= 0f) {
                        continue;
                    }
                    nrm = normalize(vec3<f32>(((d_10 * 0.9f)).x, ((d_10 * 0.9f)).y, sqrt(max((1f - dot(d_10, d_10)), 0.04f))));
                    lit = max(dot(nrm, normalize(vec3<f32>(-0.35f, -1f, 0.7f))), 0f);
                    stone = (((((deep * 1.8f) + (lightC * 0.08f)) * (0.4f + (1f * lit))) * tex) + (((lightC * pow(lit, 6f)) * 0.16f) * sharp_1));
                    col_2 = mix(col_2, stone, vec3((cover * 0.97f)));
                }
                continuing {
                    k_6 = (k_6 + 1f);
                }
            }
        }
    }
    weeds = (scenery_on(1u));
    glow = (scenery_on(2u));
    if (weeds || glow) {
        {
            cx_1 = floor((q_10.x / cw_1));
            loop {
                if !((k_7 <= 1f)) {
                    break;
                }
                {
                    n_5 = (cx_1 + k_7);
                    h_4 = (h1(((n_5 * 1.7f) + 5f)));
                    if (h_4 > 0.45f) {
                        continue;
                    }
                    top = (0.55f + (0.25f * (h1((n_5 + 2f)))));
                    base = 0.92f;
                    len_1 = (base - top);
                    sway = ((t_1 * (3.6f + (1.8f * h_4))) + (h_4 * 20f));
                    bend = (0.07f * (0.7f + (0.6f * h_4)));
                    if weeds {
                        {
                            s_4 = clamp(((base - q_10.y) / len_1), 0f, 1f);
                            x0_ = ((((n_5 + 0.5f) * cw_1) + (((sin(((q_10.y * 11f) + sway)) * 0.025f) * u.aqua_motion.x) * s_4)) + ((((aquarium_weed_offset(s_4)) * bend) * s_4) * s_4));
                            w_2 = (0.003f * (1f - (s_4 * 0.6f)));
                            we_1 = sqrt(((w_2 * w_2) + (b_15 * b_15)));
                            dx_2 = (q_10.x - x0_);
                            a_1 = (((((w_2 / we_1) * exp(((-(dx_2) * dx_2) / (we_1 * we_1)))) * smoothstep((top - b_15), ((top + 0.02f) + b_15), q_10.y)) * step(q_10.y, base)) * 0.5f);
                            col_2 = mix(col_2, mix(deep, vec3<f32>(0.02f, 0.08f, 0.05f), vec3(0.6f)), vec3(a_1));
                        }
                    }
                    if (!(glow) || (q_10.y < ((top - 0.05f) - (b_15 * 3f)))) {
                        continue;
                    }
                    s_5 = clamp(((base - q_10.y) / len_1), 0f, 1f);
                    N = 11f;
                    gap = (len_1 / (N - 1f));
                    if (b_15 > 0.012f) {
                        {
                            xs = ((((n_5 + 0.5f) * cw_1) + (((sin(((q_10.y * 11f) + sway)) * 0.025f) * u.aqua_motion.x) * s_5)) + ((((aquarium_weed_offset(s_5)) * bend) * s_5) * s_5));
                            dx_3 = (q_10.x - xs);
                            fj = (s_5 * (N - 1f));
                            ja = floor(fj);
                            pc_1 = mix(mix((pick((h1(((n_5 * 3f) + ja))))), (pick((h1((((n_5 * 3f) + ja) + 1f))))), vec3((fj - ja))), vec3(1f), vec3(0.25f));
                            line = 0f;
                            r3_ = 0f;
                            loop {
                                if !((r3_ < 3f)) {
                                    break;
                                }
                                {
                                    if (r3_ < 0.5f) {
                                        local_6 = (0.0018f + (0.0018f * s_5));
                                    } else {
                                        if (r3_ < 1.5f) {
                                            local_5 = ((0.0018f + (0.0018f * s_5)) * 4f);
                                        } else {
                                            local_5 = (0.009f + (0.008f * s_5));
                                        }
                                        local_6 = local_5;
                                    }
                                    r_5 = local_6;
                                    if (r3_ < 0.5f) {
                                        local_8 = 1.6f;
                                    } else {
                                        if (r3_ < 1.5f) {
                                            local_7 = 0.25f;
                                        } else {
                                            local_7 = 0.4f;
                                        }
                                        local_8 = local_7;
                                    }
                                    wgt = local_8;
                                    s2_2 = ((r_5 * r_5) + (b_15 * b_15));
                                    line = (line + ((((wgt * ((r_5 * r_5) / s2_2)) * exp((((-(dx_3) * dx_3) / s2_2) * 1.6f))) * sqrt(((3.1416f * s2_2) / 1.6f))) / gap));
                                }
                                continuing {
                                    r3_ = (r3_ + 1f);
                                }
                            }
                            e_1 = (0.009f + b_15);
                            line = (line * (smoothstep((top - e_1), (top + e_1), q_10.y) * smoothstep((base + e_1), (base - e_1), q_10.y)));
                            col_2 = (col_2 + ((((pc_1 * line) * 0.7f) * (0.35f + (0.65f * s_5))) * u.neon));
                            continue;
                        }
                    }
                    j0_ = floor(((s_5 * (N - 1f)) + 0.5f));
                    dj = -2f;
                    loop {
                        if !((dj <= 2f)) {
                            break;
                        }
                        {
                            j_2 = (j0_ + dj);
                            sj = (j_2 / (N - 1f));
                            if ((j_2 < 0f) || (j_2 > (N - 1f))) {
                                continue;
                            }
                            py = (base - (sj * len_1));
                            pp = vec2<f32>(((((n_5 + 0.5f) * cw_1) + (((sin(((py * 11f) + sway)) * 0.025f) * u.aqua_motion.x) * sj)) + ((((aquarium_weed_offset(sj)) * bend) * sj) * sj)), py);
                            tw = (0.45f + (0.55f * pow((0.5f + (0.5f * sin((((t_1 * (2.5f + ((h1(((n_5 * 9f) + j_2))) * 3f))) + (j_2 * 1.7f)) + (h_4 * 30f))))), 2f)));
                            pr = (0.0018f + (0.0018f * sj));
                            pc_2 = mix((pick((h1(((n_5 * 3f) + j_2))))), vec3(1f), vec3(0.25f));
                            d_11 = (q_10 - pp);
                            col_2 = (col_2 + ((((pc_2 * ((((glow_pt(d_11, pr, b_15, 0f)) * 1.6f) + ((glow_pt(d_11, (pr * 4f), b_15, 0f)) * 0.25f)) + ((glow_pt(d_11, (0.009f + (0.008f * sj)), b_15, 0f)) * 0.4f))) * tw) * (0.35f + (0.65f * sj))) * u.neon));
                        }
                        continuing {
                            dj = (dj + 1f);
                        }
                    }
                }
                continuing {
                    k_7 = (k_7 + 1f);
                }
            }
        }
    }
    col_2 = (col_2 + (aquarium_jellies(q_10, b_15)));
    col_2 = (aquarium_bubbles(col_2, q_10, b_15, mix(lightC, vec3(1f), vec3(0.4f))));
    col_2 = (col_2 + ((lightC * (aquarium_spot_beam(q_10, b_15))) * 0.8f));
    return col_2;
}
