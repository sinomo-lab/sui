# Cross-language bindings roadmap

**Status:** Active. The native Python and Node/Electron alpha foundations and
generated API coverage are implemented. Native package release, platform smoke
coverage, exact editor/virtual-table parity, browser bindings, custom WGSL, and
zero-copy binding composition remain open.

This document tracks unfinished binding work. Current setup and examples live
in the [Python guide](../../crates/sui-python/README.md), the
[Node/Electron guide](../../crates/sui-js/README.md), and the
[examples catalog](../examples.md).

## Implementation status

This status reflects the implementation reviewed at `c1a60f8`. Generation and
manifest coverage checks pass; platform and package acceptance remain separate
release gates.

| Area | Status | Remaining work |
| --- | --- | --- |
| Native Python and Node/Electron API | Implemented alpha | Preserve lifecycle, state, resource, paint, semantics, animation, and rich-document contracts during release work. |
| Generated APIs and widget classification | Implemented | Keep generation and coverage checks passing as public Rust APIs evolve. |
| Exact portable widget parity | Partial | `TextSurface` uses `TextArea` and `VirtualTable` uses `Table`; editor-surface behavior, virtual rows, and arbitrary foreign row renderers still need dedicated contracts and tests. |
| Native packages | Partial | Local builds, loaders, metadata, and declarations exist; supported artifact matrices, release CI, clean-install tests, and publication remain open. |
| Desktop smoke coverage | Partial | Host-driven tests and examples exist; supported-platform real-window lifecycle/input/render coverage remains open. |
| Browser JavaScript/WASM bindings | Not implemented | Define and implement the browser package and lifecycle; the Rust/WASM demo is an existing, separate surface. |
| User shader registration | Not implemented | Built-in shader descriptors exist; custom WGSL validation, schemas, caching, and lifecycle handling remain open. |
| Zero-copy binding surfaces | Partial | Descriptors and synchronization types exist; only CPU RGBA composition is rendered through the bindings today. |
| Stable compatibility policy | Partial | Guides and generated declarations exist; deprecation policy, package/platform support policy, and release gates remain open. |

## Implemented foundation

The workspace currently includes:

- `sinomo-ui-bindings-core`, a language-neutral retained widget, resource, event,
  paint, semantics, and host-driven runtime bridge;
- a PyO3/maturin package in `sinomo-ui-python`;
- a napi-rs package with TypeScript declarations in `sinomo-ui-js`;
- normal desktop `run`/`run_with_handle` entry points and host-driven
  `start`, `render`, event, callback-posting, and drain APIs;
- binding-safe custom widget callbacks, paths, clips, transforms, text, image
  quads, built-in shader descriptors, and semantic nodes;
- RGBA, PNG, SVG, and font resource registration;
- explicit external-texture, synchronization, backend-handle, and capability
  descriptors, with a CPU RGBA fallback for `ExternalSurface`;
- generated widget manifests, a complete Rust-widget classification, and
  cross-language compatibility tests;
- generated Python registration, `snake_case` factories, `sui.pyi`, and
  `py.typed` metadata, plus generated JavaScript options-object factories and
  matching TypeScript interfaces;
- live shared themes, selector/watch state APIs, named UI-thread messages,
  custom composite children/layout, event contexts, inspector summaries,
  renderer/HDR configuration, and window geometry/icon policy;
- thread-safe streaming rich documents with attachments and extension blocks,
  plus portable docking state and editor workspace composition;
- first-class portable animation values, transitions, springs, retained values,
  tracks, clips, timelines, players, serialized documents, and undoable editor
  operations;
- complete semantic-node snapshots with host-language queries and actionable
  hover/click/press/fill helpers for deterministic binding tests.

The checked manifest currently has complete core, Python, JavaScript,
TypeScript, documentation, and required compatibility coverage:

```bash
cargo xtask bindings generate --check
cargo xtask bindings coverage
```

Generation and coverage are required release gates. Widget classification
includes documented equivalents; exact feature parity is tracked separately
below. The current high-level manifest contains these public names, grouped
here so documentation coverage remains auditable:

- Descriptors: `TextSpan`, `StatusBarSegment`, `SegmentedControlItem`,
  `TableColumn`, `TableRow`, `TreeItem`, `LayerListItem`, `MenuItem`,
  `ToolPaletteItem`, `ColorPaletteSwatch`, `BrushPreviewSpec`, and
  `FloatingStackWindow`, `RichDocument`, `RichDocumentUpdate`, `ConstraintCase`,
  `ResponsiveSidebarState`, `MasterDetailState`, `NotificationCenter`,
  `VirtualListItem`, `VirtualListModel`, `CanvasViewport`, `CanvasStroke`,
  `CanvasShape`, `PixelCanvasState`, `PixelCanvasExport`, `DragScope`,
  `FloatingView`, `FloatingViewSnapshot`, and `FloatingWorkspaceState`, plus the
  `DockNode`, `DockFloatingGroup`, `DockLayout`, `DockState`, and
  `DockPanelSpec` docking model.
- Basic controls: `Label`, `Button`, `Icon`, `IconButton`, `Link`, `Checkbox`,
  `Switch`, `RadioButton`, `RadioGroup`, `SegmentedControl`, `Slider`,
  `NumberInput`, `Select`, `ProgressBar`, `BusyIndicator`, `TextInput`,
  `PasswordInput`, `DateTimeInput`, and `TextArea`.
- Content and data: `Breadcrumb`, `PathBar`, `ListView`, `Table`, `DataGrid`,
  `TreeView`, `LayerList`, `RichText`, `RichDocumentView`, `Image`, `Canvas`,
  `CanvasRuler`, `PixelCanvas`, `ColorSwatch`, `ColorPalette`,
  `ColorPicker`, `SimpleColorPicker`, `SignalMeter`, `StatusBadge`, `StatusBar`, and `DetailRow`.
- Containers and application widgets: `Separator`, `EmptyState`, `Surface`,
  `Toolbar`, `ToolPalette`, `PresetStrip`, `BrowserTabBar`, `ScrollView`, `OverlayHost`,
  `Menu`, `ContextMenu`, `TabBar`, `Tabs`, `Dialog`, `CommandPalette`, `StatusBarHost`,
  `Tooltip`, `Popover`, `DockPanel`, `ActionCard`, `BrushPreview`,
  `CommandGroup`, `CoverageDots`, `FramedField`, `PlacementBadge`,
  `PropertyRow`, `SectionLabel`, `SideSheet`, `BottomSheet`, `NotificationHost`,
  `DragDropHost`, `Draggable`, `DropTarget`,
  `DockWorkspace`, `FloatingWorkspace`, `FloatingStack`, and
  `ReorderableList`.
- Layout and forms: `Column`, `Row`, `Padding`, `Align`, `Background`,
  `Grid`, `AspectRatio`, `SafeArea`, `LayoutTransition`, `AdaptiveView`,
  `ConstraintView`, `ResponsiveSidebar`, `MasterDetail`, `SizedBox`, `Stack`,
  `SemanticRegion`, `FormRow`, `FieldGroup`,
  `FormSection`, `PanelSection`, `Dock`, `FixedPaneSplit`,
  `MeasuredBottomDock`, `SplitView`, `SwitchView`, `TrailingSlotRow`,
  `VirtualScrollView`, and `VirtualList`.
- Interop: `ExternalSurface`.

The manifest also classifies every public Rust `Widget` implementation. Most
portable widgets map directly to generated binding items. `ActionCard`,
`BrushPreview`, `DateTimeInput`, `PasswordInput`, `SideSheet`, and `SplitView`
use manual wrappers because they need callback, secrecy, descriptor, or
state-synchronization policy; `ReorderableList` also uses a manual wrapper to
translate its reorder event. `Spinner` is represented by `BusyIndicator`, and
`Flex` by `Column` and `Row`.

Every current public Rust `Widget` implementation now has a cross-language
binding, a manual portable wrapper, or a documented host-language equivalent.
`TextSurface` is represented by `TextArea`, while `VirtualTable` is currently
represented by `Table`; arbitrary foreign row renderers and true virtual-table
parity remain unfinished.
`RebuildOnChange` maps to retained `SwitchView`, `RebuildOnConstraints` maps to
`ConstraintView`, and standalone `ScrollBar` behavior is owned by `ScrollView`
in the host-language APIs.

## Stable design constraints

Future work should preserve these boundaries:

1. **The retained tree stays on the UI thread.** Foreign workers publish work
   through queues and wake handles; they do not mutate widgets directly.
2. **Bind public concepts, not every Rust implementation type.** The supported
   model is apps, windows, widgets, state, handles, resources, events,
   semantics, and validated paint commands.
3. **Handles may cross threads; widget objects may not.** Resource, window,
   UI, external-surface, and synchronization handles have explicit ownership
   contracts.
4. **Custom painting remains renderer-safe.** Normal callbacks build validated
   scene commands and never receive a raw `wgpu::Device`, queue, or render
   pass.
5. **GPU interop is capability-driven.** Zero-copy composition is conditional
   on backend, host, format, ownership, and synchronization support. CPU copy
   is the portable fallback.
6. **Python and JavaScript should behave alike.** Naming may follow language
   conventions, but lifecycle, state, semantics, errors, and widget behavior
   should remain compatible and be tested from the same manifest.

## Release milestones

### 1. Publish reproducible native packages

**Status: partial.** Python has maturin configuration and type metadata;
Node/Electron has a native loader, package metadata, TypeScript declarations,
and local build/consumer commands. The remaining work is artifact production,
installation validation, and release automation.

Python:

- select supported CPython versions and target triples;
- build and test wheels in CI with maturin;
- verify wheel installation in clean environments;
- publish package metadata, type information, license files, and release notes.

Node/Electron:

- select supported Node ABI, Electron, OS, and architecture combinations;
- build signed/checksummed prebuilt `.node` artifacts;
- exercise the native loader from packed tarballs in clean environments;
- publish TypeScript declarations, package metadata, license files, and release
  notes.

Shared exit criteria:

- a version is traceable to one Git commit and one Rust workspace version;
- CI tests the exact artifacts users install;
- failures report unsupported platforms instead of silently selecting an
  incompatible binary;
- package release can be rehearsed without publishing.

### 2. Add real desktop smoke coverage

**Status: partial.** Desktop entry points, deterministic host-driven tests, and
language examples exist. A supported-platform real-window acceptance matrix
remains open.

Host-driven render tests already validate the model, but release builds also
need supported-platform smoke tests that open a window and exercise:

- initial layout, paint, and semantics;
- pointer and keyboard activation;
- text/IME input and clipboard operations;
- background callback posting through `UiHandle`;
- clean shutdown and repeated app construction where the platform permits it.

Keep deterministic host-driven tests as the primary compatibility suite; use
desktop smoke tests to catch packaging, event-loop, graphics, and dynamic
library failures.

### 3. Close editor and virtual-table parity gaps

**Status: partial.** Every public Rust widget has a classification, including
documented equivalents. Keep those equivalents explicit while extending the
portable API:

- define the supported editor-surface behavior beyond the current `TextArea`
  equivalent, including styled content and large-document viewport behavior;
- provide true virtual-table row lifecycle and arbitrary foreign row-renderer
  callbacks with UI-thread ownership and retained identity;
- add matching Python/JavaScript conformance tests for the newly supported
  behavior before changing an equivalent into a direct binding or wrapper.

These are capability follow-ups. The first native alpha may retain documented
equivalents; complete classification alone must not be advertised as exact
editor or virtual-table parity.

### 4. Design browser JavaScript/WASM bindings

**Status: not implemented.** SUI's Rust/WASM demo and web renderer exist, but
the native napi-rs package does not provide a browser JavaScript API.

Browser bindings are a separate product surface, not a rebuild of the napi-rs
package. Before implementation, specify:

- ES module initialization and asynchronous WebGPU startup;
- canvas ownership and resize/device-loss behavior;
- JavaScript callback error boundaries;
- resource upload and URL/byte loading;
- accessibility DOM or AccessKit integration;
- bundler-free and common-bundler examples;
- a generated API source shared with the native TypeScript declarations where
  the lifecycle models overlap.

Exit criteria for an alpha are a documented browser matrix, one framework-free
example, one packed-package smoke test, and semantic/render compatibility for
the portable widget tier.

### 5. Extend safe shader support

**Status: not implemented for user shaders.** Validated built-in shader
descriptors are already available in both languages.

Bindings currently select validated built-in SUI shaders. User shader support
requires a separate reviewed contract for:

- WGSL validation and resource limits;
- uniform and texture schemas;
- pipeline caching and device-loss recreation;
- deterministic errors across Python and JavaScript;
- a fallback or explicit unsupported result on hosts without the capability.

Raw render-pass access is not part of this milestone.

### 6. Integrate zero-copy external surfaces

**Status: partial.** Backend handles, synchronization descriptors, and
capability tiers exist. `BindingExternalSurfaceWidget` currently paints only
the `CpuRgba8` descriptor. Native Rust external-texture APIs exist separately;
the Python/JavaScript bridge still needs shared-texture and shared-target
composition. Each backend integration must define:

- compatible formats, dimensions, color encoding, and alpha conventions;
- import/export handle ownership and lifetime;
- producer/consumer synchronization and frame reuse;
- device identity and adapter mismatch behavior;
- resize, device loss, and process failure handling;
- a safe CPU-copy fallback and diagnostics explaining why it was selected.

Land one backend at a time behind capability checks and conformance tests. Do
not describe a descriptor-only path as zero-copy support.

### 7. Stabilize documentation and compatibility policy

**Status: partial.** Language guides, examples, generated declarations, and
manifest coverage exist. Stable support and deprecation policies and the full
artifact/platform release gate remain open.

- Version the portable API tier and document deprecation expectations.
- Generate reference material from the binding specification where practical.
- Keep one end-to-end tutorial per language synchronized with real examples.
- Publish a platform/package matrix and troubleshooting guide.
- Add a changelog section for binding-specific breaking changes.
- Require `cargo xtask bindings generate --check`, coverage, language tests,
  packed artifact tests, and relevant desktop smoke tests before release.

## Deferred work

These are not required for the first native alpha release:

- browser JavaScript/WASM parity with every native interop capability;
- arbitrary raw GPU device, queue, command encoder, or render-pass access;
- automatic conversion between third-party tensor/graphics objects and every
  native backend handle;
- making foreign widget objects freely thread-safe;
- exact parity with every internal Rust-only debug or editor widget.

## Implementation references

- Native lifecycle and host-driven runtime:
  `crates/sui-bindings-core/src/application.rs`.
- Python packaging and current limitations:
  [Python guide](../../crates/sui-python/README.md) and
  `crates/sui-python/pyproject.toml`.
- Node/Electron packaging and current limitations:
  [Node/Electron guide](../../crates/sui-js/README.md) and
  `crates/sui-js/package.json`.
- Generation and coverage gates: `crates/xtask/src/main.rs`.
- Built-in shader descriptors: `crates/sui-bindings-core/src/shader.rs`.
- External-surface descriptors and binding composition:
  `crates/sui-bindings-core/src/interop.rs` and
  `crates/sui-bindings-core/src/widget_adapters.rs`.
- Native Rust external-texture support: `crates/sui-render-wgpu/src/interop.rs`.

## Definition of done

The cross-language effort is complete only when supported native packages are
installable from their registries, their published artifacts pass clean-host
tests, the portable API and compatibility policy are documented, and all
claimed platforms have real lifecycle/input/render smoke coverage. Browser,
custom shader, and zero-copy work may graduate independently, but each must be
reported as unsupported until its own exit criteria pass.
