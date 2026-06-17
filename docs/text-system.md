# SUI Text System Direction

## Purpose

This document records the intended direction for the SUI text subsystem.

It is not a description of every current implementation detail. The current stack already uses `sui-text` as the text subsystem boundary and an atlas-backed renderer path in `sui-render-wgpu`. That migration is complete. The remaining problem is architectural: text is still fed into the renderer too much like transient scene geometry, which makes text-heavy retained packet rebuilds too expensive for editor-style workloads.

The next direction is a hybrid refactor:

- keep `sui-text` as the subsystem boundary for content, shaping, layout, cursoring, selection, raster inputs, and outline extraction
- introduce persistent text layout and text run objects that can survive scene and packet rebuilds
- move renderer text submission away from per-glyph triangle expansion and toward instanced or similarly persistent text draw data
- add a dedicated text surface path for editor-grade and document-grade workloads without forcing all text through a fully specialized editor widget immediately

The goal is to support both everyday UI labels and text-heavy applications such as code editors, markdown previews, rich text panels, logs, and creative-tool text objects.

## Refactor Goal

The next major text refactor should turn text into a persistent rendering subsystem rather than a stream of transient draw commands.

The goal is not only to make the current label path cheaper. The goal is to establish an architecture that can keep renderer cost low for editor-style workloads while preserving a clean long-term foundation for UI text, creative-tool text, and fully interactive editing surfaces.

The refactor should preserve the `sui-text` subsystem boundary while making it practical to:

- reuse shaped and laid out text across retained packet rebuilds
- render visible text from persistent layout artifacts rather than rebuilding glyph geometry command by command
- update only the changed text runs, lines, paragraphs, or viewport slices during editing
- support future editor-grade text surfaces and vector or artistic text workflows without another root-level rewrite

## Target Capabilities

The refactor should improve or enable the following behavior inside the existing SUI text stack:

- persistent text layout objects with stable identities and explicit renderer-facing extraction APIs
- persistent text run and glyph-instance data that can be reused across scene and retained packet churn
- viewport-aware text rendering so large documents and previews submit only visible content plus small guard bands
- incremental updates for editing paths, including line-level or paragraph-level relayout and localized selection or caret redraw
- proper Unicode-aware line breaking, fallback handling, right-to-left and bidirectional text, and mixed-script layout
- richer paragraph, run, and writing-mode support suitable for both UI documents and editor-grade text surfaces
- a cleaner bridge between text layout in `sui-text` and text submission in `sui-render-wgpu`
- future support for vector and artistic text objects that remain editable as text rather than collapsing immediately into pixels

This work should improve both text correctness and future renderer architecture.

## Public API Direction

The refactor should preserve the text subsystem boundary, not the current convenience types.

In practice, that means `sui-text` should remain the crate that owns font management, shaping, layout, cursoring, selection geometry, raster inputs, and outline extraction.

It does not mean the current public data model should be treated as stable.

The current API was built for a small text feature set. It is good enough for basic labels, simple text input, and measurement, but it is not a strong long-term foundation for a fully featured text engine and text renderer. In particular, a future system needs to represent:

- attributed text rather than only one string plus one style
- fallback-aware layout rather than effectively one resolved face per layout
- richer paragraph and writing-mode controls
- cursoring and selection behavior that follows complex layout rather than a minimal byte-range model
- renderer-oriented extraction views over the same layout result, including glyph instances, run ranges, viewport slices, and outline data
- persistent layout identities and versioning so render work can reuse stable text state instead of rebuilding everything from raw strings

Because the codebase is still early, SUI should be willing to make breaking API changes now if they are necessary to establish the right text architecture.

The current surface may still survive in part as a convenience API for simple UI text, but it should not be treated as the canonical long-term text or text-rendering model.

## Target API Shape

The long-term text subsystem should be organized around three distinct layers.

### 1. Text Content And Styling Model

This layer should represent the source text and its layout-affecting attributes.

It should be able to grow toward:

- paragraphs and paragraph-level options
- span-based attributes
- family and fallback preferences
- alignment, wrapping, and writing-mode settings
- future typographic controls such as tracking and baseline adjustment

Even if SUI does not expose a full rich-text editor immediately, the engine should be designed around attributed text and paragraph structure instead of assuming one string with one style.

### 2. Layout Result Model

This layer should represent the result of shaping and layout.

It should be able to describe:

- paragraphs
- visual lines
- runs
- clusters
- glyph instances
- stable layout handles and layout versions
- viewport slices or visible line windows
- cursor positions and movement boundaries
- selection geometry
- measurement summaries

This is the real long-term replacement for the current `TextLayout` and `TextLine` model.

### 3. Rendering And Extraction Model

This layer should expose different derived outputs from the same layout result.

At minimum, it should support:

- glyph instances and raster inputs for UI rendering
- stable renderer-facing run or line payloads that can be cached independently from widget packet rebuilds
- extraction of visible text ranges for viewport-driven text surfaces
- outline extraction for vector workflows
- data suitable for artistic text objects that remain editable as text

UI text and artistic text should share shaping and layout foundations even when their rendering and editing behavior diverge.

## Renderer Design Direction

The renderer should stop treating text primarily as transient per-glyph vertex expansion owned by packet compilation.

The target renderer model is:

- `sui-text` owns the persistent layout result and renderer-facing extraction views
- `sui-render-wgpu` consumes stable glyph instance or run data rather than reconstructing text geometry from raw text commands on every rebuild
- text draw ops reference persistent text payloads by handle, version, visible range, style overrides, transform, and clip state
- atlas caching remains renderer-owned, but text submission becomes instance-oriented rather than six transient vertices per glyph
- editor overlays such as carets, selections, IME composition, diagnostics, and current-line highlights should remain separate overlay content and must not force bulk text payload rebuilds

The important design rule is that text layout reuse must not depend on retained packet reuse. Text should have its own persistence boundary.

## Text Surface Direction

SUI should support two text consumption modes built on the same text core.

### 1. Generic UI Text

This covers labels, buttons, menus, tables, trees, forms, property grids, and other widget text.

This path should:

- keep convenient widget-facing text APIs
- lower to persistent layout handles instead of raw string-only scene commands where practical
- reuse layout and renderer data across retained packet rebuilds

### 2. Dedicated Text Surface

This covers code editors, markdown previews, large logs, terminals, document views, and future rich text editors.

This path should be a first-class widget or family of widgets that owns:

- document storage or document adapters
- incremental relayout and viewport management
- line and cluster hit testing
- selection and caret logic
- overlay rendering for editing affordances
- renderer-facing extraction of only the visible text content

This path should not be modeled as a large pile of independent label-like draw commands.

## Refactor Plan

The next refactor should be phased.

### Phase 1: Stabilize The Core Text Model In `sui-text`

Redesign the core `sui-text` model around the types SUI actually needs long term:

- content and styling objects
- layout result objects with lines, runs, clusters, and glyph instances
- persistent layout identities and versioning
- renderer extraction views
- cursor, caret, selection, and hit-testing primitives

This phase may include breaking API changes. The goal is to stop treating today's convenience types as the long-term model.

### Phase 2: Introduce Persistent Text Layout Handles

Scene and widget code should move toward referencing text layouts through stable handles or equivalent persistent objects instead of repeatedly lowering raw text into transient packet-local work.

This phase should keep the simple widget API ergonomic while changing the renderer contract under it.

### Phase 3: Renderer Text Instance Submission

Replace transient per-glyph triangle expansion as the primary text submission path.

The preferred direction is to:

- store glyph instance data or similarly compact run data
- render text from a shared quad plus per-instance attributes
- keep atlas policy, uploads, batching, and lifetime management in `sui-render-wgpu`
- let text draw ops reference persistent text payloads instead of rebuilding packet-local text vertices command by command

This is the main near-term performance step for text-heavy retained packet rebuilds.

### Phase 4: Dedicated Text Surface For Editor-Grade Workloads

Add a dedicated text surface path for documents and editors.

This phase should introduce:

- viewport-driven line or paragraph extraction
- incremental relayout for edits
- separate overlay rendering for selections, carets, IME composition, diagnostics, and similar adornments
- document-oriented hit testing and interaction instead of widget-per-span composition

This phase is what makes high refresh rate code editors and rich previews realistic on top of the same core text system.

### Phase 5: Promote Generic Widgets Onto The New Text Path

Once the persistent layout and instance rendering path is stable, migrate generic widgets such as labels, buttons, tables, trees, breadcrumbs, menus, and inspectors to consume the same text infrastructure.

The result should be one text subsystem with two consumption modes, not a separate text stack for editors.

### Phase 6: Validation And Benchmarking

Validate the new path with:

- mixed-script text samples
- emoji and fallback fonts
- right-to-left and bidirectional cases
- multi-line wrapping and line-breaking tests
- widget-book visual comparisons
- text-input caret, selection, and IME behavior
- text-heavy retained packet benchmarks
- editor-style typing, scrolling, selection, and syntax-highlighting benchmarks

## Explicit Non-Goals For This Refactor

This refactor should stay focused on the text architecture and renderer contract. It should not try to solve every higher-level editor problem at once.

The refactor should not try to solve these problems in the same step:

- a full IDE feature model beyond text layout, rendering, and interaction fundamentals
- language intelligence, tokenization, parsing, or syntax services beyond the hooks needed to render styled text efficiently
- a fully custom document engine for every text workload before the shared persistent text model is in place
- final artistic-text authoring features beyond the core text architecture needed to support them later

Those may be valid future directions, but combining them with the core text refactor would make the change much riskier.

## Long-Term Direction Beyond The Refactor

The long-term text direction for SUI should go beyond UI labels and text inputs.

SUI is intended for creative and technical applications, so text also needs to grow toward vector-editing and artistic-text use cases.

That longer-term direction should include:

- low-level typographic controls such as tracking, baseline adjustment, and other run-level placement controls
- text objects that remain editable as text rather than collapsing immediately into pixels
- support for vector text workflows, including reliable outline conversion and transform-friendly text objects
- support for artistic text use cases where text behaves like design content rather than only UI chrome

These goals are one reason not to over-preserve the current API and not to optimize only the current packet compiler. A text subsystem designed only around basic UI measurement and packet-local glyph placement will become a blocker once SUI needs real artistic text objects or editing-grade text surfaces.

Those controls are a goal for later work, not a requirement for the first stages of this refactor.

The immediate refactor should establish a persistent text model and renderer path that makes those later capabilities feasible instead of painting SUI into another dead end.

## Design Rule

When tradeoffs appear during this refactor, prefer choices that improve the foundation for international text and creative-tool text workflows, even if they require breaking changes to the current API.

The text subsystem must serve both everyday UI text and demanding editor-style applications.

## Current Text Rendering Model

The sections above describe the long-term direction. The current implementation now also has a concrete runtime text rendering model that is important to preserve while the broader refactor continues.

### Window-level runtime controls

The active window carries a `WindowRenderOptions` bundle with the text-related controls that the dev workspace and validation surfaces expose:

- `text_hinting`, which defaults to `None` in `WindowRenderOptions`
- `stem_darkening`, which defaults to `None`

These controls are window-scoped runtime presentation settings. They are not currently independent per sample card inside the comparison surface.

Text coverage policy is a renderer-level setting on `WgpuRenderer`, not a runtime `WindowRenderOptions` field. It defaults to `TextCoveragePolicy::Linear`.

### Grayscale coverage vs LCD/subpixel rendering

SUI now distinguishes between two separate concerns:

1. **Text render mode** — grayscale atlas rendering versus LCD/subpixel atlas rendering
2. **Text coverage policy** — how grayscale coverage is mapped into final alpha

`TextRenderMode` currently defaults to `Grayscale`. LCD/subpixel rendering is available through `TextRenderMode::LcdSubpixel`, but it is intentionally conservative.

`TextCoveragePolicy::Linear` is the default grayscale coverage policy for all text colors. It maps rasterized coverage directly to atlas alpha, and the text shaders sample that coverage without an additional gamma or luminance remap. Dark-on-light and light-on-dark UI text therefore share the same coverage curve unless a caller explicitly selects a different renderer coverage policy.

### When LCD/subpixel text is allowed

LCD/subpixel text is only considered safe when the path stays compatible with physical subpixel layout. The current renderer requires:

- an axis-aligned transform
- positive X and Y scale components

Atlas glyph quads are always snapped to the physical pixel grid. Fractional glyph phase is represented by quarter-pixel raster variants in the glyph cache instead of by drawing the atlas sprite at fractional screen coordinates.

In practice, rotated, mirrored, or otherwise non-LCD-safe transforms fall back to grayscale expectations instead of trying to preserve LCD sampling through an unsafe transform.

### Hinting and stem darkening thresholds

The current small-text controls are deliberately threshold-based rather than always-on:

- `TextHinting::Slight { max_ppem }` only applies when the effective ppem is at or below `max_ppem`
- `StemDarkening::Enabled { max_ppem, amount }` only contributes when the effective ppem is at or below `max_ppem`
- the darkening amount is normalized and clamped into the `0.0..=1.0` range

The dev workspace currently seeds these controls with practical small-text defaults when they are first enabled:

- slight hinting default threshold: `18.0` ppem
- stem darkening default threshold: `18.0` ppem
- stem darkening default amount: `0.08`

This keeps medium-size UI text from being over-corrected while still helping small labels and repeated stems.

### Comparison and validation surfaces

The widget-book comparison surface is meant to make text behavior legible at a glance across:

- grayscale baseline
- grayscale + hinting
- grayscale + stem darkening
- LCD subpixel
- LCD subpixel + hinting
- LCD subpixel + hinting + stem darkening

That surface is a visual checklist for dark-on-light text, light-on-dark text, repeated stems, mixed scripts, and contrast-sensitive UI labels. The mode cards are reference views; the actual runtime renderer settings remain window-level.

## Validation And Benchmark Workflow

The current validation workflow for the text rendering model should cover both native and wasm entry points.

### Native checks

Use targeted native checks first:

```bash
cargo check -p sui-widget-book
cargo check -p sui-dev
cargo test -p sui-widget-book --lib tests::text_rendering_comparison_surface_exposes_all_render_modes -- --exact
cargo test -p sui-dev --lib tests::parses_text_comparison_web_benchmark_mode -- --exact
cargo test -p sui-dev --lib tests::parses_comparison_surface_alias -- --exact
```

### Wasm checks

Use the web target to verify that the benchmark launch path still builds:

```bash
cargo check -p sui-dev --target wasm32-unknown-unknown --no-default-features --features web
trunk build --config crates/sui-dev/web/Trunk.toml
```

### Web benchmark URLs

The web entry point now supports focused benchmark presets through the query string:

```text
http://127.0.0.1:8080/?benchmark=button-grid
http://127.0.0.1:8080/?benchmark=retained-text
http://127.0.0.1:8080/?benchmark=text-editing
http://127.0.0.1:8080/?benchmark=text-comparison
http://127.0.0.1:8080/?benchmark=comparison-surface
http://127.0.0.1:8080/?benchmark=widget-book
http://127.0.0.1:8080/?benchmark=dev
```

The `text-comparison` and `comparison-surface` aliases should launch the side-by-side text rendering checklist introduced for grayscale, hinted, darkened, and LCD-oriented validation.

## Near-term Future Work

The current model is intentionally conservative. A few follow-up directions remain important:

- keep the grayscale fallback rules conservative for transforms that are not LCD-safe
- continue benchmarking native and wasm text-heavy surfaces as renderer work lands
- make the dedicated text surface path cheaper for editor-style workloads
- add an optional analytic or vector-oriented path for large transformed text, where LCD atlas sampling is not the right fit

That future vector or analytic path should complement the current atlas-backed UI text model, not replace the small-text LCD-safe path that now exists.
