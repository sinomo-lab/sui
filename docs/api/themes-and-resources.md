# Themes and Resources

[Previous: state and events](state-events-and-async.md) · [API guide](README.md) ·
[Next: custom widgets](custom-widgets.md)

SUI separates built-in widget styling from application-owned resource data.
`DefaultTheme` is the concrete token set consumed by built-in widgets;
`ResourceRegistry` turns font and image data into stable handles before the
runtime starts.

## Built-in Themes

SUI provides a branded preset, a neutral professional preset, three branded
color schemes, a true-black alias, and three contextual control sizes:

```rust
use sui::prelude::*;

let sui = DefaultTheme::sui();
let neutral_light = DefaultTheme::neutral();
let neutral_dark = DefaultTheme::neutral_dark();
let light = DefaultTheme::light();
let dark = DefaultTheme::dark();
let high_contrast = DefaultTheme::high_contrast();
let oled = DefaultTheme::void();

let compact = dark.with_size(ControlSize::Small);
let standard = dark.with_size(ControlSize::Medium);
let touch = dark.with_size(ControlSize::Large);

assert!(compact.metrics.min_height <= standard.metrics.min_height);
assert!(standard.metrics.min_height <= touch.metrics.min_height);
```

`sui()` is the explicit name for the default branded light preset and is
equivalent to `light()`. Every built-in preset keeps structural surfaces,
hover, pressed, selection, field focus, keyboard focus, scroll chrome, and
ordinary active controls neutral. `neutral()` and `neutral_dark()` additionally
make primary actions and live-signal decoration achromatic while retaining
conventional semantic colors for informational, success, warning, and danger
feedback. They are the standard starting points when a professional interface
has no product color preference.

`void()` is the true-black/OLED alias for `high_contrast()`. The older
`with_density(ThemeDensity)` API remains public, but new interfaces should
prefer `with_size(ControlSize)`: it scopes control geometry and typography
without changing the independent text-size ramp.

## Color-usage hierarchy

The built-in widgets follow a neutral-first hierarchy designed for dense
professional software:

1. Window chrome, panels, fields, rows, menus, selected tiles, current-line
   fills, and scrollbars use only neutral surface and ink roles.
2. Hover and press move between adjacent neutral tiers. Selection fill uses
   the dedicated neutral `palette.selection`; a narrow
   `palette.selection_border` may carry the primary color without tinting the
   selected surface.
3. Focus uses a neutral strong border plus a distinct, higher-contrast neutral
   ring. Carets use normal text ink.
4. `colors.primary` and `palette.accent*` are sparse emphasis roles: explicit
   primary actions, links, thin tab or row indicators, live/busy signals, and
   decorative canvas defaults.
5. Informational, success, warning, and danger colors communicate their own
   semantics and are not replaced by the primary color.

Changing `colors.primary` therefore changes brand decoration, selection
borders, and explicit accent components without recoloring the application
background or routine interaction fills. Custom widgets should pair
`palette.selection` with `selection_border`, and use `surface_focus`,
`border_focus`, and `focus_ring` for focus rather than mixing surfaces with
`palette.accent`.

The demo theme editor exposes two deliberate layers: source colors and the
common semantic role palette used as widget defaults. It does not expose
widget-specific paint details as global theme tokens. Semantic-role edits are
stored as explicit overrides and reapplied after source, spacing, radius, or
typography changes; selecting a preset clears them. Alpha is editable for
translucent roles such as hover and selection washes.

## Widget-owned appearance

Specialized look-and-feel belongs to the widget. Canvas and color-tool widgets
therefore expose partial appearance objects whose unset fields resolve from
the common semantic theme on every paint:

```rust
use sui::prelude::*;

let canvas = Canvas::new("Editor canvas").appearance(CanvasAppearance {
    background: Some(Color::rgba(0.08, 0.09, 0.11, 1.0)),
    grid: Some(Color::rgba(1.0, 1.0, 1.0, 0.08)),
    ..CanvasAppearance::default()
});

let picker = ColorPicker::new("Paint color").appearance(ColorPickerAppearance {
    checkerboard_dark: Some(Color::rgba(0.35, 0.35, 0.35, 1.0)),
    ..ColorPickerAppearance::default()
});
```

The same pattern is available through `CanvasRulerAppearance` and
`PixelCanvasAppearance`. Applications can wrap these constructors in their own
widget factory, use a different theme system entirely, or set every appearance
field directly. Theme values remain defaults, not a mandatory styling engine.

## Applying a Static Theme

Built-in widgets expose `theme(DefaultTheme)` where styling is relevant.
Themes are explicit values; creating a facade-level `Theme` does not
automatically inject it into every descendant.

```rust
use sui::prelude::*;

fn dark_form() -> impl Widget {
    let theme = DefaultTheme::dark().with_size(ControlSize::Medium);

    Background::new(
        theme.palette.surface,
        Padding::all(
            20.0,
            Stack::vertical()
                .spacing(10.0)
                .with_child(Label::new("Sign in").theme(theme))
                .with_child(TextInput::new("Email").theme(theme))
                .with_child(PasswordInput::new("Password").theme(theme))
                .with_child(Button::primary("Continue").theme(theme)),
        ),
    )
}
```

`DefaultTheme` is `Copy`, so pass one scoped value to all controls that should
share it. Composite widgets also expose theme builders when their child chrome
uses built-in tokens.

## Live Theme Switching

Use `theme_when` and other reader builders when the theme can change without
replacing the retained tree:

```rust
use std::{cell::Cell, rc::Rc};
use sui::prelude::*;
use sui::{InvalidationKind, InvalidationRequest, InvalidationTarget};

fn theme_switcher() -> impl Widget {
    let theme = Rc::new(Cell::new(DefaultTheme::light()));

    let input_theme = Rc::clone(&theme);
    let input = TextInput::new("Preview text")
        .value("Live theme")
        .theme_when(move || input_theme.get());

    let button_theme = Rc::clone(&theme);
    let action_theme = Rc::clone(&theme);
    let toggle = Button::new("Toggle theme")
        .theme_when(move || button_theme.get())
        .on_press_with_ctx(move |ctx| {
            let next = if action_theme.get().colors.scheme == ThemeColorScheme::Light {
                DefaultTheme::dark()
            } else {
                DefaultTheme::light()
            };
            action_theme.set(next);
            // Multiple sibling controls and the background read this theme.
            for kind in [
                InvalidationKind::Measure,
                InvalidationKind::Paint,
                InvalidationKind::Semantics,
            ] {
                ctx.request(InvalidationRequest::new(
                    InvalidationTarget::Window(ctx.window_id()),
                    kind,
                ));
            }
        });

    let background_theme = Rc::clone(&theme);
    Background::new(
        theme.get().palette.surface,
        Padding::all(
            20.0,
            Stack::vertical()
                .spacing(10.0)
                .with_child(input)
                .with_child(toggle),
        ),
    )
    .brush_when(move || background_theme.get().palette.surface)
}
```

Every themed control needs access to the same reader. Keep the closure cheap;
copying a `DefaultTheme` is intentional.

## Token Layers

The most frequently used `DefaultTheme` fields are:

- `colors`: source semantic color scheme and base colors.
- `palette`: control-facing text, field, border, focus, selection, accent, and
  status colors.
- `surfaces`: window, panel, overlay, sidebar, canvas, and editor surfaces.
- `metrics`: control heights, padding, row sizes, icon sizes, and related
  geometry.
- `typography` and `text`: control typography and the general text scale.
- `radius`, `shadows`, `glows`, `motion`, and `interaction`: presentation and
  interaction tokens.
- `hdr`: HDR policy and material/luminance tokens.

When changing source fields such as `colors`, call `sync_derived_fields()` so
the derived palette, surfaces, shadows, and control metrics remain coherent:

```rust
use sui::prelude::*;

let mut theme = DefaultTheme::dark();
theme.colors.primary = Color::rgba(0.35, 0.65, 1.0, 1.0);
theme.sync_derived_fields();
```

For a one-off widget variation, prefer the widget's appearance, tone, color,
padding, or text-style builder instead of cloning and mutating an entire token
set.

## Application Theme Extensions

The facade-level `Theme` combines a `DefaultTheme`, top-level foreground and
background colors, and type-indexed application extensions. It is an explicit
configuration value, not a runtime global.

```rust
use sui::prelude::*;

#[derive(Debug)]
struct ChartTheme {
    positive: Color,
    negative: Color,
}

let theme = Theme::new()
    .with_default_widgets(DefaultTheme::dark())
    .with_extension(ChartTheme {
        positive: Color::rgba(0.2, 0.8, 0.5, 1.0),
        negative: Color::rgba(0.95, 0.3, 0.35, 1.0),
    });

let chart = theme.extension::<ChartTheme>().expect("chart theme");
assert!(chart.positive != chart.negative);
```

Any `Any + Send + Sync` value implements `ThemeExtension` through the blanket
implementation. Pass the `Theme` or the relevant extension to the widgets and
application components that consume it.

## Registering Resources

Register resources before `build` or `run`. The registry validates input and
returns typed `FontHandle` and `ImageHandle` values.

```rust,no_run
use sui::prelude::*;

fn main() -> Result<()> {
    let mut app = App::new();

    let (font, logo) = {
        let mut resources = app.resources();
        // Replace these repository assets with your application assets.
        let font = resources.font_bytes(include_bytes!(
            "../../crates/sui-text/assets/NotoSans-Regular.ttf"
        ))?;
        let logo = resources.svg_image(include_bytes!(
            "../../crates/sui-runtime/assets/sui-logo.svg"
        ))?;
        (font, logo)
    };

    let heading_style = TextStyle {
        font: Some(font),
        font_size: 24.0,
        line_height: 30.0,
        color: Color::BLACK,
        ..TextStyle::default()
    };

    let root = Stack::vertical()
        .spacing(12.0)
        .with_child(Image::new(logo).label("Company logo"))
        .with_child(Label::new("Dashboard").style(heading_style));

    app.main_window("Resources", root).run()
}
```

Available registration paths include:

- `font_bytes` or `register_font` for fonts.
- `svg_image` and `svg_image_at_size` for intrinsic or explicitly rasterized
  SVG images.
- `rgba_image` or `image` for decoded raster data.
- `embedded_svg_image(s)` for compile-time resource tables.

Methods with an explicit handle support applications that require stable IDs
across generated resource catalogs. The allocating methods are simpler for
ordinary apps.

## Runtime-Generated and External Images

A custom widget can allocate a stable widget-local slot with
`PaintCtx::widget_image_handle(slot)`, register a `RegisteredImage` for that
handle during paint, and draw it in the same frame. Reuse the slot instead of
allocating a new identity every frame.

App-owned GPU textures require the `wgpu` feature and
`WgpuExternalTextureRegistry`. The app owns backend texture lifetime; the
widget registers renderer-neutral metadata and refers to the matching image
handle. Keep this path for video, game, or renderer interop. Ordinary images
should use `ResourceRegistry`.

After renderer initialization, `WgpuExternalTextureRegistry::context()` exposes
the renderer-owned device and queue together with `adapter_info()`. Persist the
adapter name, backend, vendor/device identifiers, driver, and driver info when
an external renderer needs capture or asset provenance tied to SUI's actual
WGPU device.

## Icons and Accessibility

`App::new()` registers SUI's built-in Lucide resources. Use `Icon` for a
decorative or labeled glyph and `IconButton` for an action. An icon-only action
must have a meaningful accessible label; its glyph name is not a substitute
for the action name.
