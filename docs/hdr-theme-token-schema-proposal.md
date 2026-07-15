# HDR Theme Tokens

SUI's HDR theme layer lets an application author one semantic theme with SDR,
wide-gamut, and HDR variants. It is implemented in `sinomo-ui-widgets` and re-exported
by the `sui` facade. This document is the current API guide and authoring
tutorial; the original filename is retained for link stability.

Related reading:

- [Documentation index](./README.md)
- [HDR and wide-gamut display roadmap](./plans/hdr-wide-gamut-display-proposal.md)
- [HDR-native interface manifesto](./hdr-native-interface-manifesto.md)
- [Rendering architecture](./renderer-architecture.md)

## What the theme layer does

The theme layer resolves semantic widget intent into display-aware styling:

- a semantic color with an SDR fallback and optional wide-gamut/HDR variants
- a relative luminance budget
- a material description
- an optional effect description

It does **not** detect the monitor or configure the swapchain. Display output is
controlled separately by `WindowRenderOptions`. An HDR theme mode can be useful
for previewing and capture on an SDR machine, while an HDR-capable output can
still use a restrained or fully SDR theme.

The default is deliberately safe: `HdrThemeMode::Disabled`.

## Quick start

Start from a built-in `DefaultTheme`, select a mode, customize only the semantic
roles you need, and pass that theme to widgets:

```rust,no_run
use sui::prelude::*;
use sui::{
    HdrThemeMode, SemanticColorToken, WindowColorManagementMode,
};

fn main() -> Result<()> {
    let mut theme = DefaultTheme::dark();
    theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
    theme.hdr.color_roles.accent =
        SemanticColorToken::from_sdr(theme.colors.primary)
            .with_wide_gamut(Color::display_p3(0.10, 0.82, 0.94, 1.0))
            .with_hdr(Color::linear_display_p3(0.14, 0.94, 1.08, 1.0));

    let root = Button::primary("Render")
        .theme(theme)
        .on_press(|| println!("render requested"));

    let output = WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(WindowColorManagementMode::PreferHdr);

    App::new()
        .render_options(output)
        .window(Window::new("HDR theme example").root(root))
        .run()
}
```

`ConstrainedHdr` is the recommended application starting point. It allows
curated highlights while applying the smaller policy caps described below.
Keep an SDR value for every semantic token; unsupported variants resolve back
to it automatically.

The output preference in the example is intentionally separate. `PreferHdr`
falls back to SDR when native HDR presentation is unavailable. See the
[display roadmap](./plans/hdr-wide-gamut-display-proposal.md) for that contract.

## Mode resolution

`HdrThemeMode` controls token selection and lift budgets:

| Mode | Color selection | Luminance/material/effect behavior |
| --- | --- | --- |
| `Disabled` | SDR | Reference-white lift, flat material, no effects. |
| `WideGamutOnly` | Wide-gamut, then SDR | No lift above reference white; non-SDR color is allowed. |
| `ConstrainedHdr` | HDR, then wide-gamut, then SDR | Large areas and emissive roles use conservative caps. |
| `FullHdr` | HDR, then wide-gamut, then SDR | Large-area cap remains conservative; emissive roles may use the larger full-HDR cap. |

There is currently no automatic bridge from `WindowOutputDiagnostics` to
`HdrThemeMode`. Applications choose theme policy explicitly.

## Color tokens

### `SemanticColorToken`

```rust
use sui::Color;

pub struct SemanticColorToken {
    pub sdr: Color,
    pub wide_gamut: Option<Color>,
    pub hdr: Option<Color>,
}
```

Constructors and modifiers:

- `SemanticColorToken::from_sdr(color)` creates a complete fallback token.
- `.with_wide_gamut(color)` adds the Display-P3-style variant.
- `.with_hdr(color)` adds the linear, headroom-capable variant.
- `.resolve(mode)` applies the mode fallback order.

Use `Color::display_p3` for encoded Display-P3 values and
`Color::linear_display_p3` for linear values. Linear HDR values may exceed
`1.0`; the selected luminance policy still limits what a widget should emit.

### `HdrColorRoles`

`HdrColorRoles` maps semantic meaning to `SemanticColorToken` values:

| Group | Roles |
| --- | --- |
| Surfaces | `surface`, `surface_elevated`, `surface_outline` |
| Text | `text`, `text_muted` |
| Actions | `accent`, `accent_text`, `secondary` |
| Status | `info`, `success`, `warning`, `danger` |

Use roles rather than widget-specific names. A custom indicator should request
`Accent` or `Success`, for example, instead of reading a hard-coded button
color.

`HdrColorRoles::from_colors(ThemeColors)` derives every SDR fallback and adds
the curated wide-gamut/HDR variants provided by SUI's built-in light, dark, and
high-contrast schemes. `HdrThemeTokens::sync_semantic_defaults(colors)` replaces
the color roles from a new `ThemeColors` value. Prefer
`DefaultTheme::from_colors(colors)` when constructing a whole theme so its
palette, surfaces, and HDR roles are derived together.

## Luminance tokens

`HdrLuminanceTokens` defines relative lift budgets:

| Field | Default | Intended use |
| --- | ---: | --- |
| `reference_white` | `1.0` | Ordinary surfaces and content. |
| `focused` | `1.05` | Focus emphasis. |
| `semantic_accent` | `1.10` | Primary semantic accents. |
| `emissive_indicator` | `1.25` | Small energized indicators. |
| `alert_pulse` | `1.15` | Temporary alert emphasis. |

These values are normalized relative budgets, not physical nits. The resolver
returns a `peak_lift`; the widget or custom renderer remains responsible for
applying it consistently.

`HdrPolicyTokens` provides the safety caps:

| Field | Default | Applied to |
| --- | ---: | --- |
| `max_large_area_lift` | `1.20` | Standard, focused, and semantic-accent roles. |
| `max_constrained_lift` | `1.35` | Emissive/alert roles in constrained HDR. |
| `max_emissive_lift` | `2.00` | Emissive/alert roles in full HDR. |

Disabled mode resolves every role to reference white. Wide-gamut-only mode
also prevents lift above reference white. This keeps gamut and luminance as
independent choices.

## Material tokens

`HdrMaterialTokens` contains five `MaterialToken` values:

- `flat`
- `raised`
- `glass`
- `glossy`
- `stylized`

Each material provides `opacity`, `blur_radius`, `specular_strength`, and
`rim_light_strength`. `resolve_material_role` always returns the flat material
when HDR theming is disabled; otherwise it returns the selected role.

These are compact style descriptors, not a built-in physically based material
renderer. A custom widget decides how its paint code interprets specular and rim
strength. That keeps the token API useful without forcing hidden render state.

## Effect tokens

`HdrEffectTokens` provides `focus_ring`, `glow`, and `pulse` entries. Each
`EffectToken` contains:

- `intensity`
- `speed`
- an optional `color`

If no color is set, resolution uses the widget's resolved semantic color.
Effects resolve to `None` in disabled mode and when their intensity is not
positive. The resolver describes an effect; animation timing and invalidation
remain widget responsibilities.

## Resolving a custom widget style

Custom widgets can resolve all four layers in one call:

```rust
use sui::{
    DefaultTheme, ResolvedHdrStyle, WidgetColorRole, WidgetEffectRole,
    WidgetLuminanceRole, WidgetMaterialRole, resolve_widget_hdr_style,
};

fn status_indicator_style(theme: &DefaultTheme) -> ResolvedHdrStyle {
    resolve_widget_hdr_style(
        &theme.hdr,
        WidgetColorRole::Success,
        WidgetLuminanceRole::EmissiveIndicator,
        WidgetMaterialRole::Glossy,
        Some(WidgetEffectRole::Glow),
    )
}
```

`ResolvedHdrStyle` contains:

- `color: Color`
- `peak_lift: f32`
- `material: ResolvedMaterialStyle`
- `effect: Option<ResolvedEffectStyle>`

For more control, use the individual helpers:

| Helper | Result |
| --- | --- |
| `resolve_semantic_color` | A color selected by fallback order. |
| `resolve_luminance_role` | A mode- and policy-capped lift. |
| `resolve_material_role` | A resolved material descriptor. |
| `resolve_effect_role` | An optional effect with a concrete color. |

Treat the resolved values as a budget. In particular, clamp or map authored
HDR channels to `peak_lift` instead of allowing a custom paint path to bypass
the theme policy.

## Built-in widget support

The first built-in consumers are intentionally narrow:

| Widget | Current use |
| --- | --- |
| `Button` | Accent/focus colors and semantic lift for eligible appearances. |
| `Switch` | Emissive on-state indicator styling. |
| `Popover` | Elevated surface/border styling and an optional arrival pulse. |

Pass the configured `DefaultTheme` with `.theme(theme)` or `.theme_when(...)`.
Built-in widgets currently receive themes explicitly; application-wide
inherited theme propagation is not implemented.

Widgets outside this list continue to use the ordinary palette. That is a
graceful fallback, not an indication that setting `theme.hdr.mode` converts the
entire widget library automatically.

## Using `Theme` and extensions

The application-level `Theme` owns:

- `default_widgets: DefaultTheme`, which includes `DefaultTheme::hdr`
- type-indexed `ThemeExtensions` for application-specific data

`HdrThemeTokens` can be stored as a typed extension, but built-in widgets read
the copy inside their `DefaultTheme`. If an application stores both, it must
keep them synchronized or establish one as the source of truth.

## Authoring guidelines

- Begin with SDR and verify contrast, focus, disabled, and status states there.
- Use wide gamut to preserve color identity, not to make every color louder.
- Reserve channels above `1.0` for small or transient emphasis.
- Keep body text and large surfaces near reference white.
- Prefer `ConstrainedHdr`; make `FullHdr` an explicit product choice.
- Do not infer native HDR from GPU float-texture support.
- Respect reduced-motion behavior when implementing pulse or glint effects.
- Compare the requested mode with `window_output_diagnostics` before claiming a
  physical HDR result in diagnostics or bug reports.

## Validation

Run the widget book and open **HDR theme mode lab** to compare all four modes:

```bash
cargo run -p sinomo-ui-demo
```

Generate retained HDR and SDR inspection artifacts with:

```bash
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

Focused unit coverage lives in `sinomo-ui-widgets`:

```bash
cargo test -p sinomo-ui-widgets hdr_theme
```

When adding a token or consumer, test at least:

- fallback behavior for all four modes
- policy caps for large-area and emissive roles
- disabled-mode material/effect behavior
- the widget's SDR appearance
- wide-gamut/HDR debug captures and diagnostic metadata

See the [HDR debugging guide](./hdr-debugging.md) for capture recipes and the
[testing guide](./testing.md) for screenshot, semantics, and artifact workflows.

## Future direction

The API above is implemented. The following work remains directional and is
not part of the current contract:

1. Extend token consumption beyond `Button`, `Switch`, and `Popover` only where
   a semantic HDR treatment improves hierarchy.
2. Add inherited runtime theme propagation so built-in widgets do not require
   explicit `DefaultTheme` threading.
3. Connect display diagnostics to an opt-in adaptive policy without overriding
   application intent or accessibility preferences.
4. Define richer material/effect rendering only after the compact descriptors
   have proven useful across multiple widgets.
5. Add ambient/content-aware adaptation if platforms expose reliable signals.
6. Stabilize role names after broader widget adoption rather than expanding the
   public vocabulary speculatively.

The governing design direction remains the
[HDR-native interface manifesto](./hdr-native-interface-manifesto.md); platform
presentation work is tracked separately in the
[display roadmap](./plans/hdr-wide-gamut-display-proposal.md).

## Source map

- Token types and resolvers: `crates/sui-widgets/src/hdr_theme.rs`
- Built-in theme ownership: `crates/sui-widgets/src/theme.rs`
- Pilot consumers: `crates/sui-widgets/src/controls.rs` and
  `crates/sui-widgets/src/composites.rs`
- Top-level re-exports and typed theme extensions: `crates/sui/src/lib.rs`
- Widget-book lab: `crates/sui-demo/src/widget_book/mod.rs`
