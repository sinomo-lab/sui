# SUI User API

This document describes the public API shape application authors should use.
The Rust API exists today. Python and JavaScript bindings do not exist in this
workspace yet, but they should follow the same ownership, resource, and async
model instead of exposing the lower-level runtime directly.

## Rust Entry Point

Use `sui::prelude::*` for ordinary application code. The primary construction
types are:

- `App`: owned application builder.
- `Window`: user-facing window builder.
- `ResourceRegistry`: resource registration facade for fonts and images.
- `UiHandle`: cloneable wake handle for background work when a platform event
  loop is running.

Minimal app:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("Hello SUI", Label::new("Ready"))
        .run()
}
```

Registering resources:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let mut app = App::new();
    let logo = {
        let mut resources = app.resources();
        resources.svg_image(include_bytes!("logo.svg"))?
    };

    app.main_window("Images", Image::new(logo).label("Logo"))
        .run()
}
```

Builder-style resource setup:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .with_resources(|resources| {
            resources.font_bytes(include_bytes!("Inter-Regular.ttf"))?;
            Ok(())
        })?
        .main_window("Text", Label::new("Registered fonts are ready"))
        .run()
}
```

## Layout Helpers

Use `Stack` for simple rows and columns. Use `Flex` when a container needs
weighted children, wrapping, or main-axis distribution.

```rust,no_run
use sui::prelude::*;

fn search_row() -> impl Widget {
    Flex::horizontal()
        .gap(8.0)
        .align_items(Alignment::Center)
        .with_child(Label::new("Search"))
        .with_item(
            TextInput::new("Query"),
            FlexItem::flex(1.0).min_width(120.0),
        )
        .with_child(Button::new("Run"))
}
```

For wrapping layouts, opt in explicitly:

```rust,no_run
use sui::prelude::*;

fn tag_cloud(tags: impl IntoIterator<Item = String>) -> impl Widget {
    let mut flex = Flex::horizontal().wrap(FlexWrap::Wrap).gap(6.0);
    for tag in tags {
        flex.push(Label::new(tag));
    }
    flex
}
```

Custom widgets can use `flex_layout` and `arrange_flex` from `sui-layout`
through the `sui` facade when they need the same layout behavior without using
the retained `Flex` container.

Common item helpers cover the frequent cases:

```rust,no_run
use sui::prelude::*;

let toolbar = Flex::horizontal()
    .gap(8.0)
    .with_child(Button::new("Back"))
    .spacer()
    .with_child(Button::new("Done"));

let columns = Flex::horizontal()
    .gap(12.0)
    .with_item(left_panel(), FlexItem::fixed(240.0))
    .with_item(main_panel(), FlexItem::fill());

let cards = Flex::horizontal()
    .wrap(FlexWrap::Wrap)
    .gap(12.0)
    .with_item(card_a(), FlexItem::new().basis_gap_aware_fraction(0.5))
    .with_item(card_b(), FlexItem::new().basis_gap_aware_fraction(0.5));
```

Use `basis_gap_aware_fraction` when fractional items should add up to a full
row while accounting for the container gap. For example, two `0.5` items with a
12px gap each measure as `(width * 0.5) - 6px`, so the two items plus the gap
fit exactly.

`App::build()` returns a `Runtime` for tests, headless rendering, embedding, or
custom platform integration. `App::run()` is the default desktop/web entry
point. `App::into_application()` is an escape hatch for debug tooling and
migration code that still needs the lower-level builder.

## Threading And Async

SUI keeps widget state on the UI runtime thread. User widgets do not need to be
`Send`, and widget methods should stay synchronous. Long-running work belongs
outside the widget tree.

The cross-thread contract is:

1. Own async results or background messages outside the widget tree, usually in
   a channel, mutex-protected queue, or other application state.
2. Start the app with `App::run_with_handle`.
3. Clone `UiHandle` into worker threads or async tasks.
4. Push work into your queue, then call `UiHandle::wake()`.
5. In the root widget, handle `Event::Custom` whose kind is
   `EXTERNAL_WAKE_KIND`, drain the queue, update UI state, and invalidate what
   changed.

Sketch:

```rust,no_run
use std::sync::mpsc;
use sui::prelude::*;

fn run_app(root: impl Widget + 'static) -> Result<()> {
    let (tx, _rx) = mpsc::channel::<String>();

    App::new()
        .main_window("Async", root)
        .run_with_handle(move |ui| {
            std::thread::spawn(move || {
                let _ = tx.send("loaded".to_string());
                ui.wake();
            });
        })
}
```

This split is intentional: Rust, Python, and JavaScript can all share the same
model where the UI thread owns widgets, while background work communicates with
the UI through explicit messages and a wake handle.

## Animation API

Pure animation data is safe to prepare away from the UI thread. Prefer compiled
and reusable structures for editor-style timelines:

- Build or load a `Timeline`.
- Call `compile_shared()` to produce a `SharedCompiledTimeline`.
- Keep one `AnimationPlayer` per playback stream.
- Reuse `SampleBuffer` with `sample_into`/`tick` to avoid per-frame allocation.

The key boundary is that sampled animation values are data. Applying those
values to widgets, invalidation, or retained layer properties still happens on
the UI runtime thread.

## Binding Shape

Future Python and JavaScript bindings should wrap the same concepts rather than
mirror every Rust crate type.

Python:

```python
import sui

async def main():
    app = sui.App()
    app.window(sui.Window("Hello").root(sui.Label("Ready")))
    await app.run_async()
```

JavaScript:

```javascript
import { App, Window, Label } from "@sui/ui";

const app = new App();
app.window(new Window("Hello").root(new Label("Ready")));
await app.run();
```

Binding guidelines:

- Keep `App`, `Window`, resources, handles, animation documents, compiled
  timelines, and sampled values as the stable binding surface.
- Do not expose raw runtime graph mutation as the normal API.
- Use explicit handles for resources and windows instead of borrowing internal
  registries across async boundaries.
- Keep widget updates on the UI executor. Python `asyncio` tasks and JavaScript
  promises should enqueue messages and wake the UI, not mutate widgets from
  arbitrary threads or microtasks.
- Provide async-friendly app runners, but keep widget callbacks synchronous and
  bounded.

## Boundary Rule

Application and demo code should prefer `App`, `Window`, and
`ResourceRegistry`. Lower-level types such as `Application`, `Runtime`,
`WindowBuilder`, platform objects, and diagnostics remain available for tests,
debug tools, benchmarks, and custom embedding.
