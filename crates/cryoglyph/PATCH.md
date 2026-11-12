# Folia patch

Upstream: cryoglyph 0.1.0 from crates.io (https://github.com/iced-rs/cryoglyph), MIT OR Apache-2.0 OR Zlib;
the license files are kept beside this note. Wired in by `[patch.crates-io]` in the workspace `Cargo.toml`.
`examples/`, `benches/` and `samples/` of the published crate are left out.

Only code change: `src/shader.wgsl`, `fs_main`, mask glyphs, plus the flat color-mode varying from the vertex stage.
With iced's default `web-colors` (`ColorMode::Web`), coverage is calibrated against composited grayscale Chromium
text on Windows. The exponent is 1.05 at luminance 0.1 and below, transitioning with smoothstep to 0.4 at
luminance 0.9 and above. `ColorMode::Accurate` and color glyphs are untouched. Glyph size and advance are unchanged.

The former 1.7-to-1.0 curve was fitted to one 13px bold Chinese line. A broader matrix exposed substantial loss
of ink in small regular text and insufficient contrast for light text. `test/ui/fonts.rs` now exercises the real
iced/cryoglyph path against 120 Playwright PNGs: seven sizes, six weights, two polarities, and DPR 1/1.25/1.5/2.
The maximum allowed normalized ink error is 15%. Reference metadata and the explicit regeneration command are in
`test/ui/fixtures/fonts/README.md`; references are never updated by tests.

This is a grayscale approximation for the captured Windows font files and Chrome 156.0.8078.80, not a claim of
pixel equality with every DirectWrite/Skia version or a simulation of LCD subpixel rendering. Recheck both
polarities and the full matrix when updating iced, cosmic-text, Swash, Chromium, or the system fonts.

Small mask glyphs additionally reduce dense hinted edge coverage using their four axis neighbours.
The support window is smoothstep(0.4, 0.9) times the inverse smoothstep(1.4, 1.9), with coverage
weighted by smoothstep(0.65, 0.95). The correction is 0.14 for dark ink and 0.35 for light ink,
interpolated by the existing luminance transition. It tapers over physical glyph extents 18–26px
for dark ink and 16–20px for light ink. Larger masks skip the additional neighbour samples.
The flat extent varying comes from the existing vertex dimensions. The 150-sample Chromium matrix,
including login titles, tracked tabs and DPR 1/1.25/2, passes the existing 15% ink tolerance.
