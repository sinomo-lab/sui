# Getting Started and Application Lifecycle

[API guide](README.md) · [Next: widgets and layout](widgets-and-layout.md)

## Requirements

- Rust 1.90 or newer.
- A platform supported by `winit` and `wgpu` for the default desktop build.
- A WebAssembly target and Trunk only when building the browser demo.

Until the first registry release, use the Git repository:

```toml
[package]
name = "hello-sui"
version = "0.1.0"
edition = "2024"

[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

The left-hand name is intentional. Cargo resolves the `sinomo-ui` package, but
application code uses the shorter `sui::` crate path.

## Your First Window

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let root = Padding::all(
        24.0,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Start)
            .with_child(Label::new("Hello, SUI!").font_size(24.0))
            .with_child(Button::primary("Continue")),
    );

    App::new().main_window("Hello SUI", root).run()
}
```

`App` and its builders are owned and value-oriented. Each call consumes the
builder and returns the configured value. Construct the widget tree and
resources first, then call one terminal operation:

- `run()` starts the desktop or browser event loop and blocks until it exits.
- `run_with_handle(...)` also supplies a cloneable, thread-safe `UiHandle`
  after the event loop is ready. It sends typed commands from background work;
  its bare `wake()` method runs scheduler hooks without creating a widget
  event.
- `build()` returns a `Runtime` without starting a platform event loop. Use it
  for tests, headless rendering, embedding, or a custom host.

## Multiple Windows

Use `Window` when a title and root widget are not enough, or when adding more
than one window:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .window(
            Window::new("Editor")
                .root(Label::new("Document"))
                // Replace this repository asset with your application icon.
                .icon_svg(include_bytes!(
                    "../../crates/sui-runtime/assets/sui-logo.svg"
                )),
        )
        .window(Window::new("Inspector").root(Label::new("Properties")))
        .run()
}
```

`Window` exposes the common title, root, and icon choices. Reach for the
lower-level `WindowBuilder` only when custom embedding needs a runtime-level
window contract.

## Register Fonts and Images Before Building

Resource handles are stable and cheap to copy. Register bytes once during app
construction, then put handles in widgets or application state.

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let mut app = App::new();
    let logo = {
        let mut resources = app.resources();
        // Replace this repository asset with your application image.
        resources.svg_image(include_bytes!(
            "../../crates/sui-runtime/assets/sui-logo.svg"
        ))?
    };

    app.main_window("Resources", Image::new(logo).label("Company logo"))
        .run()
}
```

Builder-style setup is useful when registration can fail:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .with_resources(|resources| {
            // Replace this repository asset with your application font.
            resources.font_bytes(include_bytes!(
                "../../crates/sui-text/assets/NotoSans-Regular.ttf"
            ))?;
            Ok(())
        })?
        .main_window("Fonts", Label::new("The font registry is ready"))
        .run()
}
```

The built-in icon resources are registered by `App::new()` and
`Application::new()`. Application-owned SVG, RGBA, and font data still need to
be registered explicitly.

## Build Without Running

`App::build` is the clean boundary between app construction and hosting:

```rust
use sui::prelude::*;
use sui::Runtime;

fn build_runtime() -> Result<Runtime> {
    App::new()
        .main_window("Embedded", Label::new("Host-owned event loop"))
        .build()
}
```

The returned runtime contains the retained widget trees and registered
resources. It does not create or run the default platform event loop. Prefer
`sinomo-ui-testing` for tests instead of manually driving `Runtime` unless the test
is specifically about runtime or host integration.

## Application Errors

SUI uses `sui::Result<T>` and `sui::Error`. Return `Result<()>` from `main` so
resource, build, and platform startup failures can propagate with `?`. A
platform event-loop method may also return after the last window closes or
when startup fails.

## Next Steps

- Compose real interfaces with [Widgets and layout](widgets-and-layout.md).
- Add live data and callbacks with [State, events, and background work](state-events-and-async.md).
- Select a non-default target using [Platforms and Cargo features](platforms-and-features.md).
