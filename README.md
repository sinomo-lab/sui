# SUI

SUI is a Rust UI workspace built around a retained widget runtime, a renderer-neutral scene model, and a `wgpu` renderer. The repository contains the public Rust facade, runtime, platform integration, built-in widgets, deterministic testing tools, and the demo/widget-book host used for development.

The Rust API exists today. The docs also describe the intended shape for future Python and JavaScript bindings, but those bindings are not part of this workspace yet.

## Quick Start

Requirements:

- Rust 1.90 or newer
- A platform supported by `winit` and `wgpu`
- Trunk, only if you want to run the browser demo

Run the desktop demo:

```bash
cargo run -p sui-demo
```

Run the full test suite:

```bash
cargo test
```

Run focused test surfaces:

```bash
cargo test -p sui-testing
cargo test -p sui-demo -- --nocapture
```

`cargo test -p sui-demo -- --nocapture` writes visual artifacts under `target/ui-artifacts/sui-demo/widget-book`.

## Minimal App

Application code should normally import the public facade through `sui::prelude::*`.

```rust
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("Hello SUI", Label::new("Ready"))
        .run()
}
```

For a fuller API overview, see [docs/user-api.md](docs/user-api.md).

## Web Demo

The `sui-demo` crate also has a Trunk-based browser build.

From the workspace root:

```bash
trunk serve --config crates/sui-demo/web/Trunk.toml
```

Or from the web directory:

```bash
cd crates/sui-demo/web
trunk serve
```

Production builds can be created with:

```bash
trunk build --config crates/sui-demo/web/Trunk.toml --release
```

The output is written to `crates/sui-demo/web/dist`.

## Built-in Themes: the Mesh design language

The built-in themes implement **Mesh**, sinomo's design language (see the
`sinomo-ui-design` repository for the token source of truth and documentation
site). Three themes ship out of the box:

- `DefaultTheme::light()` — pure white, ink on paper, faint ink shadows.
- `DefaultTheme::dark()` — cool graphite, lifted surfaces, live glows.
- `DefaultTheme::void()` — true black for OLED: elevation is drawn with
  borders (shadow tokens are empty), whites are dimmed, glows damped.

The role tokens (`--sm-*` in the design repo) map onto `ControlPalette` /
`SurfacePalette`: three text tiers, three surface tiers plus overlay and
field, translucent borders, soft status washes (`*_soft` + `*_soft_text`
pairs), a dedicated focus color, selection, scrim, and per-scheme glow tokens
(`theme.glows`). Density tiers follow the Mesh contract — compact 28px
controls / 30px rows, comfortable 32/36, touch 36/40 with 16px type — and the
motion ladder is 70/140/220/340ms.

## Workspace Layout

- `crates/sui` - public Rust facade and prelude.
- `crates/sui-core` - shared IDs, events, geometry, color, semantics, invalidation, and error types.
- `crates/sui-animation` - pure animation timelines, documents, playback state, and sampled values.
- `crates/sui-layout` - layout constraints and reusable measure/arrange helpers.
- `crates/sui-text` - font registration, shaping, measurement, and text layout.
- `crates/sui-scene` - renderer-neutral scene representation.
- `crates/sui-runtime` - retained widget graph, event routing, layout, paint, semantics, scheduling, and diagnostics.
- `crates/sui-render-wgpu` - retained compositor and `wgpu` backend.
- `crates/sui-platform` - desktop and headless platform integration.
- `crates/sui-platform-windows` - Windows Advanced Color and DXGI probing helpers.
- `crates/sui-widgets` - built-in controls, containers, composite widgets, and themes.
- `crates/sui-debug` - reusable debug widgets and inspectors.
- `crates/sui-testing` - deterministic UI automation and expectation helpers.
- `crates/sui-tui` - accessibility-tree generated terminal UI support.
- `crates/sui-lucide` - embedded Lucide SVG icon resources.
- `crates/sui-avif` - minimal AVIF encoding primitives, including HDR still-image support.
- `crates/sui-demo` - desktop demo, widget gallery, benchmark surfaces, and visual validation host.

## Documentation

Start with [docs/README.md](docs/README.md). The most useful entry points are:

- [docs/user-api.md](docs/user-api.md) for the public Rust API.
- [docs/architecture.md](docs/architecture.md) for the runtime and frame pipeline.
- [docs/crate-architecture.md](docs/crate-architecture.md) for crate ownership and dependency boundaries.
- [docs/testing.md](docs/testing.md) for headless tests, desktop harness tests, and visual artifacts.
- [docs/renderer-architecture.md](docs/renderer-architecture.md) for scene, compositor, and renderer details.
- [docs/design.md](docs/design.md) for long-term product and API direction.

## Development Model

Most UI work follows this path:

1. `sui-platform` normalizes host input into `sui_core::Event` values.
2. `sui-runtime` routes events through the retained widget tree.
3. Widgets request invalidation, timers, or animation frames.
4. The runtime performs measure, arrange, paint, semantics, and resource work as needed.
5. The runtime emits a `SceneFrame` plus diagnostics.
6. `sui-render-wgpu` updates retained compositor state and presents the result.

Widgets should not talk directly to `wgpu` or host window APIs. They operate through runtime contexts and emit renderer-neutral scene content.

## Testing

SUI tests are semantics-first. Prefer locators based on role, accessible name, text, description, and value instead of widget internals.

Common commands:

```bash
cargo test
cargo test -p sui-testing
cargo test -p sui-demo -- --nocapture
cargo run -p sui-demo
```

See [docs/testing.md](docs/testing.md) for the testing layers and expected style.

## License

The workspace is licensed as `MIT OR Apache-2.0`.

