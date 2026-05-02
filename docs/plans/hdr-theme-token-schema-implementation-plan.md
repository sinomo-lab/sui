# HDR Theme And Token Schema Implementation Plan

**Goal:** Implement an HDR-aware theme/token system for SUI that preserves the current `DefaultTheme` workflow, adds a disciplined schema for wide-gamut and HDR-native styling, and proves the model in a small set of built-in widgets plus widget-book validation surfaces.

**Architecture:** Introduce a lightweight HDR token layer in `sui-widgets`, keep `ThemeColors` as the semantic identity baseline, and resolve HDR luminance/material/effect behavior into widget-ready values through explicit helper APIs instead of baking HDR behavior directly into every existing palette field. Because built-in widgets currently consume `DefaultTheme` directly rather than reading top-level `ThemeExtensions` from runtime contexts, the first implementation should thread HDR tokens through `DefaultTheme` itself and only then add a top-level `ThemeExtension` bridge for broader ecosystem use.

**Tech Stack:** Rust 2024, current `sui-widgets` theme model in `crates/sui-widgets/src/theme.rs`, top-level `Theme` and `ThemeExtensions` in `crates/sui/src/lib.rs`, built-in widgets in `crates/sui-widgets/src/controls.rs` / `composites.rs` / `text_surface.rs`, and HDR validation/demo coverage in `crates/sui-widget-book/src/lib.rs`.

---

## Scope guardrails

- Do **not** replace the current `ThemeColors`, `ControlPalette`, `ControlTypography`, or `ControlMetrics` model wholesale.
- Do **not** make HDR styling the default for all widgets.
- Keep the first implementation **copy-friendly and lightweight** so `DefaultTheme` can stay ergonomic and cheap to pass by value.
- Keep the first rollout to a small number of built-in widgets:
  - `Button`
  - `Switch`
  - one popup-like composite (`Popover` or `Tooltip`-adjacent styling path)
- Use widget-book and focused tests to prove the schema before widening adoption.
- Preserve SDR behavior when HDR theme mode is disabled.

---

## Current architectural constraints to respect

These facts should shape the implementation:

1. `DefaultTheme` lives in `crates/sui-widgets/src/theme.rs` and is passed directly into built-in widgets via `.theme(DefaultTheme)`.
2. Built-in widgets do **not** currently read the top-level `sui::Theme` or `ThemeExtensions` from runtime contexts.
3. Top-level `ThemeExtensions` already exist in `crates/sui/src/lib.rs`, but they are currently better suited for ecosystem-wide typed extensions than for first-wave built-in widget adoption.
4. Existing widget rendering code largely consumes `ControlPalette`, `ControlTypography`, and `ControlMetrics`, so the first HDR token rollout should resolve into those surfaces or into explicit helper methods adjacent to them.

Implication:

- the first implementation should add an HDR token layer to `DefaultTheme`
- then add a top-level `ThemeExtension` bridge as a second step for consistency with the long-term architecture

---

## Phase 1 definition of done

Phase 1 is complete when:

- SUI has a concrete `HdrThemeTokens` schema integrated into `DefaultTheme`
- the schema includes at least:
  - display policy mode
  - semantic HDR color-role tokens
  - luminance tokens
  - material tokens
  - effect tokens
  - policy tokens
- there are explicit resolver helpers that map semantic identity + state + HDR mode into widget-consumable values
- the default SDR widget appearance is unchanged when HDR mode is disabled
- at least `Button`, `Switch`, and one popup/composite path can consume the new tokens
- widget-book exposes a dedicated HDR theme/token demo or validation story
- focused tests and `cargo check` pass for touched crates

---

### Task 1: Add the core HDR token types in `sui-widgets` (test-first)
**Objective:** Create the token schema as a real Rust API without yet changing widget rendering behavior.

**Files:**
- Create: `crates/sui-widgets/src/hdr_theme.rs`
- Modify: `crates/sui-widgets/src/lib.rs`
- Test: `crates/sui-widgets/src/hdr_theme.rs`

**Step 1: Write failing unit tests for the new API surface**
Add tests covering at least:
- `HdrThemeMode::Disabled` as the safe default
- `HdrLuminanceTokens::constrained_defaults()` staying at or above `reference_white`
- `HdrColorRoles::from_default_theme(DefaultTheme::default())` preserving semantic identity fallbacks
- token structs staying `Clone + Copy + Debug + PartialEq` where practical

**Suggested API surface for the first version:**
- `pub enum HdrThemeMode { Disabled, WideGamutOnly, ConstrainedHdr, FullHdr }`
- `pub struct SemanticColorToken { sdr: Color, wide_gamut: Option<Color>, hdr: Option<Color> }`
- `pub struct HdrColorRoles { ... }`
- `pub struct HdrLuminanceTokens { ... }`
- `pub struct MaterialToken { ... }`
- `pub struct HdrMaterialTokens { ... }`
- `pub struct EffectToken { ... }`
- `pub struct HdrEffectTokens { ... }`
- `pub struct HdrPolicyTokens { ... }`
- `pub struct HdrThemeTokens { ... }`

**Step 2: Run the focused test target**
Run:
```bash
cargo test -p sui-widgets --lib hdr_theme::tests::hdr_theme_tokens_default_to_disabled_mode -- --exact
```
Expected: FAIL

**Step 3: Implement the minimal token structs and defaults**
Implementation notes:
- keep the first version lightweight and copy-friendly
- keep values relative to reference white rather than in nits
- add `from_default_theme(...)` helpers so tokens can derive from existing semantic colors cleanly

**Step 4: Re-run the focused tests and make them pass**
Run:
```bash
cargo test -p sui-widgets --lib hdr_theme::tests::hdr_theme_tokens_default_to_disabled_mode -- --exact
cargo test -p sui-widgets --lib hdr_theme::tests::hdr_color_roles_derive_from_default_theme_semantics -- --exact
```

**Step 5: Export the new module from `sui-widgets`**
Update `crates/sui-widgets/src/lib.rs` to `pub mod hdr_theme;` and `pub use` the new types.

**Step 6: Commit**
```bash
git add crates/sui-widgets/src/hdr_theme.rs crates/sui-widgets/src/lib.rs
git commit -m "feat: add hdr theme token types"
```

---

### Task 2: Thread HDR tokens into `DefaultTheme` without breaking existing behavior (test-first)
**Objective:** Make the built-in widget theme capable of carrying HDR token data.

**Files:**
- Modify: `crates/sui-widgets/src/theme.rs`
- Test: `crates/sui-widgets/src/theme.rs`

**Step 1: Write failing tests for `DefaultTheme` integration**
Add tests covering:
- `DefaultTheme::default()` includes default HDR tokens in disabled mode
- `DefaultTheme::light()` and `DefaultTheme::dark()` derive sensible HDR role colors from their existing semantic colors
- `sync_derived_fields()` preserves or re-derives HDR token data correctly when base semantic colors change

**Recommended shape:**
Add a field such as:
```rust
pub hdr: HdrThemeTokens,
```

**Step 2: Run the focused tests**
Run:
```bash
cargo test -p sui-widgets --lib theme::tests::default_theme_initializes_hdr_tokens -- --exact
```
Expected: FAIL

**Step 3: Implement the `DefaultTheme` integration**
Implementation notes:
- initialize `hdr` in `DefaultTheme::from_colors(...)`
- keep `ThemeColors` as the source of SDR semantic identity
- update `sync_derived_fields()` so HDR tokens remain aligned with semantic colors where intended
- do not change `ControlPalette` semantics yet

**Step 4: Re-run targeted tests**
Run:
```bash
cargo test -p sui-widgets --lib theme::tests::default_theme_initializes_hdr_tokens -- --exact
cargo test -p sui-widgets --lib theme::tests::sync_derived_fields_updates_hdr_semantic_fallbacks -- --exact
```

**Step 5: Commit**
```bash
git add crates/sui-widgets/src/theme.rs
git commit -m "feat: add hdr tokens to default theme"
```

---

### Task 3: Add widget-facing HDR resolution helpers (test-first)
**Objective:** Resolve semantic HDR tokens into values widgets can actually use.

**Files:**
- Modify: `crates/sui-widgets/src/hdr_theme.rs`
- Possibly modify: `crates/sui-widgets/src/theme.rs`
- Test: `crates/sui-widgets/src/hdr_theme.rs`

**Step 1: Write failing tests for resolver behavior**
Add tests covering:
- disabled mode resolves to SDR semantic values only
- wide-gamut-only mode prefers `wide_gamut` variants but clamps luminance to reference white
- constrained HDR mode allows only limited role-specific lift
- full HDR mode still respects policy tokens such as `max_large_area_lift`

**Suggested widget-facing types:**
- `pub enum WidgetLuminanceRole { Standard, Focused, SemanticAccent, EmissiveIndicator, AlertPulse }`
- `pub enum WidgetMaterialRole { Flat, Raised, Glass, Glossy, Stylized }`
- `pub struct ResolvedHdrStyle { color: Color, peak_lift: f32, material: ResolvedMaterialStyle, effect: Option<ResolvedEffectStyle> }`

**Step 2: Run the focused tests**
Run:
```bash
cargo test -p sui-widgets --lib hdr_theme::tests::disabled_mode_resolves_to_sdr_semantics -- --exact
cargo test -p sui-widgets --lib hdr_theme::tests::constrained_hdr_caps_emissive_roles_below_full_hdr -- --exact
```
Expected: FAIL

**Step 3: Implement minimal resolvers**
Recommended helpers:
- `resolve_semantic_color(...)`
- `resolve_luminance_role(...)`
- `resolve_material_role(...)`
- a combined helper like `resolve_widget_hdr_style(...)`

**Step 4: Re-run focused tests and `cargo check`**
Run:
```bash
cargo test -p sui-widgets --lib hdr_theme::tests::disabled_mode_resolves_to_sdr_semantics -- --exact
cargo test -p sui-widgets --lib hdr_theme::tests::constrained_hdr_caps_emissive_roles_below_full_hdr -- --exact
cargo check -p sui-widgets
```

**Step 5: Commit**
```bash
git add crates/sui-widgets/src/hdr_theme.rs crates/sui-widgets/src/theme.rs
git commit -m "feat: add hdr theme resolution helpers"
```

---

### Task 4: Add the top-level `ThemeExtension` bridge in `sui` (test-first)
**Objective:** Keep the schema aligned with the broader SUI theme architecture, not just built-in widgets.

**Files:**
- Modify: `crates/sui/src/lib.rs`
- Test: `crates/sui/src/lib.rs`

**Step 1: Write failing tests for extension round-tripping**
Add tests covering:
- `Theme::new().with_extension(HdrThemeExtension::default())` stores the extension
- the extension can be retrieved by type
- the extension can be layered alongside a customized `DefaultTheme`

**Recommended shape:**
- re-export the token types through `sui`
- add a distinct top-level bridge type only if needed; otherwise re-use `HdrThemeTokens` as the extension payload

**Step 2: Run the focused tests**
Run:
```bash
cargo test -p sui --lib tests::theme_extensions_round_trip_hdr_theme_tokens -- --exact
```
Expected: FAIL

**Step 3: Implement the minimal bridge**
Implementation notes:
- prefer re-exporting `HdrThemeTokens` / related enums through `sui`
- avoid duplicating the token schema unless there is a compelling ownership boundary reason

**Step 4: Re-run the exact test and `cargo check -p sui`**

**Step 5: Commit**
```bash
git add crates/sui/src/lib.rs
git commit -m "feat: expose hdr theme tokens through sui facade"
```

---

### Task 5: Pilot the new schema in `Button` (test-first)
**Objective:** Prove the schema can change a real built-in widget without destabilizing defaults.

**Files:**
- Modify: `crates/sui-widgets/src/controls.rs`
- Test: `crates/sui-widgets/src/controls.rs`

**Step 1: Write failing tests for button resolution behavior**
Add tests covering:
- default button rendering remains unchanged when HDR mode is disabled
- a button themed with constrained HDR tokens can resolve a semantic accent/focus style differently from SDR
- HDR-aware button styles do not force body text above policy limits

**Recommended implementation target:**
- map button chrome to `WidgetLuminanceRole::Standard` by default
- map focused/primary button variants to `WidgetLuminanceRole::SemanticAccent`
- optionally apply `WidgetMaterialRole::Raised` / `Glossy` only when the theme explicitly enables it

**Step 2: Run the focused tests**
Run:
```bash
cargo test -p sui-widgets --lib controls::tests::button_preserves_sdr_palette_when_hdr_mode_disabled -- --exact
cargo test -p sui-widgets --lib controls::tests::button_can_resolve_constrained_hdr_accent_style -- --exact
```
Expected: FAIL

**Step 3: Implement the minimal `Button` integration**
Keep the first pass conservative:
- preserve current palette values when HDR is disabled
- only branch into the HDR resolver when the theme mode is not disabled
- do not yet add glow or pulse behavior here

**Step 4: Re-run the focused tests**

**Step 5: Commit**
```bash
git add crates/sui-widgets/src/controls.rs
git commit -m "feat: pilot hdr theme tokens in button styling"
```

---

### Task 6: Pilot the new schema in `Switch` and one popup/composite path (test-first)
**Objective:** Prove semantic indicators and arrival effects can be expressed through tokens.

**Files:**
- Modify: `crates/sui-widgets/src/controls.rs`
- Modify: `crates/sui-widgets/src/composites.rs`
- Test: `crates/sui-widgets/src/controls.rs`
- Test: `crates/sui-widgets/src/composites.rs`

**Step 1: Write failing tests for `Switch`**
Add tests covering:
- on-state switch indicators can use `WidgetLuminanceRole::EmissiveIndicator`
- label readability remains unchanged in disabled mode
- constrained HDR does not overshoot full-HDR limits

**Step 2: Write failing tests for popup/composite arrival**
Choose one concrete composite path such as `Popover` or `Tooltip` and test:
- popup arrival can request a semantic arrival effect token
- resting state settles to a calm surface
- no effect is applied when disabled mode is active

**Step 3: Run focused tests**
Run:
```bash
cargo test -p sui-widgets --lib controls::tests::switch_on_state_can_use_emissive_indicator_role -- --exact
cargo test -p sui-widgets --lib composites::tests::popover_arrival_effect_obeys_hdr_theme_mode -- --exact
```
Expected: FAIL

**Step 4: Implement minimal integration**
Implementation notes:
- `Switch` is the best first candidate for a true emissive semantic role
- popup/composite integration should prefer a restrained pulse or rim-sweep model, not a constant glow

**Step 5: Re-run focused tests and `cargo check -p sui-widgets`**

**Step 6: Commit**
```bash
git add crates/sui-widgets/src/controls.rs crates/sui-widgets/src/composites.rs
git commit -m "feat: apply hdr theme roles to switch and popup states"
```

---

### Task 7: Add a widget-book HDR theme lab / validation story (test-first)
**Objective:** Give the new token schema a visible proving ground and a regression surface.

**Files:**
- Modify: `crates/sui-widget-book/src/lib.rs`
- Test: `crates/sui-widget-book/src/lib.rs`

**Step 1: Add a new story or panel that compares modes**
The story should expose at least:
- SDR baseline
- Wide-gamut-only
- Constrained HDR
- Full HDR

Include at minimum:
- one button sample
- one switch sample
- one popup/attention sample
- explanatory labels for the current token mode

**Step 2: Write failing tests for semantics exposure**
Add tests covering:
- the story is present in widget-book
- its comparison surfaces are named clearly in semantics
- the new HDR token modes are represented in the story content

**Suggested test names:**
- `tests::hdr_theme_lab_exposes_mode_comparison_sections`
- `tests::hdr_theme_lab_includes_emissive_indicator_and_popup_examples`

**Step 3: Run the focused tests**
Run:
```bash
cargo test -p sui-widget-book --lib tests::hdr_theme_lab_exposes_mode_comparison_sections -- --exact
```
Expected: FAIL

**Step 4: Implement the lab story**
Implementation notes:
- keep the first pass explicit and didactic, not visually overbuilt
- prefer side-by-side named sections with clear semantics labels
- use the new token schema to construct theme variants rather than hardcoding raw colors directly in the story

**Step 5: Re-run focused tests and `cargo check -p sui-widget-book`**

**Step 6: Commit**
```bash
git add crates/sui-widget-book/src/lib.rs
git commit -m "feat: add widget-book hdr theme lab"
```

---

### Task 8: Add SUI Dev inspection hooks for the schema (optional but recommended)
**Objective:** Make experimentation possible in the main development host without depending only on widget-book stories.

**Files:**
- Modify: `crates/sui-dev/src/app.rs`
- Test: `crates/sui-dev/src/app.rs`

**Step 1: Add a small inspection panel or selector set for HDR theme mode**
At minimum expose:
- current HDR theme mode
- a way to switch among modes
- lines showing whether the current window is in SDR / wide-gamut / HDR output policy for comparison against styling mode

**Step 2: Write a focused regression test**
Suggested test:
- `app::tests::settings_view_exposes_hdr_theme_mode_controls`

**Step 3: Run the focused test and verify failure**

**Step 4: Implement the minimal inspection UI**

**Step 5: Re-run the test and `cargo check -p sui-dev`**

**Step 6: Commit**
```bash
git add crates/sui-dev/src/app.rs
git commit -m "feat: expose hdr theme mode controls in sui dev"
```

---

### Task 9: Tighten documentation after the first working milestone
**Objective:** Keep the docs set coherent once the token schema exists in code.

**Files:**
- Modify: `docs/hdr-theme-token-schema-proposal.md`
- Modify: `docs/hdr-native-interface-manifesto.md`
- Modify: `docs/design.md`
- Possibly modify: `docs/README.md`

**Step 1:** Update the docs to distinguish clearly between:
- implemented schema pieces
- aspirational schema pieces
- rollout status for built-in widgets

**Step 2:** Add a short section listing the first widgets that actually consume HDR tokens.

**Step 3:** Verify with:
```bash
git diff -- docs/
```

**Step 4:** Commit with message like:
```bash
git add docs/
git commit -m "docs: update hdr theme schema rollout status"
```

---

## Recommended validation pass after Phase 1

After Tasks 1 through 7, run at least:

```bash
cargo test -p sui-widgets --lib -- --nocapture
cargo test -p sui --lib -- --nocapture
cargo test -p sui-widget-book --lib -- --nocapture
cargo check -p sui-widgets
cargo check -p sui
cargo check -p sui-widget-book
```

If Task 8 is implemented, also run:

```bash
cargo check -p sui-dev
```

---

## Suggested commit structure

Keep commits small and milestone-based:

1. `feat: add hdr theme token types`
2. `feat: add hdr tokens to default theme`
3. `feat: add hdr theme resolution helpers`
4. `feat: expose hdr theme tokens through sui facade`
5. `feat: pilot hdr theme tokens in button styling`
6. `feat: apply hdr theme roles to switch and popup states`
7. `feat: add widget-book hdr theme lab`
8. `feat: expose hdr theme mode controls in sui dev` (optional)
9. `docs: update hdr theme schema rollout status`

---

## Risks and pitfalls

### 1. Built-in widget transport mismatch
The biggest risk is trying to rely only on `ThemeExtensions` too early. Built-in widgets currently use `DefaultTheme` directly, so the first implementation must carry HDR token data through `DefaultTheme` itself or provide an equivalent direct transport.

### 2. Copy inflation
`DefaultTheme` is currently `Copy`. If HDR token structs become too large or heap-backed, the ergonomics and performance assumptions of existing widget APIs may degrade. Keep first-wave tokens lightweight.

### 3. Palette duplication
Avoid copying every existing `ControlPalette` field into a parallel HDR palette. The point of this schema is to separate identity, behavior, and resolution, not to create two giant flat palettes.

### 4. Widget overreach
Do not wire every widget into HDR in the first pass. The schema needs one or two strong pilots, not a broad noisy rollout.

### 5. Visual noise
Constrained HDR must stay the default for the initial design language. Full HDR should be opt-in and rare.

### 6. Test naming drift
This repo often needs fully qualified Rust test paths for `--exact`. Use concrete `module::tests::...` or `tests::...` names when adding targeted tests.

---

## Final recommendation

Implement this in two layers:

1. **Practical built-in widget path:** add lightweight HDR tokens to `DefaultTheme` and resolve them in a small number of widgets.
2. **Architectural bridge path:** re-export the same token schema through the top-level `ThemeExtensions` model so applications and third-party widget libraries can converge on the same concepts.

That gives SUI a real, testable HDR token system soon without pretending the current widget architecture is already fully theme-extension-driven.
