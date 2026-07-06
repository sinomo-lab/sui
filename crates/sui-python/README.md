# sui-ui Python Bindings

This package exposes the first Python binding surface for SUI. It currently
supports desktop event-loop execution through `App.run()` and
`App.run_with_handle()`, host-driven rendering through `App.start()` and
`render_widget()`, thread-safe state updates, custom widgets, safe paint
commands including styled text, widget-local RGBA image registration,
app-level RGBA/PNG/SVG image resources, app-level font byte resources, event
descriptors, custom widget semantics callbacks, built-in shader descriptors,
and external graphics interop descriptors. `ExternalSurface` can reserve layout
for an external texture descriptor and renders CPU RGBA descriptors through
SUI's image path.
Fonts, PNG images, and SVG images can be registered from bytes or local files.
The initial high-level widget set includes labels, buttons, links, checkboxes,
switches, radio buttons, sliders, number inputs, selects, progress bars, busy indicators,
single-line text inputs, multiline text areas, rich text, images, color swatches,
separators, scroll views, rows, and columns.

Custom widget paint callbacks can call `paint.draw_text(rect, text, color,
font_size=..., line_height=..., font=..., weight=..., style=...,
stretch=...)`. Register reusable fonts with `font = app.font_bytes(bytes)` and
pass the returned `FontHandle` to custom painting code.
The same paint surface also exposes paths, path clips, rounded rectangles,
drop shadows, transforms, and image quads through `Path`, `PathBuilder`,
`Transform`, and `Shadow`.
Custom widgets can implement `semantics(semantics)` and call
`semantics.node(role=..., name=..., value=...)` to expose accessibility nodes.
Render snapshots expose semantic roles, names, values, descriptions, checked
states, busy flags, editable-multiline flags, and arrays for disabled, focused,
hidden, hovered, selected, and expanded states.

Use `App.run()` for a normal desktop app. Use `App.start()` when embedding SUI
in another host loop, tests, or headless rendering.

## Build

From this directory:

```bash
maturin develop
```

or build a wheel:

```bash
maturin build --release
```

The extension module imports as `sui`.

## Examples

See:

- `examples/counter.py`
- `examples/custom_widget.py`
- `examples/external_surface.py`
