# SUI Python bindings

`sui-ui` is the native Python binding for SUI. The distribution is named
`sui-ui`, while Python programs import the extension as `sui`.

The binding supports retained widget trees, desktop event-loop execution,
host-driven rendering, thread-safe state updates, custom Python widgets,
accessibility semantics, renderer-neutral paint commands, image and font
resources, and external-surface descriptors. It is an alpha, source-built
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
        sui.Column(
            [
                sui.Label("Ready"),
                sui.Button("Close terminal with Ctrl+C", on_press=lambda: None),
            ],
            gap=8,
        )
    )
)
app.run()
```

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

## State and threading

`State` values used by an application are attached to its UI task queue when
the app starts or runs. Updates from outside the UI drain path are queued and
mark the affected windows for redraw. `UiHandle.post(callback)` is the normal
way to schedule arbitrary work back onto the UI thread.

Widget and custom-paint callbacks run synchronously on the UI thread. Keep them
short; perform blocking I/O or long computation elsewhere, then publish the
result through `State` or `UiHandle`.

## Custom widgets and resources

A Python object wrapped with `sui.Widget(object)` may implement:

- `measure(constraints)` to return a `Size`;
- `event(event)` to process binding-safe event descriptors;
- `semantics(semantics)` to expose roles, names, values, ranges, and actions;
- `paint(paint)` to emit validated scene commands.

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
- The Python surface intentionally omits Rust-native widgets whose contracts
  require local `Widget` closures, `Any` payloads, shared `Rc` state, or custom
  paint models. The checked binding manifest records those exclusions instead
  of silently treating them as missing coverage.
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
