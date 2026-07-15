# Cross-language bindings roadmap

**Status:** Active. Native Python and Node/Electron bindings have an alpha
foundation, but they are not published and do not yet cover every Rust widget
or deployment target.

This document tracks unfinished binding work. Current setup and examples live
in the [Python guide](../../crates/sui-python/README.md), the
[Node/Electron guide](../../crates/sui-js/README.md), and the
[examples catalog](../examples.md).

## Shipped foundation

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
  cross-language compatibility tests.

The checked manifest currently has complete core, Python, JavaScript,
TypeScript, documentation, and required compatibility coverage:

```bash
cargo xtask bindings generate --check
cargo xtask bindings coverage
```

The coverage command is the release gate. The current high-level manifest
contains these public names, grouped here so documentation coverage remains
auditable:

- Descriptors: `TextSpan`, `StatusBarSegment`, `SegmentedControlItem`,
  `TableColumn`, `TableRow`, `TreeItem`, `LayerListItem`, `MenuItem`,
  `ToolPaletteItem`, `ColorPaletteSwatch`, `BrushPreviewSpec`, and
  `FloatingStackWindow`.
- Basic controls: `Label`, `Button`, `Icon`, `IconButton`, `Link`, `Checkbox`,
  `Switch`, `RadioButton`, `RadioGroup`, `SegmentedControl`, `Slider`,
  `NumberInput`, `Select`, `ProgressBar`, `BusyIndicator`, `TextInput`,
  `PasswordInput`, `DateTimeInput`, and `TextArea`.
- Content and data: `Breadcrumb`, `PathBar`, `ListView`, `Table`, `DataGrid`,
  `TreeView`, `LayerList`, `RichText`, `Image`, `ColorSwatch`, `ColorPalette`,
  `ColorPicker`, `SignalMeter`, `StatusBadge`, `StatusBar`, and `DetailRow`.
- Containers and application widgets: `Separator`, `EmptyState`, `Surface`,
  `Toolbar`, `ToolPalette`, `PresetStrip`, `BrowserTabBar`, `ScrollView`,
  `Menu`, `ContextMenu`, `TabBar`, `Tabs`, `Dialog`, `StatusBarHost`,
  `Tooltip`, `Popover`, `DockPanel`, `ActionCard`, `BrushPreview`,
  `CommandGroup`, `CoverageDots`, `FramedField`, `PlacementBadge`,
  `PropertyRow`, `SectionLabel`, `SideSheet`, `FloatingStack`, and
  `ReorderableList`.
- Layout and forms: `Column`, `Row`, `Padding`, `Align`, `Background`,
  `SizedBox`, `Stack`, `SemanticRegion`, `FormRow`, `FieldGroup`,
  `FormSection`, `PanelSection`, `Dock`, `FixedPaneSplit`,
  `MeasuredBottomDock`, `SplitView`, `SwitchView`, `TrailingSlotRow`, and
  `VirtualScrollView`.
- Interop: `ExternalSurface`.

The manifest also classifies every public Rust `Widget` implementation. Most
portable widgets map directly to generated binding items. `ActionCard`,
`BrushPreview`, `DateTimeInput`, `PasswordInput`, `SideSheet`, and `SplitView`
use manual wrappers because they need callback, secrecy, descriptor, or
state-synchronization policy; `ReorderableList` also uses a manual wrapper to
translate its reorder event. `Spinner` is represented by `BusyIndicator`, and
`Flex` by `Column` and `Row`.

The intentionally Rust-only tier is `Canvas`, `CanvasRuler`, `DragDropHost`,
`Draggable`, `DropTarget`, `FloatingWorkspace`, `PixelCanvas`,
`RebuildOnChange`, `RebuildOnConstraints`, and `ScrollBar`. These widgets
expose Rust-local closures, type-erased payloads, shared
non-thread-safe state, or output/control contracts that do not have a safe
portable value model. `TextSurface` is represented by the supported `TextArea`
facade, while `VirtualTable` is represented by `Table` unless an application
implements its own virtualized foreign widget.

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

### 4. Design browser JavaScript/WASM bindings

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

Bindings currently select validated built-in SUI shaders. User shader support
requires a separate reviewed contract for:

- WGSL validation and resource limits;
- uniform and texture schemas;
- pipeline caching and device-loss recreation;
- deterministic errors across Python and JavaScript;
- a fallback or explicit unsupported result on hosts without the capability.

Raw render-pass access is not part of this milestone.

### 6. Integrate zero-copy external surfaces

The descriptor and capability foundation exists, while shared-texture and
shared-target renderer composition is unfinished. Each backend integration
must define:

- compatible formats, dimensions, color encoding, and alpha conventions;
- import/export handle ownership and lifetime;
- producer/consumer synchronization and frame reuse;
- device identity and adapter mismatch behavior;
- resize, device loss, and process failure handling;
- a safe CPU-copy fallback and diagnostics explaining why it was selected.

Land one backend at a time behind capability checks and conformance tests. Do
not describe a descriptor-only path as zero-copy support.

### 7. Stabilize documentation and compatibility policy

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

## Definition of done

The cross-language effort is complete only when supported native packages are
installable from their registries, their published artifacts pass clean-host
tests, the portable API and compatibility policy are documented, and all
claimed platforms have real lifecycle/input/render smoke coverage. Browser,
custom shader, and zero-copy work may graduate independently, but each must be
reported as unsupported until its own exit criteria pass.
