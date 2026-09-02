use sui::prelude::*;
use sui_webview::{WebViewEvent, WebViewHost};

fn main() -> Result<()> {
    let webviews = WebViewHost::new();
    let control = webviews
        .html(
            r#"<!doctype html>
            <meta charset="utf-8">
            <style>
              body { font: 16px system-ui; padding: 24px; }
              button { padding: 8px 12px; }
            </style>
            <h1>Native web content</h1>
            <button onclick="window.ipc.postMessage('hello from JavaScript')">
              Send IPC message
            </button>"#,
        )
        .accessible_name("Embedded browser example")
        .enable_ipc()
        .on_event(|_ctx, event| match event {
            WebViewEvent::Ipc { body, .. } => println!("webview IPC: {body}"),
            WebViewEvent::OperationFailed { operation, error } => {
                eprintln!("webview {operation:?} failed: {error}");
            }
            _ => {}
        })
        .configure(|builder| builder.with_devtools(cfg!(debug_assertions)));
    let handle = control.handle();

    let toolbar = Stack::horizontal()
        .spacing(8.0)
        .with_child(Button::new("Reload").on_press({
            let handle = handle.clone();
            move || {
                let _ = handle.reload();
            }
        }))
        .with_child(Button::new("Change heading").on_press(move || {
            let _ = handle
                .evaluate_script("document.querySelector('h1').textContent = 'Updated from SUI';");
        }));

    let content = Stack::vertical()
        .spacing(12.0)
        .with_child(toolbar)
        .with_child(control);

    webviews.run(
        App::new().window(
            Window::new("SUI WebView")
                .initial_size(Size::new(900.0, 640.0))
                .root(Surface::window(content).padding(Insets::all(16.0)).fill()),
        ),
    )
}
