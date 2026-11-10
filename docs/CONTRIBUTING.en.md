[简体中文](../CONTRIBUTING.md) | English

# Contributing Guide

Thanks to everyone who contributes code, documentation, issue reports and feedback to Folia-rust.

This document defines the contribution standards for Folia-rust. The key words are used as in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119): **MUST** and **MUST NOT** are hard requirements, and a PR that does not meet them will not be merged; **SHOULD** and **SHOULD NOT** are default requirements, and any exception must be justified in the PR; **MAY** is a recommendation.

## Principles

- Interfaces and animations ported from [Folia](https://github.com/chthollyphile/folia-major) **SHOULD** match the original in layout, font size, color, animation duration and easing.
- This project may add features the original does not have, and may leave some original features unported. Adding or removing features, changing the architecture and adding dependencies **SHOULD** start with an issue to agree on the direction. Small fixes and documentation corrections can go straight to a PR.
- A PR covers one topic only and **MUST NOT** include unrelated refactoring or formatting.

## Code quality

All contributors and maintainers **SHOULD** weigh code quality in this order, from most to least important:

1. **Robustness**: invalid input, network failures, missing files and similar cases are handled correctly, without crashes or data loss.
2. **Readability**: accurate names and a clear structure, so others can understand the code without the author's explanation.
3. **Maintainability**: a change in one place does not affect unrelated modules, and problems are easy to locate and fix.
4. **Efficiency**: no unnecessary computation, memory allocation or redrawing.
5. **Reusability**: each piece of logic is implemented once and called from wherever it is needed.
6. **Extensibility**: adding features of an existing kind (such as a style or a platform) only adds code and does not change existing implementations.

When two of these conflict, the one ranked higher **SHOULD** win.

Reusability and extensibility are both built on **decoupling**: modules interact only through public interfaces and do not depend on each other's internals. The project's architecture is already divided this way (see [Architecture](#architecture)), so following the module boundaries usually satisfies both, which is why they rank last. Decoupling itself **MUST** be preserved.

## Development environment

- The toolchain is pinned by `rust-toolchain.toml` and **MUST NOT** be changed in a PR. Windows uses the MSVC toolchain.
- Build with `cargo build --workspace`, run with `cargo run -p folia-app`, and make a release build with `cargo build --release -p folia-app`.
- Development-only code (debug panels, performance overlays, fake data) **MUST** be gated behind the cargo feature `dev`.

## Architecture

Dependencies in the workspace go one way, from top to bottom. A lower crate **MUST NOT** depend on a higher one:

| Crate | Responsibility |
|---|---|
| `folia-app` | Program entry point; assembly only |
| `folia-ui` | Pages, widgets, themes, settings |
| `folia-visualizer` | Lyric animation styles and backgrounds |
| `folia-render` | wgpu device, frame scheduling, post-processing, text atlas |
| `folia-player` | Decoding, output, queue, volume, equalizer |
| `folia-lyrics` | Lyric parsing, word timing, line breaking |
| `folia-omni` | Unified entry point for online platforms and the `Provider` trait |
| `folia-library` | Local music, playlists, SQLite index |
| `folia-store` | Settings, persistence, credentials |
| `folia-i18n` | Language packs |
| `folia-core` | Core data model; depends on no other folia crate |

- Online features **MUST** go through the facade of `folia-omni`. Raw platform interfaces and encryption algorithms may appear only in `providers/<platform>/`. When omni lacks a capability, add it to the trait and the facade first; omni **MUST NOT** be bypassed.
- A new style only touches `styles/<style>/`, `shaders/<style>/` and the registry; a new platform only touches `providers/<platform>/`, the feature of the same name and the registry.
- Modules use only each other's public interfaces. Each piece of logic is written once, in the lowest crate that owns the concept.

## Code standards

### Formatting and lints

- Code **MUST** pass `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
- The workspace forbids `unsafe_code`, `unwrap_used`, `expect_used`, `panic`, `dbg_macro`, `print_stdout` and `print_stderr`. These **MUST NOT** be relaxed, and **SHOULD NOT** be bypassed with `#[allow]`.
- Text files **MUST** use LF line endings.

### Naming and structure

- Modules and files use `snake_case`, types `UpperCamelCase`, constants `SCREAMING_SNAKE_CASE`. Style and platform folders keep the original ids.
- Vague module names such as `utils`, `helpers`, `common` and `manager` **MUST NOT** be used.
- Parameters, configuration, traits, generics and abstraction layers reserved for the future **MUST NOT** be added. Introduce a trait only when it has at least two implementations.
- Nesting **SHOULD NOT** exceed 3 levels, and functions **SHOULD NOT** exceed about 50 lines.
- Commented-out code, `TODO`s and empty implementations **MUST NOT** be committed.

### Error handling

- Library crates use `thiserror`; `folia-app` may use `anyhow`. Propagate errors with `?` and add context with `map_err`.
- Errors **MUST** be propagated or logged, and **MUST NOT** be silently swallowed.

### unsafe

- Allowed only in FFI bindings and where GPU or audio interop requires it. Every occurrence **MUST** have a one-line `// SAFETY:` comment.

### Threads

- The UI thread and the render thread **MUST NOT** perform network access, disk IO or decoding.
- The audio callback thread **MUST NOT** allocate memory, take locks or log.
- Mutable state **SHOULD NOT** be shared across threads. When sharing is necessary, use `Arc` with a lock-free structure or `parking_lot`.

### Rendering

- The frame loop **MUST** stop when nothing animates.
- Each frame **MUST NOT** allocate heap memory or rebuild pipelines and bind groups.
- Shaders are written in WGSL only.

### Logging

- Use `tracing` only. Record errors as `error = &err as &dyn std::error::Error` to keep the error chain.

### Dependencies

- New dependencies **SHOULD NOT** be added when the standard library or existing dependencies are enough.
- Versions **MUST** be declared in `[workspace.dependencies]` of the root `Cargo.toml`. Dependencies related to music platforms are pinned to exact versions.
- Licenses **MUST** be compatible with AGPL-3.0. Third-party crates **MUST NOT** be forked or patched.

### UI and internationalization

- Icons come from lucide only.
- Interface text **MUST** go through `folia-i18n`. Text that exists in the original keeps the original key names. A new key **MUST** be added to all four language packs: `en`, `zh-CN`, `in` and `vi`. Traditional Chinese is generated at build time and **MUST NOT** be written by hand.

### Persistence

- Setting keys that have been released **MUST NOT** be renamed. Structural changes **MUST** bump the schema version and provide a migration.
- Credentials **MUST** be stored only in the system keyring, and **MUST NOT** appear in settings files, logs or the interface.

### Cross-platform

- Paths **MUST** be handled with `std::path`. Hard-coded drive letters, separators and absolute paths **MUST NOT** be used.

## Comments

- Every feature and every standalone module (file) **MUST** have a comment describing its purpose and responsibility. Comments inside functions are added as needed.
- Comments **MUST** be concise and explain why, not restate the code. Constants and formulas ported from the original **SHOULD** cite their source in the original.
- Language: comments **SHOULD** be in English and **MAY** be in Simplified Chinese. Other languages **MUST NOT** be used. A single file **SHOULD** use one language.
- Format: single-line or multi-line `//` comments are allowed; doc comments for public APIs use `///`. `/* */` block comments and decorative separators (such as `// ======`) **MUST NOT** be used.

```rust
// Seek bar under the capsule, from `FloatingPlayerControls.tsx`.

/// Width of the drag hotspot, from `PanelHandle.tsx`.
const RAIL_WIDTH: f32 = 96.0;

// The original reads the analyser once per animation frame,
// and its 0.6 smoothing assumes that rate.
let frame_tick = matches!(message, Message::Tick);
```

## AI-assisted development

AI-assisted programming is allowed. Whoever wrote the code, the submitter is responsible for all of it, **MUST** be able to explain every change, and must ensure the code is robust.

- When all or most of the code is generated by AI, the submitter **MUST** review its readability section by section: whether names are accurate, whether the structure is clear, and whether there is redundant code or unnecessary abstraction.
- Besides the `rustfmt` and `clippy` checks required in [Before submitting](#before-submitting), contributors **MAY** use other static analysis tools when available.

When writing code with AI, flagship models **SHOULD** be used:

| Vendor | Model |
|---|---|
| Anthropic | Claude Opus 5.5 |
| DeepSeek | Current flagship version |
| OpenAI | GPT-6 Astra (the only recommended GPT model) |

- Before submitting, the submitter **MUST** review the full diff personally and verify the feature by actually running it.
- The PR **SHOULD** name the tools and models used.
- PRs opened by automated systems are closed.

## Commit messages

- Commit messages **MUST** be in English. The title is at most 50 characters, in the imperative mood, capitalized, with no trailing period.
- The body states which module, interface, data format or algorithm changed. Vague descriptions such as `fix bug` or `update code` **MUST NOT** be used.
- Commits **SHOULD** be signed.

```text
Snap lucide icons to whole pixels

icons.rs Glyph widget rounds origin; iced half-even edges shrank x.5 icons.
```

## Before submitting

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

- Tests live in `test/` at the repo root, a separate cargo workspace; `crates/` holds no tests. Tests that need the network are marked `#[ignore]` and run locally with `--ignored`.
- CI runs formatting, lints and tests on every push and PR; the all-architecture build is triggered manually in GitHub Actions.
- UI changes **SHOULD** include screenshots; for ported interfaces, ideally side by side with the original at the same spot.
- Performance-related changes **SHOULD** include measurements from a release build.

## Issues and PRs

- An issue states the version, system, reproduction steps, and expected and actual behavior. Visual issues in ported parts include a comparison screenshot of the original.
- A PR description states the problem solved, the approach and its trade-offs, differences from the original, how it was verified and the results, and what has not been verified.
- Accounts, cookies, tokens and API keys **MUST NOT** be disclosed in issues, PRs, logs or screenshots.

## License

This project is ported from [Folia](https://github.com/chthollyphile/folia-major) and released under [AGPL-3.0](../LICENSE). By submitting a contribution, you agree to license it under AGPL-3.0.
