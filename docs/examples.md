# Examples

The repository includes small programs for learning the public Rust facade,
exploring the full widget set, testing without a desktop window, and trying the
native Python and JavaScript bindings. Commands in this catalog run from the
workspace root unless a section says otherwise.

Rust examples require Rust 1.90 or newer. The desktop examples also need a
working window system and graphics backend supported by `winit` and `wgpu`.

## Rust facade examples

The Cargo package is named `sinomo-ui`, while programs import its library as
`sui`.

### Hello

[`hello.rs`](../crates/sui/examples/hello.rs) is the smallest complete desktop
application: one window containing one label.

```bash
cargo run -p sinomo-ui --example hello
```

### Quickstart

[`quickstart.rs`](../crates/sui/examples/quickstart.rs) introduces themes,
vertical layout, surfaces, and a button callback. It is the finished program
from the [quickstart tutorial](tutorials/quickstart.md).

```bash
cargo run -p sinomo-ui --example quickstart
```

### Stateful form

[`stateful_form.rs`](../crates/sui/examples/stateful_form.rs) demonstrates
retained application state, dynamic labels, conditional button enablement,
invalidation, and editable text, password, and date/time fields. The
[stateful form tutorial](tutorials/stateful-form.md) builds it step by step.

```bash
cargo run -p sinomo-ui --example stateful_form
```

### Rich document

[`rich_document.rs`](../crates/sui/examples/rich_document.rs) renders selectable
Markdown, syntax-highlighted code, links, and an expandable structured
operation block through `RichDocumentModel` and `RichDocumentView`.

```bash
cargo run -p sinomo-ui --example rich_document
```

### Typed commands

[`commands.rs`](../crates/sui/examples/commands.rs) demonstrates typed
application commands from both a widget callback and a worker thread,
application-wide multicast, lifecycle-owned window/application subscriptions,
and signal-driven presentation updates.

```bash
cargo run -p sinomo-ui --example commands
```

Check every Rust facade example without opening a window:

```bash
cargo check -p sinomo-ui --examples
```

## Demo and widget book

The `sinomo-ui-demo` application is the workspace's interactive development
host. It contains the widget book, incremental rich-document editor, adaptive
layout and overlay stories, virtual collections, a typed command-routing lab,
theme and text-rendering surfaces, renderer controls, and focused performance
views. Open the `Commands` card to exercise directed window/application
delivery, multicast, worker delivery, and scheduler-only controller wakes.

Open the native demo:

```bash
cargo run -p sinomo-ui-demo
```

Generate review artifacts without navigating the desktop UI:

```bash
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

Artifacts are written under `target/ui-artifacts/sui-demo/widget-book`. The
generator includes slower HDR and AVIF outputs, so it can take substantially
longer than an ordinary build.

### Terminal UI

The same retained application can be projected from its accessibility tree
into an interactive terminal UI:

```bash
cargo run -p sinomo-ui-demo -- --tui
```

For a non-interactive accessibility snapshot, run:

```bash
cargo run -p sinomo-ui-demo -- --tui-dump-accessibility
```

See the [TUI guide](tui.md) for structured versus spatial layouts, keyboard
controls, mouse behavior, and validation.

### Browser demo

Install [Trunk](https://trunkrs.dev/) and serve the browser build:

```bash
trunk serve --config crates/sui-demo/web/Trunk.toml
```

Then open `http://127.0.0.1:8080/`. A production build uses:

```bash
trunk build --config crates/sui-demo/web/Trunk.toml --release
```

The output is written to `crates/sui-demo/web/dist`.

## Deterministic testing example

[`sui-testing/examples/basic.rs`](../crates/sui-testing/examples/basic.rs)
builds an application in process, locates widgets through accessibility
semantics, fills a text field, clicks a button, advances the scheduled work,
and checks the resulting status. It never shows a window; `TestApp::new` may
use a hidden live backend when a display is available and otherwise uses the
headless backend.

```bash
cargo run -p sinomo-ui-testing --example basic
```

For test APIs and artifact capture, continue with the
[testing guide](testing.md).

## Python examples

The Python package is a native PyO3 extension built with Maturin. It requires
Python 3.10 or newer, Rust 1.90 or newer, and Maturin 1.x. The package is not
published yet, so build it from this checkout.

The following workspace-root commands create an isolated environment outside
the checkout, build the extension, and run every example:

```bash
python3 -m venv /tmp/sui-python-venv
source /tmp/sui-python-venv/bin/activate
python -m pip install "maturin>=1.7,<2"
(cd crates/sui-python && maturin develop)
python crates/sui-python/examples/counter.py
python crates/sui-python/examples/custom_widget.py
python crates/sui-python/examples/external_surface.py
```

The examples use `App.start()` and render in process, so they print snapshot
and event information instead of opening windows:

- [`counter.py`](../crates/sui-python/examples/counter.py) uses binding state,
  built-in controls, and a posted UI task;
- [`custom_widget.py`](../crates/sui-python/examples/custom_widget.py) defines
  measurement, painting, semantics, and event callbacks in Python;
- [`external_surface.py`](../crates/sui-python/examples/external_surface.py)
  renders the portable CPU-RGBA external-surface path.

See the [Python binding guide](../crates/sui-python/README.md) for wheel builds,
normal desktop execution, and current limitations.

## JavaScript examples

The JavaScript package is a native napi-rs addon for Node.js and Electron. It
requires Rust 1.90 or newer, Node.js with npm, and the napi-rs CLI. The package
and native binaries are not published yet, so build the addon locally first.

Install the compatible CLI once, then build and run every example from the
workspace root:

```bash
npm install --global @napi-rs/cli@3
npm --prefix crates/sui-js run build
node crates/sui-js/examples/counter.js
node crates/sui-js/examples/custom-widget.js
node crates/sui-js/examples/external-surface.js
```

The build places a native `.node` artifact next to
`crates/sui-js/index.js`. It is a generated local artifact and should not be
committed. Like the Python samples, these programs use `App.start()` and print
host-driven render information rather than opening windows:

- [`counter.js`](../crates/sui-js/examples/counter.js) uses binding state,
  built-in controls, and a posted UI task;
- [`custom-widget.js`](../crates/sui-js/examples/custom-widget.js) implements a
  custom painted and accessible widget;
- [`external-surface.js`](../crates/sui-js/examples/external-surface.js)
  exercises the CPU-RGBA external-surface fallback.

See the [JavaScript binding guide](../crates/sui-js/README.md) for package-local
commands, TypeScript declarations, desktop execution, and current limitations.
