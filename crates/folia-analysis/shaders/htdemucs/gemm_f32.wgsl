// htdemucs gemm_f32, ported from Folia upstream 8006b40f6, AGPL-3.0-only.
fn erf_f32(v: f32) -> f32 {
  let a = abs(v);
  let x = 1.0 / (1.0 + 0.3275911 * a);
  let y = 1.0 - ((((1.061405429 * x - 1.453152027) * x + 1.421413741) * x - 0.284496736) * x + 0.254829592) * x * exp(-a * a);
  return select(-y, y, v >= 0.0);
}
fn gelu(v: f32) -> f32 { return 0.5 * v * (1.0 + erf_f32(v * 0.7071067811865476)); }

struct P { m: u32, k: u32, cin: u32, kh: u32, kw: u32, sh: u32, sw: u32, ph: u32, pw: u32, dh: u32, dw: u32, hin: u32, win: u32, hout: u32, wout: u32, flags: u32 }
@group(0) @binding(0) var<storage, read> x: array<f32>;

@group(0) @binding(1) var<storage, read> w: array<f32>;
fn wload(e: u32) -> f32 { return w[e]; }
@group(0) @binding(2) var<storage, read> b: array<f32>;
@group(0) @binding(3) var<storage, read> side: array<f32>;
@group(0) @binding(4) var<storage, read_write> y: array<f32>;
@group(0) @binding(5) var<uniform> p: P;

fn gather(k: u32, n: u32) -> f32 {
  let taps = p.kh * p.kw;
  let ci = k / taps;
  let r = k - ci * taps;
  let a = r / p.kw;
  let c = r - a * p.kw;
  let ho = n / p.wout;
  let wo = n - ho * p.wout;
  let hi = i32(ho * p.sh + a * p.dh) - i32(p.ph);
  let wi = i32(wo * p.sw + c * p.dw) - i32(p.pw);
  if (hi < 0 || hi >= i32(p.hin) || wi < 0 || wi >= i32(p.win)) { return 0.0; }
  return x[(ci * p.hin + u32(hi)) * p.win + u32(wi)];
}

fn store(r: u32, c: u32, sum: f32) {
  var v = sum + b[r];
  if ((p.flags & 1u) != 0u) { v = gelu(v); }
  let o = r * p.hout * p.wout + c;
  if ((p.flags & 2u) != 0u) { y[o] = y[o] + side[r] * v; } else { y[o] = v; }
}


var<workgroup> as_: array<vec4f, 256>;
var<workgroup> bs_: array<vec4f, 256>;
@compute @workgroup_size(16, 16)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) l: vec3u, @builtin(local_invocation_index) li: u32) {
  let n = p.hout * p.wout;
  let row0 = wg.y * 64u;
  let col0 = wg.x * 64u;
  var acc: array<vec4f, 4>;
  for (var k0 = 0u; k0 < p.k; k0 += 16u) {
    // Invocation li: rows 4 (li / 16).. at k li % 16 of A, columns 4 (li % 16).. at k li / 16 of B.
    let ak = k0 + li % 16u;
    var av = vec4f(0.0);
    let bk = k0 + li / 16u;
    var bv = vec4f(0.0);
    {
      let ar = row0 + (li / 16u) * 4u + 0u;
      if (ar < p.m && ak < p.k) { av[0u] = wload(ar * p.k + ak); }
      let bc = col0 + (li % 16u) * 4u + 0u;
      if (bk < p.k && bc < n) { bv[0u] = gather(bk, bc); }
    }
    {
      let ar = row0 + (li / 16u) * 4u + 1u;
      if (ar < p.m && ak < p.k) { av[1u] = wload(ar * p.k + ak); }
      let bc = col0 + (li % 16u) * 4u + 1u;
      if (bk < p.k && bc < n) { bv[1u] = gather(bk, bc); }
    }
    {
      let ar = row0 + (li / 16u) * 4u + 2u;
      if (ar < p.m && ak < p.k) { av[2u] = wload(ar * p.k + ak); }
      let bc = col0 + (li % 16u) * 4u + 2u;
      if (bk < p.k && bc < n) { bv[2u] = gather(bk, bc); }
    }
    {
      let ar = row0 + (li / 16u) * 4u + 3u;
      if (ar < p.m && ak < p.k) { av[3u] = wload(ar * p.k + ak); }
      let bc = col0 + (li % 16u) * 4u + 3u;
      if (bk < p.k && bc < n) { bv[3u] = gather(bk, bc); }
    }
    as_[(li % 16u) * 16u + li / 16u] = vec4f(av);
    bs_[li] = vec4f(bv);
    workgroupBarrier();
    for (var kk = 0u; kk < 16u; kk++) {
      let a = as_[kk * 16u + l.y];
      let bb = bs_[kk * 16u + l.x];
      // Constant accumulator indices keep the DX12 FXC loop bounded.
      acc[0] = fma(vec4f(a[0]), bb, acc[0]);
      acc[1] = fma(vec4f(a[1]), bb, acc[1]);
      acc[2] = fma(vec4f(a[2]), bb, acc[2]);
      acc[3] = fma(vec4f(a[3]), bb, acc[3]);
    }
    workgroupBarrier();
  }
  for (var i = 0u; i < 4u; i++) {
    let r = row0 + l.y * 4u + i;
    if (r >= p.m) { continue; }
    for (var j = 0u; j < 4u; j++) {
      let c = col0 + l.x * 4u + j;
      if (c >= n) { continue; }
      store(r, c, acc[i][j]);
    }
  }
}
