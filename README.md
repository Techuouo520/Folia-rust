<div align="center">

<img src="assets/icon.png" width="128" alt="Folia-rust">

# Folia-rust

**让歌词在墙上流动，而不是在内存里堆积。**

[Folia](https://github.com/chthollyphile/folia-major) 歌词可视化音乐播放器的纯原生 Rust 重写。界面、动画、交互都按原版来，换掉 Electron，换上 GPU 直绘。

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-c2410c)](LICENSE)
[![Rust 1.99](https://img.shields.io/badge/rust-1.99.1-b7410e?logo=rust)](rust-toolchain.toml)
[![iced 0.14](https://img.shields.io/badge/UI-iced%200.14-3b82f6)](https://github.com/iced-rs/iced)
[![wgpu](https://img.shields.io/badge/GPU-wgpu%20%2F%20WGSL-6d28d9)](https://github.com/gfx-rs/wgpu)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-475569)

</div>

---

## 为什么要重写

原版 Folia 的设计无可挑剔，问题出在它脚下的技术栈：

- **一个播放器，背着一整个浏览器**：Electron 把 Chromium 和 Node.js 一起打进安装包。还没放歌，内存和磁盘就先被浏览器吃掉一大块，启动也要等浏览器起来。
- **动画要穿过好几层**：歌词动画从 React 状态到 DOM，再到 CSS、WebGL，最后交给浏览器合成器。每一层都有开销，JS 的垃圾回收还会时不时卡一下帧。
- **安静时也停不下来**：浏览器的渲染循环和各种定时器一直在跑，什么都不动时也在消耗 CPU 和 GPU。
- **在线平台要另起服务**：网易云、QQ 音乐等要在本机起 Node HTTP 服务、加载第三方 npm 包才能工作，进程多、依赖链长，上游一变就容易出问题。

Folia-rust 换掉了整个地基：

- **一个原生二进制**，没有浏览器，没有 Node，没有本地服务。
- **GPU 直绘**：界面和歌词动画由 iced + wgpu 直接画，着色器是 WGSL，从数据到屏幕只有一层。
- **不动就不画**：没有动画时不跑帧循环，空闲时 CPU 归零。
- **平台协议用 Rust 重写**：签名、加密、请求都在进程内完成，统一走一个 `Omni` 门面。

## 特性

- **封面墙首页**：无限延展的 Bravais 封面墙，拖动带惯性，滚轮平滑滑行；书脊式侧缝可展开、收起，切换页签时整墙翻转入场。
- **歌词可视化**：歌词样式逐个从原版移植，GLSL 改写为 WGSL，参数照搬原版。每一帧都只由播放时间决定，拖动进度不会跳帧。
- **歌词样式工作台**：实时预览，逐个样式调节参数，调节项由各样式自己声明。
- **六个在线平台**：一个统一的 `Omni` 门面接入网易云、酷狗、QQ 音乐、波点、汽水、Spotify，支持扫码登录、多账号切换、歌单、电台、探索页、逐字歌词和音质选择。
- **本地音乐**：扫描、元数据、SQLite 资料库、目录监视。
- **原生播放引擎**：symphonia 解码、cpal 输出、rubato 重采样。
- **五种界面语言**：简体中文、繁体中文（构建时自动生成）、English、Bahasa Indonesia、Tiếng Việt。
- **凭据只进系统密钥库**：cookie、token 不写进设置文件、日志和界面。

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
| 其余原版样式 | — | 🚧 按代码量逐个移植 |

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

需要 Rust **1.99.1**（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 工具链。

```bash
# 开发版：带控制台，日志默认 debug，数据目录 Folia-rust-dev
cargo run -p folia-app

# 发布版：开启 LTO，数据目录 Folia-rust
cargo build --release -p folia-app
```

产物是 `target/release/folia`（Windows 上是 `folia.exe`）。日志级别可以用环境变量 `FOLIA_LOG` 覆盖，语法同 `tracing-subscriber` 的 `EnvFilter`。

## 架构

一个 Cargo workspace，依赖自上而下单向：

```
folia-app            组装：日志、设置、语言、窗口
└─ folia-ui          页面、控件、主题、封面墙、登录、设置
   ├─ folia-visualizer   歌词样式与背景（iced canvas + WGSL）
   ├─ folia-render       wgpu 设施：模糊、颗粒、后期
   ├─ folia-player       播放引擎：解码、输出、队列、音量
   ├─ folia-omni         在线平台统一门面，每个平台一个 feature
   ├─ folia-library      本地扫描、元数据、SQLite 资料库
   ├─ folia-lyrics       歌词解析（LRC / YRC / TTML …）与断行
   ├─ folia-store        按领域分文件的设置、版本迁移
   └─ folia-i18n         语言包
      └─ folia-core      核心数据模型，不依赖任何 folia crate
```

## 设计原则

- **模块之间只认公开接口**：crate 之间不读写彼此的内部状态，跨模块只传 `folia-core` 的数据类型或消息。
- **一份逻辑只写一处**：被多处用到的逻辑，只在拥有这个概念的最底层模块里实现一次，其他地方都调用它。
- **扩展只动自己的地盘**：新增歌词样式或音乐平台，只加它自己的目录和一行注册。
- **平台的规则留在平台里**：签名、加密、原始接口只出现在各自平台的目录里，看着相似也不合并。
- **依赖单向，状态靠消息**：上层依赖下层；不跨线程共享可变状态，UI 线程不做网络、磁盘和解码。
- **只在有变化时重绘**：没有动画时不跑帧循环，每帧不分配堆内存。
- **视觉以原版为准**：尺寸、颜色、字重、动画时长都从原版取数，不凭感觉。

### 为什么解耦

- **能一块一块地搬**：每个模块单独编译、单独测试、单独合并，大项目才能逐步迁完。
- **改一处，全处生效**：队列列表、平台图标、字体匹配都只有一份实现，修一次，所有用到的地方一起修好。
- **换内部不动外部**：界面只认 `Omni` 门面，某个平台的协议变了，只改那个平台的目录。
- **多人并行不打架**：边界清楚，不同模块可以在不同分支上同时开发，六个在线平台就是这样并行接入的。

## 路线图

- [x] 骨架：窗口、设置、语言切换
- [x] 本地音乐扫描与播放
- [x] 歌词解析，七个歌词样式，歌词样式工作台
- [x] 封面墙首页、侧缝、工具面板、播放器面板
- [x] 六个在线平台
- [ ] 其余歌词样式与背景、视觉回归测试
- [ ] 节拍、副歌、人声分离（wgpu compute + MNN）
- [ ] 命令面板、全局快捷键、Discord 状态、壁纸模式、视频导出
- [ ] 安装包与自动更新

## 致谢

- **[Folia](https://github.com/chthollyphile/folia-major)**，作者 chthollyphile。Folia-rust 的界面、动画、交互和大量数值都来自原版，没有它就没有这个项目。
- 各平台协议的参考实现：[NeteaseCloudMusicApi Enhanced](https://github.com/NeteaseCloudMusicApiEnhanced/api-enhanced)、[KuGouMusicApi](https://github.com/MakcRe/KuGouMusicApi)、[qq-music-api](https://github.com/yakult-green-tea/qq-music-api)、[bodian-music-api](https://github.com/Mintcolour/bodian-music-api)、[qishui-music-api](https://github.com/GSQZ/qishui-music-api)、[librespot](https://github.com/librespot-org/librespot)。
- [iced](https://github.com/iced-rs/iced)、[wgpu](https://github.com/gfx-rs/wgpu)、[cosmic-text](https://github.com/pop-os/cosmic-text)、[symphonia](https://github.com/pdeljanov/Symphonia)、[cpal](https://github.com/RustAudio/cpal)，以及所有依赖的作者。

## 许可证

[GNU Affero General Public License v3.0](LICENSE)（`AGPL-3.0-only`）。

Folia-rust 移植了 [folia-major](https://github.com/chthollyphile/folia-major)（AGPL-3.0）的界面设计、动画算法、着色器、文案和资源，属于它的衍生作品，因此同样以 AGPL-3.0 发布。原项目的版权归其作者所有。分发本程序（包括发给他人的测试版）时，需要同时让接收者能获得对应的完整源代码。

`assets/platforms/` 下的平台图标是各音乐平台的商标，归各自所有者，不在 AGPL-3.0 授权范围内，仅用于标识对应平台。本项目与这些平台没有任何关联。

`assets/nebbia/nebbia-strokes.bin.gz` 是由 [AnimCJK](https://github.com/parsimonhi/animCJK) 笔画数据生成的独立数据文件，按 Arphic Public License 分发，授权文本见同目录的 `NOTICE.txt`、`COPYING.txt`、`ARPHICPL.TXT`，不在 AGPL-3.0 授权范围内。`assets/nebbia/nebbiaRaindrop.png` 来自 SardineFish 的 raindrop-fx，MIT 许可，见同目录的授权文件。`crates/cryoglyph/` 是 iced 文字渲染库 [cryoglyph](https://github.com/iced-rs/cryoglyph) 0.1.0 的副本（MIT、Apache-2.0 或 Zlib，授权文件在该目录），只改了着色器里深色文字的覆盖度，说明见 `crates/cryoglyph/PATCH.md`。
