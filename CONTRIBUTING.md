简体中文 | [English](docs/CONTRIBUTING.en.md)

# 贡献指南

感谢每一位为 Folia-rust 贡献代码、文档、问题报告与反馈的人。

本文是 Folia-rust 的代码贡献规范。**必须**、**禁止**为硬性要求，不满足的 PR 不会合并；**应当**、**不应**为默认要求，例外须在 PR 中说明理由；**可以**为建议。

## 原则

- 从 [Folia](https://github.com/chthollyphile/folia-major) 移植的界面与动画，布局、字号、配色、动画时长与缓动**应当**与原版一致。
- 本项目可以新增原版没有的功能，也可以不移植原版的部分功能。新增、删减功能，以及架构调整、新增依赖，**应当**先开 Issue 确认方向。小型修复和文档纠错可以直接提交 PR。
- 一个 PR 只处理一个主题，**禁止**夹带无关的重构与格式化。

## 代码质量

所有贡献者与维护者**应当**按以下顺序权衡代码质量，从最重要到次要：

1. **健壮性**：异常输入、网络失败、文件缺失等情况都能正确处理，不崩溃、不丢数据。
2. **可读性**：命名准确、结构清晰，他人不需要作者解释也能读懂。
3. **可维护性**：修改一处不牵连无关模块，问题容易定位和修复。
4. **高效性**：没有多余的计算、内存分配与重绘。
5. **可复用性**：同一逻辑只实现一次，供各处调用。
6. **可扩展性**：新增同类功能（如样式、平台）时只需添加代码，不改动已有实现。

两项冲突时，**应当**优先满足排序靠前的一项。

可复用性与可扩展性都建立在**解耦**之上：模块之间只通过公开接口交互，不依赖对方的内部实现。本项目的架构已经按此划分（见[架构](#架构)），遵守模块边界，这两项通常就能满足，所以排在最后。解耦本身**必须**保证。

## 开发环境

- 工具链由 `rust-toolchain.toml` 固定，**禁止**在 PR 中修改。Windows 使用 MSVC 工具链。
- 构建 `cargo build --workspace`，运行 `cargo run -p folia-app`，发布构建 `cargo build --release -p folia-app`。
- 开发专用代码（调试面板、性能浮层、假数据）**必须**放在 cargo feature `dev` 之后。

## 架构

workspace 内依赖方向自上而下单向，下层**禁止**依赖上层：

| crate | 职责 |
|---|---|
| `folia-app` | 程序入口，只做组装 |
| `folia-ui` | 页面、控件、主题、设置页 |
| `folia-visualizer` | 歌词动画样式与背景 |
| `folia-render` | wgpu 设备、帧调度、后期处理、文字图集 |
| `folia-player` | 解码、输出、队列、音量、均衡器 |
| `folia-lyrics` | 歌词解析、逐字时序、断行 |
| `folia-omni` | 在线平台统一入口与 `Provider` trait |
| `folia-library` | 本地音乐、歌单、SQLite 索引 |
| `folia-store` | 设置、持久化、凭据 |
| `folia-i18n` | 语言包 |
| `folia-core` | 核心数据模型，不依赖其他 folia crate |

- 在线功能**必须**经 `folia-omni` 的门面接口；平台原始接口与加密算法只能出现在 `providers/<平台>/`。omni 缺能力时先补 trait 与门面，**禁止**绕过。
- 新样式只改 `styles/<样式>/`、`shaders/<样式>/` 与注册表；新平台只改 `providers/<平台>/`、同名 feature 与注册表。
- 模块之间只用对方的公开接口。同一逻辑只写一份，放在拥有该概念的最底层 crate。

## 代码规范

### 格式与 Lint

- **必须**通过 `cargo fmt --all --check` 与 `cargo clippy --workspace --all-targets -- -D warnings`。
- workspace 已禁止 `unsafe_code`、`unwrap_used`、`expect_used`、`panic`、`dbg_macro`、`print_stdout`、`print_stderr`，**禁止**放宽；**不应**用 `#[allow]` 绕过。
- 文本文件**必须**使用 LF 换行。

### 命名与结构

- 模块与文件 `snake_case`，类型 `UpperCamelCase`，常量 `SCREAMING_SNAKE_CASE`；样式与平台目录沿用原版 id。
- **禁止**使用 `utils`、`helpers`、`common`、`manager` 等含糊的模块名。
- **禁止**为未来预留参数、配置、trait、泛型与抽象层；trait 至少有两个实现才引入。
- 嵌套**不应**超过 3 层，函数**不应**超过约 50 行。
- **禁止**提交注释掉的代码、`TODO` 与空实现。

### 错误处理

- 库 crate 用 `thiserror`，`folia-app` 可用 `anyhow`。错误用 `?` 传递、`map_err` 补上下文。
- 错误**必须**传递或记录，**禁止**静默吞掉。

### unsafe

- 只允许出现在 FFI 绑定与必要的 GPU、音频互操作处，每一处**必须**附一行 `// SAFETY:` 说明。

### 线程

- UI 线程与渲染线程**禁止**做网络、磁盘 IO 与解码。
- 音频回调线程**禁止**分配内存、加锁、打日志。
- 跨线程**不应**共享可变状态；确需共享时用 `Arc` 加无锁结构或 `parking_lot`。

### 渲染

- 没有动画时**必须**停止帧循环。
- 每帧**禁止**分配堆内存、重建管线与绑定组。
- 着色器只用 WGSL。

### 日志

- 只用 `tracing`。错误记作 `error = &err as &dyn std::error::Error`，保留错误链。

### 依赖

- 能用标准库与已有依赖解决的，**不应**新增依赖。
- 版本**必须**在根 `Cargo.toml` 的 `[workspace.dependencies]` 统一声明；音乐平台相关依赖钉死版本。
- 许可证**必须**与 AGPL-3.0 兼容。**禁止**分叉或修补第三方 crate。

### 界面与国际化

- 图标只用 lucide。
- 界面文字**必须**走 `folia-i18n`，原版已有的文案沿用原版的键名；新增键**必须**同时加到 `en`、`zh-CN`、`in`、`vi` 四份语言包。繁体中文由构建生成，**禁止**手写。

### 持久化

- 已发布的设置键名**禁止**改名；结构变更**必须**递增 schema 版本并提供迁移。
- 凭据**必须**只存系统密钥库，**禁止**出现在设置文件、日志与界面中。

### 跨平台

- 路径**必须**用 `std::path` 处理，**禁止**写死盘符、分隔符与绝对路径。

## 注释规范

- 每个功能、每个独立的模块（文件）**必须**有注释，说明它的用途与职责。函数内部的注释按需添加。
- 注释**必须**简洁明了，写“为什么”，**禁止**复述代码。照原版移植的常数与公式**应当**注明原版出处。
- 语言：**应当**用英文，可以用简体中文，**禁止**其他语言；同一文件**应当**统一。
- 格式：可以用单行或多行 `//`；公开 API 的文档注释用 `///`。**禁止** `/* */` 块注释与装饰性分割线（如 `// ======`）。

```rust
// Seek bar under the capsule, from `FloatingPlayerControls.tsx`.

/// Width of the drag hotspot, from `PanelHandle.tsx`.
const RAIL_WIDTH: f32 = 96.0;

// The original reads the analyser once per animation frame,
// and its 0.6 smoothing assumes that rate.
let frame_tick = matches!(message, Message::Tick);
```

## AI 辅助开发

本项目允许使用 AI 编程。无论代码由谁编写，提交者都对全部内容负责，**必须**能解释每一处改动，并确保代码的健壮性。

- 代码全部或大部分由 AI 生成时，提交者**必须**逐段检查可读性：命名是否准确、结构是否清晰、有无冗余代码与多余的抽象。
- 除[提交前检查](#提交前检查)要求的 `rustfmt` 与 `clippy` 外，有条件的**可以**再用其他静态分析工具检查代码。

使用 AI 写代码时**应当**使用旗舰模型：

| 厂商 | 型号 |
|---|---|
| Anthropic | Claude Opus 5.5 |
| DeepSeek | 当前旗舰版本 |
| OpenAI | GPT-6 Astra（GPT 系列仅限此型号） |

- 提交前**必须**亲自审查完整 diff，并实际运行验证功能。
- PR 中**应当**注明使用的工具与型号。
- 自动化系统发起的 PR 一律关闭。

## 提交信息

- **必须**用英文。标题不超过 50 个字符，祈使句，首字母大写，不加句号。
- 正文写改了哪个模块、接口、数据格式或算法，**禁止** `fix bug`、`update code` 之类的空泛描述。
- 提交**应当**签名。

```text
Snap lucide icons to whole pixels

icons.rs Glyph widget rounds origin; iced half-even edges shrank x.5 icons.
```

## 提交前检查

```bash
cargo fmt --all --check
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo build --workspace
```

```bash
cargo test --manifest-path test/Cargo.toml
```

- 测试在仓库根目录的 `test/`，是独立的 cargo workspace，`crates/` 里不放测试。需要联网的测试标了 `#[ignore]`，只在本地用 `--ignored` 跑。
- 每次推送与 PR 都会由 CI 跑格式、Lint 与测试；全架构构建在 GitHub Actions 里手动触发。
- 界面改动**应当**附截图；移植的界面最好与原版同一位置对比。
- 性能相关的改动**应当**附 release 构建下的测量数据。

## Issue 与 PR

- Issue 写明版本、系统、复现步骤、预期与实际行为；移植部分的视觉问题附原版对比截图。
- PR 描述写明：解决的问题、方案与取舍、与原版的差异、验证方式与结果、未验证的部分。
- **禁止**在 Issue、PR、日志或截图中公开账号、Cookie、Token、API Key。

## 许可证

本项目移植自 [Folia](https://github.com/chthollyphile/folia-major)，以 [AGPL-3.0](LICENSE) 发布。提交贡献即同意以 AGPL-3.0 授权你的贡献。
