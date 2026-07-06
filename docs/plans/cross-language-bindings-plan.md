# Python and JavaScript Binding Plan

This plan adds Python and JavaScript support without turning SUI into a
different framework per language. The Rust API remains the most complete API.
The foreign-language APIs expose a smaller, stable surface focused on:

- building ordinary widget trees
- writing custom widgets safely
- sending work across threads into the UI runtime
- painting with renderer-neutral scene commands
- opting into controlled graphics and GPU interop where the host application
  owns the backend-specific details

## Design Principles

1. Keep the UI tree on the SUI UI thread.
   Foreign callbacks run synchronously on the UI thread. Background Python
   threads, JavaScript workers, async tasks, and native engines communicate
   through message queues and wake handles.

2. Bind to the public SUI concepts, not every Rust type.
   The stable binding surface should be `App`, `Window`, widgets, resources,
   handles, events, geometry, colors, text styles, animation data, and paint
   commands. Do not expose raw runtime graph mutation as the normal API.

3. Make handles thread-safe, not widget objects.
   Widget instances are UI-thread objects. Handles such as `UiHandle`,
   `WindowHandle`, `ResourceHandle`, `ImageHandle`, `ShaderHandle`, and
   `ExternalSurfaceHandle` are cloneable and safe to send across threads.

4. Keep painting safe and retained-renderer friendly.
   Foreign widgets paint into a command builder backed by `sui-scene`. They
   cannot receive raw `wgpu::Device`, `wgpu::Queue`, or render-pass access from
   normal paint callbacks.

5. Treat GPU interop as an explicit capability layer.
   Zero-copy interop is backend and host dependent. SUI should provide the
   rendezvous points and synchronization contracts, while applications or
   language-specific graphics libraries provide the actual backend handles.

## Crate Shape

Add shared binding infrastructure first, then language adapters:

```text
crates/
  sui-bindings-core/    # language-neutral dynamic widget and command bridge
  sui-python/           # PyO3/maturin package
  sui-js/               # napi-rs desktop package
  sui-js-web/           # wasm-bindgen/web package, optional split if needed
```

`sui-bindings-core` should depend on `sui`, but not on PyO3, napi, V8, or a
specific JavaScript engine. Its job is to own:

- dynamic foreign widget adapters
- callback vtables
- UI-thread message queues
- handle registries
- paint command validation
- error boundaries for foreign callbacks
- language-neutral interop descriptors

The language crates translate Python or JavaScript objects into those shared
adapters.

## Public API Shape

### Python

```python
import sui

counter = sui.State(0)

def increment(ctx):
    counter.set(counter.get() + 1)

app = sui.App()
app.window(
    sui.Window("Counter").root(
        sui.Column(
            sui.Label(lambda: f"Count: {counter.get()}"),
            sui.Button("Increment", on_click=increment),
            gap=8,
        )
    )
)
app.run()
```

Custom widget:

```python
class Meter(sui.Widget):
    def __init__(self, value):
        self.value = value

    def measure(self, ctx, constraints):
        return constraints.clamp(sui.Size(160, 28))

    def paint(self, ctx):
        rect = ctx.bounds
        ctx.fill_rect(rect, sui.Color.rgb(0.11, 0.12, 0.14))
        ctx.fill_rect(
            sui.Rect(rect.x, rect.y, rect.width * self.value, rect.height),
            sui.Color.rgb(0.25, 0.68, 0.46),
        )

    def semantics(self, ctx):
        ctx.node(role="progress_bar", name="Meter", value=self.value)
```

Thread-safe update:

```python
def load_data(ui, model):
    data = expensive_work()
    ui.post(lambda: model.set(data))

app.run_with_handle(lambda ui: threading.Thread(
    target=load_data,
    args=(ui, model),
    daemon=True,
).start())
```

### JavaScript

```javascript
import { App, Window, Column, Label, Button, State } from "@sui/ui";

const count = new State(0);

const app = new App();
app.window(
  new Window("Counter").root(
    new Column([
      new Label(() => `Count: ${count.get()}`),
      new Button("Increment", () => count.set(count.get() + 1)),
    ], { gap: 8 })
  )
);

await app.run();
```

Custom widget:

```javascript
class Meter extends Widget {
  constructor(value) {
    super();
    this.value = value;
  }

  measure(ctx, constraints) {
    return constraints.clamp(new Size(160, 28));
  }

  paint(ctx) {
    const r = ctx.bounds;
    ctx.fillRect(r, Color.rgb(0.11, 0.12, 0.14));
    ctx.fillRect(new Rect(r.x, r.y, r.width * this.value, r.height),
      Color.rgb(0.25, 0.68, 0.46));
  }
}
```

Worker-thread update:

```javascript
app.runWithHandle((ui) => {
  worker.onmessage = (event) => {
    ui.post(() => model.set(event.data));
  };
});
```

## Threading Contract

Expose three object categories:

1. UI-thread objects:
   `App` before run, widget instances, `PaintCtx`, `EventCtx`, `MeasureCtx`,
   `ArrangeCtx`, and `SemanticsCtx`.

2. Thread-safe handles:
   `UiHandle`, `WindowHandle`, `WidgetHandle`, `State<T>`, resource handles,
   shader handles, and external surface handles.

3. Immutable snapshots:
   events, semantics snapshots, diagnostics, scene snapshots, captured images,
   text layout measurements, and animation samples.

Rules:

- A foreign widget callback must never be invoked concurrently for the same
  widget.
- Foreign callbacks must be bounded. Long-running work must leave the UI thread.
- `State.set` from a worker queues a UI-thread mutation and wake unless the
  state was explicitly created as local-only.
- `UiHandle.post(fn)` runs `fn` on the UI thread, then applies invalidation.
- Exceptions thrown by callbacks are caught and converted into SUI errors or
  diagnostic error widgets. They must not unwind across FFI.

## Custom Widget Bridge

`sui-bindings-core` should add a Rust adapter similar to:

```rust
pub trait ForeignWidgetVTable: Send + Sync + 'static {
    fn debug_name(&self, id: ForeignWidgetId) -> &'static str;
    fn event(&self, id: ForeignWidgetId, ctx: ForeignEventCtx, event: ForeignEvent);
    fn measure(
        &self,
        id: ForeignWidgetId,
        ctx: ForeignMeasureCtx,
        constraints: Constraints,
    ) -> Size;
    fn arrange(&self, id: ForeignWidgetId, ctx: ForeignArrangeCtx, bounds: Rect);
    fn paint(&self, id: ForeignWidgetId, ctx: ForeignPaintCtx);
    fn semantics(&self, id: ForeignWidgetId, ctx: ForeignSemanticsCtx);
    fn children(&self, id: ForeignWidgetId) -> Vec<ForeignWidgetHandle>;
}
```

The adapter implements `sui_runtime::Widget` and forwards into the selected
language runtime on the UI thread. The vtable is thread-safe, but the actual
language object access is serialized by the binding runtime.

Initial child support should be explicit:

- leaf custom widgets can omit `children`
- container custom widgets return retained child handles
- generated or virtual children can be added later after the basic bridge is
  stable

## Safe Low-Level Painting API

Bindings should expose a `Painter` or `PaintCtx` with commands that map directly
to `sui-scene`:

- `clear(color)`
- `fill_rect(rect, brush)`
- `stroke_rect(rect, brush, width)`
- `fill_path(path, brush)`
- `stroke_path(path, brush, stroke)`
- `fill_rrect(rect, radii, brush)`
- `draw_shadow(rect, radii, shadow)`
- `draw_text(rect, text, style)`
- `draw_text_layout(origin, layout)`
- `draw_image(rect, image, options)`
- `draw_image_quad(points, image, options)`
- `push_clip_rect(rect)`, `push_clip_path(path)`, `pop_clip()`
- `push_transform(transform)`, `pop_transform()`
- `draw_shader_rect(rect, shader, uniforms, resources)`
- `draw_external_surface(rect, surface, options)`

Validation belongs in the shared bridge:

- balanced clip and transform stacks
- finite geometry
- bounded path complexity per command
- valid image and shader handles
- valid uniform layout and byte length
- no direct access to renderer internals from paint callbacks

## Shader API

Keep custom shaders fragment-oriented at first.

```python
shader = app.resources.shader(
    label="heatmap",
    wgsl="""
      @fragment
      fn fragment(input: SuiFragmentInput) -> @location(0) vec4<f32> {
          let t = input.local_position.x;
          return vec4<f32>(t, 0.2, 1.0 - t, 1.0);
      }
    """,
    uniforms={"gain": "f32"},
)
```

Renderer-facing additions:

- add `ShaderHandle` to `sui-core`
- add `ShaderRegistry` snapshot to `sui-scene::SceneFrame`
- add `SceneCommand::DrawCustomShaderRect`
- compile and cache WGSL in `sui-render-wgpu`
- expose a stable shader input contract: local position, bounds, viewport,
  dpi, time, color-management metadata, and user uniforms

Do not expose arbitrary render pipelines in the first version. A custom pipeline
API can come later behind an explicit interop feature.

## Graphics Interop

Interop has two independent directions.

### Embedding SUI in another renderer or app

Expose a host-driven runtime:

```text
SuiRuntime
  add_window(title, root) -> WindowId
  handle_event(window_id, Event)
  tick(time_seconds)
  drain_ready_events()
  render_scene(window_id) -> SceneFrame

SuiRenderer
  render_offscreen(scene_frame) -> SuiTexture
  render_to_external_target(scene_frame, target_descriptor)
```

The host owns the main loop and sends normalized events into SUI. SUI returns a
scene frame or renders into a host-supplied target if the backend can support
that target.

### Embedding external graphics in SUI

Expose an `ExternalSurface` widget:

```python
surface = sui.ExternalSurface(
    desired_size=(640, 360),
    backend="wgpu",
    renderer=my_renderer,
)

root = sui.Stack([
    surface,
    sui.Overlay(...),
])
```

`ExternalSurface` participates in SUI layout, hit testing, clipping,
accessibility naming, and z-order. The external renderer owns its content.

Interop tiers:

1. CPU fallback:
   external code publishes RGBA8 frames. SUI uploads them as regular images.

2. Shared texture:
   external code publishes a backend texture descriptor plus synchronization
   metadata. SUI samples that texture during composition.

3. Shared render target:
   SUI allocates or accepts a target for the external renderer to draw into
   before SUI composites overlays.

Backend descriptors should be explicit and feature-gated:

```rust
pub enum ExternalTextureDescriptor {
    Wgpu {
        size: Size,
        format: ExternalTextureFormat,
        color_space: ColorSpace,
        handle: ExternalBackendHandle,
        sync: ExternalSync,
    },
    Native {
        backend: NativeGraphicsBackend,
        platform_handle: ExternalBackendHandle,
        size: Size,
        format: ExternalTextureFormat,
        sync: ExternalSync,
    },
    CpuRgba8 {
        size: Size,
        pixels: Arc<[u8]>,
        generation: u64,
    },
}
```

The normal bindings should not promise that every native graphics object can be
shared. They should report capabilities and fall back to CPU upload when the
host/backend cannot provide compatible handles.

## Implementation Phases

### Phase 1: Shared Binding Core

- Add `sui-bindings-core`.
- Define foreign widget IDs, handle registries, callback vtables, and error
  boundaries.
- Implement `ForeignWidget` as a `Widget` adapter.
- Add `UiHandle.post` and a shared UI-thread queue abstraction.
- Add tests with a mock foreign language backend.

### Phase 2: Binding-Safe Paint Surface

- Add a binding-facing `PaintCommandBuilder`.
- Map supported commands to `PaintCtx`.
- Validate command inputs and stack balance.
- Add tests that custom foreign widgets can emit text, image, paths, clips,
  transforms, rounded rects, and invalidations.

### Phase 3: Custom Shader Support

- Add `ShaderHandle` and `ShaderRegistry`.
- Add `DrawCustomShaderRect` to `sui-scene`.
- Implement shader compilation, reflection/validation, uniform upload, and
  cache invalidation in `sui-render-wgpu`.
- Expose shader registration through `ResourceRegistry`.
- Add headless capture tests for custom shaders.

### Phase 4: Python Package

- Add `sui-python` using PyO3 and maturin.
- Bind geometry, colors, events, handles, resources, basic widgets, `State`,
  custom widgets, and paint commands.
- Keep desktop `run()` as the primary entrypoint. Add `run_with_handle`.
- Add async helpers that post back to the UI thread instead of mutating widgets
  from Python worker tasks.
- Add Python examples and smoke tests.

### Phase 5: JavaScript Packages

- Add a native desktop package for Node/Electron-style applications.
- Add TypeScript declarations for the stable API.
- Add a web/WASM package where browser support is desired.
- Keep the same conceptual API across desktop and web, but allow capability
  differences for GPU interop.
- Add JS examples and smoke tests.

### Phase 6: Graphics Interop

- Promote `RendererInterop` from a stub into a capability report.
- Add public renderer APIs for host-driven offscreen rendering and rendering
  into a supplied target descriptor.
- Add `ExternalSurface` and external texture descriptors.
- Implement CPU fallback first, then same-device/shared-texture paths.
- Add capability checks, synchronization hooks, diagnostics, and examples for
  wgpu-py, JavaScript WebGPU, and a Rust-native wgpu host.

### Phase 7: Documentation and Stability

- Document threading rules prominently for both languages.
- Document which objects are UI-thread-only, thread-safe handles, and immutable
  snapshots.
- Add compatibility tests that render equivalent simple apps in Rust, Python,
  and JavaScript.
- Keep the binding API semver-stable even while the Rust API continues to grow.

## Initial Non-Goals

- Full parity with the Rust prelude.
- Arbitrary `wgpu::RenderPass` access from widget paint callbacks.
- Making Rust `Widget` require `Send + Sync`.
- Making Python or JavaScript own the retained runtime graph directly.
- Guaranteed zero-copy interop across all graphics libraries and platforms.
