# Pathfinder-Inspired Text Rendering Improvement Plan

Goal: Improve small-text readability and overall text quality in SUI by adding a real LCD/subpixel text path, small-size hinting controls, stem-darkening experiments, and benchmark/visual comparison surfaces without replacing the existing atlas-first renderer architecture.

Architecture: Keep SUI’s current atlas-backed text pipeline as the default rendering path, but split text rendering policy into explicit modes instead of treating all text as grayscale coverage. Add a true subpixel atlas path that preserves per-channel coverage from swash, then layer in optional small-size hinting and stem darkening behind renderer-facing settings and benchmark surfaces in sui-dev. Reserve any Pathfinder-style analytic/vector text work as a later hybrid path for selected cases such as large transformed text.

Tech Stack: sui-render-wgpu, sui-dev, sui-widget-book, swash, wgpu, Trunk web benchmarks, existing SUI renderer settings hooks.

---

## Why this plan exists

Pathfinder’s text rendering story highlights four areas that SUI can concretely improve today:

1. Preserve subpixel coverage instead of collapsing RGB masks to grayscale.
2. Use slight hinting at small ppem sizes.
3. Add stem darkening/font dilation for thin UI text.
4. Measure quality and performance with dedicated benchmark and visual comparison surfaces.

Current SUI state relevant to this plan:

- `crates/sui-render-wgpu/src/scene.rs:1916-1923` uses `SwashRender` with `SwashFormat::Subpixel`.
- `crates/sui-render-wgpu/src/scene.rs:2021-2043` immediately averages the RGB subpixel mask into grayscale alpha.
- `crates/sui-render-wgpu/src/scene.rs:1916-1920` builds the swash scaler with `.hint(false)`.
- `crates/sui-render-wgpu/src/lib.rs:115-150` already exposes `TextCoveragePolicy` hooks.
- `crates/sui-dev/src/app.rs:406-520` already exposes renderer controls in the dev workspace.
- `crates/sui-dev/web/index.html` and `crates/sui-dev/src/lib.rs` now support wasm benchmark modes.

This means the shortest path is not “replace the renderer with Pathfinder”, but rather “use SUI’s policy hooks to preserve more of the raster signal and expose small-text controls explicitly”.

---

## Non-goals

- Do not replace the atlas renderer with a full vector/analytic text renderer in this iteration.
- Do not ship LCD/subpixel AA for arbitrary transformed text.
- Do not silently change all text rendering defaults without side-by-side validation.
- Do not couple this work to a larger text subsystem rewrite beyond the existing persistent-layout direction already documented in `docs/text-system.md`.

---

## Deliverables

By the end of this plan, the repo should contain:

- A renderer-facing text mode that distinguishes grayscale coverage from LCD/subpixel coverage.
- A true subpixel atlas path that preserves RGB coverage from swash.
- Hinting controls for small-size text rendering.
- Stem-darkening controls/experiments for small-size text rendering.
- Dev workspace UI controls and explanatory copy for the new text modes.
- Dedicated visual comparison/benchmark surfaces for native and wasm validation.
- Tests covering policy parsing, cache key behavior, and representative raster conversion logic.
- Documentation describing the new modes and how to validate them.

---

## Task 1: Introduce explicit text render modes at the renderer boundary

Objective: Replace the current “coverage policy only” mental model with explicit render modes so grayscale AA, subpixel AA, and future modes can be reasoned about independently.

Files:
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/text.rs`
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`

Step 1: Write failing tests for the new renderer mode model

Add tests that express the desired API surface, for example:

```rust
#[test]
fn text_render_mode_defaults_to_grayscale() {
    assert_eq!(TextRenderMode::default(), TextRenderMode::Grayscale);
}

#[test]
fn lcd_text_render_mode_has_distinct_cache_identity() {
    assert_ne!(
        TextAtlasColorMode::from(TextRenderMode::Grayscale),
        TextAtlasColorMode::from(TextRenderMode::LcdSubpixel),
    );
}
```

Step 2: Run the tests to verify failure

Run:

```bash
cargo test -p sui-render-wgpu text_render_mode_defaults_to_grayscale -- --exact
```

Expected: FAIL — the new types do not exist yet.

Step 3: Add the minimal implementation

Create an explicit renderer-facing mode, for example:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextRenderMode {
    #[default]
    Grayscale,
    LcdSubpixel,
}
```

Then update the glyph cache key and any atlas metadata so grayscale and subpixel entries do not alias.

Step 4: Run tests to verify pass

Run:

```bash
cargo test -p sui-render-wgpu text_render_mode_defaults_to_grayscale -- --exact
cargo check -p sui-render-wgpu
```

Expected: PASS

Step 5: Commit

```bash
git add crates/sui-render-wgpu/src/lib.rs crates/sui-render-wgpu/src/text.rs crates/sui-render-wgpu/src/scene.rs
git commit -m "feat: add explicit text render modes"
```

---

## Task 2: Preserve real LCD/subpixel coverage instead of collapsing to grayscale

Objective: Keep RGB coverage from swash `SubpixelMask` glyphs and feed it through a renderer path that can use per-channel coverage.

Files:
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-render-wgpu/src/text.rs`
- Modify: `crates/sui-render-wgpu/src/gpu.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/scene.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`

Step 1: Write failing tests for subpixel preservation

Add tests that make the desired behavior explicit. At minimum, add a helper-level test proving RGB channels are not averaged away in LCD mode.

Example test shape:

```rust
#[test]
fn subpixel_mask_preserves_distinct_rgb_channels_in_lcd_mode() {
    let source = [255u8, 128u8, 32u8, 255u8];
    let converted = convert_subpixel_texel_for_mode(source, TextRenderMode::LcdSubpixel);
    assert_eq!(converted[0], 255);
    assert_eq!(converted[1], 128);
    assert_eq!(converted[2], 32);
}
```

Step 2: Run tests to verify failure

Run:

```bash
cargo test -p sui-render-wgpu subpixel_mask_preserves_distinct_rgb_channels_in_lcd_mode -- --exact
```

Expected: FAIL

Step 3: Implement the minimal path

Implementation requirements:

- Split `SwashImageContent::SubpixelMask` handling into:
  - grayscale path
  - LCD/subpixel path
- In LCD mode, keep per-channel coverage.
- Add an atlas/shader path that multiplies text color with per-channel mask coverage instead of one scalar alpha.
- Keep the color-glyph path unchanged.

A minimal helper structure could look like:

```rust
pub(crate) enum AtlasTextContent {
    GrayscaleAlpha,
    LcdSubpixel,
    Color,
}
```

And the shader-side interpretation should preserve separate RGB coverage for LCD glyphs.

Step 4: Validate the implementation

Run:

```bash
cargo test -p sui-render-wgpu subpixel_mask_preserves_distinct_rgb_channels_in_lcd_mode -- --exact
cargo check -p sui-render-wgpu
cargo check -p sui-dev
cargo check -p sui-dev --target wasm32-unknown-unknown --no-default-features --features web
```

Expected: PASS

Step 5: Commit

```bash
git add crates/sui-render-wgpu/src/scene.rs crates/sui-render-wgpu/src/text.rs crates/sui-render-wgpu/src/gpu.rs crates/sui-render-wgpu/src/lib.rs
git commit -m "feat: preserve lcd subpixel glyph coverage"
```

---

## Task 3: Gate LCD/subpixel rendering to safe conditions

Objective: Prevent incorrect LCD AA on transformed or unsuitable text.

Files:
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/scene.rs`

Step 1: Write failing tests describing when LCD is allowed

Example:

```rust
#[test]
fn lcd_text_is_rejected_for_non_axis_aligned_transform() {
    assert!(!allows_lcd_text(transform_with_rotation()));
}

#[test]
fn lcd_text_is_allowed_for_axis_aligned_translation() {
    assert!(allows_lcd_text(Transform::IDENTITY));
}
```

Step 2: Run to verify failure

Run:

```bash
cargo test -p sui-render-wgpu lcd_text_is_allowed_for_axis_aligned_translation -- --exact
```

Expected: FAIL

Step 3: Implement the guard logic

The first implementation should be conservative. LCD mode should only be used when all of the following are true:

- transform is axis-aligned
- no rotation or shear
- glyph atlas pixel snapping is enabled
- renderer is using the atlas path, not a fallback path intended for arbitrary transforms

Pseudo-code:

```rust
fn effective_text_render_mode(
    requested: TextRenderMode,
    transform: Transform,
) -> TextRenderMode {
    if requested == TextRenderMode::LcdSubpixel
        && is_axis_aligned(transform)
    {
        TextRenderMode::LcdSubpixel
    } else {
        TextRenderMode::Grayscale
    }
}
```

Step 4: Verify pass

Run:

```bash
cargo test -p sui-render-wgpu lcd_text_is_rejected_for_non_axis_aligned_transform -- --exact
cargo check -p sui-render-wgpu
```

Step 5: Commit

```bash
git add crates/sui-render-wgpu/src/scene.rs crates/sui-render-wgpu/src/lib.rs
git commit -m "feat: gate lcd text to axis-aligned atlas cases"
```

---

## Task 4: Add slight hinting support for small text sizes

Objective: Make small text more readable by allowing hinting below a tunable ppem threshold.

Files:
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-dev/src/app.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/scene.rs`

Step 1: Write failing tests for hinting policy

Example:

```rust
#[test]
fn slight_hinting_enables_below_threshold() {
    let config = TextHinting::Slight { max_ppem: 18.0 };
    assert!(config.should_hint(14.0));
    assert!(!config.should_hint(24.0));
}
```

Step 2: Run to verify failure

Run:

```bash
cargo test -p sui-render-wgpu slight_hinting_enables_below_threshold -- --exact
```

Expected: FAIL

Step 3: Implement the minimal feature

Add a renderer-facing hinting setting, for example:

```rust
pub enum TextHinting {
    None,
    Slight { max_ppem: f32 },
}
```

Then update swash scaler creation so hinting is used only when the policy says it should be.

Current code to replace or extend:
- `crates/sui-render-wgpu/src/scene.rs:1916-1920`

Step 4: Wire through dev settings

Add controls in `RenderSettingsTab` for:
- hinting enabled/disabled
- max hinted ppem threshold

Step 5: Verify pass

Run:

```bash
cargo test -p sui-render-wgpu slight_hinting_enables_below_threshold -- --exact
cargo check -p sui-dev
```

Step 6: Commit

```bash
git add crates/sui-render-wgpu/src/lib.rs crates/sui-render-wgpu/src/scene.rs crates/sui-dev/src/app.rs
git commit -m "feat: add small-text slight hinting controls"
```

---

## Task 5: Add optional stem darkening/font dilation

Objective: Improve small dark-on-light UI text by slightly increasing effective stroke weight at small sizes.

Files:
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-dev/src/app.rs`
- Test: `crates/sui-render-wgpu/src/scene.rs`

Step 1: Write failing tests for the darkening policy

Example:

```rust
#[test]
fn stem_darkening_applies_only_below_threshold() {
    let config = StemDarkening::Enabled { max_ppem: 18.0, amount: 0.08 };
    assert!(config.effective_amount(14.0) > 0.0);
    assert_eq!(config.effective_amount(24.0), 0.0);
}
```

Step 2: Run to verify failure

Run:

```bash
cargo test -p sui-render-wgpu stem_darkening_applies_only_below_threshold -- --exact
```

Expected: FAIL

Step 3: Implement the minimal behavior

Start with a conservative experiment:
- make it optional
- apply only to grayscale and LCD text paths used for non-color glyphs
- scale by ppem threshold

Do not attempt a perfect FreeType clone in the first pass. The first milestone is a controllable, benchmarkable bias.

Possible implementation points:
- coverage remap during `swash_image_to_rgba`
- or a tiny dilation/convolution pass before atlas upload

Step 4: Expose controls in sui-dev

Add settings for:
- on/off
- amount
- max ppem threshold

Step 5: Verify pass

Run:

```bash
cargo test -p sui-render-wgpu stem_darkening_applies_only_below_threshold -- --exact
cargo check -p sui-dev
cargo check -p sui-dev --target wasm32-unknown-unknown --no-default-features --features web
```

Step 6: Commit

```bash
git add crates/sui-render-wgpu/src/lib.rs crates/sui-render-wgpu/src/scene.rs crates/sui-dev/src/app.rs
git commit -m "feat: add optional small-text stem darkening"
```

---

## Task 6: Build a side-by-side text rendering comparison surface

Objective: Make regressions and improvements obvious by rendering the same text samples through multiple modes side by side.

Files:
- Modify: `crates/sui-widget-book/src/lib.rs`
- Modify: `crates/sui-dev/src/app.rs`
- Test: `crates/sui-widget-book/tests/desktop_e2e.rs`
- Optional docs: `docs/text-system.md`

Step 1: Write a failing test describing the comparison surface

Example:

```rust
#[test]
fn text_rendering_comparison_surface_exposes_all_render_modes() {
    let app = build_text_rendering_comparison_application();
    // Assert expected labels/semantics for grayscale, lcd, hinted, darkened variants.
}
```

Step 2: Run to verify failure

Run:

```bash
cargo test -p sui-widget-book text_rendering_comparison_surface_exposes_all_render_modes -- --exact
```

Expected: FAIL

Step 3: Implement the comparison surface

The surface should include:
- dark text on light background
- light text on dark background
- small label text (10–14 px)
- medium UI text
- mixed-script samples
- repeated stems such as `ill`, `scroll`, `minimum`, `Hello`, `Ж`, `中`
- a mode matrix:
  - grayscale baseline
  - grayscale + hinting
  - grayscale + stem darkening
  - lcd subpixel
  - lcd subpixel + hinting
  - lcd subpixel + hinting + stem darkening

Step 4: Verify pass

Run:

```bash
cargo check -p sui-widget-book
cargo check -p sui-dev
```

Step 5: Commit

```bash
git add crates/sui-widget-book/src/lib.rs crates/sui-dev/src/app.rs
# plus tests/docs if added
git commit -m "feat: add text rendering comparison surface"
```

---

## Task 7: Extend wasm benchmark support to text-focused benchmark presets

Objective: Measure the real-world impact of text improvements on the web build.

Files:
- Modify: `crates/sui-dev/src/lib.rs`
- Modify: `crates/sui-dev/web/index.html`
- Modify: `crates/sui-dev/web/README.md`
- Optional: `crates/sui-dev/src/app.rs`

Step 1: Write failing tests for benchmark mode parsing if needed

Add tests for new benchmark presets:

```rust
#[test]
fn parses_text_editing_web_benchmark_mode() {
    let mode = parse_web_launch_mode("benchmark=text-editing");
    assert_eq!(mode.benchmark, Some(WebBenchmarkKind::TextEditing));
}
```

Step 2: Run tests to verify failure if the preset does not yet exist

Run:

```bash
cargo test -p sui-dev --lib parses_text_editing_web_benchmark_mode
```

Step 3: Implement benchmark presets and docs

Ensure wasm query-string benchmark modes cover:
- `button-grid`
- `retained-text`
- `text-editing`
- optional comparison-surface benchmark once Task 6 exists

Step 4: Verify pass

Run:

```bash
cargo test -p sui-dev --lib
trunk build --config crates/sui-dev/web/Trunk.toml
```

Step 5: Commit

```bash
git add crates/sui-dev/src/lib.rs crates/sui-dev/web/index.html crates/sui-dev/web/README.md
# plus any supporting files
git commit -m "feat: add text-focused wasm benchmark presets"
```

---

## Task 8: Document the resulting text rendering model

Objective: Make the implementation understandable and maintainable.

Files:
- Modify: `docs/text-system.md`
- Optional new doc: `docs/plans/` follow-up notes or `docs/renderer-architecture.md`

Step 1: Update docs with the new model

Document:
- the distinction between grayscale and LCD/subpixel text
- when LCD is allowed and when it falls back
- hinting and darkening thresholds
- benchmark procedure for native and wasm
- future work: optional analytic/vector text path for large transformed text

Step 2: Verify docs are accurate against commands

Run:

```bash
cargo check -p sui-dev
cargo check -p sui-dev --target wasm32-unknown-unknown --no-default-features --features web
trunk build --config crates/sui-dev/web/Trunk.toml
```

Step 3: Commit

```bash
git add docs/text-system.md docs/renderer-architecture.md docs/plans/
git commit -m "docs: describe improved text rendering model"
```

---

## Suggested implementation order

Recommended execution order:

1. Task 1 — explicit text render modes
2. Task 2 — preserve true LCD/subpixel coverage
3. Task 3 — conservative gating rules for LCD mode
4. Task 4 — slight hinting support
5. Task 5 — stem darkening
6. Task 6 — side-by-side comparison surface
7. Task 7 — wasm benchmark presets and measurements
8. Task 8 — docs

This order minimizes risk because each stage remains testable and reversible.

---

## Benchmark and validation checklist

Native validation:

```bash
cargo check -p sui-render-wgpu
cargo check -p sui-widget-book
cargo check -p sui-dev
cargo test -p sui-render-wgpu
cargo test -p sui-dev --lib
```

Wasm validation:

```bash
cargo check -p sui-dev --target wasm32-unknown-unknown --no-default-features --features web
cd crates/sui-dev/web
trunk build --config Trunk.toml
trunk serve --config Trunk.toml
```

Browser benchmark URLs:

```text
http://127.0.0.1:8080/?benchmark=button-grid
http://127.0.0.1:8080/?benchmark=retained-text
http://127.0.0.1:8080/?benchmark=text-editing
```

Add comparison-surface URL if Task 6 introduces one.

Validation criteria:
- LCD mode visibly preserves color-fringe-aware subpixel edge detail instead of grayscale blur.
- Rotated/sheared text automatically falls back to grayscale.
- Hinting improves small text legibility without obvious distortion.
- Stem darkening improves thin strokes without noticeably muddying medium-size text.
- Benchmark surfaces do not regress catastrophically on wasm or native.

---

## Risks and pitfalls

- LCD AA can look wrong under transforms, translucent compositing, or unknown backgrounds.
- Hinting can improve small text but degrade shape fidelity if applied too broadly.
- Stem darkening can easily be overdone; keep defaults conservative.
- Cache keys must include all render-mode-affecting state to avoid stale glyph reuse.
- Do not silently reuse grayscale atlas entries for LCD mode or vice versa.
- Do not try to bundle a full analytic/vector text renderer into this iteration.

---

## Future work after this plan

If the above ships cleanly and benchmarks well, the next exploration can be:
- a hybrid analytic glyph path for large text or transformed text
- a vector-outline glyph cache keyed by persistent layout handles
- renderer selection rules such as “small UI text = atlas, large display text = analytic”

That would be the closest architectural bridge from SUI’s current renderer toward Pathfinder’s highest-quality text ideas without forcing a rewrite first.
