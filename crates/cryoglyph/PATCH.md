# Folia patch

Upstream: cryoglyph 0.1.0 from crates.io (https://github.com/iced-rs/cryoglyph), MIT OR Apache-2.0 OR Zlib;
the license files are kept beside this note. Wired in by `[patch.crates-io]` in the workspace `Cargo.toml`.
`examples/`, `benches/` and `samples/` of the published crate are left out.

Only change: `src/shader.wgsl`, `fs_main`, mask glyphs. With iced's default `web-colors` (`ColorMode::Web`)
the coverage of a glyph's edge pixels is blended in gamma space, so dark text on a light background comes out
about 16% heavier than Chrome on Windows (measured on 13px Microsoft YaHei UI Bold, #1f1f1f on #e6e6e6).
Dark text now raises the coverage to a power: 1.7 at luminance 0.1 and below, easing to 1.0 (unchanged) at
0.5 and above. `ColorMode::Accurate` is untouched. The vertex stage passes the color mode to the fragment stage
as a flat varying.
