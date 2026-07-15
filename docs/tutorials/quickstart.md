# Build Your First SUI Application

This tutorial builds a small native window with text, layout, a theme, and an
interactive button. The finished program is also checked into the repository as
[`crates/sui/examples/quickstart.rs`](../../crates/sui/examples/quickstart.rs).

## Prerequisites

You need:

- Rust 1.90 or newer;
- a desktop supported by `winit` and `wgpu`;
- the native graphics and window-system libraries normally required by Rust
  desktop applications on your platform.

From a SUI workspace checkout, verify the toolchain and example before starting:

```bash
rustc --version
cargo check -p sinomo-ui --example quickstart
```

## 1. Create a project

Create a binary crate:

```bash
cargo new sui-quickstart
cd sui-quickstart
```

Until `sinomo-ui` is published to crates.io, add the Git dependency to
`Cargo.toml`:

```toml
[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

The Cargo package is named `sinomo-ui`, while the dependency alias is `sui`.
That is why Rust source imports `sui::...`. The default features select the
desktop platform and `wgpu` renderer.

When developing against a local checkout, replace the Git source with a path:

```toml
[dependencies]
sui = { package = "sinomo-ui", path = "../sui/crates/sui" }
```

## 2. Build the widget tree

Replace `src/main.rs` with:

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let theme = DefaultTheme::light();

    let content = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new("Your first SUI window")
                .theme(theme)
                .font_size(24.0)
                .line_height(30.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new("Widgets are retained Rust values arranged into one tree.")
                .theme(theme)
                .color(theme.palette.text_muted),
        )
        .with_child(
            Button::new("Say hello")
                .theme(theme)
                .on_press(|| println!("Hello from SUI!")),
        );

    let root = Surface::window(content)
        .theme(theme)
        .padding(Insets::all(24.0))
        .fill();

    App::new()
        .window(Window::new("SUI Quickstart").root(root))
        .run()
}
```

Run it:

```bash
cargo run
```

Selecting the button with a pointer or keyboard prints `Hello from SUI!` in
the terminal.

## 3. Understand the pieces

`sui::prelude::*` imports the application, widget, layout, geometry, event, and
theme types used by ordinary applications. Import individual types instead when
you want a stricter public module boundary.

`App` owns application-wide resources and windows. `Window` owns one root
widget. Calling `run` hands the completed retained widget tree to SUI's platform
event loop.

`Stack::vertical` measures children from top to bottom. `spacing` inserts a
fixed gap between neighboring children, while `Alignment::Start` keeps each
child at the leading edge. Use `Flex` when children need growth, wrapping, or
main-axis distribution.

`Surface::window` paints the window background from the selected theme. Its
`fill` modifier asks the surface to occupy the available root size, and its
padding keeps content away from the window edge.

`Button::on_press` stores the closure in the retained button. The runtime calls
it after pointer activation or the accessible keyboard activation keys. A
descriptive button label doubles as its accessible name, so prefer action text
such as `Save profile` over vague labels such as `Click here`.

## 4. Theme consistently

SUI's built-in Mesh themes are plain values. `DefaultTheme` is `Copy`, so one
theme value can be applied to every widget in a subtree:

```rust,ignore
let theme = DefaultTheme::dark();

let panel = Surface::panel(
    Label::new("Dark interface")
        .theme(theme)
        .color(theme.palette.text),
)
.theme(theme)
.padding(Insets::all(16.0));
```

Available starting points include:

- `DefaultTheme::light()` for a light interface;
- `DefaultTheme::dark()` for the standard dark interface;
- `DefaultTheme::void()` for a true-black OLED-oriented interface;
- `DefaultTheme::touch()` for larger touch-oriented control metrics.

Apply the same theme to controls and their containing surfaces. Theme values do
not currently cascade like CSS, which makes each retained widget's styling
explicit and predictable.

## 5. Choose the right layout container

Use `Stack` for a fixed row or column:

```rust,ignore
let actions = Stack::horizontal()
    .spacing(8.0)
    .with_child(Button::new("Cancel"))
    .with_child(Button::new("Save"));
```

Use `Flex` when a child must consume remaining space:

```rust,ignore
let search = Flex::horizontal()
    .gap(8.0)
    .align_items(Alignment::Center)
    .with_item(TextInput::new("Search"), FlexItem::flex(1.0).min_width(120.0))
    .with_child(Button::new("Find"));
```

Use `Padding`, `SizedBox`, and `Surface` for local spacing, explicit size hints,
and visual grouping. Avoid baking window dimensions into leaf widgets; let the
root layout pass constraints down the tree.

## 6. Build without opening a window

`App::build()` returns the retained `Runtime` without starting a platform event
loop. It is useful for tests, headless rendering, and embedding:

```rust
use sui::prelude::*;
use sui::Runtime;

fn build_runtime() -> Result<Runtime> {
    App::new()
        .main_window("Headless", Label::new("Ready"))
        .build()
}
```

For a form with external application state, editable inputs, dynamic labels,
and explicit invalidation, continue with [Build a Stateful
Form](stateful-form.md).

## Troubleshooting

If compilation cannot find `sui`, check that the dependency key is `sui` and
the package override is `package = "sinomo-ui"`.

If the application builds but cannot create a window, first run the repository
demo with `cargo run -p sui-demo`. A failure there usually points to missing
system windowing or graphics dependencies rather than application code.

If a widget is present but has the wrong colors, verify that the same
`DefaultTheme` value was passed to both the widget and its surrounding
`Surface`.
