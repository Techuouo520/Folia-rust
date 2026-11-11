简体中文 | [English](docs/README.en.md)

<div align="center">

<img src="assets/icon.png" width="128" alt="Folia-rust">

# Folia-rust

**让歌词在墙上流动，而不是在内存里堆积。**

[Folia](https://github.com/chthollyphile/folia-major) 歌词可视化音乐播放器的纯原生 Rust 重写。沿用原版的界面与动画设计，换掉 Electron，换上 GPU 直绘。

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-c2410c)](LICENSE)
[![Rust 1.99](https://img.shields.io/badge/rust-1.99.1-b7410e?logo=rust)](rust-toolchain.toml)
[![iced 0.14](https://img.shields.io/badge/UI-iced%200.14-3b82f6)](https://github.com/iced-rs/iced)
[![wgpu](https://img.shields.io/badge/GPU-wgpu%20%2F%20WGSL-6d28d9)](https://github.com/gfx-rs/wgpu)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-475569)

</div>

---

## 简介

原版 Folia 基于 Electron，运行时带着完整的 Chromium 与 Node.js，在线平台还要在本机另起 HTTP 服务。Folia-rust 把它重写成一个原生程序：

- **单个二进制**：没有浏览器、没有 Node、没有本地服务，平台协议在进程内完成。
- **GPU 直绘**：界面与歌词动画由 iced + wgpu 绘制，着色器为 WGSL。
- **不动不画**：没有动画时停止帧循环，空闲时不占 CPU。

安卓端见 [Folia-rust-android](https://github.com/Techuouo520/Folia-rust-android)，与本项目共用 Rust 核心，开发中。

## 特性

- **洛雪自定义音源**：粘贴下载链接或从文件导入 LX Music 脚本，支持多个音源独立启停、优先级排序与网易云、酷狗、QQ 音乐；逐源尝试、音质降级，全部失败后回落平台接口。
- **封面墙首页**：无限延展的封面墙，拖动带惯性；书脊式侧缝可展开、收起。
- **歌词可视化**：歌词样式从原版逐个移植，随音乐实时律动。
- **歌词样式工作台**：实时预览，按样式调节参数。
- **在线平台**：网易云、酷狗、QQ 音乐、波点、汽水、Spotify，支持扫码登录、多账号、歌单、电台、探索页、逐字歌词与音质选择。
- **本地音乐**：扫描、元数据、资料库、目录监视。
- **多语言**：简体中文、繁体中文、English、Bahasa Indonesia、Tiếng Việt。
- **凭据安全**：cookie、token 只存系统密钥库。

## 歌词样式

| 样式 | 标识 | 状态 |
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
| 其余原版样式 | — | 🚧 移植中 |

## 在线平台

| 平台 | 搜索 | 播放 | 歌词 | 登录 | 歌单 / 电台 / 探索 |
|---|:---:|:---:|:---:|:---:|:---:|
| 网易云音乐 | ✅ | ✅ | ✅ | 扫码 | ✅ |
| 酷狗音乐 | ✅ | ✅ | ✅ 逐字 KRC | 扫码 | ✅ |
| QQ 音乐 | ✅ | ✅ | ✅ 逐字 QRC | 扫码 | ✅ |
| 波点音乐 | ✅ | ✅ | ✅ 逐字 | 扫码 | ✅ |
| 汽水音乐 | ✅ | ✅ | ✅ | 扫码 | ✅ |
| Spotify | ✅ | Premium | ✅ 逐行 | 浏览器 | ✅ |
| YouTube Music | 🚧 | — | — | — | 默认不编译 |

> 会员歌曲需要对应平台的会员账号。YouTube Music 的音频需要执行 Google 的 BotGuard 脚本，在不内嵌 JS 引擎的前提下暂时做不到，所以默认不编译。

## 构建

需要 Rust 1.99.1（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 工具链。

```bash
git clone https://github.com/Techuouo520/Folia-rust.git
cd Folia-rust
cargo build --release -p folia-app
```

产物为 `target/release/folia`（Windows 上为 `folia.exe`），运行时需要同目录下的 `assets/`。

## 路线图

- [x] 骨架：窗口、设置、语言切换
- [x] 本地音乐扫描与播放
- [x] 歌词解析，九个歌词样式，歌词样式工作台
- [x] 实时频谱
- [x] 封面墙首页、侧缝、工具面板、播放器面板
- [x] 六个在线平台
- [ ] 其余歌词样式与背景、视觉回归测试
- [ ] 节拍、副歌、人声分离（wgpu compute + MNN）
- [ ] 命令面板、全局快捷键、Discord 状态、壁纸模式、视频导出
- [ ] 安装包与自动更新

## 贡献

参与开发前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，其中包含代码规范、提交要求与 PR 流程。

## 致谢

- **[Folia](https://github.com/chthollyphile/folia-major)**，作者 chthollyphile。Folia-rust 的界面、动画、交互和大量数值都来自原版，没有它就没有这个项目。
- 各平台协议的参考实现：[NeteaseCloudMusicApi Enhanced](https://github.com/NeteaseCloudMusicApiEnhanced/api-enhanced)、[KuGouMusicApi](https://github.com/MakcRe/KuGouMusicApi)、[qq-music-api](https://github.com/yakult-green-tea/qq-music-api)、[bodian-music-api](https://github.com/Mintcolour/bodian-music-api)、[qishui-music-api](https://github.com/GSQZ/qishui-music-api)、[librespot](https://github.com/librespot-org/librespot)。
- [iced](https://github.com/iced-rs/iced)、[wgpu](https://github.com/gfx-rs/wgpu)、[cosmic-text](https://github.com/pop-os/cosmic-text)、[symphonia](https://github.com/pdeljanov/Symphonia)、[cpal](https://github.com/RustAudio/cpal)，以及所有依赖的作者。

## 许可证

[GNU Affero General Public License v3.0](LICENSE)（`AGPL-3.0-only`）。

Folia-rust 移植了 [folia-major](https://github.com/chthollyphile/folia-major)（AGPL-3.0）的界面设计、动画算法、着色器、文案和资源，属于它的衍生作品，因此同样以 AGPL-3.0 发布。原项目的版权归其作者所有。分发本程序（包括发给他人的测试版）时，需要同时让接收者能获得对应的完整源代码。

`assets/platforms/` 下的平台图标是各音乐平台的商标，归各自所有者，不在 AGPL-3.0 授权范围内，仅用于标识对应平台。本项目与这些平台没有任何关联。

`assets/nebbia/nebbia-strokes.bin.gz` 是由 [AnimCJK](https://github.com/parsimonhi/animCJK) 笔画数据生成的独立数据文件，按 Arphic Public License 分发，授权文本见同目录的 `NOTICE.txt`、`COPYING.txt`、`ARPHICPL.TXT`，不在 AGPL-3.0 授权范围内。`assets/nebbia/nebbiaRaindrop.png` 来自 SardineFish 的 raindrop-fx，MIT 许可，见同目录的授权文件。`crates/cryoglyph/` 是 iced 文字渲染库 [cryoglyph](https://github.com/iced-rs/cryoglyph) 0.1.0 的副本（MIT、Apache-2.0 或 Zlib，授权文件在该目录），只改了着色器里深色文字的覆盖度，说明见 `crates/cryoglyph/PATCH.md`。
