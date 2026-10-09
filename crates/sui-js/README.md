# SUI JavaScript bindings

`@sui/ui` brings SUI to JavaScript in two forms with one API:

- **Node.js and Electron**: a native addon (napi-rs) that opens desktop
  windows;
- **browsers**: a WebAssembly build that draws on a `<canvas>`.

Both run the app on the host's event loop, so `await`, timers, `fetch`, and
other I/O keep working while windows are open, and callbacks may be `async`.
The package is ESM with TypeScript declarations in `index.d.ts`. Node loads the
addon, and bundlers and browsers load the WebAssembly build, through the
`node` and `browser` export conditions.

The binding supports retained widget trees, host-driven rendering for tests,
JavaScript-owned custom widgets, accessibility semantics, renderer-neutral
paint commands, image and font resources, live theme handles, renderer/HDR
policy, and external-surface descriptors. It is an alpha, source-built
package; the npm package and prebuilt binaries are not published yet.

## Prerequisites

- Rust 1.90 or newer and Cargo;
- Node.js 20.19 or newer;
- for the addon: napi-rs CLI 3.x (`npm install --global @napi-rs/cli@3`), and
  for opening windows a desktop supported by SUI's `winit` and `wgpu` backends;
- for the browser build: the `wasm32-unknown-unknown` Rust target
  (`rustup target add wasm32-unknown-unknown`) and the `wasm-bindgen` CLI
  matching the workspace's `wasm-bindgen` crate
  (`cargo install wasm-bindgen-cli --version 0.2.129`).

## Build

From `crates/sui-js`, build the Node addon:

```bash
npm run build
```

It runs `napi build --platform --release`; the generated `.node` file stays
next to `index.js`, where the loader looks for it. From the workspace root,
build the browser module into `crates/sui-js/web/`:

```bash
cargo xtask web build --release
```

Both outputs are local build artifacts and are not committed.

## Open a window

```javascript
import * as sui from "@sui/ui";

const count = new sui.State(0);

const app = new sui.App().window(
  new sui.Window("Hello from JavaScript").root(
    sui.column(
      [
        sui.label(count),
        sui.button("Increment", { onPress: () => count.set(Number(count.get()) + 1) }),
      ],
      { gap: 8 },
    ),
  ),
);

await app.run();
```

`app.run()` opens the app's windows and returns a promise that resolves when
the app exits and rejects if it fails. Builders such as `Window.root()` and
`App.window()` return their object, so they chain. `run` takes options:

- `onReady(ui)` runs once the windows exist and receives the `UiHandle`. If it
  throws or rejects, the app exits and `run()` rejects with that error.
- `signal` exits the app when an `AbortSignal` aborts.
- `canvas` (browser only) is the canvas, or its element id, for the first
  window. It defaults to the element with id `sui-main-canvas`, or a new canvas
  filling the page. SUI keeps the canvas the size of its container: the
  closest ancestor with a `data-sui-resize-container` attribute, else its
  parent, else the viewport.

`ui.requestExit()` ends the app, `ui.emit(name, payload)` sends a message to
`app.on(name, listener)` listeners, and `await ui.showFileDialog(options)`
resolves with the chosen paths, or `null` when the user cancels. Browsers have
no native file dialogs, so there it rejects. One app runs at a time per
process or page.

On Windows, SUI raises the system timer resolution while an app runs, as
browsers do, so Node's timers can schedule frames at full rate.

## Callbacks, async work, and errors

Callbacks such as `onPress`, `onChange`, `state.watch`, and `app.on`
listeners may be `async`:

```javascript
sui.button("Load", {
  async onPress() {
    status.set("Loading…");
    const response = await fetch(url);
    status.set(response.ok ? "Loaded" : "Failed");
  },
});
```

A callback that throws, or whose promise rejects, does not stop the app: the
error goes to the handler set with `sui.setErrorHandler(handler)`, which logs
with `console.error` by default. Callbacks whose result SUI uses, such as a
`rebuildOnChange` builder, a `state.select` selector, or custom-widget
`measure` and `paint`, must be synchronous.

State changes made anywhere, whether in a timer, after an `await`, or in a
callback, update the windows that show the state.

## Run the examples

After building the addon, run these from `crates/sui-js`:

```bash
node examples/counter.js
node examples/custom-widget.js
node examples/external-surface.js
node examples/window.js
```

The first three render without a platform through `App.start()` and print
snapshot or event information; `window.js` opens a desktop window.

- [`counter.js`](examples/counter.js) covers `State`, built-in controls, an
  async callback, and rerendering.
- [`custom-widget.js`](examples/custom-widget.js) supplies JavaScript
  measurement, event, semantics, and paint callbacks.
- [`external-surface.js`](examples/external-surface.js) renders a CPU-RGBA
  texture through `ExternalSurface`.
- [`window.js`](examples/window.js) runs a window next to Node timers and an
  async callback.
- [`web/index.html`](examples/web/index.html) runs in a browser: after
  `cargo xtask web build`, serve `crates/sui-js` over HTTP (for example with
  `python -m http.server --directory crates/sui-js`) and open
  `/examples/web/index.html`.

## Widgets

JavaScript factories use `lowerCamelCase` and collect optional values in an
options object.

Interactive controls accept `enabled`, a boolean or a `State`. `column(...)`
and `row(...)` take `justify`, `alignItems`, and `wrap`; wrap a direct child in
`flexItem(child, { grow: 1 })` to size it along the main axis, and use
`spacer()` to fill the remaining space. `rebuildOnChange(states, build)`
rebuilds a subtree whenever one of `states` changes. Host-driven tests can
call `runningApp.advanceTime(seconds)`,
`settleAnimations()`, and `screenshotPng()`, set `setMotionPreference(...)`,
and scope `find`/`getOne` queries with `within: node.id`.
`FocusController` and `focusScope(child, { controller })` move keyboard focus
from code. `clipboardText()` and `setClipboardText()` reach the clipboard. Pass a
`ScrollController` as `controller` to `scrollView` or `virtualScrollView` to
scroll from code with `scrollTo` and `scrollToItem`. Labels take typography
options, `listView` accepts `ListItem` objects beside strings, and `grid`
accepts explicit column and row tracks with `gridCell` placement.

The portable media surface includes both `colorPicker(...)` and the compact,
mode-selectable `simpleColorPicker(...)` (`hsl`, `hsv`, `rgb`, or `oklch`).
Editor shells can use `DockState`, serializable `DockLayout`/`DockNode` values,
stable `DockPanelSpec` descriptors, and `dockWorkspace(...)` without exposing
Rust-local widget ownership.
Responsive composition includes idiomatic `grid(...)`, `aspectRatio(...)`,
`safeArea(...)`, and `layoutTransition(...)` factories without exposing Rust
track or animation implementation types.
`adaptiveView(...)`, `constraintView(...)`, `responsiveSidebar(...)`, and
`masterDetail(...)` retain each branch or pane while exposing ordinary
JavaScript state objects and options callbacks.
`bottomSheet(...)` uses height-oriented options and the same retained shown
state and dismissal callbacks as `sideSheet(...)`.
Thread-safe `NotificationCenter` producers feed `notificationHost(...)`, and
`overlayHost(...)` creates an independent stacking root for embedded regions.
`commandPalette(...)` exposes retained shown state and an options-object
dismissal callback; querying, ranking, and execution remain JavaScript policy.
`VirtualListModel` provides stable keyed, thread-safe incremental text rows;
`virtualList(...)` realizes only visible rows and exposes options-object
selection and near-edge callbacks.
`virtualTable(name, columns, model, options)` does the same for a table: a
thread-safe `TableModel` holds keyed `VirtualTableRow` values, each
`VirtualTableColumn` sets widths, alignment, and resizing, and selection,
row activation, header activation, and column resizing report string keys.
Sorting stays application policy: reorder the model in `onHeaderActivate` and
show the direction with `model.setSort(columnKey, "ascending")`.
`CanvasViewport`, `CanvasStroke`, and `CanvasShape` are value descriptors used
by retained `canvas(...)` and `canvasRuler(...)` factories.
Portable `DragScope`, `dragDropHost(...)`, `draggable(...)`, and
`dropTarget(...)` use text payloads and JavaScript callbacks, including
external file hover/drop delivery.
`FloatingWorkspaceState`, `FloatingView`, and `floatingWorkspace(...)` expose
stable same-window editor panes with move, resize, visibility, z-order, and
maximize controls.
`PixelCanvasState`, `PixelCanvasExport`, and `pixelCanvas(...)` expose editable
pixel documents, brush/tool controls, undo/redo requests, viewport commands,
and `Uint8Array` RGBA exports.

Streaming Markdown uses a thread-safe `RichDocument` model and a retained
`richDocumentView(...)`. `appendMarkdown(...)` preserves the native incremental
tail-reparse behavior; attachment and extension blocks take JavaScript options
objects rather than positional Rust-style argument lists.

```javascript
const theme = sui.Theme.dark();
const app = new sui.App();
app.setTheme(theme);
app.configureRendering({
  outputColorPrimaries: "display-p3",
  colorManagement: "prefer-wide-gamut",
});

theme.setAccent(new sui.Color(0.2, 0.55, 1, 1));
```

Use `theme.color(...)`/`setColor(...)` for source colors and
`theme.number(...)`/`setNumber(...)` for spacing, radii, breakpoints, and
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

When running directly from this checkout, import `./index.js` from
`crates/sui-js` (or `./crates/sui-js/index.js` from the workspace root)
instead of `@sui/ui`.

Use `App.start()` for embedding, deterministic tests, or host-driven rendering:

```javascript
const running = app.start();
const snapshot = running.render();
console.log(snapshot.commandCount, snapshot.semanticsCount);
```

The returned `RunningApp` can render windows, dispatch binding event
descriptors, drain queued work, request redraws, and expose window handles. It
does not create or present a native surface by itself. After driving it,
`await running.idle()` waits for async callbacks to settle and applies their
updates. On Node, `await running.saveScreenshot(path)` writes a PNG.

`running.setInspectorTracing()` and `running.inspect()` expose a
renderer-neutral diagnostic snapshot: the accessibility nodes, focus and
pending phases, frame/widget timings, plus structured reactive, command,
invalidation, rebuild, and privacy-safe event-route histories.

## Animation and semantic testing

Animations use ordinary JavaScript objects and options bags. `AnimationValue`
supports scalar, point/vector, size, rectangle, color, and transform values;
`Transition`, `Spring`, and `AnimatedValue` cover local motion. Reusable motion
uses `Keyframe`, `AnimationTrack`, `AnimationClip`, and `AnimationTimeline`,
with `AnimationPlayer`, serializable `AnimationDocument`, and undoable
`AnimationEditor` APIs for tools. Timelines carry named markers that players
report as the playhead passes them, players loop once, repeat, or ping-pong
with optional start and loop delays, and `Stagger` computes delays for
`AnimatedValue.setTargetAfter` so a group can cascade. `Presence` shows and
hides a widget with an enter and exit animation.

```javascript
const zero = sui.AnimationValue.scalar(0);
const one = sui.AnimationValue.scalar(1);
const track = new sui.AnimationTrack("card", "layer.opacity");
track.addKeyframe(new sui.Keyframe(0, zero));
track.addKeyframe(new sui.Keyframe(1, one, { easing: "ease-out" }));
const clip = new sui.AnimationClip("fade", 0, 1);
clip.addTrack(track);
const timeline = new sui.AnimationTimeline(1);
timeline.addClip(clip);
```

`RenderSnapshot.find(...)` and `getOne(...)` query complete semantic nodes by
role, name, text, description, focus, and visibility. Nodes retain hierarchy,
bounds, actions, values, and interaction state; pass one to `running.hover`,
`click`, `press`, or `fill` for locator-style deterministic tests.

## State and messages

`state.select(fn)` creates a retained derived state, and
`state.watch(fn, { signal })` calls `fn` with each new value until its
`StateSubscription` unsubscribes or is disposed (`using sub = state.watch(…)`),
or until the signal aborts. Selectors suppress unchanged values.
`await state.changed()` resolves with the next value, and
`for await (const value of state.changes())` iterates over changes.

For application services, register a listener with `app.on(name, listener)`,
which returns a disposable `Subscription`, or wait for one message with
`await app.once(name)`; publish with `uiHandle.emit(name, payload)`.

Custom-widget and paint callbacks run synchronously during a frame. Keep them
short; do slow work in async callbacks and publish the result through `State`.

## Custom widgets and resources

An object wrapped with `new sui.Widget(object)` may implement:

- `measure(constraints)` to return a `Size`;
- `event(event)` to process binding-safe event descriptors;
- `semantics(semantics)` to expose roles, names, values, ranges, and actions;
- `paint(paint)` to emit validated scene commands.

The `event` callback receives `(event, context)`. `EventContext` exposes focus,
handled state, targeted paint/layout/semantics requests, animation-frame
requests, pointer capture, clipboard access, stable IDs, bounds, routing phase,
and frame time. JavaScript callbacks that only declare one parameter continue
to work because extra arguments are ignored by the language.

`new sui.Widget(callbacks, children)` creates a retained foreign composite.
Use `measureWithChildren(constraints, childSizes)` and
`arrange(bounds, childSizes)` for custom layout; return one `Rect` per child.
Children are painted after the custom paint commands and are automatically
included in semantics unless the callback explicitly includes them.

The paint surface supports styled text, paths and path clips, rounded
rectangles, shadows, transforms, image quads, and validated built-in shaders.
Applications can register RGBA, PNG, or SVG images and TrueType, OpenType,
WOFF, or WOFF2 fonts from bytes, and on Node also from files.
`await app.fontUrl(url)` fetches and registers a font, such as a web font the
page also uses; a `FontHandle`'s `family` reads the font's family name. See [`examples/custom-widget.js`](examples/custom-widget.js) for a
complete custom control.

`ExternalSurface` accepts CPU-upload, shared-texture, and shared-render-target
descriptors. The CPU-RGBA path renders today. Shared descriptors are validated
and retained for host integration, but zero-copy renderer composition is not
implemented yet.

## TypeScript

`index.d.ts` declares the package for both entry points. A local project can
depend on the package directory while developing:

```json
{
  "dependencies": {
    "@sui/ui": "file:../path/to/sui/crates/sui-js"
  }
}
```

Build the addon or the browser module before importing it.

## Current limitations

- The npm package, native binaries, and release automation are not published;
  users build from source.
- On Node, the app's event loop runs between Node's own tasks and checks for
  input at least every 8 ms; creating the first window blocks Node briefly.
- In the browser, apps cannot read files (`pngFile`, `svgFile`, `fontFile`),
  show file dialogs, or take screenshots; pass bytes instead.
- Every public Rust widget is classified as directly bound, manually wrapped,
  or represented by a documented JavaScript-level equivalent. Some equivalents
  intentionally expose portable value models instead of Rust closure or `Any`
  implementation details.
- Custom WGSL, arbitrary shader resources, and uniforms are not exposed;
  custom paint can use only validated built-in shaders.
- Shared textures and shared render targets are descriptor-level APIs today;
  only the portable CPU-upload external surface is rendered end to end.
- The API is pre-release and may change before the first stable release.

## Validate binding changes

From `crates/sui-js`, build the addon and run the test suite:

```bash
npm test
```

Run the same suite on the browser build, in Node:

```bash
npm run test:web
```

Set `SUI_DESKTOP_TESTS=1` to include the tests that open desktop windows.

## More documentation

- [Examples catalog](../../docs/examples.md)
- [Rust API guide](../../docs/api/README.md)
- [Testing guide](../../docs/testing.md)
- [Cross-language binding roadmap](../../docs/plans/cross-language-bindings-plan.md)
- [Documentation index](../../docs/README.md)
