# SUI HDR Theme And Token Schema Proposal

## Companion Documents

- [Documentation Index](./README.md)
- [SUI Design](./design.md)
- [SUI HDR-Native Interface Manifesto](./hdr-native-interface-manifesto.md)
- [HDR And Wide-Gamut Display Support Proposal](./plans/hdr-wide-gamut-display-proposal.md)
- [Rendering Architecture](./renderer-architecture.md)

This document started as a design proposal for an HDR-aware theme and token schema in SUI. Parts of the model are now implemented in code, while other sections remain aspirational. The purpose of this document is therefore twofold:

- describe the schema that exists today
- preserve the larger design direction for later rollout phases

## Milestone 1 status

### Implemented today

The first milestone now exists in the codebase:

- `HdrThemeMode`, `SemanticColorToken`, `HdrColorRoles`, `HdrLuminanceTokens`, `HdrMaterialTokens`, `HdrEffectTokens`, `HdrPolicyTokens`, and `HdrThemeTokens` are real Rust APIs in `sui-widgets`
- `DefaultTheme` now carries `hdr: HdrThemeTokens`
- built-in resolver helpers map semantic roles and HDR mode into widget-ready values
- the top-level `sui` facade re-exports the HDR token types and supports storing `HdrThemeTokens` in `ThemeExtensions`
- the first built-in widgets that consume HDR tokens are:
  - `Button`
  - `Switch`
  - `Popover`
- `sui-demo` includes an HDR theme lab that compares SDR baseline, wide-gamut-only, constrained HDR, and full HDR modes
- `sui-demo` includes HDR theme mode controls plus inspection text that compares styling mode against current output policy diagnostics

### Still aspirational or incomplete

The current implementation does **not** yet mean that SUI has finished HDR-native theming. The following remain future-facing:

- broad widget adoption beyond the first pilot widgets
- inherited runtime theme propagation for built-in widgets instead of explicit `DefaultTheme` threading
- richer adaptive policies tied to ambient context, content brightness, or per-display headroom
- more advanced material/effect systems beyond the first lightweight token structs and helpers
- a stable long-term naming pass for every semantic role the broader design may want

## Summary

SUI should keep the current theme system, but expand it conceptually into four layers:

1. **Foundation tokens** for layout, typography, spacing, radii, shadows, blur, and motion.
2. **Color identity tokens** for semantic UI color roles independent of display mode.
3. **Luminance and material tokens** for HDR behavior, emissive accents, and stylized surfaces.
4. **Derived control tokens** that resolve the previous layers into widget-ready colors, metrics, and effects for a specific display policy.

The key design decision is:

> HDR behavior should not be baked into every existing palette color field. It should be modeled as a separate layer that can be composed with semantic color identity and then resolved per display mode.

That lets SUI preserve its current theme ergonomics for normal widgets while gaining a disciplined path for wide-gamut and HDR-native interfaces.

## Why a new schema is needed

The current theme model in `sui-widgets` is already a good foundation.

Today, `DefaultTheme` contains:

- `colors: ThemeColors`
- `palette: ControlPalette`
- `typography: ControlTypography`
- `metrics: ControlMetrics`
- plus spacing, text scale, radii, shadows, blur, containers, perspective, and aspect tokens

At the top level, `Theme` already supports typed extensibility through `ThemeExtensions`.

That means the current system is already close to what SUI needs. The missing piece is not "a theme system from scratch." The missing piece is a token layer that can express:

- wide-gamut variants of semantic colors
- constrained HDR vs full HDR styling intent
- luminance behavior above reference white
- material and effect classes for emissive or stylized widgets
- stateful HDR effects such as pulses, glints, and energized indicators
- display-adaptive fallback rules

## Design goals

The schema should:

- preserve the current `DefaultTheme` workflow for ordinary widget use
- fit cleanly into the top-level `Theme` + `ThemeExtensions` model
- distinguish **color identity** from **luminance behavior**
- distinguish **wide gamut** from **HDR brightness**
- allow constrained HDR by default and full HDR by opt-in
- allow widgets to request semantic effects without hardcoding presentation details
- degrade cleanly to SDR
- avoid forcing every widget to understand display hardware directly

## Non-goals

The schema should not:

- force every current widget to become HDR-aware immediately
- require every application to use HDR tokens
- make HDR styling the default for all text and surfaces
- encode low-level renderer internals directly into widget token names
- assume one fixed platform HDR model

## Core model

The proposal uses the following conceptual layers.

### Layer 1: Foundation tokens

These are the existing low-level structural tokens that should remain display-agnostic.

Examples already present in `DefaultTheme`:

- `fonts`
- `spacing`
- `breakpoints`
- `containers`
- `text`
- `font_weights`
- `tracking`
- `leading`
- `radius`
- `shadows`
- `blur`
- `perspective`
- `aspect`
- `metrics`

These should remain the base of the schema and should not become HDR-specific.

### Layer 2: Semantic color identity tokens

These represent meaning, not display output.

Examples already present in `ThemeColors`:

- `base_100`
- `base_200`
- `base_300`
- `base_content`
- `primary`
- `primary_content`
- `secondary`
- `secondary_content`
- `accent`
- `accent_content`
- `neutral`
- `neutral_content`
- `info`
- `info_content`
- `success`
- `success_content`
- `warning`
- `warning_content`
- `error`
- `error_content`

These are close to what SUI needs, but the schema should reinterpret them as **semantic identities**, not as final display-ready widget colors.

That means the conceptual meaning of `primary` is:

- the primary action hue and color identity
- not necessarily the final SDR/HDR output value used for every state

### Layer 3: Luminance and material tokens

This is the new layer.

It defines how semantic colors and surfaces behave when SUI is allowed to use headroom above reference white.

This layer should not replace `ThemeColors`. It should extend them.

### Layer 4: Derived widget tokens

These are resolved tokens used by widgets directly.

Today, this layer is represented mainly by:

- `ControlPalette`
- `ControlTypography`
- `ControlMetrics`

The proposed model keeps those derived structures, but allows their values to be computed from:

- semantic color identity
- luminance behavior
- material intent
- state
- display policy

## Proposed schema structure

At a conceptual level, the theme system should look like this:

```rust
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub default_widgets: DefaultTheme,
    pub extensions: ThemeExtensions,
}

pub struct DefaultTheme {
    pub foundation: ThemeFoundationTokens,
    pub colors: ThemeColors,
    pub palette: ControlPalette,
    pub typography: ControlTypography,
    pub metrics: ControlMetrics,
}

pub struct HdrThemeExtension {
    pub mode: HdrThemeMode,
    pub color_roles: HdrColorRoles,
    pub luminance: HdrLuminanceTokens,
    pub materials: HdrMaterialTokens,
    pub effects: HdrEffectTokens,
    pub policies: HdrPolicyTokens,
}
```

This does not mean `DefaultTheme` must literally be split immediately. It means the schema should treat those concepts separately even if the first implementation stores them differently.

## Recommended integration strategy

The code now follows a pragmatic two-step integration strategy:

1. carry the HDR-aware schema directly inside `DefaultTheme` so built-in widgets can consume it immediately
2. also re-export the same token payload through the top-level `ThemeExtensions` model so application-wide theme composition can converge on the same schema

### Current path in code

- `DefaultTheme` remains the main built-in widget theme
- `ThemeColors`, `ControlPalette`, `ControlTypography`, and `ControlMetrics` remain the SDR semantic and derived baseline
- `HdrThemeTokens` now lives directly on `DefaultTheme` as the first-wave transport for built-in widgets
- the top-level `Theme` model can still carry `HdrThemeTokens` as a typed extension for broader ecosystem use
- built-in widgets opt into explicit HDR resolvers instead of duplicating a second giant flat palette

### Longer-term path

The current implementation is intentionally incremental. Future work can still move more of the experience toward inherited runtime theme propagation once built-in widgets stop depending on explicit `DefaultTheme` threading.

## Proposed token families

## 1. Display policy tokens

These govern the overall intent for how HDR styling should behave.

```rust
pub enum HdrThemeMode {
    Disabled,
    WideGamutOnly,
    ConstrainedHdr,
    FullHdr,
}
```

### Meaning

- `Disabled`: use ordinary SDR styling behavior
- `WideGamutOnly`: permit richer gamut, but no luminance above reference white
- `ConstrainedHdr`: allow selective headroom for accents and effects
- `FullHdr`: permit stronger HDR-native materials and stylization

### Why this matters

This is the high-level styling intent. It is not the same as display capability.

The display may support HDR, but the application theme may still choose `ConstrainedHdr`.

## 2. Semantic HDR color-role tokens

These extend the existing semantic colors with HDR-aware roles.

```rust
pub struct HdrColorRoles {
    pub reference_white: SemanticColorToken,
    pub primary_action: SemanticColorToken,
    pub secondary_action: SemanticColorToken,
    pub accent: SemanticColorToken,
    pub success: SemanticColorToken,
    pub warning: SemanticColorToken,
    pub danger: SemanticColorToken,
    pub info: SemanticColorToken,
    pub selection: SemanticColorToken,
    pub focus: SemanticColorToken,
    pub indicator_live: SemanticColorToken,
    pub indicator_record: SemanticColorToken,
    pub indicator_armed: SemanticColorToken,
}
```

Each `SemanticColorToken` should conceptually contain at least:

```rust
pub struct SemanticColorToken {
    pub sdr: Color,
    pub wide_gamut: Option<Color>,
    pub hdr: Option<Color>,
}
```

### Meaning

- `sdr`: fallback / baseline identity
- `wide_gamut`: richer gamut equivalent with no extra headroom required
- `hdr`: optional variant intended for above-reference-white or HDR-native composition

### Important rule

These are still identity tokens. They do not yet encode when or how to use brightness above white. That comes from luminance behavior.

## 3. Luminance behavior tokens

This is the most important new family.

```rust
pub struct HdrLuminanceTokens {
    pub reference_white: f32,
    pub subdued_surface_max: f32,
    pub standard_chrome_max: f32,
    pub focus_lift_max: f32,
    pub semantic_accent_max: f32,
    pub emissive_indicator_max: f32,
    pub alert_pulse_peak: f32,
    pub stylized_material_peak: f32,
}
```

These values should be defined **relative to reference white**, not in absolute nits.

### Example conceptual defaults

For `ConstrainedHdr`:

```text
reference_white = 1.0
subdued_surface_max = 1.0
standard_chrome_max = 1.0
focus_lift_max = 1.08
semantic_accent_max = 1.15
emissive_indicator_max = 1.25
alert_pulse_peak = 1.35
stylized_material_peak = 1.5
```

For `FullHdr`, some values may rise, but only for appropriate widget classes.

### Why this matters

This separates:

- what a color means
- from how bright that meaning is allowed to become

That is essential for disciplined HDR UI.

## 4. Material tokens

These define surface character rather than simple color.

```rust
pub struct HdrMaterialTokens {
    pub panel: MaterialToken,
    pub popup: MaterialToken,
    pub button: MaterialToken,
    pub selected_button: MaterialToken,
    pub input: MaterialToken,
    pub glass_panel: MaterialToken,
    pub stylized_widget: MaterialToken,
    pub indicator: MaterialToken,
}
```

```rust
pub struct MaterialToken {
    pub base_surface_role: MaterialSurfaceRole,
    pub edge_highlight_strength: f32,
    pub inner_emission_strength: f32,
    pub gloss_strength: f32,
    pub translucency_strength: f32,
    pub texture_emission_mix: f32,
}
```

### Meaning

This layer gives SUI a path for:

- lit-from-within controls
- glass-like panels
- glossy selected surfaces
- stylized HDR textures

without turning every widget token into a giant hand-authored pile of colors.

## 5. Effect tokens

These define time-based or state-based HDR effects.

```rust
pub struct HdrEffectTokens {
    pub focus_glint: EffectToken,
    pub popup_arrival: EffectToken,
    pub alert_pulse: EffectToken,
    pub selected_energy: EffectToken,
    pub indicator_breathing: EffectToken,
}
```

```rust
pub struct EffectToken {
    pub enabled: bool,
    pub duration_ms: u32,
    pub attack_ms: u32,
    pub decay_ms: u32,
    pub peak_lift: f32,
    pub spatial_extent: f32,
    pub repeat_limit: u32,
}
```

### Meaning

These tokens let widgets request semantic effect classes instead of hardcoding animation and luminance behavior manually.

Examples:

- popup border sweep
- focus glint on a thumb or knob
- one-time alert pulse on a badge
- slow breathing effect for a live indicator

## 6. Policy tokens

These govern restraint and fallback.

```rust
pub struct HdrPolicyTokens {
    pub allow_hdr_text: bool,
    pub allow_hdr_body_text: bool,
    pub allow_hdr_overlays: bool,
    pub require_backplate_for_hdr_overlay_text: bool,
    pub max_large_area_lift: f32,
    pub max_idle_effect_lift: f32,
    pub reduce_motion_respects_hdr_effects: bool,
    pub clamp_to_reference_white_when_unfocused: bool,
}
```

### Why this matters

This is how SUI prevents its own HDR system from becoming visually undisciplined.

## Proposed relationship to current theme types

## `ThemeColors`

### Current role

`ThemeColors` is the base semantic color table.

### Proposed role

Keep it as the canonical semantic color layer for built-in widgets.

Future HDR-aware schemas should treat it as:

- the SDR-safe semantic baseline
- the fallback source for derived palette values
- the required minimum for all widgets, even if they ignore HDR extensions

## `ControlPalette`

### Current role

`ControlPalette` contains resolved widget-ready colors such as:

- `text`
- `surface`
- `surface_hover`
- `surface_pressed`
- `surface_focus`
- `border`
- `focus_ring`
- `accent`
- `accent_text`

### Proposed role

Keep it as the main resolved widget palette, but derive it from:

- `ThemeColors`
- current state
- display policy
- optional `HdrThemeExtension`

That means `ControlPalette` remains useful, but it should become the output of a token resolver rather than the only place where style meaning lives.

## `ControlTypography`

### Current role

This is small and pragmatic: body size and line height.

### Proposed role

Keep it, but do not overload it with HDR brightness behavior.

Instead, text luminance policy should live in HDR policy tokens, because the key question is not font size. The key question is whether specific text categories are allowed to exceed reference white.

## `ControlMetrics`

### Current role

This defines hit sizes, padding, track sizes, border widths, focus ring widths, and related geometry.

### Proposed role

Keep it as geometry and interaction structure.

Do not bake HDR styling into metrics. If some HDR effects need geometric controls, add them separately to effect/material tokens.

## Naming strategy

The schema should use names that describe intent, not immediate rendering tricks.

### Prefer names like

- `indicator_live`
- `focus_glint`
- `alert_pulse`
- `material_sheen`
- `emissive_indicator_max`
- `max_large_area_lift`
- `require_backplate_for_hdr_overlay_text`

### Avoid names like

- `super_glow`
- `neon_mode`
- `hdr_punch`
- `flashy_primary`
- `mega_bloom`

The schema should speak in terms of:

- role
- luminance behavior
- material behavior
- policy

## Resolution model

The schema should conceptually resolve in this order:

1. **Theme foundation**
2. **Semantic color identity**
3. **State**
4. **Display policy** (`Disabled`, `WideGamutOnly`, `ConstrainedHdr`, `FullHdr`)
5. **Display capability** (actual available gamut/headroom)
6. **HDR policy constraints**
7. **Derived control palette / material / effect output**

This gives SUI a clean separation between:

- authoring intent
- application preference
- renderer capability
- final resolved style

## Widget-facing API direction

Widgets should not need to understand all low-level token families.

Instead, SUI should eventually offer widget-facing style requests like:

```rust
pub enum WidgetLuminanceRole {
    Standard,
    Focused,
    SemanticAccent,
    EmissiveIndicator,
    AlertPulse,
}

pub enum WidgetMaterialRole {
    Flat,
    Raised,
    Glass,
    Glossy,
    Stylized,
}
```

Then the widget asks for a role, and the theme resolver decides the actual output.

This keeps widget code simple and makes the theme system responsible for discipline.

## Example mapping for built-in widgets

### Button

Base tokens:

- color identity: `primary_action`
- luminance role: `Standard`
- material role: `Flat` or `Raised`

Selected premium button:

- color identity: `primary_action`
- luminance role: `SemanticAccent`
- material role: `Glossy`
- optional effect: `focus_glint`

### Switch / toggle on-state indicator

Base tokens:

- color identity: `indicator_live` or `selection`
- luminance role: `EmissiveIndicator`
- material role: `Flat`

### Popup

Base tokens:

- color identity: `neutral`
- luminance role: `Standard`
- material role: `Panel` or `Glass`
- effect: `popup_arrival`

### Warning badge

Base tokens:

- color identity: `warning`
- luminance role: `AlertPulse`
- material role: `Flat`

## Recommended first implementation shape

The cleanest first implementation would be:

1. create a new typed theme extension in `sui` or `sui-widgets`, conceptually named `HdrThemeExtension`
2. keep `DefaultTheme` unchanged except for helper accessors
3. let widgets opt into querying the extension if present
4. begin by using the extension only in:
   - validation views
   - stylized experimental widgets
   - selected indicator and popup states
5. only later widen usage into more built-in widgets

This is the safest route because it matches the repo’s existing extensibility model.

## Example conceptual Rust shape

```rust
#[derive(Debug, Clone)]
pub struct HdrThemeExtension {
    pub mode: HdrThemeMode,
    pub color_roles: HdrColorRoles,
    pub luminance: HdrLuminanceTokens,
    pub materials: HdrMaterialTokens,
    pub effects: HdrEffectTokens,
    pub policies: HdrPolicyTokens,
}

impl Default for HdrThemeExtension {
    fn default() -> Self {
        Self {
            mode: HdrThemeMode::ConstrainedHdr,
            color_roles: HdrColorRoles::from_default_theme(DefaultTheme::default()),
            luminance: HdrLuminanceTokens::constrained_defaults(),
            materials: HdrMaterialTokens::default(),
            effects: HdrEffectTokens::default(),
            policies: HdrPolicyTokens::default(),
        }
    }
}
```

This is not a literal code commitment. It is the intended shape of the schema.

## Example authoring policy

A good theme authoring workflow should be:

1. define semantic base colors in `ThemeColors`
2. optionally provide wide-gamut and HDR variants in `HdrColorRoles`
3. define luminance ceilings by role in `HdrLuminanceTokens`
4. define material personality in `HdrMaterialTokens`
5. define temporal effects in `HdrEffectTokens`
6. let widgets resolve final palette values per state and display mode

This gives SUI a usable gradient from simple themes to sophisticated HDR-native themes.

## Fallback doctrine

A token schema is only good if fallback is clean.

### SDR fallback

If the display or renderer is SDR-only:

- use `ThemeColors` and standard `ControlPalette`
- ignore HDR headroom behavior
- preserve semantic identity and hierarchy
- preserve materials as much as possible via contrast, texture, and non-HDR polish

### Wide-gamut SDR fallback

If wide gamut is available but HDR headroom is not:

- prefer `wide_gamut` variants when present
- keep all luminance at or below reference white
- preserve richer color without introducing fake HDR behavior

### Constrained HDR fallback

If full HDR is unavailable but constrained HDR is practical:

- allow selective luminance lift for roles like indicators and popup effects
- clamp large-area surfaces and text according to policy tokens

## Validation requirements for the schema

Before the schema should be considered stable, SUI should be able to validate:

- the same semantic theme in SDR, wide-gamut SDR, constrained HDR, and full HDR modes
- body text remaining readable and not over-bright
- indicator and alert roles visibly differentiating from standard chrome
- popup arrival and focus effects remaining restrained
- stylized widgets degrading gracefully on SDR paths
- no accidental large-area brightness inflation

## Recommended file ownership if implemented later

If this proposal is implemented, likely ownership should look like:

- `crates/sui-widgets/src/theme.rs`
  - continue owning `DefaultTheme`, `ThemeColors`, `ControlPalette`, `ControlTypography`, `ControlMetrics`
- `crates/sui/src/lib.rs`
  - continue owning top-level `Theme` and `ThemeExtensions`
- a future file such as `crates/sui-widgets/src/hdr_theme.rs` or `crates/sui/src/hdr_theme.rs`
  - own `HdrThemeExtension` and related token families

## Final recommendation

SUI should not replace its current theme model.

It should evolve it by treating the current model as:

- a strong SDR and wide-gamut baseline
- a semantic identity layer
- and a derivation source for more advanced HDR-native styling

The right schema for SUI is therefore:

- **foundationally compatible** with `DefaultTheme`
- **extension-based** for HDR-specific behavior at first
- **role-driven** rather than effect-driven
- **display-adaptive** rather than hardcoded
- **strictly separated** between color identity, luminance behavior, material behavior, and final widget palette resolution

## One-sentence summary

> SUI should represent HDR-native styling as an extension of its current semantic theme model: keep `ThemeColors` as the identity layer, add typed HDR luminance/material/effect tokens beside it, and resolve those layers into widget-ready palette values according to state, policy, and actual display capability.
