# SUI Python bindings

`sui-ui` is the native Python binding for SUI. The distribution is named
`sui-ui`, while Python programs import the extension as `sui`.
The wheel includes generated `sui.pyi` and `py.typed` metadata sourced from the
same binding specification as the native wrappers.

The binding supports retained widget trees, desktop event-loop execution,
host-driven rendering, thread-safe state updates, custom Python widgets,
accessibility semantics, renderer-neutral paint commands, image and font
resources, live theme handles, renderer/HDR policy, and external-surface
descriptors. It is an alpha, source-built
package; prebuilt wheels are not published yet.

## Prerequisites

- Python 3.10 or newer;
- Rust 1.90 or newer and Cargo;
- Maturin 1.x (`maturin>=1.7,<2`);
- for `App.run()`, a desktop supported by SUI's `winit` and `wgpu` backends.

Use a virtual environment. If you are working in the SUI checkout, placing the
environment outside the repository avoids adding local environment files to the
worktree:

```bash
python3 -m venv /tmp/sui-python-venv
source /tmp/sui-python-venv/bin/activate
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2"
```

## Build for development

From `crates/sui-python`:

```bash
maturin develop
python -c 'import sui; print(sui.App)'
```

From the workspace root, keep Maturin in the package directory so it reads the
adjacent `pyproject.toml`:

```bash
(cd crates/sui-python && maturin develop)
python -c 'import sui; print(sui.App)'
```

`maturin develop --release` produces an optimized development build. To create
an installable wheel instead, run one of:

```bash
# From crates/sui-python
maturin build --release

# From the workspace root
(cd crates/sui-python && maturin build --release)
```

Maturin prints the resulting wheel path when the build completes.

## Run the examples

After `maturin develop`, run these commands from `crates/sui-python`:

```bash
python examples/counter.py
python examples/custom_widget.py
python examples/external_surface.py
```

Or run them from the workspace root:

```bash
python crates/sui-python/examples/counter.py
python crates/sui-python/examples/custom_widget.py
python crates/sui-python/examples/external_surface.py
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
import sui

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

Widget factories use Python `snake_case`; keyword arguments are preferred for
optional configuration. The original `PascalCase` names remain as compatibility
aliases while applications migrate.

The portable media surface includes both `color_picker(...)` and the compact,
mode-selectable `simple_color_picker(...)` (`hsl`, `hsv`, or `rgb`).
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

Use `theme.color(...)`/`set_color(...)` for source semantic colors and
`theme.number(...)`/`set_number(...)` for spacing, radii, breakpoints, and
motion durations. Derived control palettes and metrics are synchronized by the
binding.

`App.run()` blocks until the desktop application exits. Use
`App.run_with_handle(callback)` when startup code needs the thread-safe
`UiHandle` after the event loop is ready.

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
`AnimationEditor` APIs for tools.

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

`State` values used by an application are attached to its UI task queue when
the app starts or runs. Updates from outside the UI drain path are queued and
mark the affected windows for redraw. `UiHandle.post(callback)` is the normal
way to schedule arbitrary work back onto the UI thread.

Widget and custom-paint callbacks run synchronously on the UI thread. Keep them
short; perform blocking I/O or long computation elsewhere, then publish the
result through `State` or `UiHandle`.

`state.select(callable)` creates a retained derived state, and
`state.watch(callable)` returns an explicit `StateSubscription`. Selectors
suppress unchanged values; subscriptions can be released with `unsubscribe()`.

For application services and worker results, register a named handler with
`app.on(name, callback)` and publish through `ui_handle.emit(name, payload)`.
The binding maps this dynamic-language message API onto the UI task queue,
preserving UI-thread delivery without exposing Rust generic command keys.

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

After a Maturin build, run all three Python examples as the package-level smoke
test.

## More documentation

- [Examples catalog](../../docs/examples.md)
- [Rust API guide](../../docs/api/README.md)
- [Testing guide](../../docs/testing.md)
- [Cross-language binding roadmap](../../docs/plans/cross-language-bindings-plan.md)
- [Documentation index](../../docs/README.md)
