# Changelog

All notable changes to SUI are documented in this file. SUI follows Semantic
Versioning, with the usual expectation that the API may change during the
`0.x` series.

## [0.6.0] - 2026-10-30

This release makes the JavaScript package async and runs it in browsers on a
WebAssembly build, removes the APIs deprecated in 0.4, and makes small-change
frames much cheaper: retained layers scroll on the compositor, draws batch
across packets with clips applied in shaders, and windows on every platform
redraw only what changed.

### Upgrading from 0.5

- Update the SUI crates together from `0.5` to `0.6`, and `sinomo-ui-lucide` to
  `1.47.3`, which bundles the same Lucide 1.47.0 icons for the `0.6` family.
- Clear 0.5's deprecation warnings before updating: each names its
  replacement, and 0.6 removes the deprecated APIs.

### Breaking changes

- Removed the APIs deprecated in 0.4, as the 0.4 upgrade notes announced.
  Code that compiled without deprecation warnings is unaffected. Use the
  replacements the warnings named:
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
  - `App.run()` returns a promise and runs the app on Node's event loop
    instead of blocking; pass `{ onReady(ui) { … } }` instead of calling
    `App.runWithHandle`.
  - `UiHandle.post`, `callLater`, and `callEvery`, `Timer`, and `UiTaskQueue`
    are gone; Node's own timers and promises work while the app runs.
  - `UiHandle.showFileDialog(options)` returns a promise of the paths instead
    of taking a callback.
  - Widget factories are `lowerCamelCase` only; the `PascalCase` positional
    functions such as `Button` are no longer exported.
  - `RunningApp.windowId(index)` is now `windowHandle(index)`.

### JavaScript

- Added a WebAssembly build that runs the same API in browsers: build it with
  `cargo xtask web build`, and bundlers and browsers load it through the
  package's `browser` export condition. `App.run({ canvas })` draws on a
  canvas and keeps it the size of its container.
- Callbacks may be `async`, and errors from callbacks, including rejected
  promises, go to `setErrorHandler` (by default `console.error`) without
  stopping the app.
- Added promise helpers: `app.once(name)`, `state.changed()`, the
  `state.changes()` async iterator, and `running.idle()` to wait for async
  callbacks in tests. `App.run` and listeners take an `AbortSignal`.
- `app.on` returns a `Subscription`; it and `StateSubscription` are
  disposable with `using`.
- Builders such as `Window.root()` and `App.window()` return their object for
  chaining; `Point`, `Size`, `Rect`, and `Color` have `equals` and `toString`;
  added `Color.rgba`, `Color.black`, `Color.white`, `version`, and
  `RunningApp.saveScreenshot`.
- Fixed `rebuildOnChange`, which rendered nothing, and custom widget `event`
  callbacks that return nothing, whose context requests were dropped.
- A virtual table's `selected` state that holds a string now receives the
  selected key as a string, matching the key type of its callbacks.
- Added `app.fontUrl(url)` to fetch and register a font, and
  `FontHandle.family` (also in Python) to read a registered font's family
  name. Invalid font data now fails at registration.
- Added a `node:test` suite that runs on both the addon and the WebAssembly
  build.

### Python

- The sdist places the license file at its root, where its metadata declares
  it. The release workflow checks each sdist's contents and metadata, and
  runs `twine check`, before uploading it.
- The examples open interactive desktop windows with `App.run()` instead of
  printing snapshots, and paint the theme's window background with a root
  `surface`, as the README's examples now do.

### Rust

- Added `DesktopPlatform::start_pumped` and `PumpedDesktop`, and
  `App::start_pumped_with_handle`, to drive the desktop event loop one turn at
  a time from another framework's event loop.
- Added `DesktopPlatform::start_web` and `with_web_canvas`, and
  `App::start_web_with_handle`, to run in the browser on a chosen canvas and
  learn when the app exits.
- Fonts can be WOFF or WOFF2 as well as TrueType or OpenType, in Rust and in
  the bindings. Added `RegisteredFont::try_from_bytes`, which checks the data,
  and `RegisteredFont::family_name`; `font_bytes` and `register_font_bytes`
  now fail for data that is not a font.
- Offscreen renderers share compiled pipelines, and the shared cache did not
  tell single-source and dual-source text pipelines apart. A renderer without
  dual-source blending could leave its text pipeline for later renderers,
  which then drew LCD text in grayscale. Text pipelines are now cached per
  variant.
- On Windows, closing the last window no longer returns from `run` before the
  window is destroyed, which left it on screen until the process exited.
- `ContextMenu` keeps its panel positions, and with them its paint,
  semantics, and hit testing, when a parent such as a toolbar measures it
  again after arranging it.
- `VirtualTable` publishes its viewport only when arranged, not when a parent
  measures provisional sizes, which made layout observers alternate between
  sizes forever.
- Touch input no longer opens tooltips, which could cover the menu or action
  a finger just activated.

### Performance

- On Android, a focused window no longer presents every display frame while
  its UI is unchanged. Desktop windows keep presenting so that variable
  refresh rate does not lower the monitor's refresh rate.
- In the browser, a focused canvas no longer wakes on every display frame
  while nothing changes, matching Android.
- Bundled fallback fonts on the web and Android, and fonts registered by the
  app, are shared with the text system instead of copied into the heap.
- Release web builds run Binaryen's `wasm-opt -Os` (Trunk for the demo,
  `cargo xtask web build --release` for the JavaScript package when
  `wasm-opt` is installed or `WASM_OPT` is set) and enable WebAssembly SIMD.
  The demo module drops from 16.6 MB to 11.7 MB.
- Per-widget layout, paint, and semantics phases only read the clock when
  `SUI_PROFILE_WIDGET_TIMINGS` is set, and the layout graph uses a faster
  hasher for widget IDs.
- JavaScript widget callbacks and class-instance checks cross the
  WebAssembly boundary fewer times per call.
- Scrolling and translating a retained layer is now compositor-only: the
  renderer reuses the layer's composed geometry and vertex buffers and moves
  them with a GPU offset, and rectangular clips are applied as scissors.
- Layout change detection no longer rebuilds a node index for every moved
  widget, which made each scroll step quadratic in the number of widgets.
- Translated layout bounds keep their exact size, and descendants that move
  with their paint boundary within f32 rounding stay composition-only, so
  retained content far from the origin is no longer repainted on scroll.
- Text packets only compare the backdrop under themselves, so text scrolling
  over a uniform background keeps its rasterized glyphs.
- Added `scroll-flat` and `scroll-retained` fixtures to `sui-bench`.
- Small-change frames do less whole-tree and whole-scene work:
  - The renderer reorders each fragment's draws so draws that share a
    pipeline and clip are adjacent, without moving any draw past one it
    overlaps, and drops clip rectangles that contain the whole draw. The
    controls grid goes from 1,201 to 647 draw calls per frame.
  - The runtime keeps the graph from before layout by swapping buffers
    instead of copying a snapshot, and diffs the two graphs directly.
  - Stack metadata is only rewritten when it changed, so unchanged frames
    keep sharing scene command lists with the renderer.
  - The frame image registry is reused while its images are unchanged, and
    unused text layouts are pruned in batches rather than every frame.
  - Appending a scene to an empty one shares its commands instead of copying
    them, and semantics assembly uses a faster hasher.
- Rectangular clips are applied in the scene shaders instead of with
  scissors: every vertex and glyph instance carries its clip in framebuffer
  pixels, snapped to the same edges a scissor uses, and moves with its
  fragment. Draws with different clips can then share a draw call, and runs
  of up to 16 consecutive packets are merged into cached groups that are
  rebuilt only when a member changes. The 1,200-widget controls grid goes
  from 647 to 22 draw calls per frame, which matters most in browsers, where
  each GPU call goes through JavaScript. Vertex and instance formats grow by
  16 bytes each.
- Partial redraw: when a window renders through its persistent intermediate
  target (always in browsers, and for HDR and wide-gamut output), a frame
  clears and redraws only the region where its packets changed, and redraws
  nothing when none did. Reordered packets, a resized target, a different
  image registry, external images, frames drawn straight to the swapchain,
  and damage over half the target fall back to a full redraw.
- Native SDR windows (Windows, macOS, Linux, Android, and iOS) redraw
  partially too: they draw into a persistent frame in the swapchain's format
  and copy it to the swapchain, when the swapchain accepts copies. Frames
  that change everything pay for the extra copy. The persistent frame holds
  one extra window-sized texture, which `trim_memory` frees while the window
  has no surface.
- Path clips use a stencil-only `Stencil8` attachment whose contents are
  discarded after each pass instead of stored, which saves tile-based mobile
  GPUs a write of the whole attachment per clipped pass.
- Suspending a window (backgrounding an Android app) releases only its
  surface and keeps the renderer's retained scene. Resuming with the same
  size and scale presents the existing frame instead of relaying out and
  rebuilding everything.
- Added `WgpuRenderer::trim_memory` and `MemoryTrimLevel`. Desktop and mobile
  hosts call it with `Background` on suspend, freeing image textures,
  staging memory, and hidden frames, and with `Critical` on the system's
  memory warning, also freeing text atlases and the retained scene, which
  the next frame rebuilds.
- The image texture cache expires textures unused for 2 seconds instead of
  120 frames, so 120 Hz displays no longer evict twice as fast, and keeps
  idle textures within a budget of 64 MB on mobile and 256 MB elsewhere.

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

- Published the Python binding on PyPI as `sinomo-ui`: abi3 wheels for
  CPython 3.10 and newer on Windows x64, macOS arm64 and x64 (macOS 11+), and
  Linux x64 and aarch64 (glibc 2.28+), plus an sdist. A new release workflow
  builds them, tests each wheel from a clean install, and publishes with
  trusted publishing.
- Added `sinomo_ui.aio` with `AsyncRunner`, `run_on_ui`, and `file_dialog` to
  run asyncio coroutines beside the UI loop.
- Made `Point`, `Size`, `Rect`, `Color`, `Constraints`, and resource and
  window handles compare and hash by value, gave common objects readable
  reprs, and made `StateSubscription` a context manager.

- Renamed the Python distribution to `sinomo-ui` and the import package to
  `sinomo_ui`; use `import sinomo_ui as sui`. The native extension now lives
  in `sinomo_ui._native` behind a regular Python package, which also exposes
  `__version__`.
- Removed the `PascalCase` widget factory aliases such as `sui.Button`; use
  the `snake_case` factories such as `sui.button`. `PascalCase` names now
  always refer to classes.
- Built wheels against the stable CPython ABI (`abi3`) for Python 3.10 and
  newer, so one wheel per platform covers every supported interpreter.
- Reported exceptions raised in callbacks with their full traceback through
  `sys.excepthook`, or a handler installed with `set_exception_handler`.
  Previously most were discarded silently.
- Made Ctrl+C stop `App.run()` with `KeyboardInterrupt`. A callback raising
  `KeyboardInterrupt` or `SystemExit` now stops the app, and `run()` or the
  driving `RunningApp` method re-raises it. Added `UiHandle.request_exit()`.
- Kept `int` values as `int` in `State`; selection widgets write their index
  back as an `int`.
- Made `EventContext.request_paint()` callable without a rectangle, as
  documented; it previously failed and the error was hidden.

### Python and JavaScript

- Added `UiHandle.call_later` and `call_every` timers, native file dialogs
  through `UiHandle.show_file_dialog`, app-level `clipboard_text` and
  `set_clipboard_text`, and `FocusController` with `focus_scope` to move
  keyboard focus from code. JavaScript gains `UiHandle.requestExit`.
- Added host-driven test helpers: `RunningApp.advance_time`,
  `settle_animations`, `frame_time`, offscreen `screenshot_png` and
  `save_screenshot`, a module-level `set_motion_preference`, and `within=`
  on semantic queries. Role queries accept any spelling of a role name, and
  `get_one` takes the same filters as `find`.
- Made `RunningApp.drain_ready_events()` dispatch due timer and animation
  events; it previously discarded them, so custom-widget timers and
  animation frames never ran in host-driven tests.

- Made state-bound `dialog` and `popover` open states follow later changes,
  and write user dismissal back. `dialog` gained `description`, `modal`,
  `dismiss_on_scrim`, `max_width`, `actions`, and `on_dismiss`; `popover`
  gained `on_open_change`.
- Gave `tabs` content panels, an `on_change` callback, and a selection that
  follows its bound state.
- Made state-bound labels of buttons, checkboxes, switches, and radio buttons
  update in place.
- Added `enabled` to every control that can be disabled, as a boolean or a
  `State`.
- Gave `column` and `row` `justify`, `align_items`, and `wrap`, and added
  `flex_item` for per-child grow, shrink, basis, size limits, and
  `align_self`, plus `spacer`.
- Gave `button` `appearance`, `tone`, `icon`, `min_width`, `semantic_name`,
  and `description`.
- Gave `label` `color`, `font_size`, `line_height`, `weight`, `single_line`,
  and `selectable`.
- Added `ListItem` with detail, trailing text, an icon, and accessibility text;
  `list_view` accepts it beside plain strings.
- Gave `grid` explicit column and row tracks (fixed, `auto`, fractions, and
  `minmax`) and added `grid_cell` for explicit placement and spans.
- Added `ScrollController` to scroll a `scroll_view` or `virtual_scroll_view`
  from code and read its offset and extents.
- Added `rebuild_on_change(states, build)` to rebuild a subtree whenever a
  watched state changes, and `semantic_name` on `label`, `checkbox`, `switch`,
  and `radio_button`.
- Gave `text_input`, `text_area`, and `password_input` `read_only`,
  `on_submit`, and `on_focus_change`. Semantic snapshots no longer report
  read-only fields as editable.
- Bound `VirtualTable` directly as `virtual_table` instead of mapping it to
  `table`. It realizes only visible rows of a thread-safe `TableModel` of keyed
  `VirtualTableRow` values that any thread may update, and selects by row key,
  writing the key back to a bound `State` as an integer.
  `VirtualTableColumn` sets widths, alignment, resizing, and the sort
  indicator; `on_row_activate`, `on_header_activate`, `on_column_resize`, and
  `on_near_end` report keys. Sorting stays application policy: reorder the
  model and show the direction with `TableModel.set_sort`.

### Rust

- Added `UiHandle::request_exit` and `CommandSender::request_exit` to end the
  platform event loop from application code on any thread.
- Added `Tabs::selected_when`, and `label_when` to `Button`, `Checkbox`,
  `Switch`, and `RadioButton`.
- Added `FocusScopeState::request_focus`, which moves focus into a scope even
  when the person put it elsewhere, and re-exported `OsClipboardBackend`.

## [0.4.1] - 2026-10-04

This release adds observable background tasks, makes reactive updates and text
rendering cheaper, improves platform font rendering, and adds touch scroll
inertia.

### Highlights

- Added `Task`, `TaskState`, and `TaskHandle` for background work with progress,
  results, errors, and stale-worker protection. `refresh` keeps the previous
  result visible while loading, cancellation restores it, and `TaskHandle::run`
  completes the task from a future on the application's executor.
- Added `batch` to coalesce notifications, `combine` and `combine_named` to
  derive values from two to four observables, and `changed().await` to wait for
  a signal, task, or other observable to change.
- Added borrowed signal reads and in-place updates with `with`, `modify`, and
  `mark_changed`, plus pointer-compared and copy-on-write `Arc` updates with
  `set_arc` and `modify_arc`. Selector clones share one source subscription
  and a cached result, avoiding repeated work for multiple observers.
- Skipped clipped glyphs before rasterization and split grayscale masks from
  color and LCD glyphs. A grayscale atlas page now uses 4 MiB of GPU memory
  instead of 16 MiB each on the CPU and GPU; only pending uploads keep CPU
  pixels. Clipping also preserves tall marks and bitmap emoji at line edges.
- Followed Windows ClearType and Linux fontconfig/Xft smoothing preferences,
  improved Linux and Android slight hinting and coverage, and loaded Android
  system fonts with bundled fallbacks. Runtime text-policy changes now update
  retained text correctly.
- Added coasting touch scrolling to `ScrollView` and `VirtualScrollView`.
  Both expose `on_offset_change` and `on_offset_change_with_ctx` for wheel,
  keyboard, touch, fling, scroll bar, and programmatic moves. The widget book's
  navigation follows flings and keeps the selected entry after a jump.

### Other changes

- Fixed open contours rendering as fans on Adreno 740 under Chrome's Vulkan
  WebGPU backend and made tooltip borders continuous around their tails.
- Kept menu and context-menu keyboard shortcuts on one line so long
  combinations stay aligned and vertically centered.
- Added download progress while the web demo loads its Wasm module and fonts,
  with startup errors displayed on the loading screen.
- Made Chrome text comparisons match the requested grayscale or LCD mode,
  added aggregate ink and alignment diagnostics, and added a light/dark,
  scale, and antialiasing comparison matrix.
- Fixed concurrent selector updates losing changes and reactive diagnostics
  recording older versions when notifications arrive out of order.
- Updated accessibility, Python, Node, Wasm, and Linux dependencies.

### Compatibility and release notes

- Update the SUI crates together to `0.4.1`. `sinomo-ui-lucide` remains at
  `1.47.1`, with the same bundled Lucide 1.47.0 icons.
- Selector closures now read signal values while the source is locked; they
  must not write to that signal. Debug builds report same-signal reentrant
  writes with a named panic instead of deadlocking. Observers still run
  synchronously on the writing thread; batching defers them until the batch
  ends on that thread.
- Rust 1.90 remains the minimum supported version. Browser support remains
  alpha, Android remains experimental, and Python and JavaScript bindings
  remain source-built and are not part of the registry release.

## [0.4.0] - 2026-10-01

This release refreshes the default theme, gives the built-in widgets one API
vocabulary, moves animation into the runtime with reduced-motion support, and
follows HDR and wide-gamut outputs.

### Highlights

- Refreshed the default themes. Every palette role is derived in OKLCH from a
  few source colors: neutrals, primary, secondary, status colors, and a
  nine-hue decorative palette. Built-in controls are restyled, primary buttons
  glow in themes with glows, and shadows are exact Gaussian blurs that can sit
  behind, outside, or inside their box.
- Gave the built-in widgets one API vocabulary, documented in
  `docs/api/conventions.md`. Every callback has a `_with_ctx` twin; state is
  `checked`, `selected`, `enabled`, `read_only`, or `open`, set with `p`,
  `p_when`, or `p_from`; layout uses `gap` and `corner_radius`; color
  overrides are `colors(...)`; a single child is `child(...)` and data entries
  are `item(...)`; and on/off flags take a `bool`. Inputs, collections, and
  items can be disabled.
- Moved animation into the runtime. `ctx.animate` drives `Motion` and
  `Progress` values, transitions keep their momentum when retargeted,
  `AnimationSpec` chooses a tween or a spring, frames follow the display's
  refresh, and retained layers move, fade, or scale without repainting. An
  app-wide `MotionPreference` follows the system's reduced-motion setting.
- Added choreography: `Presence` animates a child in and out, `KeyedStack`
  keeps a keyed list in step with an observable and staggers its changes,
  transitions can start after a delay, and timelines gain ping-pong loops,
  delays, and markers.
- Tone mapping keeps hue and leaves SDR content alone, Display P3 colors reach
  wide-gamut outputs, HDR themes fall back to what the output can show, and
  apps can capture their own windows.
- `TextSurface` edits wrapped 20,000-line documents in milliseconds and lays
  out right-to-left text; `paint_text` places wrapped text as a block; and
  `Paragraph` lays text out once to measure and paint.
- Drag and drop follows modifier keys, scrolls near edges, draws custom
  previews, and cancels on Escape; `ReorderableList` reorders from the
  keyboard.
- Node graphs gain labeled, colored ports with connection rules, undo and redo,
  copy and paste, and context menus.
- `TestApp` runs headless and in parallel by default and can simulate SDR,
  wide-gamut, and HDR displays, and headless renderers share one GPU device, so
  a full workspace test run takes about a quarter of the time.
- The widget book shows all 60 component stories on one page, and most demos
  are redesigned.

### Other changes

- `interaction_preview` pins hover, press, or focus visuals, and `show_inline`
  lays tooltips, popovers, context menus, selects, and dialogs out open in
  place, for documentation and screenshots.
- `Button::focus_on_press(false)` leaves focus where it is, so a toolbar button
  can act on the editor being typed in, and
  `window_command_dispatches_signal` reports a window's recent commands.
- `Surface::shadow`, `Surface::glow`, and `ShadowBox` cast theme shadows and
  glows; `Color::oklch`, `Color::mix_oklab`, `Color::contrast_ratio`, and an
  OKLCH color picker mode support the new color model.
- `FloatingWorkspace::transparent` floats views over other content,
  `FloatingViewConfig::closable` gives a view a close button, and
  `Widget::hit_test_self` lets a container pass points through.
- Fixed linear gradients, which now paint every stop and follow translated
  layers; focus rings around context menus and popovers; adaptive layouts
  taking focus on their first layout; and `Select`'s accessibility actions.

### Upgrading from 0.3

- Update the SUI crates together from `0.3` to `0.4`, and `sinomo-ui-lucide` to
  `1.47.1`, which bundles the same Lucide 1.47.0 icons for the `0.4` family.
- Renamed APIs keep their old names as deprecated aliases, so they compile with
  a warning that names the replacement. The aliases will be removed in 0.5.
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

### Compatibility and release notes

- Rust 1.90 remains the minimum supported version.
- Linux webview builds require the system WebKitGTK 4.1 development libraries.
- Browser support remains alpha and Android remains experimental. The Python
  and JavaScript bindings remain source-built and are not part of this registry
  release; their widget parameters now use the same names as Rust.

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
[0.4.0]: https://github.com/sinomo-lab/sui/compare/v0.3.0...v0.4.0
[0.4.1]: https://github.com/sinomo-lab/sui/compare/v0.4.0...v0.4.1
[0.5.0]: https://github.com/sinomo-lab/sui/compare/v0.4.1...v0.5.0
[0.6.0]: https://github.com/sinomo-lab/sui/compare/v0.5.0...v0.6.0
