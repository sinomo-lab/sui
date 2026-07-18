# SUI

**A flexible UI toolkit for Rust, powered by `wgpu`.**

SUI takes its name from 水 (*sui*, water): the toolkit should take the shape of
the application, not the other way around. Its retained widget tree and
reactive state are tools, not rules. An application can use the complete
stack, replace parts of it, or embed only the runtime. Custom code joins
through a small widget contract, so SUI remains useful without owning the
application's architecture.

SUI also handles several hard edges of modern application UI. Virtual
collections keep identity and scroll position stable while data changes. The
document model consumes streaming Markdown without throwing away finished
layout. Typed application messages are routed independently of the paint
tree, and managed overlays and responsive panes preserve focus as structure
changes. The same semantics power accessibility, UI testing, and inspection.

Rendering follows the same idea: widgets emit renderer-neutral scenes, while
a retained `wgpu` compositor handles the normal GPU path. SUI is aimed at
technical and creative software where ordinary controls need to work naturally
beside custom graphics.

**[Explore the live SUI widget book →](https://sinomo-lab.github.io/sui/)**

## Quick start

SUI requires Rust 1.90 or newer and the system libraries normally required by
`winit` and `wgpu` on the target platform.

```toml
[dependencies]
sui = { package = "sinomo-ui", version = "0.2" }
```

The package is named `sinomo-ui` because the `sui` registry namespace is
occupied. The dependency alias keeps application imports concise.

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let content = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Start)
        .with_child(Label::new("Your first SUI window").font_size(24.0))
        .with_child(
            Button::primary("Continue")
                .on_press(|| println!("Hello from SUI!")),
        );

    App::new()
        .main_window("Hello SUI", Padding::all(24.0, content))
        .run()
}
```

Run the checked quickstart or open the complete widget book:

```bash
cargo run -p sinomo-ui --example quickstart
cargo run -p sinomo-ui-demo
```

The [quickstart tutorial](https://github.com/sinomo-lab/sui/blob/main/docs/tutorials/quickstart.md)
continues from this example.

## Platform status

SUI is pre-1.0 software. The Rust API may continue to evolve during the `0.x`
series.

| Surface | Status | Notes |
| --- | --- | --- |
| Rust desktop | Available | Linux, macOS, and Windows through `winit` and `wgpu` |
| Headless/testing | Available | Deterministic runtime, semantic interaction, rendering, and screenshots |
| Web | Alpha | Rust/Wasm and WebGPU; used by the live widget book |
| Android | Experimental | Native-activity host with lifecycle-aware surface management |
| Python | Alpha | Working native binding, currently built from source |
| JavaScript | Alpha | Working but incomplete Node/Electron binding, currently built from source |

The web path runs SUI applications through Rust/Wasm and WebGPU; it is not a
DOM framework. Applications authored in JavaScript can use the separate
Node/Electron API. See [Platforms and Cargo features](https://github.com/sinomo-lab/sui/blob/main/docs/api/platforms-and-features.md)
and the [JavaScript binding guide](https://github.com/sinomo-lab/sui/blob/main/crates/sui-js/README.md)
for the current boundaries.

## Documentation

- [Start here](https://github.com/sinomo-lab/sui/blob/main/docs/README.md)
- [API guide](https://github.com/sinomo-lab/sui/blob/main/docs/api/README.md)
- [Examples](https://github.com/sinomo-lab/sui/blob/main/docs/examples.md)
- [Testing](https://github.com/sinomo-lab/sui/blob/main/docs/testing.md)
- [Architecture](https://github.com/sinomo-lab/sui/blob/main/docs/architecture.md)
- [Crate boundaries](https://github.com/sinomo-lab/sui/blob/main/docs/crate-architecture.md)

Generate API documentation for the checked-out revision with:

```bash
cargo doc -p sinomo-ui --no-deps --open
```

## Feature flags

The `sinomo-ui` facade enables `desktop` and `wgpu` by default. Disable default
features when embedding the renderer-neutral runtime or selecting another
platform host.

| Feature | Purpose |
| --- | --- |
| `desktop` | Desktop event loop and window integration; also enables `wgpu` |
| `web` | Browser/WebAssembly integration; also enables `wgpu` |
| `mobile` | Mobile integration, currently Android; also enables `wgpu` |
| `wgpu` | Renderer facade and external texture integration |
| `testing` | Compatibility flag; high-level test APIs live in `sinomo-ui-testing` |

## Repository development

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The widget book also serves as SUI's integration and visual-validation host.
Generate its artifact bundle with:

```bash
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

Artifacts are written under `target/ui-artifacts/sui-demo/widget-book`.
Contributors should read [CONTRIBUTING.md](https://github.com/sinomo-lab/sui/blob/main/CONTRIBUTING.md);
security issues should follow the [security policy](https://github.com/sinomo-lab/sui/blob/main/SECURITY.md).

## License

SUI is licensed under the [MIT License](https://github.com/sinomo-lab/sui/blob/main/LICENSE).
The bundled `sinomo-ui-lucide` crate retains its upstream ISC license.
