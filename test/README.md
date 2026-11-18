# 测试入口

日常开发运行受影响的用例或测试包。跨模块改动、准备合并时集中运行一次完整离线测试。纯测试精简和历史整理只验证保留或修改的回归，并核验生产源码一致性。

```powershell
cargo test --manifest-path test/Cargo.toml -p test-ui
cargo test --manifest-path test/Cargo.toml --workspace
```

默认用例保留关键行为、兼容性和真实故障回归。优先扩充已有文件，用数据表合并同类输入，复用已有覆盖。人工出图、整页遍历和一次性诊断放在本地临时验证流程，结论写入工作区schedule.md。小调整不重复运行全量、live和压力测试。

## GitHub CI

CI 在 `main`、`test-*` 分支推送和 PR 更新时运行。改动先在测试分支通过全部检查，再合入 `main`。CI 使用下列明确的特性组合，硬件 GPU 回归会标记为跳过。

```powershell
cargo test --manifest-path test/Cargo.toml --workspace --features test-ui/ci,test-render/ci,test-visualizer/ci,test-visual-lumiere/ci,test-visual-monet/ci
```

`stress` 和 `live` 是单独的本地入口，通过显式特性和 `--ignored` 启用。

## 长时间资源回归

```powershell
cargo test --release --manifest-path test/Cargo.toml -p test-ui --features stress panel_text_survives_long_monet_playback -- --ignored --nocapture --test-threads=1
cargo test --release --manifest-path test/Cargo.toml -p test-visual-lumiere --features stress atlas_over_a_whole_song -- --ignored --nocapture --test-threads=1
cargo test --release --manifest-path test/Cargo.toml -p test-visualizer --features stress nebbia_gpu_300_frames -- --ignored --nocapture --test-threads=1
```

莫奈图集回归默认运行 24000 帧，覆盖历史上第 21298 帧出现的溢出。`FOLIA_ATLAS_FRAMES` 可指定正整数用于短程检查，结束时关闭工作台，再将播放主视图与全新渲染器比较。完整时长回归应使用默认帧数。

## 真实服务检查

```powershell
cargo test --manifest-path test/Cargo.toml -p test-omni --features live -- --ignored --nocapture
```

这些用例访问真实平台，应在需要验证对应接口时按用例名称筛选运行。默认离线测试使用本地 HTTP fixture。

## 视觉参考

自动像素回归使用的 PNG、元数据和生成脚本随测试维护。字体矩阵通过 `samples.json` 中的 `reference` 字段共用完全相同的 PNG，各字号、字重、DPI 的断言继续独立执行。参考图更新需显式运行生成命令并审查变化。

完整页面人工参考图可通过 `ui/capture_surfaces.mjs` 按需生成到临时目录。对应生成方法见 `ui/fixtures/fonts/README.md`。
