[简体中文](../README.md) | English

<div align="center">

<img src="../assets/icon.png" width="128" alt="Folia-rust">

# Folia-rust

**Let lyrics flow across the wall, not pile up in memory.**

A pure native Rust rewrite of the [Folia](https://github.com/chthollyphile/folia-major) lyric visualizer music player. It keeps the original interface and animation design, drops Electron, and draws everything on the GPU.

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-c2410c)](../LICENSE)
[![Rust 1.99](https://img.shields.io/badge/rust-1.99.1-b7410e?logo=rust)](../rust-toolchain.toml)
[![iced 0.14](https://img.shields.io/badge/UI-iced%200.14-3b82f6)](https://github.com/iced-rs/iced)
[![wgpu](https://img.shields.io/badge/GPU-wgpu%20%2F%20WGSL-6d28d9)](https://github.com/gfx-rs/wgpu)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-475569)

</div>

---

## Overview

The original Folia is built on Electron: it ships a full Chromium and Node.js, and its online platforms need a separate local HTTP service. Folia-rust rewrites it as a native program:

- **A single binary**: no browser, no Node, no local service. Platform protocols run in-process.
- **Drawn on the GPU**: the interface and lyric animations are rendered with iced + wgpu, with WGSL shaders.
- **Idle means idle**: the frame loop stops when nothing animates, so an idle window uses no CPU.

## Features

- **Cover wall home**: an endless wall of covers with inertial dragging, and a spine-style side seam that opens and closes.
- **Lyric visualization**: lyric styles ported one by one from the original, moving with the music in real time.
- **Lyric style workbench**: live preview and per-style tuning.
- **Online platforms**: NetEase Cloud Music, KuGou, QQ Music, Bodian, Qishui and Spotify, with QR sign-in, multiple accounts, playlists, radio, explore pages, word-by-word lyrics and audio quality selection.
- **Local music**: scanning, metadata, a library and folder watching.
- **Languages**: Simplified Chinese, Traditional Chinese, English, Bahasa Indonesia and Tiếng Việt.
- **Credential safety**: cookies and tokens are stored only in the system keyring.

## Lyric styles

| Style | ID | Status |
|---|---|---|
| 流光 | `classic` | ✅ |
| 云阶 | `partita` | ✅ |
| 静止 | `still` | ✅ |
| 倾诉 | `tilt` | ✅ |
| 回环 | `claddagh` | ✅ |
| 心象 | `cadenza` | ✅ |
| 雾笺 | `nebbia` | ✅ |
| 绘光 | `lumiere` | ✅ |
| 莫奈 | `monet` | ✅ |
| Other original styles | — | 🚧 In progress |

## Online platforms

| Platform | Search | Playback | Lyrics | Sign-in | Playlists / Radio / Explore |
|---|:---:|:---:|:---:|:---:|:---:|
| NetEase Cloud Music | ✅ | ✅ | ✅ | QR code | ✅ |
| KuGou | ✅ | ✅ | ✅ word-by-word KRC | QR code | ✅ |
| QQ Music | ✅ | ✅ | ✅ word-by-word QRC | QR code | ✅ |
| Bodian | ✅ | ✅ | ✅ word-by-word | QR code | ✅ |
| Qishui | ✅ | ✅ | ✅ | QR code | ✅ |
| Spotify | ✅ | Premium | ✅ line-by-line | Browser | ✅ |
| YouTube Music | 🚧 | — | — | — | Not built by default |

> Paid tracks require a paid account on the platform. YouTube Music audio requires running Google's BotGuard script, which is not possible without embedding a JS engine, so it is not built by default.

## Building

Requires Rust 1.99.1 (pinned by `rust-toolchain.toml`). Windows requires the MSVC toolchain.

```bash
git clone https://github.com/Techuouo520/Folia-rust.git
cd Folia-rust
cargo build --release -p folia-app
```

The output is `target/release/folia` (`folia.exe` on Windows). It needs the `assets/` folder next to it at runtime.

## Roadmap

- [x] Skeleton: window, settings, language switching
- [x] Local music scanning and playback
- [x] Lyric parsing, nine lyric styles, lyric style workbench
- [x] Live spectrum
- [x] Cover wall home, side seam, tools panel, player panel
- [x] Six online platforms
- [ ] Remaining lyric styles and backgrounds, visual regression tests
- [ ] Beat, chorus and vocal separation (wgpu compute + MNN)
- [ ] Command palette, global shortcuts, Discord presence, wallpaper mode, video export
- [ ] Installer and auto-update

## Contributing

Please read [CONTRIBUTING.en.md](CONTRIBUTING.en.md) before contributing. It covers code standards, commit requirements and the PR process.

## Acknowledgements

- **[Folia](https://github.com/chthollyphile/folia-major)** by chthollyphile. The interface, animations, interactions and many of the values in Folia-rust come from the original. This project would not exist without it.
- Reference implementations of the platform protocols: [NeteaseCloudMusicApi Enhanced](https://github.com/NeteaseCloudMusicApiEnhanced/api-enhanced), [KuGouMusicApi](https://github.com/MakcRe/KuGouMusicApi), [qq-music-api](https://github.com/yakult-green-tea/qq-music-api), [bodian-music-api](https://github.com/Mintcolour/bodian-music-api), [qishui-music-api](https://github.com/GSQZ/qishui-music-api), [librespot](https://github.com/librespot-org/librespot).
- [iced](https://github.com/iced-rs/iced), [wgpu](https://github.com/gfx-rs/wgpu), [cosmic-text](https://github.com/pop-os/cosmic-text), [symphonia](https://github.com/pdeljanov/Symphonia), [cpal](https://github.com/RustAudio/cpal), and the authors of every dependency.

## License

[GNU Affero General Public License v3.0](../LICENSE) (`AGPL-3.0-only`).

Folia-rust ports the interface design, animation algorithms, shaders, text and assets of [folia-major](https://github.com/chthollyphile/folia-major) (AGPL-3.0). It is a derivative work and is therefore also released under AGPL-3.0. Copyright of the original project belongs to its authors. When you distribute this program, including test builds sent to others, you must also make the complete corresponding source code available to the recipients.

The platform icons under `assets/platforms/` are trademarks of their respective music platforms and owners. They are not covered by AGPL-3.0 and are used only to identify those platforms. This project is not affiliated with any of these platforms.

`assets/nebbia/nebbia-strokes.bin.gz` is a separate data file generated from [AnimCJK](https://github.com/parsimonhi/animCJK) stroke data and distributed under the Arphic Public License; see `NOTICE.txt`, `COPYING.txt` and `ARPHICPL.TXT` in the same folder. It is not covered by AGPL-3.0. `assets/nebbia/nebbiaRaindrop.png` comes from SardineFish's raindrop-fx under the MIT license; see the license file in the same folder. `crates/cryoglyph/` is a copy of the iced text rendering library [cryoglyph](https://github.com/iced-rs/cryoglyph) 0.1.0 (MIT, Apache-2.0 or Zlib; license files are in that folder). Only the coverage of dark text in its shader is changed; see `crates/cryoglyph/PATCH.md`.
