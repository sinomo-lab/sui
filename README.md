# SUI

SUI is a retained-mode UI toolkit for Rust. It combines a renderer-neutral
scene model, a `wgpu` renderer, accessible built-in widgets, deterministic UI
testing, and native Python and Node/Electron bindings in one workspace.

> **Release status:** SUI is pre-release software. The Rust desktop API and
> testing stack are usable today, while browser, mobile, Python, JavaScript,
> and native HDR support have the limitations listed below. The crates and
> language packages have not been published yet, so API and package names may
> still change before the first stable release.

## Quick start

SUI requires Rust 1.90 or newer and the system libraries normally needed by
`winit` and `wgpu` on your platform.

Until the first crates.io release, add the repository dependency:

```toml
[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

The package is named `sinomo-ui` because the `sui` registry namespace is
occupied. Aliasing it as `sui` keeps application imports concise.

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("Hello SUI", Label::new("Ready"))
        .run()
}
```

From a workspace checkout, run the same program with:

```bash
cargo run -p sinomo-ui --example hello
```

Continue with the detailed [first-application tutorial](https://github.com/sinomo-lab/sui/blob/main/docs/tutorials/quickstart.md),
then build an editable [stateful form](https://github.com/sinomo-lab/sui/blob/main/docs/tutorials/stateful-form.md).

## What is included

- Retained widgets with explicit measure, arrange, event, paint, and semantics
  passes.
- Common controls, form inputs, layout containers, composite application
  widgets, and the built-in Mesh light, dark, high-contrast, void/OLED, and
  touch themes.
- Editable text, text areas, password fields, and local date/time fields with
  caret movement, selection, clipboard operations, and IME input.
- A renderer-neutral scene representation and retained `wgpu` compositor.
- AccessKit-backed accessibility plus an accessibility-tree generated TUI.
- Headless automation, semantic locators, screenshots, and visual artifact
  generation.
- Native Python and Node/Electron bindings for the shared high-level model.

The [widget and layout reference](https://github.com/sinomo-lab/sui/blob/main/docs/api/widgets-and-layout.md) lists the
public building blocks and points to focused API guides.

## Platform status

| Surface | Status | Notes |
| --- | --- | --- |
| Rust desktop | Available | `winit` + `wgpu` on Linux, macOS, and Windows |
| Headless/testing | Available | Deterministic runtime, semantics, rendering, and screenshots |
| Accessibility TUI | Available | Generated from the same semantic tree as assistive technologies |
| Rust browser | Alpha | WebAssembly demo through the `web` feature and Trunk |
| Rust Android | Experimental | Native-activity entry point through the `mobile` feature |
| Python | Alpha | Local PyO3/maturin build; packages are not published |
| Node/Electron | Alpha | Local napi-rs build; packages are not published |
| Browser JavaScript | Planned | No JavaScript/WASM package yet |

Native HDR output is currently strongest on Windows. The renderer, color
management, diagnostics, and SDR fallback paths are cross-platform; macOS EDR
and broader Linux native-HDR integration remain roadmap work.

## Learn by example

| Example | Run it | Demonstrates |
| --- | --- | --- |
| Hello | `cargo run -p sinomo-ui --example hello` | Minimal application and window |
| Quickstart | `cargo run -p sinomo-ui --example quickstart` | Layout, theming, and callbacks |
| Stateful form | `cargo run -p sinomo-ui --example stateful_form` | External state and editable inputs |
| Widget book | `cargo run -p sui-demo` | Built-in widgets, themes, renderer settings, and demos |
| TUI | `cargo run -p sui-demo -- --tui` | Keyboard-driven semantic-tree interface |

See the [complete examples catalog](https://github.com/sinomo-lab/sui/blob/main/docs/examples.md) for Rust, testing,
Python, and JavaScript examples.

## Feature flags

The `sinomo-ui` package enables `desktop` and `wgpu` by default.

| Feature | Purpose |
| --- | --- |
| `desktop` | Desktop event loop and window integration; also enables `wgpu` |
| `web` | Browser platform integration; also enables `wgpu` |
| `mobile` | Mobile platform integration; also enables `wgpu` |
| `wgpu` | Public `wgpu` renderer facade |
| `testing` | Reserved facade feature for test-oriented consumers |

Custom embedders can disable default features and opt into the layers they
need. Programs that call `App::run()` need a platform feature.

## Documentation

The [documentation index](https://github.com/sinomo-lab/sui/blob/main/docs/README.md) separates learning material, API
reference, internals, and active roadmap work.

- [Tutorials](https://github.com/sinomo-lab/sui/blob/main/docs/tutorials/README.md) — build a first app and a stateful
  form step by step.
- [API guide](https://github.com/sinomo-lab/sui/blob/main/docs/api/README.md) — application lifecycle, widgets, layout,
  inputs, resources, custom widgets, testing, and accessibility.
- [Examples](https://github.com/sinomo-lab/sui/blob/main/docs/examples.md) — runnable Rust, Python, and JavaScript samples.
- [Testing](https://github.com/sinomo-lab/sui/blob/main/docs/testing.md) — semantic automation, screenshots, and artifacts.
- [TUI](https://github.com/sinomo-lab/sui/blob/main/docs/tui.md) — run and embed the accessibility-generated terminal UI.
- [Architecture](https://github.com/sinomo-lab/sui/blob/main/docs/architecture.md) — understand the retained runtime and
  frame pipeline.
- [Contributing](https://github.com/sinomo-lab/sui/blob/main/CONTRIBUTING.md) — build, validate, document, and submit
  changes.
- [Security policy](https://github.com/sinomo-lab/sui/blob/main/SECURITY.md) — report vulnerabilities privately and check
  the supported-version policy.

Generate local Rust API documentation with:

```bash
cargo doc -p sinomo-ui --no-deps --open
```

## Repository development

Launch the desktop widget book:

```bash
cargo run -p sui-demo
```

Run the standard validation suite:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Generate the visual validation bundle explicitly:

```bash
cargo run -p sui-demo --bin sui-demo-artifacts
```

Artifacts are written under `target/ui-artifacts/sui-demo/widget-book`.
Ordinary tests do not generate this slower bundle.

For crate ownership and dependency boundaries, see the
[crate architecture](https://github.com/sinomo-lab/sui/blob/main/docs/crate-architecture.md).
Contributors should read
[CONTRIBUTING.md](https://github.com/sinomo-lab/sui/blob/main/CONTRIBUTING.md)
before opening a change.

## License

SUI is licensed under the [MIT License](https://github.com/sinomo-lab/sui/blob/main/LICENSE).
The bundled `sui-lucide`
crate retains its upstream ISC license.
