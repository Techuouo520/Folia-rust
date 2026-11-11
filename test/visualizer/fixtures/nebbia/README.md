# Nebbia reference frames

These PNGs were rendered offscreen from main `cb9ad72` before the GPU cache changes, using the public `nebbia::Renderer` in release mode. They are Rust regression references, not screenshots of the original Electron application.

- Resolution: 1920 × 1080, scale 1, RGBA8Unorm output.
- GPU: Intel Arc A580, Vulkan, Intel driver 101.6987.
- `main-06/10/15.png`: empty lyrics, time 6.0/10.5/15.0 s, default tuning except busSpeed 1, energy 0.9, Midnight theme. The fixed spotlight and breath lobe are defined in `nebbia_gpu::sample`.
- `main-song-06/10/15.png`: `nebbia_song()` in `test/visualizer/lib.rs`, the same times, Midnight theme, default tuning and energy 0.9. AnimCJK medians load from `assets/nebbia/nebbia-strokes.bin.gz` before capture; glyph workers must settle. No real music, account or user settings are read.
- The capture used six prepare/render cycles at each fixed timestamp. On main these repeat the same frame; on the optimized renderer they finish all six building textures.

`nebbia_matches_main_pixels` compares every RGB channel and checks opaque alpha. The background references retain the original limits: mean error below 0.5/255 and fewer than 0.25% of channels differing by more than 12/255. The historical writing references include the unreadable eight-pixel stroke floor and half-size mask: after the legibility correction, their foreground changes are bounded by mean error below 0.75/255 and fewer than 1% of channels differing by more than 12/255. `nebbia_legibility` separately requires small 是 to retain an internal gap and 是/想/虽/然 to respond to stroke-width adjustments. Reference PNGs remain unchanged; regenerating them from the optimized renderer to make a failed test pass is forbidden.

Run visual and cache checks with `cargo test --release --manifest-path test/Cargo.toml -p test-visualizer nebbia_gpu:: -- --nocapture --test-threads=1`. Set `WGPU_BACKEND=dx12` or `vulkan` before starting Cargo to select the backend. GPU tests are ignored only with the existing `ci` feature.

Run isolated timing with `cargo test --release --manifest-path test/Cargo.toml -p test-visualizer nebbia_gpu::nebbia_gpu_300_frames -- --nocapture --test-threads=1`. Both resolutions warm up for 12 frames, then measure 300 frames at 60 Hz playback increments from 6.2 s, using `sample`. Two timestamp-only pass boundaries enclose all renderer passes, including incremental building fills; creation, upload and readback are outside the GPU interval. Median is the average of sorted samples 149 and 150; p95 uses nearest rank 285. Devices without timestamp queries report submit-to-completion wall time instead. Test workspace release optimization applies equally to main and the changed renderer.
