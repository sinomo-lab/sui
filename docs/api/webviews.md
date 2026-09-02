# Embedded Webviews

[Previous: node graph editor](node-graphs.md) · [API guide](README.md)

`sinomo-ui-webview` is an optional desktop library for placing native web
content inside a retained SUI layout. It uses WRY and the operating-system web
engine rather than rasterizing a browser into the WGPU scene:

- Windows: WebView2
- macOS: WKWebView
- Linux/X11: WebKitGTK

Add both packages under their conventional import names:

```toml
[dependencies]
sui = { package = "sinomo-ui", version = "0.2" }
sui-webview = { package = "sinomo-ui-webview", version = "0.2" }
```

## Create and Run

A `WebViewHost` owns the native child views. Widgets created from a host must be
run with that same host:

```rust,no_run
use sui::prelude::*;
use sui_webview::WebViewHost;

fn main() -> Result<()> {
    let webviews = WebViewHost::new();
    let browser = webviews.url("https://example.com");

    webviews.run(
        App::new().main_window(
            "Documentation",
            Padding::all(16.0, browser),
        ),
    )
}
```

`WebViewHost::run` preserves renderer settings configured on `App`. For an app
that already constructs `DesktopPlatform`, attach the host explicitly:

```rust,no_run
# use sui::prelude::*;
# use sui::DesktopPlatform;
# use sui_webview::WebViewHost;
# let webviews = WebViewHost::new();
# let browser = webviews.url("about:blank");
let platform = webviews.attach(DesktopPlatform::new().with_vsync_enabled(true));
App::new()
    .main_window("Web", browser)
    .run_with_platform(platform)?;
# Ok::<(), sui::Error>(())
```

## Control and Events

`WebViewHandle` is cloneable and thread-safe. It queues navigation, HTML,
JavaScript, history, focus, visibility, zoom, reload, and print operations onto
the UI thread. Calls made before native creation remain queued. Once the widget
has been dropped, operations return `WebViewClosed`.

Use `WebView::on_event` for creation, page-load, document-title, IPC, and
operation-failure notifications. The callback runs as a typed widget command
on SUI's UI thread and receives `EventCtx`, so it can invalidate state or post
normal application commands.

IPC is intentionally opt-in:

```rust,no_run
# use sui::prelude::*;
# use sui_webview::{WebViewEvent, WebViewHost};
# let webviews = WebViewHost::new();
let browser = webviews
    .html("<button onclick=\"window.ipc.postMessage('save')\">Save</button>")
    .enable_ipc()
    .on_event(|_ctx, event| {
        if let WebViewEvent::Ipc { uri, body } = event {
            println!("IPC from {uri}: {body}");
        }
    });
```

Every loaded document can call the bridge after `enable_ipc`; validate the
active origin and message format according to the application's trust model.

## Application-Owned Policy

The crate does not select navigation, permission, download, protocol, proxy,
storage, clipboard, user-agent, or content-security policy. Configure those
directly on WRY's builder:

```rust,no_run
# use sui_webview::WebViewHost;
# let webviews = WebViewHost::new();
let browser = webviews
    .url("https://docs.example.test")
    .configure(|builder| {
        builder
            .with_navigation_handler(|url| url.starts_with("https://docs.example.test/"))
            .with_clipboard(false)
    });
```

The callback may be invoked more than once when a suspended or structurally
detached native view is recreated, so captured configuration must be reusable.
SUI installs optional page/title/IPC callbacks before calling `configure`;
setting the same WRY callback in `configure` intentionally replaces that piece
of SUI event delivery.

For shared cookies, custom-protocol context, or an explicit browser data
directory, construct `sui_webview::wry::WebContext` and pass it to
`WebViewHost::with_web_context`. The host retains it for the native views while
the application remains responsible for the selected persistence policy.

## Native-Child Boundaries

An embedded webview is a native rectangular child above the WGPU surface. Its
bounds follow the widget's final axis-aligned presentation bounds, and the
operating system sends pointer, keyboard, IME, and accessibility interaction
directly to the web engine. This has several deliberate limitations:

- SUI paint transforms are represented by the axis-aligned bounding box; the
  page itself is not rotated or skewed.
- SUI clips, rounded corners, opacity, effects, and overlay stacking cannot be
  composited over a native child. Hide the webview while presenting UI that
  must cover it.
- Headless rendering exposes the document semantic placeholder but does not
  rasterize web content into screenshots.
- WRY's raw child-window path supports Linux X11. Wayland requires a GTK-owned
  container, which SUI's winit host does not currently provide; startup returns
  an explicit error instead of silently creating a detached view.
- Platform runtimes and packaging remain application prerequisites (for
  example WebView2 on Windows and WebKitGTK development/runtime packages on
  Linux).

Run the complete example with:

```bash
cargo run -p sinomo-ui-webview --example embedded
```
