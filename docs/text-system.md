# SUI Text System Direction

## Purpose

This document records the intended direction for the SUI text subsystem.

It is not a description of the current implementation. The current code still uses the existing `sui-text` shaping and layout stack plus renderer-owned glyph rasterization. This document describes the planned refactor target and the constraints for getting there safely.

The near-term implementation direction is to rebuild the core text stack on top of `cosmic-text` while keeping `sui-text` as the text subsystem boundary. That migration should improve multi-line layout, Unicode-aware line breaking, fallback handling, right-to-left text, and any vertical layout behavior the upstream engine already supports, without trying to invent new SUI-specific shaping behavior in the same refactor. See [text-system.md](./text-system.md) for the concrete migration direction and scope boundaries.

Longer term, the text subsystem should also grow toward editing-grade control for creative tools. That includes low-level typographic controls such as tracking and baseline adjustment, plus support for vector and artistic text workflows where text remains an editable design object instead of only a rasterized UI label.

## Near-Term Refactor Goal

The next major text refactor should move SUI toward using `cosmic-text` as the core text engine underneath `sui-text`.

The goal is not only to replace the current shaping and rendering path. The goal is to turn text into a more complete rendering subsystem with better language coverage, better layout behavior, and a cleaner long-term foundation for both UI text and creative-tool text workflows.

The refactor should adopt the capabilities that `cosmic-text` already provides. It should not add SUI-specific text shaping or layout features beyond what `cosmic-text` can support during this migration.

## Target Capabilities For This Refactor

The refactor should improve or enable the following behavior inside the existing SUI text stack:

- multi-line text layout as a first-class path rather than a thin extension of single-line shaping
- proper Unicode-aware line breaking and wrapping
- font fallback driven by the underlying text engine rather than a mostly single-face layout model
- right-to-left and bidirectional text handling
- any vertical or top-to-bottom layout behavior that `cosmic-text` can provide without custom SUI extensions
- more reliable glyph placement and metrics for mixed-script text, emoji, and international input
- a cleaner bridge between text layout in `sui-text` and glyph rasterization in `sui-render-wgpu`

This work should improve both text correctness and future renderer architecture.

## Public API Direction

The refactor should preserve the text subsystem boundary, not the current type boundary.

In practice, that means `sui-text` should remain the crate that owns font management, shaping, layout, cursoring, selection geometry, raster inputs, and outline extraction.

It does not mean the current public data model should be treated as stable.

The current API was built for a very small text feature set. It is good enough for basic labels, simple text input, and measurement, but it is not a strong long-term foundation for a fully featured text engine. In particular, a future system needs to represent:

- attributed text rather than only one string plus one style
- fallback-aware layout rather than effectively one resolved face per layout
- richer paragraph and writing-mode controls
- cursoring and selection behavior that follows complex layout rather than a minimal byte-range model
- renderer-oriented raster output and vector-oriented outline output as separate views over the same layout result

Because the codebase is still early, SUI should be willing to make breaking API changes now if they are necessary to establish the right text architecture.

The current surface may still survive in part as a convenience API for simple UI text, but it should not be treated as the canonical long-term text model.

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
- cursor positions and movement boundaries
- selection geometry
- measurement summaries

This is the real long-term replacement for the current `TextLayout` and `TextLine` model.

### 3. Rendering And Extraction Model

This layer should expose different derived outputs from the same layout result.

At minimum, it should support:

- glyph instances and raster inputs for UI rendering
- outline extraction for vector workflows
- data suitable for artistic text objects that remain editable as text

UI text and artistic text should share shaping and layout foundations even when their rendering and editing behavior diverge.

## Planned Migration Shape

The migration should be phased.

### Phase 1: Redesign The Core Text Model In `sui-text`

Redesign the core `sui-text` data model first.

The goal of this phase is to stop treating the current API as the long-term architecture. Before wiring in `cosmic-text`, define the core text concepts that SUI actually wants to preserve:

- content and style input objects
- layout result objects
- cursor and selection primitives
- rendering and outline extraction views

This phase may include breaking API changes. The important constraint is to land on a model that can represent fallback-aware, multi-run, multi-line, and future editing-grade text correctly.

### Phase 2: `cosmic-text` Layout Adapter

Add a `cosmic-text` backed implementation that translates `Buffer` and layout-run output into SUI's new core layout objects.

This phase should cover:

- measurement
- line metrics
- glyph placement
- caret geometry
- selection geometry
- fallback-aware layout data

The output still needs to map into SUI-owned layout types so the rest of the stack can depend on SUI abstractions rather than raw upstream types.

### Phase 3: Mixed-Font And Fallback-Correct Layout Data

`cosmic-text` based layout will naturally produce runs that may use different fonts and fallback faces.

This phase should ensure SUI can represent mixed-font output correctly throughout the subsystem instead of flattening it back into a single-face abstraction.

The key rule is to fix the data model at the root instead of pretending all shaped output still belongs to one face.

### Phase 4: Renderer Text Rasterization Upgrade

Once layout is stable, the renderer should move away from relying on the current outline-heavy text rasterization path.

The preferred direction is to use `cosmic-text` and `swash` style glyph raster data for atlas generation and glyph caching, while keeping SUI's retained compositor and scene-frame boundaries intact.

The renderer should continue to own GPU policy, atlas uploads, batching, and cache lifetimes. The text engine should provide better layout and raster inputs, not bypass the renderer architecture.

### Phase 5: Validation And Removal Of The Legacy Path

Before removing the old implementation, validate the new path with:

- mixed-script text samples
- emoji and fallback fonts
- right-to-left and bidirectional cases
- multi-line wrapping and line-breaking tests
- widget-book visual comparisons
- text-input caret, selection, and IME behavior

Only after those cases are reliable should the legacy shaping path be removed.

## Explicit Non-Goals For This Refactor

This migration should stay within the capability envelope that `cosmic-text` already offers.

The refactor should not try to solve these problems in the same step:

- a custom rich-text document model beyond what current SUI text APIs need
- a bespoke text-editing engine independent of `cosmic-text`
- SUI-specific shaping features that the upstream engine does not support
- full typographic authoring controls beyond what the initial redesigned text model needs for forward compatibility

Those may be valid future directions, but combining them with the backend migration would make the change much riskier.

## Long-Term Direction Beyond The Refactor

The long-term text direction for SUI should go beyond UI labels and text inputs.

SUI is intended for creative and technical applications, so text also needs to grow toward vector-editing and artistic-text use cases.

That longer-term direction should include:

- low-level typographic controls such as tracking, baseline adjustment, and other run-level placement controls
- text objects that remain editable as text rather than collapsing immediately into pixels
- support for vector text workflows, including reliable outline conversion and transform-friendly text objects
- support for artistic text use cases where text behaves like design content rather than only UI chrome

These goals are one reason not to over-preserve the current API. A text subsystem designed only around basic UI measurement and glyph placement will become a blocker once SUI needs real artistic text objects or editing-grade typographic controls.

Those controls are a goal for later work, not a requirement for the initial `cosmic-text` migration.

The immediate refactor should establish a text engine and renderer path that makes those later capabilities feasible instead of painting SUI into another dead end.

## Design Rule

When tradeoffs appear during this migration, prefer choices that improve the foundation for international text and creative-tool text workflows, even if they require breaking changes to the current API.

The text subsystem must serve both everyday UI text and demanding editor-style applications.