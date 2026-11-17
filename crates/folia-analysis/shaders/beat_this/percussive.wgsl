// Original Folia HPSS spectral energy, with constant-index median comparisons for native DX12.
struct Params { frames: u32, first: u32, count: u32 }
@group(0) @binding(0) var<storage, read> mel: array<f32>;
@group(0) @binding(1) var<storage, read_write> energy: array<f32>;
@group(0) @binding(2) var<uniform> p: Params;

const BANDS: i32 = 128;
const HALF: i32 = 8;
const TAPS: i32 = 17;

fn magnitude(frame: i32, band: i32) -> f32 {
  if (frame < 0 || frame >= i32(p.frames) || band < 0 || band >= BANDS) { return 0.0; }
  let v = mel[u32(frame * BANDS + band)];
  // expm1 without its cancellation near 0.
  if (v < 1e-3) { return max(0.0, v + 0.5 * v * v); }
  return exp(v) - 1.0;
}

fn median(values: ptr<function, array<f32, 17>>) -> f32 {
  // Fixed odd-even comparisons preserve the median and compile on DX12 FXC.
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  { let a = (*values)[1]; let b = (*values)[2]; (*values)[1] = min(a, b); (*values)[2] = max(a, b); }
  { let a = (*values)[3]; let b = (*values)[4]; (*values)[3] = min(a, b); (*values)[4] = max(a, b); }
  { let a = (*values)[5]; let b = (*values)[6]; (*values)[5] = min(a, b); (*values)[6] = max(a, b); }
  { let a = (*values)[7]; let b = (*values)[8]; (*values)[7] = min(a, b); (*values)[8] = max(a, b); }
  { let a = (*values)[9]; let b = (*values)[10]; (*values)[9] = min(a, b); (*values)[10] = max(a, b); }
  { let a = (*values)[11]; let b = (*values)[12]; (*values)[11] = min(a, b); (*values)[12] = max(a, b); }
  { let a = (*values)[13]; let b = (*values)[14]; (*values)[13] = min(a, b); (*values)[14] = max(a, b); }
  { let a = (*values)[15]; let b = (*values)[16]; (*values)[15] = min(a, b); (*values)[16] = max(a, b); }
  { let a = (*values)[0]; let b = (*values)[1]; (*values)[0] = min(a, b); (*values)[1] = max(a, b); }
  { let a = (*values)[2]; let b = (*values)[3]; (*values)[2] = min(a, b); (*values)[3] = max(a, b); }
  { let a = (*values)[4]; let b = (*values)[5]; (*values)[4] = min(a, b); (*values)[5] = max(a, b); }
  { let a = (*values)[6]; let b = (*values)[7]; (*values)[6] = min(a, b); (*values)[7] = max(a, b); }
  { let a = (*values)[8]; let b = (*values)[9]; (*values)[8] = min(a, b); (*values)[9] = max(a, b); }
  { let a = (*values)[10]; let b = (*values)[11]; (*values)[10] = min(a, b); (*values)[11] = max(a, b); }
  { let a = (*values)[12]; let b = (*values)[13]; (*values)[12] = min(a, b); (*values)[13] = max(a, b); }
  { let a = (*values)[14]; let b = (*values)[15]; (*values)[14] = min(a, b); (*values)[15] = max(a, b); }
  return (*values)[8];
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= p.count) { return; }
  let at = p.first + id.x;
  let frame = i32(at);
  var percussive = 0.0;
  var total = 0.0;
  var taps: array<f32, 17>;
  for (var band = 0; band < BANDS; band++) {
    taps[0] = magnitude(frame + 0 - HALF, band);
    taps[1] = magnitude(frame + 1 - HALF, band);
    taps[2] = magnitude(frame + 2 - HALF, band);
    taps[3] = magnitude(frame + 3 - HALF, band);
    taps[4] = magnitude(frame + 4 - HALF, band);
    taps[5] = magnitude(frame + 5 - HALF, band);
    taps[6] = magnitude(frame + 6 - HALF, band);
    taps[7] = magnitude(frame + 7 - HALF, band);
    taps[8] = magnitude(frame + 8 - HALF, band);
    taps[9] = magnitude(frame + 9 - HALF, band);
    taps[10] = magnitude(frame + 10 - HALF, band);
    taps[11] = magnitude(frame + 11 - HALF, band);
    taps[12] = magnitude(frame + 12 - HALF, band);
    taps[13] = magnitude(frame + 13 - HALF, band);
    taps[14] = magnitude(frame + 14 - HALF, band);
    taps[15] = magnitude(frame + 15 - HALF, band);
    taps[16] = magnitude(frame + 16 - HALF, band);
    let harmonic = median(&taps);
    taps[0] = magnitude(frame, band + 0 - HALF);
    taps[1] = magnitude(frame, band + 1 - HALF);
    taps[2] = magnitude(frame, band + 2 - HALF);
    taps[3] = magnitude(frame, band + 3 - HALF);
    taps[4] = magnitude(frame, band + 4 - HALF);
    taps[5] = magnitude(frame, band + 5 - HALF);
    taps[6] = magnitude(frame, band + 6 - HALF);
    taps[7] = magnitude(frame, band + 7 - HALF);
    taps[8] = magnitude(frame, band + 8 - HALF);
    taps[9] = magnitude(frame, band + 9 - HALF);
    taps[10] = magnitude(frame, band + 10 - HALF);
    taps[11] = magnitude(frame, band + 11 - HALF);
    taps[12] = magnitude(frame, band + 12 - HALF);
    taps[13] = magnitude(frame, band + 13 - HALF);
    taps[14] = magnitude(frame, band + 14 - HALF);
    taps[15] = magnitude(frame, band + 15 - HALF);
    taps[16] = magnitude(frame, band + 16 - HALF);
    let hit = median(&taps);
    let value = magnitude(frame, band);
    let e = value * value;
    let hh = harmonic * harmonic;
    let pp = hit * hit;
    if (hh + pp > 0.0) { percussive += e * pp / (hh + pp); }
    total += e;
  }
  energy[2u * at] = percussive;
  energy[2u * at + 1u] = total;
}
