# Changelog

All notable changes to SUI are documented in this file. SUI follows Semantic
Versioning, with the usual expectation that the API may change during the
`0.x` series.

## [0.2.0]

This release expands SUI from a retained widget and rendering foundation into
a fuller application toolkit while keeping the facade renderer-neutral.

### Highlights

- Added dependency-tracked `Signal<T>` bindings, observable selectors,
  keyed subtree reconciliation, automatic pass invalidation, retained local
  state, and rebuild diagnostics.
- Added the `VirtualCollection` foundation and keyed `VirtualList`, windowed
  collection models, variable-height extents, anchoring, follow-end behavior,
  selection, keyboard navigation, row retention, and virtual table state.
- Added `RichDocumentModel` and `RichDocumentView` with incremental streaming
  Markdown, selection across blocks, code actions and highlighting, links,
  images, attachments, extensible structured blocks, cached layouts, and rich
  accessibility semantics.
- Added typed widget, window, and application command routing, lifecycle-owned
  controllers and subscriptions, scheduler-only wakes, application multicast,
  and command/invalidation traces.
- Added window-managed overlay policy for dialogs, popovers, menus, tooltips,
  command palettes, notifications, drawers, and bottom sheets, including
  collision-aware placement, nesting, modality, dismissal, and focus restore.
- Added resizable split state, responsive sidebars, master-detail navigation,
  adaptive views, container queries, grid, intrinsic content extents, wrapping
  toolbars, aspect ratio, safe areas, and retained layout transitions.
- Added a live application inspector covering semantics and accessibility,
  stable widget IDs and bounds, event routes, rebuild and invalidation reasons,
  scheduler work, virtual collection statistics, and paint damage.
- Reduced widget-construction and retained text-layout overhead, refreshed
  dependencies, and made overlay scroll bars compact until pointer hover.
- Corrected Android window and `wgpu::Surface` creation to follow
  `Resumed`/`Suspended`, retaining the runtime while native surfaces are absent.

## [0.1.0]

Initial public alpha release of the Rust workspace.

### Highlights

- Added a retained-mode application runtime with explicit measure, arrange,
  event, paint, and accessibility passes.
- Added a renderer-neutral scene model and a retained `wgpu` renderer with
  text, image, vector path, clipping, compositing, and color-management
  support.
- Added the built-in widget library, responsive layout primitives, editable
  text controls, data views, overlays, drag and drop, canvas surfaces, and
  Mesh light, dark, high-contrast, OLED, and touch themes.
- Added desktop integration for Linux, macOS, and Windows, plus alpha browser
  support and experimental Android support behind explicit facade features.
- Added deterministic headless testing, semantic locators, screenshot and HDR
  artifact support, AccessKit integration, and an accessibility-tree generated
  terminal UI.
- Added runnable facade examples, the widget-book demo, bundled Lucide icon
  resources, AVIF/HDR helpers, and architecture and API guides.
- Added a language-neutral binding core and source-built native Python and
  Node/Electron bindings with a generated, coverage-checked widget surface.

### Release boundaries

- The Rust API is pre-release and may change before `1.0.0`.
- Browser support is alpha, Android support is experimental, and native HDR
  output is currently strongest on Windows.
- The Python and Node/Electron packages are not part of this registry release;
  their source remains available in the repository for local builds.
- Browser JavaScript bindings, prebuilt Python wheels, prebuilt Node/Electron
  addons, custom WGSL, and zero-copy external-surface composition are not yet
  published or supported release surfaces.

[0.1.0]: https://github.com/sinomo-lab/sui/releases/tag/v0.1.0
[0.2.0]: https://github.com/sinomo-lab/sui/compare/v0.1.0...v0.2.0
