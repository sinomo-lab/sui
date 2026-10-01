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
equivalent to `light()`. The presets share one visual language: pure surfaces
and vibrant decoration.

| Preset | Surfaces | Primary |
| --- | --- | --- |
| `light()` | Pure white and gray | SUI azure `#1762F4` |
| `dark()` | Dark, with a faint constant blue tint | SUI azure `#1762F4` |
| `void()` | True black; panels separated by borders | SUI azure `#1762F4` |
| `neutral()` | Same as `light()` | Near-black |
| `neutral_dark()` | Untinted twin of `dark()` | Near-white |

Every preset keeps the same semantic status colors and the nine-hue
decorative palette. `void()` is the true-black/OLED alias for
`high_contrast()`. Built-in presets are derived once and cached, so calling
them repeatedly is cheap.

The older `with_density(ThemeDensity)` API remains public, but new interfaces
should prefer `with_size(ControlSize)`: it scopes control geometry and
typography without changing the independent text-size ramp.

## Source colors and derived roles

`ThemeColors` holds only source colors:

- `neutrals: NeutralRamp`: every structural tier. These are `window`,
  `subtle` (sidebars and chrome), `panel`, `overlay`, `control` (recessed
  fills and tracks), `button` (the raised neutral face), `field`, four border
  weights, and four text weights. `border_control` is the 3:1 outline of
  unselected checkboxes, radios, and switches. `text_tertiary` stays at
  4.5:1 or better on the panel.
- `primary`/`on_primary` and `secondary`/`on_secondary`: brand and live-signal
  colors.
- `info`, `success`, `warning`, and `danger` with their `on_*` content colors.
- `decorative: DecorativeColors`: nine categorical hues (red, orange, amber,
  green, teal, cyan, blue, violet, magenta) with matched OKLCH lightness and
  chroma.

Everything widgets paint is derived from these values in OKLCH, so editing
one source keeps its dependents consistent:

- **Interaction states:** light themes darken on hover and press; dark
  themes lighten on hover. Hue and chroma are preserved.
- **Soft washes and outlines:** authored as translucency and flattened onto
  the panel.
- **Legible tone text:** adjusted in lightness until it reaches 4.5:1 on its
  soft wash, or 6:1 in dark themes.
- **Keyboard focus ring:** the primary, lightened if needed to stay visible
  on the panel.
- **Glows and HDR variants:** glows, Display P3 variants, and extended-range
  HDR variants all come from the same sources.

`DefaultTheme::tone_roles(tone)` returns the complete `ToneRoles` for a
semantic tone. It covers solid, content, hover, pressed, soft, text, and
border. `theme.decorative.get(DecorativeHue::Violet)` returns the same role
set for a decorative hue, and `theme.decorative.categorical(index)` assigns
hues to data categories in a stable order.

## Color-usage hierarchy

The built-in widgets follow one hierarchy designed for dense professional
software:

1. Window chrome, panels, fields, rows, menus, and scrollbars use only neutral
   surface and ink roles. Light surfaces are achromatic.
2. Ordinary buttons, select triggers, and segmented thumbs use the neutral
   button face with a strong outline and full-strength ink. Hover and press
   step through adjacent neutral tiers.
3. Unselected choice controls show a 3:1 control outline on the field well.
   Selected choices, primary actions, slider fills, tab underlines, progress,
   and links use the primary color.
4. Selection fills stay neutral (`palette.selection`); a narrow
   `palette.selection_border` or indicator carries the primary color.
5. Keyboard focus draws a primary-colored ring
   (`palette.focus_ring`) outside a neutral `border_focus`.
6. Status colors communicate only their own semantics. Decorative hues are
   for categorical emphasis such as tags, avatars, node categories, and chart
   series, never for status.

Custom widgets should pair `palette.selection` with `selection_border`, use
`palette.button*` for neutral raised controls, `border_control` for
unselected indicators, and `focus_ring` for focus.

The demo theme editor exposes two deliberate layers. Source colors (brand,
status, surfaces, fills, borders, ink, and decorative) are listed by job, with
the WCAG contrast of each "on" color and text level shown beside it. Semantic
roles follow the sources unless explicitly overridden; overrides are reapplied
after source, spacing, radius, or typography changes, and selecting a preset
clears them. The editor does not expose widget-specific paint details as
global theme tokens. Its preview shows contrast checks, every surface tier,
the decorative palette, motion, and widget book stories in the edited theme.
"Copy as Rust" produces a function that rebuilds the theme from
`ThemeColors` with `DefaultTheme::from_colors`, and "Use as app theme" applies
it to the whole demo.

To edit theme colors in your own tools, `SimpleColorPicker` offers an OKLCH
mode. Its lightness, chroma, and hue sliders match the space themes derive
their roles in, so a lightness change looks the same across hues. Chroma
beyond the editing space's gamut is reduced to fit, and the chroma track marks
where that starts:

```rust
use sui::prelude::*;

let theme = DefaultTheme::sui();
let picker = SimpleColorPicker::from_color("Primary", theme.colors.primary)
    .mode(SimpleColorPickerMode::Oklch)
    .on_change(|color| println!("primary is now {color:?}"));
```

## Widget-owned appearance

Specialized look-and-feel belongs to the widget. Canvas and color-tool widgets
therefore expose partial appearance objects whose unset fields resolve from
the common semantic theme on every paint:

```rust
use sui::prelude::*;

let canvas = Canvas::new("Editor canvas").colors(CanvasColors {
    background: Some(Color::rgba(0.08, 0.09, 0.11, 1.0)),
    grid: Some(Color::rgba(1.0, 1.0, 1.0, 0.08)),
    ..CanvasColors::default()
});

let picker = ColorPicker::new("Paint color").colors(ColorPickerColors {
    checkerboard_dark: Some(Color::rgba(0.35, 0.35, 0.35, 1.0)),
    ..ColorPickerColors::default()
});
```

The same pattern is available through `CanvasRulerColors` and
`PixelCanvasColors`. Applications can wrap these constructors in their own
widget factory, use a different theme system entirely, or set every color
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
                .gap(10.0)
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
                .gap(10.0)
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

- `colors`: source colors: the neutral ramp, brand, status, and decorative
  hues.
- `palette`: derived control-facing text, field, button, border, focus,
  selection, accent, and status roles.
- `decorative`: derived roles for the nine decorative hues.
- `surfaces`: window, panel, overlay, sidebar, canvas, and editor surfaces.
- `metrics`: control heights, padding, row sizes, icon sizes, and related
  geometry.
- `typography` and `text`: control typography and the general text scale.
- `radius`, `shadows`, `glows`, `motion`, and `interaction`: presentation and
  interaction tokens.
- `hdr`: HDR policy and material/luminance tokens.

When changing source fields such as `colors`, call `sync_derived_fields()` so
the derived palette, decorative roles, surfaces, shadows, glows, HDR variants,
and control metrics remain coherent:

```rust
use sui::prelude::*;

let mut theme = DefaultTheme::dark();
theme.colors.primary = Color::rgba(0.35, 0.65, 1.0, 1.0);
theme.sync_derived_fields();
```

For a one-off widget variation, prefer the widget's appearance, tone, color,
padding, or text-style builder instead of cloning and mutating an entire token
set.

## Shadows and glows

`theme.shadows` holds CSS-style shadow scales: `box_shadow` for elevation,
`inset`, `drop`, and `text`, each a `ThemeShadow` of up to two layers. A
layer's `blur` is the CSS blur radius. `theme.glows` holds the halos live
signals wear: `accent` for live and primary signals and `secondary` for voice.
Light themes have no glows.

Surfaces cast shadows by elevation or by token, and glow by tone. A
`ShadowBox` does the same around a child that paints its own face:

```rust,ignore
Surface::panel(content).corner_radius(12.0).shadow(|theme| theme.shadows.box_shadow.lg);
Surface::field(content).corner_radius(8.0).shadow(|theme| theme.shadows.inset.sm);
Surface::panel(content).corner_radius(12.0).glow(GlowTone::Accent);
ShadowBox::new(image).corner_radius(12.0).shadow(|theme| theme.shadows.box_shadow.md);
```

Custom widgets paint them with `paint_theme_shadow` before their fill,
`paint_theme_inset_shadow` after it, and `paint_theme_glow`. Resolve a glow for
the window's output with `theme.glow_for_output(tone, ctx.output_color_range())`:
on HDR outputs, where the theme's HDR mode allows, the halo takes the tone's HDR
color. Enabled primary buttons and busy spinners glow in themes that have
glows; `Button::glow(false)` turns a primary button's glow off.

At the lowest level, `PaintCtx::draw_shadow` takes a `ShadowParams`. Its
`placement` puts the shadow behind its box (`Behind`, as CSS `box-shadow`),
around it only (`Outside`, leaving the box clear, for glows and translucent
surfaces), or inside it (`Inside`, as CSS `inset`). The renderer draws the
Gaussian blur of the rounded box, scaled with the current transform.

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
        .gap(12.0)
        .with_child(Image::new(logo).semantic_name("Company logo"))
        .with_child(Label::new("Dashboard").text_style(heading_style));

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
