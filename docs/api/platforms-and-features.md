# Platforms and Cargo Features

[Previous: testing and accessibility](testing-and-accessibility.md) ·
[API guide](README.md)

The `sinomo-ui` package separates the renderer-neutral runtime from optional
platform event loops and the WGPU renderer. Select the smallest feature set
that matches the host.

## Feature Reference

| Feature | Enabled by default | Enables |
| --- | --- | --- |
| `desktop` | Yes | Desktop platform integration and `wgpu` |
| `wgpu` | Yes | `WgpuRenderer`, renderer capabilities, and external texture interop |
| `web` | No | Browser/WebAssembly platform integration and `wgpu` |
| `mobile` | No | Mobile platform integration and `wgpu`; current app entry point is Android |
| `testing` | No | Currently an empty compatibility feature; high-level test APIs live in `sinomo-ui-testing` |

`desktop`, `web`, and `mobile` all pull in `sinomo-ui-platform`; each also enables
`wgpu`. The core facade, widgets, layout, scene, text, and retained runtime are
available without those features.

## Desktop

The default dependency enables the supported Linux, macOS, and Windows desktop
path:

```toml
[dependencies]
sui = { package = "sinomo-ui", git = "https://github.com/sinomo-lab/sui" }
```

`App::run()` and `App::run_with_handle(...)` are available. They build the
runtime, create the platform event loop and WGPU renderer, and return when the
event loop exits or startup fails.

Desktop platform support ultimately depends on compatible `winit` windowing
and `wgpu` adapter/surface support on the target machine. Test the real target
and graphics stack before shipping; a successful headless test does not prove
surface creation or presentation on every driver.

The platform layer also normalizes native file hover/drop events and provides
the asynchronous `NativeFileDialogs` service on Linux, macOS, and Windows.
Linux builds use the selected native portal/backend support from the platform
dependency; test dialogs in the actual desktop session and packaging format.
See [Overlays and desktop interaction](overlays-and-desktop.md) for the portable
request, file-handle, and `DragDropHost` APIs.

## Runtime-only or Custom Embedding

Disable defaults to construct widgets and a runtime without selecting the
facade's platform or renderer:

```toml
[dependencies]
sui = {
    package = "sinomo-ui",
    git = "https://github.com/sinomo-lab/sui",
    default-features = false,
}
```

`App::build()` remains available. `App::run()` is not part of this feature
combination, because there is no selected event-loop host. Use the returned
`Runtime` from an embedding layer, or add `sinomo-ui-testing` as a dev dependency for
the supported automation harness.

The lower-level `Application::run()` has an explicit no-platform error path,
but normal facade code should use `App::build()` rather than relying on that
error.

## Browser and WebAssembly

Select the web platform without pulling the desktop feature:

```toml
[dependencies]
sui = {
    package = "sinomo-ui",
    git = "https://github.com/sinomo-lab/sui",
    default-features = false,
    features = ["web"],
}
```

Build for `wasm32-unknown-unknown`. `App::run()` and
`App::run_with_handle(...)` are available under the `web` feature and use the
browser event-loop path. The repository's browser demo is the reference host:

```bash
rustup target add wasm32-unknown-unknown
trunk serve --config crates/sui-demo/web/Trunk.toml
trunk build --config crates/sui-demo/web/Trunk.toml --release
```

Web output uses WebGPU where available through `wgpu`. Browser security and
activation rules still apply to clipboard, focus, input methods, and other
host services. Verify the browsers and deployment headers supported by the
product rather than treating desktop behavior as proof of browser behavior.

Web builds support asynchronous open/save file handles through the same dialog
service, subject to browser user-activation and security policy. Files expose
byte-oriented `read`/`write` operations rather than host filesystem paths, and
folder selection is intentionally unavailable.

The Rust/Wasm application surface is implemented. The workspace's native
JavaScript binding targets Node/Electron; it is not a browser JavaScript/DOM
binding.

## Android

Android is experimental and uses the mobile feature:

```toml
[dependencies]
sui = {
    package = "sinomo-ui",
    git = "https://github.com/sinomo-lab/sui",
    default-features = false,
    features = ["mobile"],
}
```

On `target_os = "android"`, the facade exports `AndroidApp` and provides
`App::run_android(...)` and `run_android_with_handle(...)`. These methods take
the native-activity handle supplied by the Android host. The ordinary
`App::run()` method is not the Android entry point.

SUI follows Android's native-window lifecycle: it does not create a Winit
window or `wgpu::Surface` until the host delivers `Resumed`. On `Suspended`,
the platform layer drops all GPU surfaces before the native `SurfaceView`
becomes invalid, while retaining the runtime and widget tree. A later
`Resumed` restores the surfaces, refreshes the viewport, and schedules a new
frame automatically. Application code should not need to synthesize redraws
or rebuild its UI around these transitions.

Treat package metadata, lifecycle integration, permissions, input, soft
keyboard behavior, graphics adapter availability, and device testing as part
of the Android application. The `mobile` feature does not provide an iOS entry
point in the current public facade.

## WGPU-specific APIs

The `wgpu` feature adds:

- `WgpuRenderer` and `RendererCapabilities`.
- Renderer interop and external texture registry types.
- `App::feathering(...)` and `feather_width(...)`.
- `App::external_texture_registry(...)`.

Most applications should remain renderer-neutral and use built-in widgets,
`PaintCtx`, registered images, and `WindowRenderOptions`. Feature-gated WGPU
types are intended for renderer configuration, app-owned GPU textures, debug
tools, and embedding.

Do not retain raw backend resources inside an ordinary widget. Put external
textures in `WgpuExternalTextureRegistry`, keep their lifetime application
owned, and refer to them from scene content through stable image handles.

## Render and Output Options

`App::render_options(WindowRenderOptions)` applies initial options to every
window. Runtime-level helpers can inspect or change per-window options after
construction. The option model includes text coverage and hinting, dynamic
range, color primaries, tone mapping, and output color-management policy.

Capabilities vary by adapter, surface, display, operating system, and browser.
An option expresses requested policy; use renderer and window diagnostics to
observe what the active path supports. Validate HDR or wide-gamut output on
real capable displays and preserve an SDR fallback.

## Testing Across Feature Sets

Before release, validate at least:

```bash
cargo check -p sinomo-ui
cargo check -p sinomo-ui --no-default-features
cargo test -p sinomo-ui-testing
cargo test --workspace
```

Also build every target the product advertises. Host-only `cargo check` does
not compile target-specific WebAssembly or Android code.

## Binding Packages

The Rust facade is the stable center of this guide. Native Python and
Node/Electron crates are present as alpha surfaces but are not registry
packages and have their own host-driven API details. See the binding READMEs in
`crates/sui-python` and `crates/sui-js` when shipping those artifacts; do not
infer browser JavaScript or Python wheel availability from the Rust features.
