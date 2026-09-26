# Changelog

All notable changes to SUI are documented in this file. SUI follows Semantic
Versioning, with the usual expectation that the API may change during the
`0.x` series.

## [0.3.0] - 2026-09-26

This release adds application and editor surfaces, improves text quality and
retained rendering, and expands the source-built language bindings.

### Highlights

- Added retained docking workspaces, editor cursor grabbing and raw mouse motion,
  safe initial desktop window placement, and GPU interoperability contracts.
- Expanded Python and JavaScript bindings for the newer Rust widget and editor
  surfaces. These bindings remain source-built and are not registry publications.
- Reused paragraph shaping and glyph measurements across layout widths, added
  size-only text measurement, and reduced repeated Flex layout probes.
- Retained widget output, scene fragments, renderer packets, GPU batches, and
  shared scene command storage to reduce repeated frame preparation work.
- Overlapped GPU preparation with CPU startup and added reproducible widget,
  frame, shrinkwrap-conversation, and editorial-flow benchmarks.
- Improved font hinting, transformed text rasterization, LCD coverage and atlas
  sampling; isolated node-edge animation repaint work and analytic grid dots.
- Added provider-neutral image icons, bounded browser tabs, compact dialogs,
  and reserved scrollbar gutters.
- Updated dependencies and bundled Lucide icons to 1.47.0.
- Made the Shrinkwrap and Editorial text demos follow shared theme colors,
  typography, and radii, including theme changes while paused and cached text
  layouts at unchanged window sizes.
- Added a size-focused web release profile and kept the demo within the existing
  12 MiB uncompressed Wasm budget without removing features.
- Restored Rust 1.90 compatibility, corrected offscreen demo-picker test
  interactions, and resolved strict workspace Clippy findings.
- Bundled complete Noto Sans Arabic and Hebrew fallback fonts for offline web
  and Android text, and corrected caret hit testing across right-to-left runs.
- Restored corrupted Unicode, emoji, and IME sample text in the demo and its
  text-editing benchmarks.
- Continued VSync presentation and animation/timer delivery inside native
  window move/resize loops, including while the title bar is held still.
- Preferred compatible Direct3D12 hardware on Windows to improve native window
  drag pacing, retaining explicit backend selection and automatic fallback.
- Kept focused, visible VSync windows presenting cached content so an idle UI
  does not lower a variable-refresh monitor's refresh rate.
- Reused GPU vertex buffers, upload staging storage, and output-transform
  resources to avoid repeated allocations and interaction-time frame stalls.
- Fixed inflated frame-time/FPS readings after idle mouse motion by attributing
  input costs and latency only to a pending redraw across native and test hosts.
- Made the demo FPS counter measure host frame cadence, including VSync and
  idle intervals, while reporting render work separately.
- Retained action cards independently so hover, press, and focus transitions
  repaint only the affected cards instead of rebuilding an entire picker grid.
- Centralized builder/runtime resource registration so rejected duplicate handles
  preserve the original resource, and shared native/web GPU resource setup.
- Shared button press, text-change, and caret lifecycle logic across controls;
  organized composites, binding infrastructure, and renderer internals by feature.
- Made binding signatures language-neutral with explicit numeric and identifier
  types, and changed export verification to follow Rust modules and declarations.
- Fixed selector observer write-back deadlocks and removed obsolete reactive
  dependencies after completed widget phases while preserving cached phases.
- Bounded text-layout and renderer path caches, preserving layouts and geometry
  still owned by live widgets or frames, and avoided full reindexing on collection
  appends.
- Matched node-graph spatial bounds to configured Bézier curvature so visible
  curves remain available for hit testing and culling.
- Added the optional `sinomo-ui-webview` crate for WRY-backed native child
  webviews with retained layout, lifecycle synchronization, thread-safe
  controls, typed page/title/IPC events, and application-owned browser policy.
- Prevented synchronous live-test flushes from advancing future animation
  frames based only on render wall time, avoiding timeouts in repeating demos.
- Added affine transformed widget subtrees and Canvas-hosted normal widgets.
  Canvas now uniformly scales paint, input, semantics, text, images, and nested
  layout by default, with screen-space/custom zoom policies and touch pinch;
  retained node widgets use the same path.
- Added the `sinomo-ui-nodes` graph-editor library with typed nodes, edges,
  handles, controlled and uncontrolled observable state, retained custom node
  widgets, dynamic measurement, incremental spatial indexing, subflows,
  resizing, edge reconnection, per-element semantics, lifecycle events,
  animated viewport and edge behavior, controls, minimap, and graph-owned
  appearance overrides.
- Added a comprehensive `sui-demo` node-editor workspace behind the
  default-enabled `nodes` feature, covering controlled and uncontrolled graphs,
  retained node controls, every edge family, subflows, editing, indexing,
  semantics, and viewport telemetry.

### Compatibility and release notes

- Update SUI crate dependencies together from `0.2` to `0.3`. Lucide keeps its
  independent `1.47.0` version and now depends on the `0.3` SUI family.
- `Event::RawMouseMotion` and `WindowEvent::Moved` require handling in exhaustive
  event matches. Public widget appearance/configuration structs gained fields,
  including `ControlPalette::selection_border`; update explicit struct literals.
- Rust 1.90 remains the minimum supported version.
- Linux webview builds require the system WebKitGTK 4.1 development libraries;
  the Rust bindings do not install the browser engine.
- Browser support remains alpha and Android remains experimental. Native
  webviews are an optional crate; Python wheels and Node addons are not part of
  this Rust registry release.

## [0.2.1]

This release refines interaction behavior and rendering consistency across
widgets, overlays, demos, and accessibility surfaces.

### Highlights

- Added configurable clipboard ownership and a shared editable-text controller
  for consistent text editing, selection, and copy behavior.
- Added recursive context-menu submenus with collision-aware cascade placement
  and corrected activation lifecycle handling.
- Improved list and tree row padding, typography, drop-target feedback,
  slider updates, status badges, and pixel-canvas brush defaults.
- Stabilized accessibility roots, empty-state actions, focus behavior, and
  synchronous event processing in the test harness.
- Kept nested retained layers synchronized with dialog and popover opacity and
  translation animations.
- Expanded the theme editor and motion demo while preserving scroll state and
  preventing focus-ring clipping.

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
[0.2.1]: https://github.com/sinomo-lab/sui/compare/v0.2.0...v0.2.1
[0.3.0]: https://github.com/sinomo-lab/sui/compare/v0.2.1...v0.3.0
