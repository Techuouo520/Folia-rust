// beatThis ATTENTION, ported from Folia upstream 8006b40f6, AGPL-3.0-only.
struct P { n: u32, heads: u32, width: u32, dmodel: u32, seqStride: u32, batchStride: u32 }
@group(0) @binding(0) var<storage, read> qkv: array<f32>;
@group(0) @binding(1) var<storage, read_write> o: array<f32>; // [rows, dmodel]
@group(0) @binding(2) var<uniform> p: P;
const TILE = 32u;
var<workgroup> ks: array<vec4f, 256>; // [TILE keys][8 x vec4]
var<workgroup> vs: array<vec4f, 256>;
@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) l: vec3u) {
  let b = wg.z;
  let h = wg.y;
  let qi = wg.x * 64u + l.x;
  let valid = qi < p.n;
  let qrow = b * p.batchStride + min(qi, p.n - 1u) * p.seqStride;
  var q: array<vec4f, 8>;
  var acc: array<vec4f, 8>;
  let qBase = qrow * p.width + h * 32u;
  {
    q[0u] = vec4f(qkv[qBase + 4u * 0u], qkv[qBase + 4u * 0u + 1u], qkv[qBase + 4u * 0u + 2u], qkv[qBase + 4u * 0u + 3u]) * 0.17677669529663687;
    acc[0u] = vec4f(0.0);
  }
{
    q[1u] = vec4f(qkv[qBase + 4u * 1u], qkv[qBase + 4u * 1u + 1u], qkv[qBase + 4u * 1u + 2u], qkv[qBase + 4u * 1u + 3u]) * 0.17677669529663687;
    acc[1u] = vec4f(0.0);
  }
{
    q[2u] = vec4f(qkv[qBase + 4u * 2u], qkv[qBase + 4u * 2u + 1u], qkv[qBase + 4u * 2u + 2u], qkv[qBase + 4u * 2u + 3u]) * 0.17677669529663687;
    acc[2u] = vec4f(0.0);
  }
{
    q[3u] = vec4f(qkv[qBase + 4u * 3u], qkv[qBase + 4u * 3u + 1u], qkv[qBase + 4u * 3u + 2u], qkv[qBase + 4u * 3u + 3u]) * 0.17677669529663687;
    acc[3u] = vec4f(0.0);
  }
{
    q[4u] = vec4f(qkv[qBase + 4u * 4u], qkv[qBase + 4u * 4u + 1u], qkv[qBase + 4u * 4u + 2u], qkv[qBase + 4u * 4u + 3u]) * 0.17677669529663687;
    acc[4u] = vec4f(0.0);
  }
{
    q[5u] = vec4f(qkv[qBase + 4u * 5u], qkv[qBase + 4u * 5u + 1u], qkv[qBase + 4u * 5u + 2u], qkv[qBase + 4u * 5u + 3u]) * 0.17677669529663687;
    acc[5u] = vec4f(0.0);
  }
{
    q[6u] = vec4f(qkv[qBase + 4u * 6u], qkv[qBase + 4u * 6u + 1u], qkv[qBase + 4u * 6u + 2u], qkv[qBase + 4u * 6u + 3u]) * 0.17677669529663687;
    acc[6u] = vec4f(0.0);
  }
{
    q[7u] = vec4f(qkv[qBase + 4u * 7u], qkv[qBase + 4u * 7u + 1u], qkv[qBase + 4u * 7u + 2u], qkv[qBase + 4u * 7u + 3u]) * 0.17677669529663687;
    acc[7u] = vec4f(0.0);
  }
  var m = -3.0e38;
  var sum = 0.0;
  for (var k0 = 0u; k0 < p.n; k0 += TILE) {
    // 32 keys x 8 vec4 = 256 vec4 each for K and V, four per invocation.
    {
      let idx = l.x + 0u * 64u;
      let kj = k0 + idx / 8u;
      var kv = vec4f(0.0);
      var vv = vec4f(0.0);
      if (kj < p.n) {
        let base = (b * p.batchStride + kj * p.seqStride) * p.width + h * 32u + (idx % 8u) * 4u;
        let kb = base + p.dmodel;
        let vb = base + 2u * p.dmodel;
        kv = vec4f(qkv[kb], qkv[kb + 1u], qkv[kb + 2u], qkv[kb + 3u]);
        vv = vec4f(qkv[vb], qkv[vb + 1u], qkv[vb + 2u], qkv[vb + 3u]);
      }
      ks[idx] = kv;
      vs[idx] = vv;
    }
{
      let idx = l.x + 1u * 64u;
      let kj = k0 + idx / 8u;
      var kv = vec4f(0.0);
      var vv = vec4f(0.0);
      if (kj < p.n) {
        let base = (b * p.batchStride + kj * p.seqStride) * p.width + h * 32u + (idx % 8u) * 4u;
        let kb = base + p.dmodel;
        let vb = base + 2u * p.dmodel;
        kv = vec4f(qkv[kb], qkv[kb + 1u], qkv[kb + 2u], qkv[kb + 3u]);
        vv = vec4f(qkv[vb], qkv[vb + 1u], qkv[vb + 2u], qkv[vb + 3u]);
      }
      ks[idx] = kv;
      vs[idx] = vv;
    }
{
      let idx = l.x + 2u * 64u;
      let kj = k0 + idx / 8u;
      var kv = vec4f(0.0);
      var vv = vec4f(0.0);
      if (kj < p.n) {
        let base = (b * p.batchStride + kj * p.seqStride) * p.width + h * 32u + (idx % 8u) * 4u;
        let kb = base + p.dmodel;
        let vb = base + 2u * p.dmodel;
        kv = vec4f(qkv[kb], qkv[kb + 1u], qkv[kb + 2u], qkv[kb + 3u]);
        vv = vec4f(qkv[vb], qkv[vb + 1u], qkv[vb + 2u], qkv[vb + 3u]);
      }
      ks[idx] = kv;
      vs[idx] = vv;
    }
{
      let idx = l.x + 3u * 64u;
      let kj = k0 + idx / 8u;
      var kv = vec4f(0.0);
      var vv = vec4f(0.0);
      if (kj < p.n) {
        let base = (b * p.batchStride + kj * p.seqStride) * p.width + h * 32u + (idx % 8u) * 4u;
        let kb = base + p.dmodel;
        let vb = base + 2u * p.dmodel;
        kv = vec4f(qkv[kb], qkv[kb + 1u], qkv[kb + 2u], qkv[kb + 3u]);
        vv = vec4f(qkv[vb], qkv[vb + 1u], qkv[vb + 2u], qkv[vb + 3u]);
      }
      ks[idx] = kv;
      vs[idx] = vv;
    }
    workgroupBarrier();
    // Constant trip counts so the compiler unrolls and keeps s[] in registers; keys past the end
    // of the sequence get a score of -3e38, whose exp is exactly 0.
    var s: array<f32, 32>;
    var tileMax = -3.0e38;
    {
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[0u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[0u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[0u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[0u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[0u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[0u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[0u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[0u * 8u + 7u], dotv); }
      s[0u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 0u < p.n);
      tileMax = max(tileMax, s[0u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[1u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[1u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[1u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[1u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[1u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[1u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[1u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[1u * 8u + 7u], dotv); }
      s[1u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 1u < p.n);
      tileMax = max(tileMax, s[1u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[2u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[2u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[2u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[2u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[2u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[2u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[2u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[2u * 8u + 7u], dotv); }
      s[2u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 2u < p.n);
      tileMax = max(tileMax, s[2u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[3u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[3u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[3u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[3u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[3u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[3u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[3u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[3u * 8u + 7u], dotv); }
      s[3u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 3u < p.n);
      tileMax = max(tileMax, s[3u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[4u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[4u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[4u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[4u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[4u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[4u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[4u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[4u * 8u + 7u], dotv); }
      s[4u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 4u < p.n);
      tileMax = max(tileMax, s[4u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[5u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[5u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[5u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[5u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[5u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[5u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[5u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[5u * 8u + 7u], dotv); }
      s[5u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 5u < p.n);
      tileMax = max(tileMax, s[5u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[6u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[6u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[6u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[6u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[6u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[6u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[6u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[6u * 8u + 7u], dotv); }
      s[6u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 6u < p.n);
      tileMax = max(tileMax, s[6u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[7u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[7u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[7u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[7u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[7u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[7u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[7u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[7u * 8u + 7u], dotv); }
      s[7u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 7u < p.n);
      tileMax = max(tileMax, s[7u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[8u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[8u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[8u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[8u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[8u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[8u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[8u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[8u * 8u + 7u], dotv); }
      s[8u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 8u < p.n);
      tileMax = max(tileMax, s[8u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[9u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[9u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[9u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[9u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[9u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[9u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[9u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[9u * 8u + 7u], dotv); }
      s[9u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 9u < p.n);
      tileMax = max(tileMax, s[9u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[10u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[10u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[10u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[10u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[10u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[10u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[10u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[10u * 8u + 7u], dotv); }
      s[10u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 10u < p.n);
      tileMax = max(tileMax, s[10u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[11u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[11u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[11u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[11u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[11u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[11u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[11u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[11u * 8u + 7u], dotv); }
      s[11u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 11u < p.n);
      tileMax = max(tileMax, s[11u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[12u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[12u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[12u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[12u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[12u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[12u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[12u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[12u * 8u + 7u], dotv); }
      s[12u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 12u < p.n);
      tileMax = max(tileMax, s[12u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[13u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[13u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[13u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[13u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[13u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[13u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[13u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[13u * 8u + 7u], dotv); }
      s[13u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 13u < p.n);
      tileMax = max(tileMax, s[13u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[14u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[14u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[14u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[14u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[14u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[14u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[14u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[14u * 8u + 7u], dotv); }
      s[14u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 14u < p.n);
      tileMax = max(tileMax, s[14u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[15u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[15u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[15u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[15u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[15u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[15u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[15u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[15u * 8u + 7u], dotv); }
      s[15u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 15u < p.n);
      tileMax = max(tileMax, s[15u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[16u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[16u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[16u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[16u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[16u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[16u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[16u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[16u * 8u + 7u], dotv); }
      s[16u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 16u < p.n);
      tileMax = max(tileMax, s[16u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[17u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[17u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[17u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[17u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[17u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[17u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[17u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[17u * 8u + 7u], dotv); }
      s[17u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 17u < p.n);
      tileMax = max(tileMax, s[17u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[18u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[18u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[18u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[18u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[18u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[18u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[18u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[18u * 8u + 7u], dotv); }
      s[18u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 18u < p.n);
      tileMax = max(tileMax, s[18u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[19u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[19u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[19u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[19u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[19u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[19u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[19u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[19u * 8u + 7u], dotv); }
      s[19u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 19u < p.n);
      tileMax = max(tileMax, s[19u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[20u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[20u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[20u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[20u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[20u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[20u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[20u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[20u * 8u + 7u], dotv); }
      s[20u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 20u < p.n);
      tileMax = max(tileMax, s[20u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[21u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[21u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[21u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[21u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[21u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[21u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[21u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[21u * 8u + 7u], dotv); }
      s[21u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 21u < p.n);
      tileMax = max(tileMax, s[21u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[22u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[22u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[22u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[22u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[22u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[22u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[22u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[22u * 8u + 7u], dotv); }
      s[22u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 22u < p.n);
      tileMax = max(tileMax, s[22u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[23u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[23u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[23u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[23u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[23u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[23u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[23u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[23u * 8u + 7u], dotv); }
      s[23u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 23u < p.n);
      tileMax = max(tileMax, s[23u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[24u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[24u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[24u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[24u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[24u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[24u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[24u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[24u * 8u + 7u], dotv); }
      s[24u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 24u < p.n);
      tileMax = max(tileMax, s[24u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[25u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[25u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[25u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[25u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[25u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[25u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[25u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[25u * 8u + 7u], dotv); }
      s[25u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 25u < p.n);
      tileMax = max(tileMax, s[25u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[26u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[26u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[26u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[26u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[26u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[26u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[26u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[26u * 8u + 7u], dotv); }
      s[26u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 26u < p.n);
      tileMax = max(tileMax, s[26u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[27u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[27u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[27u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[27u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[27u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[27u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[27u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[27u * 8u + 7u], dotv); }
      s[27u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 27u < p.n);
      tileMax = max(tileMax, s[27u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[28u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[28u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[28u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[28u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[28u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[28u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[28u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[28u * 8u + 7u], dotv); }
      s[28u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 28u < p.n);
      tileMax = max(tileMax, s[28u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[29u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[29u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[29u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[29u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[29u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[29u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[29u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[29u * 8u + 7u], dotv); }
      s[29u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 29u < p.n);
      tileMax = max(tileMax, s[29u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[30u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[30u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[30u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[30u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[30u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[30u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[30u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[30u * 8u + 7u], dotv); }
      s[30u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 30u < p.n);
      tileMax = max(tileMax, s[30u]);
    }
{
      var dotv = vec4f(0.0);
      { dotv = fma(q[0u], ks[31u * 8u + 0u], dotv); }
{ dotv = fma(q[1u], ks[31u * 8u + 1u], dotv); }
{ dotv = fma(q[2u], ks[31u * 8u + 2u], dotv); }
{ dotv = fma(q[3u], ks[31u * 8u + 3u], dotv); }
{ dotv = fma(q[4u], ks[31u * 8u + 4u], dotv); }
{ dotv = fma(q[5u], ks[31u * 8u + 5u], dotv); }
{ dotv = fma(q[6u], ks[31u * 8u + 6u], dotv); }
{ dotv = fma(q[7u], ks[31u * 8u + 7u], dotv); }
      s[31u] = select(-3.0e38, dotv.x + dotv.y + dotv.z + dotv.w, k0 + 31u < p.n);
      tileMax = max(tileMax, s[31u]);
    }
    let mNew = max(m, tileMax);
    let r = exp(m - mNew);
    sum *= r;
    { acc[0u] *= r; }
{ acc[1u] *= r; }
{ acc[2u] *= r; }
{ acc[3u] *= r; }
{ acc[4u] *= r; }
{ acc[5u] *= r; }
{ acc[6u] *= r; }
{ acc[7u] *= r; }
    m = mNew;
    {
      let e = exp(s[0u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[0u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[0u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[0u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[0u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[0u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[0u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[0u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[0u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[1u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[1u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[1u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[1u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[1u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[1u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[1u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[1u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[1u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[2u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[2u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[2u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[2u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[2u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[2u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[2u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[2u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[2u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[3u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[3u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[3u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[3u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[3u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[3u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[3u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[3u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[3u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[4u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[4u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[4u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[4u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[4u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[4u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[4u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[4u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[4u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[5u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[5u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[5u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[5u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[5u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[5u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[5u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[5u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[5u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[6u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[6u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[6u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[6u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[6u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[6u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[6u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[6u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[6u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[7u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[7u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[7u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[7u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[7u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[7u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[7u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[7u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[7u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[8u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[8u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[8u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[8u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[8u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[8u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[8u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[8u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[8u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[9u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[9u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[9u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[9u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[9u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[9u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[9u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[9u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[9u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[10u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[10u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[10u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[10u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[10u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[10u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[10u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[10u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[10u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[11u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[11u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[11u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[11u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[11u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[11u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[11u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[11u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[11u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[12u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[12u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[12u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[12u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[12u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[12u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[12u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[12u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[12u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[13u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[13u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[13u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[13u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[13u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[13u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[13u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[13u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[13u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[14u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[14u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[14u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[14u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[14u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[14u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[14u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[14u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[14u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[15u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[15u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[15u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[15u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[15u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[15u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[15u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[15u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[15u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[16u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[16u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[16u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[16u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[16u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[16u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[16u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[16u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[16u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[17u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[17u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[17u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[17u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[17u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[17u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[17u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[17u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[17u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[18u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[18u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[18u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[18u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[18u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[18u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[18u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[18u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[18u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[19u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[19u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[19u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[19u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[19u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[19u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[19u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[19u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[19u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[20u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[20u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[20u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[20u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[20u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[20u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[20u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[20u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[20u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[21u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[21u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[21u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[21u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[21u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[21u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[21u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[21u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[21u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[22u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[22u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[22u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[22u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[22u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[22u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[22u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[22u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[22u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[23u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[23u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[23u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[23u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[23u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[23u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[23u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[23u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[23u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[24u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[24u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[24u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[24u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[24u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[24u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[24u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[24u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[24u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[25u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[25u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[25u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[25u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[25u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[25u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[25u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[25u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[25u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[26u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[26u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[26u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[26u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[26u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[26u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[26u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[26u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[26u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[27u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[27u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[27u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[27u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[27u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[27u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[27u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[27u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[27u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[28u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[28u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[28u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[28u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[28u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[28u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[28u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[28u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[28u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[29u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[29u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[29u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[29u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[29u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[29u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[29u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[29u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[29u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[30u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[30u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[30u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[30u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[30u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[30u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[30u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[30u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[30u * 8u + 7u], acc[7u]); }
    }
{
      let e = exp(s[31u] - m);
      sum += e;
      { acc[0u] = fma(vec4f(e), vs[31u * 8u + 0u], acc[0u]); }
{ acc[1u] = fma(vec4f(e), vs[31u * 8u + 1u], acc[1u]); }
{ acc[2u] = fma(vec4f(e), vs[31u * 8u + 2u], acc[2u]); }
{ acc[3u] = fma(vec4f(e), vs[31u * 8u + 3u], acc[3u]); }
{ acc[4u] = fma(vec4f(e), vs[31u * 8u + 4u], acc[4u]); }
{ acc[5u] = fma(vec4f(e), vs[31u * 8u + 5u], acc[5u]); }
{ acc[6u] = fma(vec4f(e), vs[31u * 8u + 6u], acc[6u]); }
{ acc[7u] = fma(vec4f(e), vs[31u * 8u + 7u], acc[7u]); }
    }
    workgroupBarrier();
  }
  if (!valid) { return; }
  let gate = 1.0 / (1.0 + exp(-qkv[qrow * p.width + 3u * p.dmodel + h]));
  let scale = gate / sum;
  let oBase = qrow * p.dmodel + h * 32u;
  {
    let v = acc[0u] * scale;
    o[oBase + 4u * 0u] = v.x;
    o[oBase + 4u * 0u + 1u] = v.y;
    o[oBase + 4u * 0u + 2u] = v.z;
    o[oBase + 4u * 0u + 3u] = v.w;
  }
{
    let v = acc[1u] * scale;
    o[oBase + 4u * 1u] = v.x;
    o[oBase + 4u * 1u + 1u] = v.y;
    o[oBase + 4u * 1u + 2u] = v.z;
    o[oBase + 4u * 1u + 3u] = v.w;
  }
{
    let v = acc[2u] * scale;
    o[oBase + 4u * 2u] = v.x;
    o[oBase + 4u * 2u + 1u] = v.y;
    o[oBase + 4u * 2u + 2u] = v.z;
    o[oBase + 4u * 2u + 3u] = v.w;
  }
{
    let v = acc[3u] * scale;
    o[oBase + 4u * 3u] = v.x;
    o[oBase + 4u * 3u + 1u] = v.y;
    o[oBase + 4u * 3u + 2u] = v.z;
    o[oBase + 4u * 3u + 3u] = v.w;
  }
{
    let v = acc[4u] * scale;
    o[oBase + 4u * 4u] = v.x;
    o[oBase + 4u * 4u + 1u] = v.y;
    o[oBase + 4u * 4u + 2u] = v.z;
    o[oBase + 4u * 4u + 3u] = v.w;
  }
{
    let v = acc[5u] * scale;
    o[oBase + 4u * 5u] = v.x;
    o[oBase + 4u * 5u + 1u] = v.y;
    o[oBase + 4u * 5u + 2u] = v.z;
    o[oBase + 4u * 5u + 3u] = v.w;
  }
{
    let v = acc[6u] * scale;
    o[oBase + 4u * 6u] = v.x;
    o[oBase + 4u * 6u + 1u] = v.y;
    o[oBase + 4u * 6u + 2u] = v.z;
    o[oBase + 4u * 6u + 3u] = v.w;
  }
{
    let v = acc[7u] * scale;
    o[oBase + 4u * 7u] = v.x;
    o[oBase + 4u * 7u + 1u] = v.y;
    o[oBase + 4u * 7u + 2u] = v.z;
    o[oBase + 4u * 7u + 3u] = v.w;
  }
}
