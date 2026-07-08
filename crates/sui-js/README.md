# @sui/ui JavaScript Bindings

This package exposes the first Node/Electron binding surface for SUI through
napi-rs. It currently supports host-driven rendering through `App.start()` and
`renderWidget()`, desktop event-loop execution through `App.run()` and
`App.runWithHandle()`, thread-safe state updates, custom widgets, safe paint
commands including styled text, widget-local RGBA image registration,
app-level RGBA/PNG/SVG image resources, app-level font byte resources, event
descriptors, custom widget semantics callbacks, built-in shader descriptors,
and external graphics interop descriptors. `ExternalSurface` can reserve layout
for an external texture descriptor and renders CPU RGBA descriptors through
SUI's image path.
Fonts, PNG images, and SVG images can be registered from bytes or local files.
The initial high-level widget set includes labels, buttons, icons, icon buttons, links, checkboxes,
switches, radio buttons, radio groups, segmented controls, breadcrumbs/path bars, list views, tables/data grids, tree views, layer lists, menus, context menus, tab bars, tabs, dialogs, signal meters, status badges, status bars, status bar hosts, detail rows, sliders, number inputs, selects, progress bars, busy indicators,
single-line text inputs, multiline text areas, rich text, images, color swatches, color palettes, color pickers,
separators, empty states, surfaces, toolbars, tool palettes, preset strips, browser tab bars, scroll views, padding, alignment, backgrounds, sized boxes, stacks, semantic regions, form rows, field groups, form sections, panel sections, dock panels, tooltips, popovers, rows, and columns.
Descriptor classes include `TextSpan`, `StatusBarSegment`,
`SegmentedControlItem`, `TableColumn`, `TableRow`, `TreeItem`,
`LayerListItem`, `MenuItem`, `ToolPaletteItem`, and `ColorPaletteSwatch`.

Custom widget paint callbacks can call `paint.drawText(rect, text, color,
fontSize, lineHeight, font, weight, style, stretch)`. Register reusable fonts
with `const font = app.fontBytes(bytes)` and pass the returned `FontHandle` to
custom painting code.
The same paint surface also exposes paths, path clips, rounded rectangles,
drop shadows, transforms, and image quads through `Path`, `PathBuilder`,
`Transform`, and `Shadow`.
Custom widgets can implement `semantics(semantics)` and call
`semantics.node(role, name, value)` to expose accessibility nodes.
Render snapshots expose semantic roles, names, values, descriptions, checked
states, busy flags, editable-multiline flags, and arrays for disabled, focused,
hidden, hovered, selected, and expanded states.

Use `App.run()` for a normal desktop app. Use `App.start()` when embedding SUI
in another host loop, tests, or headless rendering. Browser/WASM bindings are
not implemented yet.

## Build

From this directory, build the native `.node` artifact with napi-rs:

```bash
napi build --platform --release
```

The generated `.node` file must live next to `index.js` for the package loader.

## Examples

See:

- `examples/counter.js`
- `examples/custom-widget.js`
- `examples/external-surface.js`
