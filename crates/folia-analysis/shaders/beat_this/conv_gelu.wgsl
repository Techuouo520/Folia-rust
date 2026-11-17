// beatThis CONV_GELU, ported from Folia upstream 8006b40f6, AGPL-3.0-only.
fn erf_f32(v: f32) -> f32 {
  let a = abs(v);
  let x = 1.0 / (1.0 + 0.3275911 * a);
  let y = 1.0 - ((((1.061405429 * x - 1.453152027) * x + 1.421413741) * x - 0.284496736) * x + 0.254829592) * x * exp(-a * a);
  return select(-y, y, v >= 0.0);
}
fn gelu(v: f32) -> f32 { return 0.5 * v * (1.0 + erf_f32(v * 0.7071067811865476)); }

struct P { fout: u32, frames: u32, cin: u32, cout: u32, kf: u32 }
@group(0) @binding(0) var<storage, read> x: array<f32>;
@group(0) @binding(1) var<storage, read> w: array<f32>;   // [kf, 3, cin, cout] = [K, cout]
@group(0) @binding(2) var<storage, read> b: array<f32>;
@group(0) @binding(3) var<storage, read_write> y: array<f32>;
@group(0) @binding(4) var<uniform> p: P;
var<workgroup> as_: array<f32, 1024>; // [64 rows][16 k]
var<workgroup> bs_: array<f32, 1024>; // [16 k][64 cols]

fn gather(row: u32, k: u32) -> f32 {
  let ci = k % p.cin;
  let tap = k / p.cin;
  let dt = tap % 3u;
  let df = tap / 3u;
  let t = row % p.frames;
  let fo = row / p.frames;
  let ti = i32(t) + i32(dt) - 1;
  if (ti < 0 || ti >= i32(p.frames)) { return 0.0; }
  return x[((fo * p.kf + df) * p.frames + u32(ti)) * p.cin + ci];
}

@compute @workgroup_size(16, 16)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) l: vec3u, @builtin(local_invocation_index) li: u32) {
  let m = p.fout * p.frames;
  let kTotal = p.kf * 3u * p.cin;
  let row0 = wg.y * 64u;
  let col0 = wg.x * 64u;
  var acc: array<array<f32, 4>, 4>;
  for (var k0 = 0u; k0 < kTotal; k0 += 16u) {
    {
      let idx = li + 0u * 256u;
      let ar = row0 + idx / 16u;
      let ak = k0 + idx % 16u;
      var av = 0.0;
      if (ar < m && ak < kTotal) { av = gather(ar, ak); }
      as_[idx] = av;
      let bk = k0 + idx / 64u;
      let bc = col0 + idx % 64u;
      var bv = 0.0;
      if (bk < kTotal && bc < p.cout) { bv = w[bk * p.cout + bc]; }
      bs_[idx] = bv;
    }
{
      let idx = li + 1u * 256u;
      let ar = row0 + idx / 16u;
      let ak = k0 + idx % 16u;
      var av = 0.0;
      if (ar < m && ak < kTotal) { av = gather(ar, ak); }
      as_[idx] = av;
      let bk = k0 + idx / 64u;
      let bc = col0 + idx % 64u;
      var bv = 0.0;
      if (bk < kTotal && bc < p.cout) { bv = w[bk * p.cout + bc]; }
      bs_[idx] = bv;
    }
{
      let idx = li + 2u * 256u;
      let ar = row0 + idx / 16u;
      let ak = k0 + idx % 16u;
      var av = 0.0;
      if (ar < m && ak < kTotal) { av = gather(ar, ak); }
      as_[idx] = av;
      let bk = k0 + idx / 64u;
      let bc = col0 + idx % 64u;
      var bv = 0.0;
      if (bk < kTotal && bc < p.cout) { bv = w[bk * p.cout + bc]; }
      bs_[idx] = bv;
    }
{
      let idx = li + 3u * 256u;
      let ar = row0 + idx / 16u;
      let ak = k0 + idx % 16u;
      var av = 0.0;
      if (ar < m && ak < kTotal) { av = gather(ar, ak); }
      as_[idx] = av;
      let bk = k0 + idx / 64u;
      let bc = col0 + idx % 64u;
      var bv = 0.0;
      if (bk < kTotal && bc < p.cout) { bv = w[bk * p.cout + bc]; }
      bs_[idx] = bv;
    }
    workgroupBarrier();
    for (var kk = 0u; kk < 16u; kk++) {
      var a: array<f32, 4>;
      var bb: array<f32, 4>;
      { a[0u] = as_[(l.y * 4u + 0u) * 16u + kk]; }
{ a[1u] = as_[(l.y * 4u + 1u) * 16u + kk]; }
{ a[2u] = as_[(l.y * 4u + 2u) * 16u + kk]; }
{ a[3u] = as_[(l.y * 4u + 3u) * 16u + kk]; }
      { bb[0u] = bs_[kk * 64u + l.x * 4u + 0u]; }
{ bb[1u] = bs_[kk * 64u + l.x * 4u + 1u]; }
{ bb[2u] = bs_[kk * 64u + l.x * 4u + 2u]; }
{ bb[3u] = bs_[kk * 64u + l.x * 4u + 3u]; }
      {
        { acc[0u][0u] += a[0u] * bb[0u]; }
{ acc[0u][1u] += a[0u] * bb[1u]; }
{ acc[0u][2u] += a[0u] * bb[2u]; }
{ acc[0u][3u] += a[0u] * bb[3u]; }
      }
{
        { acc[1u][0u] += a[1u] * bb[0u]; }
{ acc[1u][1u] += a[1u] * bb[1u]; }
{ acc[1u][2u] += a[1u] * bb[2u]; }
{ acc[1u][3u] += a[1u] * bb[3u]; }
      }
{
        { acc[2u][0u] += a[2u] * bb[0u]; }
{ acc[2u][1u] += a[2u] * bb[1u]; }
{ acc[2u][2u] += a[2u] * bb[2u]; }
{ acc[2u][3u] += a[2u] * bb[3u]; }
      }
{
        { acc[3u][0u] += a[3u] * bb[0u]; }
{ acc[3u][1u] += a[3u] * bb[1u]; }
{ acc[3u][2u] += a[3u] * bb[2u]; }
{ acc[3u][3u] += a[3u] * bb[3u]; }
      }
    }
    workgroupBarrier();
  }
  for (var i = 0u; i < 4u; i++) {
    let r = row0 + l.y * 4u + i;
    if (r >= m) { continue; }
    for (var j = 0u; j < 4u; j++) {
      let c = col0 + l.x * 4u + j;
      if (c >= p.cout) { continue; }
      y[r * p.cout + c] = gelu(acc[i][j] + b[c]);
    }
  }
}
