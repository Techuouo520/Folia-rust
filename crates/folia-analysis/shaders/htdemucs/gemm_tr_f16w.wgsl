// htdemucs gemm_tr_f16w, ported from Folia upstream 8006b40f6, AGPL-3.0-only.
fn erf_f32(v: f32) -> f32 {
  let a = abs(v);
  let x = 1.0 / (1.0 + 0.3275911 * a);
  let y = 1.0 - ((((1.061405429 * x - 1.453152027) * x + 1.421413741) * x - 0.284496736) * x + 0.254829592) * x * exp(-a * a);
  return select(-y, y, v >= 0.0);
}
fn gelu(v: f32) -> f32 { return 0.5 * v * (1.0 + erf_f32(v * 0.7071067811865476)); }

struct P { m: u32, k: u32, cin: u32, hin: u32, win: u32, axis: u32, flags: u32 }
@group(0) @binding(0) var<storage, read> x: array<f32>;

@group(0) @binding(1) var<storage, read> w: array<u32>;
fn wload(e: u32) -> f32 {
  let pair = unpack2x16float(w[e >> 1u]);
  return select(pair.x, pair.y, (e & 1u) == 1u);
}
@group(0) @binding(2) var<storage, read> b: array<f32>;
@group(0) @binding(3) var<storage, read_write> y: array<f32>;
@group(0) @binding(4) var<uniform> p: P;

fn gather(k: u32, n: u32) -> f32 {
  let ci = k / 2u;
  let t = k - ci * 2u;
  if (p.axis == 0u) {
    let q = n / p.win;
    let w = n - q * p.win;
    if (q < t || q - t >= p.hin) { return 0.0; }
    return x[(ci * p.hin + q - t) * p.win + w];
  }
  let h = n / (p.win + 1u);
  let q = n - h * (p.win + 1u);
  if (q < t || q - t >= p.win) { return 0.0; }
  return x[(ci * p.hin + h) * p.win + q - t];
}

fn store(r: u32, c: u32, sum: f32) {
  let co = r / 4u;
  let ph = r - co * 4u;
  var v = sum + b[co];
  if ((p.flags & 1u) != 0u) { v = gelu(v); }
  if (p.axis == 0u) {
    let q = c / p.win;
    let w = c - q * p.win;
    let o = 4u * q + ph;
    if (o < 2u || o - 2u >= 4u * p.hin) { return; }
    y[(co * 4u * p.hin + o - 2u) * p.win + w] = v;
  } else {
    let h = c / (p.win + 1u);
    let q = c - h * (p.win + 1u);
    let o = 4u * q + ph;
    if (o < 2u || o - 2u >= 4u * p.win) { return; }
    y[(co * p.hin + h) * 4u * p.win + o - 2u] = v;
  }
}


var<workgroup> as_: array<vec4f, 256>;
var<workgroup> bs_: array<vec4f, 256>;
@compute @workgroup_size(16, 16)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) l: vec3u, @builtin(local_invocation_index) li: u32) {
  let n = select(p.hin * (p.win + 1u), (p.hin + 1u) * p.win, p.axis == 0u);
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
