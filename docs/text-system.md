# Text System

SUI has one text pipeline for ordinary labels, attributed documents, editable
controls, and large editor surfaces. `sui-text` owns shaping and layout;
widgets retain the resulting layouts; scene commands refer to those layouts by
handle and version; and `sui-render-wgpu` turns the selected glyphs into atlas
instances.

This document describes the implementation that ships today. For application
examples, see [Input and text editing](api/input-and-editing.md). For the full
performance and image-quality procedure, see
[Text rendering benchmarks](text-rendering-benchmarks.md).

## Architecture at a Glance

```text
TextDocument / plain text
          |
          v
TextSystem through LayoutContext
          |
          +-- TextLayout (document, lines, runs, clusters, glyphs)
          |
          `-- PersistentTextLayout (stable handle + layout version)
                         |
                         v
       DrawShapedText / DrawShapedTextWindow
                         |
                         v
         WGPU glyph atlas + text instances
```

Application code normally enters this pipeline through `Label`, `RichText`,
`TextInput`, `TextArea`, or `TextSurface`. A custom widget can use the same
pipeline through `MeasureCtx::layout()` and `PaintCtx`.

## Documents and Styles

`TextDocument` is the source model for attributed text. It contains
`TextParagraph` values, and each paragraph contains one or more `TextSpan`
values:

- `TextStyle` describes the font handle or family stack, font size, line
  height, color, weight, slant, stretch, and OpenType features for a span.
- `TextParagraphStyle` describes alignment, wrapping, direction, and writing
  mode for a paragraph.
- `TextSpan` owns a string and its `TextStyle`.
- `TextDocument::from_plain_text` is the convenience path for one style. It
  turns newline-delimited text into separate paragraphs.
- `TextDocument::plain_text` reconstructs the paragraphs with newline
  separators.

For ordinary attributed display, construct a document and give it to
`RichText`:

```rust
use sui::prelude::*;

fn build_status() -> impl Widget {
    let body = TextStyle::new(Color::WHITE);
    let mut strong = body.clone();
    strong.weight = FontWeight::BOLD;

    let paragraph = TextParagraph::from_spans(vec![
        TextSpan::new("Build: ", body),
        TextSpan::new("ready", strong),
    ]);

    RichText::new(TextDocument {
        paragraphs: vec![paragraph],
    })
}
```

`RichText` retains a persistent layout, supports pointer selection, and can
publish selection through a `SelectionScope`. Its optional
`RichTextSourceMap` maps copied display ranges back to original source text,
which is useful for rendered Markdown.

## Layout Results

`TextSystem` is the lower-level `sui-text` entry point. The runtime exposes it
to widgets as `LayoutContext`, with these current operations:

- `measure_text` and `measure_document` return a `TextMeasurement`.
- `shape_text` lays out plain text in a box and returns a `TextLayout`.
- `layout_document` lays out a `TextLayoutRequest`.
- `shape_text_persistent` and `layout_document_persistent` additionally pin
  the result under a stable `TextLayoutHandle`.

`TextLayoutRequest::with_box_size` supplies the layout box. Its width controls
wrapping; the final layout retains both the requested box and its natural text
measurement.

A `TextLayout` is an immutable, cloneable view over shared layout storage. It
keeps the normalized `TextDocument` and exposes:

- paragraph, visual-line, run, cluster, glyph, and resolved-face slices;
- aggregate measurement and bounds;
- `caret` / `caret_rect` and `hit_test_point`;
- `selection_geometry`, `selection_rects`, and `selection_bounds`;
- `line_window`, which creates a view over only the runs, clusters, and glyphs
  belonging to a line range.

Cursor and selection offsets are UTF-8 byte offsets. Editable widgets clamp
movement and mutations to grapheme boundaries before asking the layout for
geometry.

The layout cache is owned by `TextSystem`. Its key includes the document's
text, layout-affecting span and paragraph styles, resolved faces, and optional
box size. Paint-only color changes can reuse the same geometry. Cache counters
are available through `TextSystem::layout_cache_snapshot`.

## Persistent Layouts and Scene Handoff

`PersistentTextLayout` pairs a `TextLayout` with a stable
`TextLayoutHandle`. The layout itself also has a `TextLayoutVersion`. Reusing a
handle when text, style, or constraints change gives scene and renderer code a
stable identity while the version prevents stale geometry from being used.

Built-in widgets follow this pattern during measurement:

```rust,ignore
let handle = self.layout.as_ref().map(|layout| layout.handle());
self.layout = ctx
    .layout()
    .layout_document_persistent(handle, request)
    .ok();
```

During paint, use one of the current `PaintCtx` methods:

- `draw_persistent_text_layout` for the complete layout;
- `draw_persistent_text_layout_with_color` for a paint-time color override;
- `draw_persistent_text_layout_window` for a visible line range;
- `draw_persistent_text_layout_window_with_color` for both a range and color
  override.

These calls refresh the layout in the runtime registry and add either
`SceneCommand::DrawShapedText` or
`SceneCommand::DrawShapedTextWindow`. The command carries the handle and
version rather than a copy of every glyph. At frame preparation time, the
renderer resolves the command against the registry. A window command iterates
only the glyphs in its clamped line range.

`PaintCtx::draw_text` remains a convenience path. It creates a stable handle
from the text run, shapes it persistently, and falls back to a raw `DrawText`
command only if shaping fails.

## TextSurface

`TextSurface` is the document-oriented editable widget. Use it for code,
logs, previews, and other multiline content that needs scrolling plus styled
ranges. Its public configuration includes:

- `value`, `current_value`, `set_value`, and `on_change`;
- `placeholder`, `read_only`, padding, and minimum size;
- `wrap` and `direction`;
- `style_spans` for durable attributed ranges;
- `style_overlays` for syntax, diagnostics, search matches, current-line
  styling, preview styling, or an application-defined overlay kind;
- `selection_scope` for coordinated selection with other widgets.

The surface maintains an indexed line table and indexed style ranges. In
unwrapped multiline mode it keeps persistent layouts per line, invalidates the
affected line coverage after edits, and shapes only the visible window plus
the caret line. Scrolling therefore does not require shaping every line.

For wrapped text and smaller documents, the surface retains one persistent
layout and submits a `DrawShapedTextWindow` for the visible lines. In both
modes it clips to the viewport. Selection rectangles, current-line fill,
caret, and IME composition position are painted separately from the text
instances, so caret and selection changes do not rewrite the document text.

Overlapping style ranges are resolved in source order: later style spans
override earlier spans, and style overlays are applied after ordinary spans.

## Shared Editing Behavior

`EditorState` and `EditorDocument` are private `sui-widgets` implementation
types, not application-facing state containers. They are shared by
`TextInput`, `TextArea`, and `TextSurface` so the controls agree on:

- insertion, deletion, and selection replacement;
- grapheme-aware horizontal movement and word/line navigation;
- pointer selection and shift-extended keyboard selection;
- select all, copy, cut, and paste;
- IME composition start, update, commit, and cancellation;
- selection, preferred vertical-navigation position, and scroll offsets;
- text revision, line starts, and dirty-line coverage used by `TextSurface`.

The widgets translate platform events into internal editor commands, then use
the command result to request layout, paint, or semantics invalidation at the
smallest relevant level. Applications should keep domain state in their own
model and consume `on_change`; they do not need to mirror caret or composition
state.

The application-facing controls are:

- `TextInput`: single-line text. Initial, pasted, and programmatic newlines
  are removed.
- `TextArea` (also exported as `MultilineTextInput`): multiline text, with an
  optional `on_submit` policy for plain Enter.
- `PasswordInput`: a `TextInput` wrapper that masks graphemes visually and
  marks editable semantics as password content. Its Rust value is still
  plaintext.
- `DateTimeInput`: a `TextInput` wrapper with a default
  `YYYY-MM-DD HH:MM` placeholder. Parsing, validation, locale, and timezone
  policy remain application responsibilities.

All four concrete field types expose current/programmatic value methods and
change callbacks. The editable controls also expose selection and clipboard
methods for composite widgets with an `EventCtx`. Menus can route
`TextCommand::{SelectAll, Copy, Cut, Paste}` to a retained input or
`TextSurface` instead of reaching into its private editor state.

Editable semantics include the current text, selection, read-only/password
flags, and supported actions. Native IME candidate windows receive the current
caret rectangle through `PaintCtx`.

## WGPU Rendering Policies

The WGPU renderer consumes resolved glyphs as `TextAtlasInstance` values. The
glyph cache distinguishes the face, glyph, scale, fractional X phase, atlas
color mode, subpixel order, hinting, stem darkening, and variable weight.
Color and coverage remapping are instance/shader data, so changing them does
not duplicate glyph raster entries.

There are two render modes:

- `TextRenderMode::Grayscale` is the default and is safe under arbitrary
  transforms.
- `TextRenderMode::LcdSubpixel` uses RGB or BGR coverage when the output path
  is known to match a physical LCD subpixel layout.

LCD rendering requires an explicit `TextSubpixelOrder::Rgb` or `Bgr`, an
axis-aligned transform, and positive X/Y scale. Requests with no subpixel order
or with rotated or mirrored transforms fall back to grayscale. Axis-aligned
glyphs use quarter-pixel X-phase atlas variants while their quads are snapped
to the physical pixel grid.

`TextRenderPolicy` can override render mode, subpixel order, hinting, stem
darkening, and coverage for a scoped part of a scene. A custom widget brackets
content with `PaintCtx::push_text_render_policy` and
`PaintCtx::pop_text_render_policy`.

Window defaults are deliberately conservative:

- slight hinting up to `96.0` ppem;
- no stem darkening;
- perceptual grayscale coverage;
- no LCD subpixel order.

`TextCoveragePolicy::Perceptual` resolves a color-aware coverage boost for
light-on-dark and dark-on-light text. `Linear`, `Gamma`, `CoverageBoost`, and
`TwoCoverageMinusCoverageSq` are available for explicit comparison or product
policy.

## Diagnostics and Benchmark Workflow

Set `SUI_PROFILE_TEXT_TIMINGS=1` before starting a native process to enable
per-thread request, hit/miss, and layout-time collection in `sui-text`. Runtime
performance snapshots also report layout and renderer text-cache deltas,
glyph instances, and upload bytes.

Run the focused correctness checks from the repository root:

```bash
cargo test -p sui-text
cargo test -p sui-widgets text_surface
cargo test -p sui-demo --lib widget_book::tests::text_rendering_comparison_surface_exposes_all_render_modes -- --exact
cargo test -p sui-demo --lib tests::parses_text_comparison_web_benchmark_mode -- --exact
cargo test -p sui-demo --lib tests::parses_comparison_surface_alias -- --exact
```

For visual inspection, run the widget book and open its text rendering
comparison surface:

```bash
cargo run -p sui-demo
```

For browser profiling, serve the web demo:

```bash
trunk serve --config crates/sui-demo/web/Trunk.toml
```

Then open the focused presets:

```text
http://127.0.0.1:8080/?benchmark=retained-text&warmup=60&frames=180
http://127.0.0.1:8080/?benchmark=text-editing&warmup=60&frames=180
http://127.0.0.1:8080/?benchmark=text-comparison&warmup=30&frames=120
```

Use [Text rendering benchmarks](text-rendering-benchmarks.md) for the ignored
renderer microbenchmark, desktop interaction benchmark, DPR quality matrix,
snapshot environment variables, expected signals, and reporting template.
Compare performance runs on the same machine and commit; use the comparison
surface and captures for perceptual review rather than treating changed-pixel
percentage as a complete quality score.

## Current Limits and Future Work

The current system is intentionally optimized for atlas-backed UI text and
practical editable controls. These boundaries remain:

- `TextWritingMode::Vertical` exists in the document model, but the current
  layout implementation is horizontal.
- `TextSurface` owns a `String`-backed editor document. It does not yet accept a
  rope, external document adapter, or virtualized storage provider.
- Unwrapped multiline surfaces incrementally shape visible per-line layouts;
  wrapped surfaces still lay out the complete document before submitting a
  visible line window.
- Display hardware is not yet used to choose RGB versus BGR automatically, so
  LCD rendering remains explicit opt-in and grayscale is the portable default.
- Large transformed or artistic text still uses the glyph atlas; there is no
  separate outline/vector text rendering path.
- The shared `EditorState` is private. Applications that build a wholly custom
  editor cannot currently reuse it as a public editing engine.

Future work should improve these boundaries without creating a second shaping
or rendering stack. Generic widgets, rich documents, and editor surfaces
should continue to share `TextDocument`, `TextLayout`, persistent handles, and
the same renderer policies.
