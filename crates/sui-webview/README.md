# SUI WebView

`sui-webview` adds optional native embedded web content to SUI desktop apps.
It uses [WRY](https://github.com/tauri-apps/wry) and the operating system web
engine: WebView2 on Windows, WKWebView on macOS, and WebKitGTK on Linux/X11.

The crate deliberately owns only embedding mechanics: widget layout,
native-child bounds, lifecycle, control operations, and SUI event delivery.
Navigation policy, permissions, custom protocols, storage, CSP, and all other
content/security choices remain application responsibilities and can be set
through `WebView::configure`.

```rust,no_run
use sui::prelude::*;
use sui_webview::WebViewHost;

fn main() -> Result<()> {
    let webviews = WebViewHost::new();
    let browser = webviews
        .url("https://example.com")
        .accessible_name("Documentation");

    webviews.run(
        App::new()
            .main_window("Embedded web content", Padding::all(16.0, browser)),
    )
}
```

See `docs/api/webviews.md` in the SUI repository for platform limitations and
the complete control/event example.

## Linux build prerequisites

Install WebKitGTK 4.1, including its development headers and pkg-config files.
On Arch Linux, use `sudo pacman -S --needed webkit2gtk-4.1`; this also installs
JavaScriptCore and the GTK dependencies. On Debian/Ubuntu, install
`libwebkit2gtk-4.1-dev`. Cargo fetches the Rust bindings, while these native
libraries come from the system package manager.

Verify detection with:

```sh
pkg-config --modversion webkit2gtk-4.1 javascriptcoregtk-4.1
```
