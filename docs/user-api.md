# SUI User API

This page is the compact entry point for SUI's supported application-facing
API. The detailed reference is split into focused pages under [`docs/api`](./api/README.md),
and exact method signatures are available from generated Rust documentation.

## Rust entry point

Depend on the Cargo package as `sui` and import the prelude in ordinary
application code:

```toml
[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("Hello SUI", Label::new("Ready"))
        .run()
}
```

The public facade is intentionally the normal boundary for applications:

- `App` owns application resources and one or more windows.
- `Window` configures a user-facing window and its retained root widget.
- `ResourceRegistry` registers fonts and images and returns stable handles.
- `UiHandle` wakes a running platform event loop after background work.
- `Widget` is the protocol for custom retained widgets.
- Built-in controls, containers, composites, geometry, events, semantic types,
  text types, and themes are re-exported by the facade.

Prefer these types over importing internal workspace crates. Lower-level
`Application`, `Runtime`, scene, and renderer types remain available for
embedding, tests, and tooling, but they carry more integration responsibility.

## Find the right guide

| Task | Guide |
| --- | --- |
| Configure an app, window, or resources | [Getting started](./api/getting-started.md) |
| Choose a control or layout container | [Widgets and layout](./api/widgets-and-layout.md) |
| Edit text, passwords, or date/time values | [Input and editing](./api/input-and-editing.md) |
| Handle events or update shared state | [State, events, and async](./api/state-events-and-async.md) |
| Apply Mesh themes or register assets | [Themes and resources](./api/themes-and-resources.md) |
| Implement `Widget` directly | [Custom widgets](./api/custom-widgets.md) |
| Automate UI and verify accessibility | [Testing and accessibility](./api/testing-and-accessibility.md) |
| Select desktop, web, or mobile features | [Platforms and features](./api/platforms-and-features.md) |

For a guided introduction, follow [Build your first SUI application](./tutorials/quickstart.md)
and [Build a stateful form](./tutorials/stateful-form.md). Every tutorial links
to checked Cargo example source.

## API conventions

SUI uses a few conventions consistently:

- Constructors such as `Label::new` and `TextInput::new` take stable semantic
  names or content; builder methods configure the retained value before it is
  inserted into the tree.
- Layout is constraint-based. Use `Stack` for simple rows and columns and
  `Flex` for growth, wrapping, or distribution.
- Widgets own transient interaction details such as focus, caret, and text
  selection. Application state owns domain values and receives changes through
  callbacks.
- State changes become visible through explicit invalidation. Request only the
  passes that may have changed: measure, paint, semantics, resources, or
  animation.
- Widgets stay on the UI thread. Background tasks publish results through a
  queue and call `UiHandle::wake()`; the retained tree consumes them when SUI
  delivers the external wake event.
- Accessibility semantics are part of the widget contract, not optional test
  metadata. Tests and the generated TUI consume the same tree.

## Build, run, and document

Check all public Rust examples:

```bash
cargo check -p sinomo-ui --examples
```

Run the widget book to inspect real controls and their semantics:

```bash
cargo run -p sinomo-ui-demo
```

Generate browsable rustdoc for the checked-out version:

```bash
cargo doc -p sinomo-ui --no-deps --open
```

Because SUI is pre-release, the generated docs are the source of truth for
exact signatures. The hand-written guides explain the intended patterns and
call out platform or stability limits.

## Python and JavaScript

Native bindings implement the same high-level ownership model but are alpha
and not published yet:

- [Python/PyO3 guide](../crates/sui-python/README.md)
- [Node/Electron napi-rs guide](../crates/sui-js/README.md)

Both expose normal desktop execution and a host-driven mode for embedding or
tests. They do not expose arbitrary runtime-graph mutation as the default API.
Browser JavaScript/WASM bindings, package publication, custom user WGSL, and
zero-copy external-surface composition remain roadmap items; see the
[active binding plan](./plans/cross-language-bindings-plan.md).

## Stability boundary

The public Rust facade, desktop runtime, built-in widgets, and deterministic
testing model are implemented. This is still a `0.1.0` pre-release workspace:
semver compatibility is not promised, language packages are local builds, and
browser/mobile/native-HDR surfaces vary by platform. Check the
[platform matrix](../README.md#platform-status) before choosing a deployment
target.
