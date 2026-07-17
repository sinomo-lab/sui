# SUI Rust API Guide

This guide is the release-oriented entry point for building Rust applications
with SUI. It documents the public `sui` facade provided by the `sinomo-ui`
Cargo package, the contracts application code can rely on, and the boundaries
where lower-level runtime APIs are appropriate.

SUI is currently a pre-release API. Prefer the facade and
`sui::prelude::*` over importing workspace implementation crates directly;
that keeps application code on the narrowest and most stable surface.

## Start Here

1. [Getting started and application lifecycle](getting-started.md) installs
   the crate, creates windows, registers resources, and explains `build`
   versus `run`.
2. [Widgets and layout](widgets-and-layout.md) covers the built-in widget
   families and the `Stack`, `Flex`, constraint, scrolling, and responsive
   layout contracts.
3. [Overlays and desktop interaction](overlays-and-desktop.md) covers managed
   dialogs, popovers, menus, focus, collision-aware placement, notifications,
   file dialogs, and platform file drops.
4. [Virtual collections](virtual-collections.md) covers keyed incremental
   models, variable-height realization, anchoring, follow-end, row retention,
   virtual tables, and tree accessibility.
5. [Rich documents](rich-documents.md) covers streaming Markdown, retained
   blocks, document-spanning selection, code, attachments, and extension
   renderers.
6. [Input and text editing](input-and-editing.md) covers editable fields,
   selection, clipboard commands, IME, form state, and security boundaries.
7. [State, events, and background work](state-events-and-async.md) explains
   retained local state, external reader/callback state, invalidation, timers,
   animation frames, and `UiHandle`.
8. [Themes and resources](themes-and-resources.md) shows static and live
   themes, control sizing, typed theme extensions, fonts, images, and icons.
9. [Custom widgets](custom-widgets.md) walks through the `Widget` lifecycle,
   accessible interaction, painting, and child ownership.
10. [Testing and accessibility](testing-and-accessibility.md) uses
   `sinomo-ui-testing` for semantics-first, deterministic interaction tests and
   explains the public accessibility contract.
11. [Platforms and Cargo features](platforms-and-features.md) lists the
   supported execution surfaces, feature gates, and current caveats.

## Which API Level Should I Use?

| Need | Start with | Escape hatch |
| --- | --- | --- |
| Normal application | `App`, `Window`, built-in widgets | None normally needed |
| Register fonts and images | `App::resources` or `App::with_resources` | `Application` registration methods |
| Test or embed without an event loop | `App::build` | `Runtime` and `HeadlessPlatform` |
| Custom drawing or interaction | Implement `Widget` | Scene and text types re-exported by `sui` |
| Custom platform integration | `Application`, `WindowBuilder` | `Runtime` and platform crates |
| UI automation | `sinomo-ui-testing` | Direct normalized event dispatch |

`App` is the recommended application builder. `Application`, `Runtime`, and
`WindowBuilder` are deliberately public for embedding, debug tools, and custom
platform hosts; using them is not required to build an ordinary desktop or web
application.

## Imports and Naming

The Cargo package name is `sinomo-ui`, while its Rust library name is `sui`.
Alias the dependency as `sui` and import the prelude:

```toml
[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("My app", Label::new("Ready"))
        .run()
}
```

The prelude contains the normal app, widget, layout, theme, animation, and
custom-widget context types. Less common event details and diagnostics remain
available as explicit `sui::TypeName` imports.

## API Reference Generation

Generate the exact Rust API for the checked-out revision with:

```bash
cargo doc -p sinomo-ui --no-deps --open
```

This guide describes usage and contracts; generated rustdoc remains the source
for exhaustive method signatures. For runtime architecture and ownership, see
[Architecture](../architecture.md) and [Crate architecture](../crate-architecture.md).
