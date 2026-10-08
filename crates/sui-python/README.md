# SUI Python bindings

`sinomo-ui` is the native Python binding for SUI. Install the `sinomo-ui`
distribution and import the `sinomo_ui` package; the examples use the short
alias `sui`:

```python
import sinomo_ui as sui
```

The package wraps a native `sinomo_ui._native` extension and ships typed stubs
(`_native.pyi` and `py.typed`) generated from the same binding specification
as the native wrappers. Wheels target the stable CPython ABI (`abi3`), so one
wheel per platform covers Python 3.10 and newer.

The binding supports retained widget trees, desktop event-loop execution,
host-driven rendering, thread-safe state updates, custom Python widgets,
accessibility semantics, renderer-neutral paint commands, image and font
resources, live theme handles, renderer/HDR policy, and external-surface
descriptors. It is an alpha, source-built
package; prebuilt wheels are not published yet.

## Prerequisites

- Rust 1.90 or newer and Cargo;
- [uv](https://docs.astral.sh/uv/), which provides Python 3.10 and the
  development tools;
- for `App.run()`, a desktop supported by SUI's `winit` and `wgpu` backends.

## Build for development

From `crates/sui-python`, create the development environment:

```bash
uv sync
```

This creates `.venv` with the pinned Python version (`.python-version`, the
oldest supported release), installs maturin, pytest, and mypy from the locked
`dev` group, and builds `sinomo_ui` in editable mode. After changing Rust
code, rebuild the extension in place:

```bash
uv run maturin develop --uv
```

Add `--release` for an optimized development build. To create an installable
wheel instead:

```bash
uv run maturin build --release
```

Maturin prints the resulting wheel path when the build completes. Wheels use
the stable ABI, so the `cp310-abi3` wheel installs on every supported Python
version.

## Run the examples

After `uv sync`, run these commands from `crates/sui-python`:

```bash
uv run python examples/counter.py
uv run python examples/custom_widget.py
uv run python examples/external_surface.py
```

Or run them from the workspace root:

```bash
uv run --project crates/sui-python python crates/sui-python/examples/counter.py
uv run --project crates/sui-python python crates/sui-python/examples/custom_widget.py
uv run --project crates/sui-python python crates/sui-python/examples/external_surface.py
```

The examples deliberately use `App.start()`. They render in process and print
snapshot or event information; they do not open desktop windows.

- [`counter.py`](examples/counter.py) covers `State`, built-in controls, a
  posted UI task, and rerendering.
- [`custom_widget.py`](examples/custom_widget.py) supplies Python measurement,
  event, semantics, and paint callbacks.
- [`external_surface.py`](examples/external_surface.py) renders a CPU-RGBA
  texture through `ExternalSurface`.

## Open a desktop window

Use `App.run()` when Python owns the normal desktop event loop:

```python
import sinomo_ui as sui

app = sui.App()
app.window(
    sui.Window("Hello from Python").root(
        sui.column(
            [
                sui.label("Ready"),
                sui.button("Close terminal with Ctrl+C", on_press=lambda: None),
            ],
            gap=8,
        )
    )
)
app.run()
```

Widget factories use Python `snake_case` (`sui.label(...)`, `sui.button(...)`);
keyword arguments are preferred for optional configuration. `PascalCase` names
are classes, such as `sui.App`, `sui.State`, and descriptors like
`sui.TableColumn`.

Interactive controls accept `enabled=`, a `bool` or a `State`; a disabled
control ignores input and reports itself as disabled to assistive technology.
`column(...)` and `row(...)` take `justify` (`"start"`, `"center"`, `"end"`,
`"space-between"`, `"space-around"`, `"space-evenly"`), `align_items`
(`"start"`, `"center"`, `"end"`, `"stretch"`), and `wrap`. Wrap a direct child
in `flex_item(child, grow=1)` to size it along the main axis with `grow`,
`shrink`, `basis`, minimum and maximum sizes, and `align_self`; `spacer()`
fills the remaining space:

```python
toolbar = sui.row(
    [sui.label("Untitled"), sui.spacer(), sui.button("Save", enabled=dirty)],
    gap=8,
    align_items="center",
)
```

Buttons take `appearance` (`"filled"`, `"tonal"`, `"outline"`, `"ghost"`),
`tone` (`"accent"`, `"danger"`, ...), `icon`, and `min_width`; a primary action
is `appearance="filled", tone="accent"`. Text fields take `read_only`,
`on_submit(text)`, and `on_focus_change(focused)`.

The portable media surface includes both `color_picker(...)` and the compact,
mode-selectable `simple_color_picker(...)` (`hsl`, `hsv`, `rgb`, or `oklch`).
Editor shells can use `DockState`, serializable `DockLayout`/`DockNode` values,
stable `DockPanelSpec` descriptors, and `dock_workspace(...)` without exposing
Rust-local widget ownership.
Responsive composition includes idiomatic `grid(...)`, `aspect_ratio(...)`,
`safe_area(...)`, and `layout_transition(...)` factories without exposing Rust
track or animation implementation types.
`adaptive_view(...)`, `constraint_view(...)`, `responsive_sidebar(...)`, and
`master_detail(...)` retain each branch or pane while exposing ordinary Python
state objects and callbacks.
`bottom_sheet(...)` uses height-oriented options and the same retained shown
state and dismissal callbacks as `side_sheet(...)`.
Thread-safe `NotificationCenter` producers feed `notification_host(...)`, and
`overlay_host(...)` creates an independent stacking root for embedded regions.
`command_palette(...)` exposes a retained shown state and dismissal callback;
querying, ranking, and command execution remain ordinary Python policy.
`VirtualListModel` provides stable keyed, thread-safe incremental text rows;
`virtual_list(...)` realizes only visible rows and exposes Python selection and
near-edge callbacks.
`CanvasViewport`, `CanvasStroke`, and `CanvasShape` are value descriptors used
by retained `canvas(...)` and `canvas_ruler(...)` factories.
Portable `DragScope`, `drag_drop_host(...)`, `draggable(...)`, and
`drop_target(...)` use text payloads and Python callbacks, including external
file hover/drop delivery.
`FloatingWorkspaceState`, `FloatingView`, and `floating_workspace(...)` expose
stable same-window editor panes with move, resize, visibility, z-order, and
maximize controls.
`PixelCanvasState`, `PixelCanvasExport`, and `pixel_canvas(...)` expose editable
pixel documents, brush/tool controls, undo/redo requests, viewport commands,
and byte-oriented RGBA exports.

Streaming Markdown uses a thread-safe `RichDocument` model and a retained
`rich_document_view(...)`. `append_markdown(...)` preserves the native
incremental tail-reparse behavior; attachment and extension blocks use Python
keyword arguments and metadata mappings.

Use a live `Theme` handle for application-wide styling. Changes are posted to
the UI queue when needed and do not require rebuilding the foreign widget tree:

```python
theme = sui.Theme.dark()
app = sui.App(theme=theme)
app.window(sui.Window("Themed").root(sui.button("Save")))
running = app.start()

theme.set_accent(sui.Color.rgba(0.2, 0.55, 1.0, 1.0))
running.drain()
```

Use `theme.color(...)`/`set_color(...)` for source colors and
`theme.number(...)`/`set_number(...)` for spacing, radii, breakpoints, and
motion durations. Color tokens are the neutral ramp (`window`, `subtle`,
`panel`, `overlay`, `control`, `button`, `field`, `border`, `border-strong`,
`border-control`, `text`, `text-secondary`, `text-tertiary`, and their
hover/active variants), brand and status colors (`primary`, `secondary`,
`info`, `success`, `warning`, `danger`, each with an `on-*` content color),
and the decorative hues (`red`, `orange`, `amber`, `green`, `teal`, `cyan`,
`blue`, `violet`, `magenta`). Derived control palettes and metrics are
synchronized by the binding. Changing the primary/accent color re-derives
primary actions, checked controls, links, focus rings, thin indicators,
selection borders, and glows; surfaces, selection fills, fields, menus, and
scroll chrome remain neutral.

`App.run()` blocks until the desktop application exits, with the GIL
released so other Python threads keep running. Use
`App.run_with_handle(callback)` when startup code needs the thread-safe
`UiHandle` after the event loop is ready; `ui_handle.request_exit()` ends the
loop from any thread.

Ctrl+C stops a running app: `run()` raises `KeyboardInterrupt` within a
fraction of a second. A callback that raises `KeyboardInterrupt` or
`SystemExit` (for example `sys.exit()` in a button handler) also stops the
loop, and `run()` re-raises that exception. Call `run()` from the main thread,
once per process: the platform event loop cannot be created twice.

Use `App.start()` for embedding, deterministic tests, or host-driven rendering:

```python
running = app.start()
snapshot = running.render()
print(snapshot.command_count, snapshot.semantics_count)
```

The returned `RunningApp` can render windows, dispatch binding event
descriptors, drain posted work, request redraws, and expose window handles. It
does not create or present a native desktop surface by itself.

`running.set_inspector_tracing()` and `running.inspect()` expose a
renderer-neutral diagnostic snapshot: the accessibility nodes, focus and
pending phases, frame/widget timings, plus structured reactive, command,
invalidation, rebuild, and privacy-safe event-route histories.

## Animation and semantic testing

Animations use ordinary Python objects. `AnimationValue` supports scalar,
point/vector, size, rectangle, color, and transform values; `Transition`,
`Spring`, and `AnimatedValue` cover local motion. Reusable motion uses
`Keyframe`, `AnimationTrack`, `AnimationClip`, and `AnimationTimeline`, with
`AnimationPlayer`, serializable `AnimationDocument`, and undoable
`AnimationEditor` APIs for tools. Timelines carry named markers that players
report as the playhead passes them, players loop once, repeat, or ping-pong
with optional start and loop delays, and `Stagger` computes delays for
`AnimatedValue.set_target_after` so a group can cascade. `Presence` shows and
hides a widget with an enter and exit animation.

```python
zero = sui.AnimationValue.scalar(0)
one = sui.AnimationValue.scalar(1)
track = sui.AnimationTrack("card", "layer.opacity")
track.add_keyframe(sui.Keyframe(0, zero))
track.add_keyframe(sui.Keyframe(1, one, easing="ease-out"))
clip = sui.AnimationClip("fade", 0, 1)
clip.add_track(track)
timeline = sui.AnimationTimeline(1)
timeline.add_clip(clip)
```

`RenderSnapshot.find(...)` and `get_one(...)` query complete semantic nodes by
role, name, text, description, focus, and visibility. Nodes retain hierarchy,
bounds, actions, values, and interaction state; pass one to `running.hover`,
`click`, `press`, or `fill` for locator-style deterministic tests.

## State and threading

`State` holds a `str`, `int`, `float`, or `bool`, and `get()` returns the
same Python type that was stored. Selection widgets write their index back as
an `int`. Widgets bound to a `State` (labels, checked values, selections, and
the open state of `dialog`, `popover`, and the sheets) follow later changes to
it, and write user changes back.

`State` values used by an application are attached to its UI task queue when
the app starts or runs. Updates from outside the UI drain path are queued and
mark the affected windows for redraw. `UiHandle.post(callback)` is the normal
way to schedule arbitrary work back onto the UI thread.

Widget and custom-paint callbacks run synchronously on the UI thread. Keep them
short; perform blocking I/O or long computation elsewhere, then publish the
result through `State` or `UiHandle`.

A `ScrollController` scrolls a `scroll_view` or `virtual_scroll_view` from
code. `scroll_to(x=..., y=...)` and `scroll_to_item(index)` are thread-safe
requests applied at the view's next layout; `offset`, `max_offset`,
`viewport_size`, and `content_size` report the latest layout:

```python
scroll = sui.ScrollController()
log = sui.scroll_view(sui.column(lines), controller=scroll)
scroll.scroll_to(y=scroll.max_offset.y)
```

Use `rebuild_on_change(states, build)` when the structure of a subtree depends
on state, such as a list whose length changes. `build()` runs on the UI thread
when the widget is created and again whenever one of `states` changes; its
widgets replace the previous subtree:

```python
items = sui.State(3)
listing = sui.rebuild_on_change(
    [items],
    lambda: sui.column([sui.label(f"Item {i}") for i in range(items.get())]),
)
```

Widgets that only change their content, such as a label's text, do not need a
rebuild; pass the `State` to the widget instead.

`state.select(callable)` creates a retained derived state, and
`state.watch(callable)` returns an explicit `StateSubscription`. Selectors
suppress unchanged values; subscriptions can be released with `unsubscribe()`.

For application services and worker results, register a named handler with
`app.on(name, callback)` and publish through `ui_handle.emit(name, payload)`.
The binding maps this dynamic-language message API onto the UI task queue,
preserving UI-thread delivery without exposing Rust generic command keys.

## Errors in callbacks

Callbacks have no Python caller to receive their exceptions, so SUI reports
each one with its full traceback through `sys.excepthook` and keeps the app
running. Install your own handler to log, collect, or re-raise them:

```python
errors = []
previous = sui.set_exception_handler(errors.append)
```

The handler receives the exception object; its traceback is in
`__traceback__`. Pass `None` to restore the default. If the handler itself
raises, both exceptions are reported through `sys.excepthook`.

`KeyboardInterrupt` and `SystemExit` are never reported this way: they stop
the app and are re-raised by the call that drove it, `App.run()` for desktop
apps or the `RunningApp` method (`click`, `drain`, `render`, ...) in
host-driven tests.

## Custom widgets and resources

A Python object wrapped with `sui.Widget(object)` may implement:

- `measure(constraints)` to return a `Size`;
- `event(event)` to process binding-safe event descriptors;
- `semantics(semantics)` to expose roles, names, values, ranges, and actions;
- `paint(paint)` to emit validated scene commands.

For interaction services, implement `event_with_context(event, context)`.
`EventContext` exposes focus, handled state, targeted paint/layout/semantics
requests, animation-frame requests, pointer capture, clipboard access, stable
IDs, bounds, routing phase, and frame time. The one-argument `event(event)`
callback remains supported for simple and existing widgets.

`sui.Widget(callbacks, children=[...])` creates a retained foreign composite.
Use `measure_with_children(constraints, child_sizes)` and
`arrange(bounds, child_sizes)` for custom layout; return one `Rect` per child.
Children are painted after the custom paint commands and are automatically
included in semantics unless the callback explicitly includes them.

The paint surface supports styled text, paths and path clips, rounded
rectangles, shadows, transforms, image quads, and validated built-in shaders.
Applications can register fonts and RGBA, PNG, or SVG images from bytes or
files. See [`examples/custom_widget.py`](examples/custom_widget.py) for a
complete custom control.

`ExternalSurface` accepts CPU-upload, shared-texture, and shared-render-target
descriptors. The CPU-RGBA path renders today. Shared descriptors are validated
and retained for host integration, but zero-copy renderer composition is not
implemented yet.

## Current limitations

- Wheels and release automation are not published; users build from source.
- `App.run()` can run once per process, so notebooks cannot re-run a desktop
  app in the same kernel. Use `App.start()` for repeatable host-driven runs.
- Every public Rust widget is classified as directly bound, manually wrapped,
  or represented by a documented Python-level equivalent. Some equivalents
  intentionally expose portable value models instead of Rust closure or `Any`
  implementation details.
- Desktop `run` entry points exist, but the repository does not yet have broad
  real-window smoke coverage for every supported platform.
- Custom WGSL, arbitrary shader resources, and uniforms are not exposed;
  custom paint can use only validated built-in shaders.
- Shared textures and shared render targets are descriptor-level APIs today;
  only the portable CPU-upload external surface is rendered end to end.
- The API is pre-release and may change before the first stable release.

## Validate binding changes

Rust-side binding tests do not require an installed extension module:

```bash
cargo test -p sinomo-ui-python
```

The Python test suite runs against the extension built by `uv sync` or
`uv run maturin develop --uv`, from `crates/sui-python`. It includes the
examples:

```bash
uv run pytest
```

Tests that open real desktop windows are skipped unless enabled:

```bash
SUI_DESKTOP_TESTS=1 uv run pytest
```

## More documentation

- [Examples catalog](../../docs/examples.md)
- [Rust API guide](../../docs/api/README.md)
- [Testing guide](../../docs/testing.md)
- [Cross-language binding roadmap](../../docs/plans/cross-language-bindings-plan.md)
- [Documentation index](../../docs/README.md)
