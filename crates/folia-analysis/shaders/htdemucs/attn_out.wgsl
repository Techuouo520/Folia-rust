// htdemucs attn_out, ported from Folia upstream 8006b40f6, AGPL-3.0-only.
struct P { nq: u32, nk: u32, vr0: u32, head0: u32 }
@group(0) @binding(0) var<storage, read> s: array<f32>;
@group(0) @binding(1) var<storage, read> kv: array<f32>;
@group(0) @binding(2) var<storage, read_write> o: array<f32>;
@group(0) @binding(3) var<uniform> p: P;

var<workgroup> as_: array<vec4f, 256>;
var<workgroup> bs_: array<vec4f, 256>; // [16 keys][16 query quads], [16 keys][16 d quads]
@compute @workgroup_size(16, 16)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) l: vec3u, @builtin(local_invocation_index) li: u32) {
  let h = p.head0 + wg.z;
  let row0 = wg.y * 64u;
  var acc: array<vec4f, 4>;
  for (var j0 = 0u; j0 < p.nk; j0 += 16u) {
    // Invocation li: queries and dimensions 4 (li / 16).. at key li % 16.
    let jj = li % 16u;
    let j = j0 + jj;
    var a = vec4f(0.0);
    var b = vec4f(0.0);
    {
      let r = (li / 16u) * 4u + 0u;
      let i = row0 + r;
      if (j < p.nk) {
        if (i < p.nq) { a[0u] = s[(wg.z * p.nq + i) * p.nk + j]; }
        b[0u] = kv[(p.vr0 + h * 64u + r) * p.nk + j];
      }
    }
    {
      let r = (li / 16u) * 4u + 1u;
      let i = row0 + r;
      if (j < p.nk) {
        if (i < p.nq) { a[1u] = s[(wg.z * p.nq + i) * p.nk + j]; }
        b[1u] = kv[(p.vr0 + h * 64u + r) * p.nk + j];
      }
    }
    {
      let r = (li / 16u) * 4u + 2u;
      let i = row0 + r;
      if (j < p.nk) {
        if (i < p.nq) { a[2u] = s[(wg.z * p.nq + i) * p.nk + j]; }
        b[2u] = kv[(p.vr0 + h * 64u + r) * p.nk + j];
      }
    }
    {
      let r = (li / 16u) * 4u + 3u;
      let i = row0 + r;
      if (j < p.nk) {
        if (i < p.nq) { a[3u] = s[(wg.z * p.nq + i) * p.nk + j]; }
        b[3u] = kv[(p.vr0 + h * 64u + r) * p.nk + j];
      }
    }
    as_[jj * 16u + li / 16u] = vec4f(a);
    bs_[jj * 16u + li / 16u] = vec4f(b);
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
    if (r >= p.nq) { continue; }
    for (var j = 0u; j < 4u; j++) {
      let d = l.x * 4u + j;
      o[(h * 64u + d) * p.nq + r] = acc[i][j];
    }
  }
}
