# Changelog

All notable changes to SUI are documented in this file. SUI follows Semantic
Versioning, with the usual expectation that the API may change during the
`0.x` series.

## [Unreleased]

### Icons

- Replace bundled Lucide assets with Tabler Icons 3.50.0 outline icons under
  MIT, retaining the upstream copyright and license. Built-in `IconGlyph`
  variants and widget APIs keep their semantic names and use Tabler geometry.
- Replace the `sinomo-ui-lucide` crate (`sui_lucide`) with
  `sinomo-ui-tabler` (`sui_tabler`), versioned with the SUI workspace.
  Direct users should migrate `LucideIcon` to `TablerIcon`, choose Tabler's
  icon names, and replace `IconGlyph::lucide_icon()` with `tabler_icon()`.
  Raw image handles use a new Tabler namespace.

## [0.6.1] - 2026-11-01

This release lets event handlers open URLs, adds file and folder dialogs on
Android, scrolls text that overflows text fields and code blocks, makes touch
taps more reliable, and can elide labels that don't fit.

### Upgrading from 0.6.0

- Update the SUI crates together to `0.6.1`. `sinomo-ui-lucide` stays at
  `1.47.3`.
- Struct literals of `FileDialogSelection` need `folders` (or
  `..Default::default()`), and those of `ActionTilePaint` need
  `text_overflow`.
- Building the browser package needs `wasm-bindgen-cli` 0.2.130.

### Rust

- Added `EventCtx::open_url`, which opens a URL in the default browser or the
  app registered for its scheme on desktop, Android, and the web. Call it from
  any handler, such as a `Link`'s `on_open_with_ctx`; links still open nothing
  on their own. `LocalUrlOpenerBackend` records the URLs a test opens.
- A `UrlPolicy` decides which URLs open. The default allows only absolute
  `http`, `https`, and `mailto` URLs, so untrusted Markdown cannot launch
  local files or other protocol handlers. Install another with
  `App::url_policy`, extend the default with `UrlPolicy::allows_by_default`,
  or allow everything with `UrlPolicy::allow_all`.
- `NativeFileDialogs` works on Android 8.0 and later through the system
  document picker. Files chosen there have no `path` and are read and written
  through their document providers.
- Folder dialogs return `PlatformFolder` handles in
  `FileDialogSelection::folders`, which list, find, and create entries without
  a filesystem path, on desktop, Android, and in browsers with
  `showDirectoryPicker`.
- `TextInput`, `PasswordInput`, and `DateTimeInput` scroll text wider than the
  field to keep the caret in view, instead of pinning the caret over hidden
  text. Editable semantics report the offset as `scroll_x`, and read-only text
  keeps the field's `text_style`.
- Sideways touch drags on a wide `RichDocumentView` code block pan the code
  instead of selecting text, and clicks in a code block scrolled by the wheel
  now land under the pointer.
- A tap that moves focus off an editor keeps the soft keyboard up until the
  finger lifts, so the relayout no longer moves the tapped button away. Touch
  presses on `Button` and `IconButton` are judged by where the button was when
  the finger went down.
- Added `ElidedLine` and `paint_text_line_elided`, which end text too wide for
  its line in "…". `SegmentedControl`, `BrowserTabBar`, and `ActionTilePaint`
  take a `text_overflow` option: `TextOverflow::Ellipsis` elides labels
  instead of clipping them, and `SegmentedControl` then widens the segments
  that need it.
- Added `ActionTileGeometry`, which `paint_action_tile` now returns, with
  `ActionTileGeometry::height` for the height a tile needs.
- Updated dependencies, including PyO3 0.30, napi 3.16, resvg 0.49, and
  wasm-bindgen 0.2.130.

### Python and JavaScript

- Added `open_url(url)` (`openUrl`), which can serve as a link's `on_open`
  (`onOpen`), with `set_url_policy` (`setUrlPolicy`) and
  `url_allowed_by_default` (`urlAllowedByDefault`) to choose which URLs open.

## [0.6.0] - 2026-10-30

This release makes the JavaScript package async and runs it in browsers on a
WebAssembly build, removes the APIs deprecated in 0.4, and makes small-change
frames much cheaper, down to redrawing only what changed on every platform.

### Upgrading from 0.5

- Update the SUI crates together from `0.5` to `0.6`, and `sinomo-ui-lucide` to
  `1.47.3`, which bundles the same Lucide 1.47.0 icons for the `0.6` family.
- Clear 0.5's deprecation warnings before updating: each names its
  replacement, and 0.6 removes the deprecated APIs.

### Breaking changes

- Removed the APIs deprecated in 0.4. Code without deprecation warnings is
  unaffected; the replacements are:
  - Types: `Modal` → `Dialog`, `Drawer` → `SideSheet`, `BusyIndicator` →
    `Spinner`, `ComboBox` → `Select`, `SpinBox` → `NumberInput`,
    `MultilineTextInput` → `TextArea`, `Divider` → `Separator`, `DataGrid` →
    `Table`, `PathBar` → `Breadcrumb`, `ResizablePane` → `SplitView`, and the
    `CanvasAppearance`, `CanvasRulerAppearance`, `PixelCanvasAppearance`, and
    `ColorPickerAppearance` aliases → their `*Colors` types. The unused
    `Style` struct is gone.
  - Builder methods: `spacing` → `gap`, `line_spacing` → `cross_gap`,
    `radius` → `corner_radius`, `disabled()` → `enabled(false)`,
    `appearance` → `colors`, `shown` → `open`, `collapsed()` →
    `expanded(false)`, `without_border()` → `show_border(false)`,
    `content`/`with_content`/`with_child` → `child` or `item`,
    `activate_with_content` → `activate_with_child`, `subtitle` → `detail`,
    `TreeItem::children` → `items`, `Select::expanded` → `open`,
    `Icon::label` and `Image::label` → `semantic_name`, and the checked-state
    names `on`, `on_when`, `selected`, `on_toggle`, and
    `on_select` → `checked`, `checked_when`, and `on_change` (and
    `on_select_with_ctx` → `on_change_with_ctx`). `FloatingStack::theme`,
    which had no effect, is gone.
  - Accessors and setters: `child()` → `child_pod()`, `items`/`items_mut`/
    `item`/`set_item` on `Flex` → their `flex_*` names, `is_on`/`is_selected`
    → `is_checked`, `set_on`/`set_selected` → `set_checked`, `is_shown`/
    `set_shown` → `is_open`/`set_open`, `label()` → `current_label()`,
    `color()` → `current_color()`, `TextInput::name()` → `accessible_name()`,
    and `state()` on workspaces → `shared_state()`.
  - `dynamic(fallback, reader)` constructors → `new(fallback)` with
    `text_when`, `label_when`, or `document_when`; `Label::style` and
    `style_when` → `text_style` and `text_style_when`;
    `RebuildOnChange::new` and `new_observable` → `key_when` and `key_from`.
  - `register_builtin_icon_resources` is gone; built-in icons need no
    registration.
  - `RadioButton::on_select`, which fired on every activation, has no
    replacement; `on_change` reports only actual changes.
- The JavaScript package `@sui/ui` is now ESM and async-first, and needs
  Node.js 20.19 or newer (CommonJS code can still `require` it):
  - `App.run()` returns a promise and runs on Node's event loop instead of
    blocking; pass `{ onReady(ui) { … } }` instead of calling
    `App.runWithHandle`.
  - `UiHandle.post`, `callLater`, `callEvery`, `Timer`, and `UiTaskQueue` are
    gone; Node's own timers and promises work while the app runs.
  - `UiHandle.showFileDialog(options)` returns a promise instead of taking a
    callback.
  - Widget factories are `lowerCamelCase` only; `PascalCase` functions such
    as `Button` are no longer exported.
  - `RunningApp.windowId(index)` is now `windowHandle(index)`.

### JavaScript

- Added a WebAssembly build of the same API for browsers, built with
  `cargo xtask web build` and loaded through the package's `browser` export
  condition. `App.run({ canvas })` draws on a canvas sized to its container.
- Callbacks may be `async`, and their errors and rejected promises go to
  `setErrorHandler` (by default `console.error`) without stopping the app.
- Added `app.once(name)`, `state.changed()`, `state.changes()`, and
  `running.idle()`, and `App.run` and listeners take an `AbortSignal`.
  Subscriptions are disposable with `using`.
- Builders chain; `Point`, `Size`, `Rect`, and `Color` have `equals` and
  `toString`; added `Color.rgba`, `Color.black`, `Color.white`, `version`,
  and `RunningApp.saveScreenshot`.
- Added `app.fontUrl(url)` and `FontHandle.family` (also in Python). Invalid
  font data now fails at registration.
- Fixed `rebuildOnChange`, which rendered nothing, and custom widget `event`
  callbacks that return nothing, whose context requests were dropped. A
  virtual table's string `selected` state now receives string keys.
- Added a `node:test` suite that runs on both the addon and the WebAssembly
  build.

### Python

- The sdist places the license file at its root, as its metadata declares,
  and the release workflow checks each sdist before uploading it.
- The examples open interactive windows with `App.run()` instead of printing
  snapshots.

### Rust

- Added `DesktopPlatform::start_pumped`, `PumpedDesktop`, and
  `App::start_pumped_with_handle` to drive the desktop event loop from another
  framework's loop, and `DesktopPlatform::start_web`, `with_web_canvas`, and
  `App::start_web_with_handle` to run on a chosen browser canvas.
- Fonts can be WOFF or WOFF2. Added `RegisteredFont::try_from_bytes` and
  `RegisteredFont::family_name`; `font_bytes` and `register_font_bytes` now
  fail for data that is not a font.
- Fixed LCD text drawn in grayscale after a renderer without dual-source
  blending shared its text pipeline.
- On Windows, closing the last window no longer leaves it on screen until the
  process exits.
- Fixed `ContextMenu` panels misplaced after a parent measured them again, and
  `VirtualTable` viewport observers alternating between sizes forever.
- Touch input no longer opens tooltips.

### Performance

- Windows redraw only the regions that changed, and nothing when none did:
  browsers, HDR, and wide-gamut outputs through their persistent target, and
  native SDR windows through a persistent frame copied to the swapchain. The
  frame costs one window-sized texture, which `trim_memory` frees while the
  window has no surface.
- Scrolling and translating a retained layer is compositor-only, including
  content far from the origin, and text scrolling over a uniform background
  keeps its rasterized glyphs.
- Rectangular clips are applied in the scene shaders, and draws are reordered
  and merged across packets into cached groups, so the 1,200-widget controls
  grid goes from 1,201 to 22 draw calls per frame. Vertex and instance formats
  grow by 16 bytes each.
- Small-change frames skip most whole-tree and whole-scene work, and layout
  change detection is no longer quadratic in the number of widgets.
- On Android and in browsers, an unchanged window no longer presents every
  display frame. Desktop windows keep presenting so that variable refresh rate
  does not lower the monitor's refresh rate.
- Suspending a window, such as by backgrounding an Android app, keeps its
  retained scene, so resuming at the same size shows the existing frame. Added
  `WgpuRenderer::trim_memory` and `MemoryTrimLevel`, which hosts call on
  suspend and on the system's memory warning.
- Path clips use a `Stencil8` attachment discarded after each pass, saving
  tile-based mobile GPUs a write per clipped pass.
- The image texture cache expires textures unused for 2 seconds rather than
  120 frames, within a budget of 64 MB on mobile and 256 MB elsewhere.
- Release web builds use WebAssembly SIMD and run `wasm-opt -Os` (for the
  JavaScript package, when `wasm-opt` is installed or `WASM_OPT` is set). The
  demo module drops from 16.6 MB to 11.7 MB.
- Bundled and app fonts are shared with the text system instead of copied,
  per-widget phases read the clock only when `SUI_PROFILE_WIDGET_TIMINGS` is
  set, and JavaScript callbacks cross the WebAssembly boundary less often.
- Added `scroll-flat` and `scroll-retained` fixtures to `sui-bench`.

## [0.5.0] - 2026-10-08

This release publishes the Python binding on PyPI as `sinomo-ui`, brings its
API close to the Rust widgets and services, and makes callback errors,
Ctrl+C, and state-bound widgets behave correctly in Python and JavaScript.

### Upgrading from 0.4

- Update the SUI crates together from `0.4` to `0.5`, and `sinomo-ui-lucide` to
  `1.47.2`, which bundles the same Lucide 1.47.0 icons for the `0.5` family.

### Breaking changes

- The Python distribution is now `sinomo-ui` and the import package
  `sinomo_ui` (previously `sui-ui` and `sui`); use `import sinomo_ui as sui`.
- Python widget factories are `snake_case` only; the `PascalCase` aliases
  such as `sui.Button` are gone.
- Layout in the pass right after a window resize now uses the new viewport
  size instead of the previous one.

### Python

- Published on PyPI as `sinomo-ui`: abi3 wheels for CPython 3.10 and newer on
  Windows x64, macOS arm64 and x64 (macOS 11+), and Linux x64 and aarch64
  (glibc 2.28+), plus an sdist.
- Added `sinomo_ui.aio` with `AsyncRunner`, `run_on_ui`, and `file_dialog` to
  run asyncio coroutines beside the UI loop.
- Exceptions raised in callbacks are reported with their traceback through
  `sys.excepthook` or `set_exception_handler`; most used to be discarded.
- Ctrl+C stops `App.run()` with `KeyboardInterrupt`, and a callback raising
  `KeyboardInterrupt` or `SystemExit` stops the app and re-raises from
  `run()`. Added `UiHandle.request_exit()`.
- Value types such as `Point` and `Color`, and resource and window handles,
  compare and hash by value; common objects have readable reprs;
  `StateSubscription` is a context manager; and the package has
  `__version__`.
- `State` keeps `int` values as `int`, and `EventContext.request_paint()`
  works without a rectangle.

### Python and JavaScript

- Added `UiHandle.call_later` and `call_every`, `UiHandle.show_file_dialog`,
  app-level `clipboard_text` and `set_clipboard_text`, and `FocusController`
  with `focus_scope`. JavaScript gains `UiHandle.requestExit`.
- Added host-driven test helpers: `RunningApp.advance_time`,
  `settle_animations`, `frame_time`, `screenshot_png`, `save_screenshot`,
  `set_motion_preference`, and `within=` on semantic queries.
  `RunningApp.drain_ready_events()` now dispatches due timers and animation
  frames instead of discarding them.
- State-bound `dialog`, `popover`, and `tabs` follow later changes and write
  the user's changes back, and state-bound labels of buttons, checkboxes,
  switches, and radio buttons update in place.
- Widgets gained most of their Rust options: `enabled` on every control;
  `justify`, `align_items`, `wrap`, `flex_item`, and `spacer` for rows and
  columns; grid tracks and `grid_cell`; `tabs` panels; more `button`,
  `label`, and `dialog` options; `ListItem`; and `read_only`, `on_submit`,
  and `on_focus_change` on text fields.
- Added `ScrollController`, `rebuild_on_change(states, build)`, and
  `semantic_name` on labels and toggles.
- Bound `VirtualTable` as `virtual_table`, which realizes only the visible
  rows of a thread-safe, keyed `TableModel` and selects by row key.

### Rust

- Added `UiHandle::request_exit` and `CommandSender::request_exit` to end the
  event loop from any thread.
- Added `Tabs::selected_when`, and `label_when` to `Button`, `Checkbox`,
  `Switch`, and `RadioButton`.
- Added `FocusScopeState::request_focus` and re-exported
  `OsClipboardBackend`.

## [0.4.1] - 2026-10-04

This release adds observable background tasks, makes reactive updates and text
rendering cheaper, improves platform font rendering, and adds touch scroll
inertia.

### Highlights

- Added `Task`, `TaskState`, and `TaskHandle` for background work with
  progress, results, errors, cancellation, and stale-worker protection.
  `refresh` keeps the previous result visible while loading, and
  `TaskHandle::run` completes a task from a future.
- Added `batch` to coalesce notifications, `combine` and `combine_named` to
  derive values from two to four observables, and `changed().await` to wait
  for an observable to change.
- Added borrowed signal reads and in-place updates (`with`, `modify`,
  `mark_changed`) and `Arc` updates (`set_arc`, `modify_arc`). Selector clones
  share one source subscription and cached result.
- Clipped glyphs are skipped before rasterization, and grayscale glyphs get
  their own atlas pages, which use 4 MiB of GPU memory instead of 16 MiB on
  both the CPU and GPU.
- Text follows Windows ClearType and Linux fontconfig/Xft smoothing
  preferences, Linux and Android hinting is improved, and Android loads
  system fonts with bundled fallbacks.
- `ScrollView` and `VirtualScrollView` coast after touch flings and report
  every move through `on_offset_change`.

### Other changes

- Fixed open contours rendering as fans under Chrome's Vulkan WebGPU backend
  on some mobile GPUs, and made tooltip borders continuous around their tails.
- Menu keyboard shortcuts stay on one line.
- The web demo shows download progress and startup errors while it loads.
- Fixed concurrent selector updates losing changes.
- Updated accessibility, Python, Node, Wasm, and Linux dependencies.

### Compatibility and release notes

- Update the SUI crates together to `0.4.1`. `sinomo-ui-lucide` remains at
  `1.47.1`.
- Selector closures read signal values while the source is locked and must
  not write to that signal; debug builds panic with a message instead of
  deadlocking. Observers still run synchronously on the writing thread, or
  when a `batch` ends.

## [0.4.0] - 2026-10-01

This release refreshes the default theme, gives the built-in widgets one API
vocabulary, moves animation into the runtime with reduced-motion support, and
follows HDR and wide-gamut outputs.

### Highlights

- Refreshed the default themes: every palette role is derived in OKLCH from a
  few source colors, built-in controls are restyled, and shadows are exact
  Gaussian blurs that can sit behind, outside, or inside their box.
- Gave the built-in widgets one API vocabulary, documented in
  `docs/api/conventions.md`: every callback has a `_with_ctx` twin, state is
  set with `p`, `p_when`, or `p_from`, and layout, colors, children, and
  on/off flags use the same names everywhere.
- Moved animation into the runtime: `ctx.animate` drives `Motion` and
  `Progress` values with tweens or springs that keep their momentum when
  retargeted, retained layers animate without repainting, and
  `MotionPreference` follows the system's reduced-motion setting.
- Added `Presence`, `KeyedStack` with staggered changes, delayed transitions,
  and timeline ping-pong loops, delays, and markers.
- Tone mapping keeps hue and leaves SDR content alone, Display P3 colors reach
  wide-gamut outputs, HDR themes fall back to what the output can show, and
  apps can capture their own windows.
- `TextSurface` edits large wrapped documents and lays out right-to-left text,
  `paint_text` places wrapped text as a block, and `Paragraph` lays text out
  once to measure and paint.
- Drag and drop follows modifier keys, scrolls near edges, draws custom
  previews, and cancels on Escape; `ReorderableList` reorders from the
  keyboard.
- Node graphs gain labeled, colored ports with connection rules, undo and
  redo, copy and paste, and context menus.
- `TestApp` runs headless and in parallel by default and can simulate SDR,
  wide-gamut, and HDR displays.
- The widget book shows all 60 component stories on one page, and most demos
  are redesigned.

### Other changes

- `interaction_preview` pins hover, press, or focus visuals, and `show_inline`
  lays overlays out open in place, for documentation and screenshots.
- `Button::focus_on_press(false)` leaves focus where it is, and
  `window_command_dispatches_signal` reports a window's recent commands.
- Added `Surface::shadow`, `Surface::glow`, `ShadowBox`, `Color::oklch`,
  `Color::mix_oklab`, `Color::contrast_ratio`, and an OKLCH color picker mode.
- Added `FloatingWorkspace::transparent`, `FloatingViewConfig::closable`, and
  `Widget::hit_test_self`, which lets a container pass points through.
- Python and JavaScript widget parameters use the same names as Rust.
- Fixed linear gradients, focus rings around context menus and popovers,
  adaptive layouts taking focus on their first layout, and `Select`'s
  accessibility actions.

### Upgrading from 0.3

- Update the SUI crates together from `0.3` to `0.4`, and `sinomo-ui-lucide` to
  `1.47.1`, which bundles the same Lucide 1.47.0 icons for the `0.4` family.
- Renamed APIs keep their old names as deprecated aliases that name the
  replacement. The aliases will be removed in 0.5.
- These changes need code edits:
  - `ThemeColors` is a source model; the `base_*`, `*_content`, `accent`, and
    `neutral` fields are gone, and `error` is now `danger`.
  - Diagnostics moved to `sui::diagnostics`, the prelude is smaller,
    `sui::Padding` is the padding widget and insets are `sui::Insets`, and the
    `testing` feature is gone.
  - Every `_with_ctx` callback receives the `EventCtx` first.
  - `TabBar`, `Tabs`, and `SegmentedControl` return `Option<usize>` from
    `selected_index()`, and `read_only` takes a `bool`.
  - Single on/off builders take a `bool`: pass `true` to `fill_width`,
    `fill_height`, `fill_child_width`, `fill_child_height`, `single_line`,
    `destructive`, `separator_before`, `activate_with_child`, `appear`,
    `fit_on_first_layout`, and `TextCellPaint::numeric`.
  - `SizedBox::child(widget)` sets the child, and containers read theirs with
    `child_pod()`. `Link::url(url)` sets the URL; `Link::from_url` builds a
    link labeled with its URL.
  - `PaintCtx` can no longer request invalidations, and `paint_aligned_text`
    and `paint_single_line_aligned_text` became `paint_text` and
    `paint_text_line`.
  - Struct literals of `ShadowParams`, `FloatingViewSnapshot`,
    `WindowOutputDiagnostics`, `DragEvent`, `DragPreview`, and node-graph
    `Handle` need their new fields, and those of `LayerProperties`,
    `PlaybackState`, `FloatingViewConfig`, and `NodeGraphConfig` can end with
    `..Default::default()`. Exhaustive matches on `NodeGraphEvent` need the
    new variants.
  - Drag sources allow several `DropEffects`, and `allowed_effect` is the
    effect a drop takes when no key asks for another.
  - `CommandSender` and `UiHandle` sends return a sequence number, so a send
    used as a `()` expression needs a semicolon.
- These changes compile but behave differently:
  - Built-in controls are restyled, and a shadow's `blur` is now the CSS blur
    radius, so theme shadows match their CSS tokens and look different.
  - `Clamp` and `Reinhard` tone mapping keep hue and pass SDR content through
    unchanged.
  - `TestApp` runs headless unless a test asks for `live(true)` or
    `visible(true)`.

## [0.3.0] - 2026-09-26

This release adds application and editor surfaces, improves text quality and
retained rendering, and expands the source-built language bindings.

### Highlights

- Added the `sinomo-ui-nodes` graph-editor library: typed nodes, edges, and
  handles, controlled or uncontrolled state, custom node widgets, subflows,
  resizing, reconnection, per-element semantics, animated viewports,
  controls, and a minimap. The demo gains a node-editor workspace behind the
  default `nodes` feature.
- Added the optional `sinomo-ui-webview` crate for WRY-backed native child
  webviews with typed page, title, and IPC events and application-owned
  browser policy.
- Added retained docking workspaces, editor cursor grabbing and raw mouse
  motion, safe initial window placement, and GPU interoperability contracts.
- Added affine-transformed widget subtrees and widgets hosted in a `Canvas`,
  which scales their paint, input, semantics, and layout, with screen-space
  or custom zoom and touch pinch.
- Added provider-neutral image icons, bounded browser tabs, compact dialogs,
  and reserved scroll bar gutters.
- Text reuses shaping across layout widths, renders hinted, transformed, and
  LCD text more accurately, bundles Noto Sans Arabic and Hebrew fallbacks for
  the web and Android, and places the caret correctly across right-to-left
  runs.
- Rendering retains widget output, scene fragments, packets, and GPU buffers
  instead of preparing them again each frame, and GPU setup overlaps CPU
  startup.
- On Windows, SUI prefers compatible Direct3D 12 hardware. Native windows keep
  presenting and animating during move and resize loops, and focused VSync
  windows keep presenting so an idle UI doesn't lower a variable-refresh
  monitor's refresh rate.
- Fixed selector write-back deadlocks, bounded the text-layout and path
  caches, and made node-graph culling follow edge curvature.
- Added widget, frame, and text-flow benchmarks; the demo's FPS counter
  measures host frame cadence, and idle mouse motion no longer inflates
  frame-time readings.
- Expanded the source-built Python and JavaScript bindings for the newer
  widget and editor surfaces.
- Updated dependencies and bundled Lucide icons to 1.47.0, and restored Rust
  1.90 compatibility.

### Compatibility and release notes

- Update SUI crate dependencies together from `0.2` to `0.3`. Lucide keeps its
  independent `1.47.0` version and now depends on the `0.3` SUI family.
- Exhaustive event matches need `Event::RawMouseMotion` and
  `WindowEvent::Moved`, and struct literals of widget appearance and
  configuration structs, such as `ControlPalette`, need their new fields.
- Linux webview builds require the system WebKitGTK 4.1 development
  libraries.

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
[0.4.0]: https://github.com/sinomo-lab/sui/compare/v0.3.0...v0.4.0
[0.4.1]: https://github.com/sinomo-lab/sui/compare/v0.4.0...v0.4.1
[0.5.0]: https://github.com/sinomo-lab/sui/compare/v0.4.1...v0.5.0
[0.6.0]: https://github.com/sinomo-lab/sui/compare/v0.5.0...v0.6.0
[0.6.1]: https://github.com/sinomo-lab/sui/compare/v0.6.0...v0.6.1
