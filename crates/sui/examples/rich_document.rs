use sui::prelude::*;

fn main() -> Result<()> {
    let theme = DefaultTheme::dark();
    let document = RichDocumentModel::from_markdown(
        r#"# Deployment report

SUI keeps this document selectable across blocks. Follow the [runbook](https://example.test/runbook) for details.

- [x] Build artifacts
- [x] Run tests
- [ ] Publish release

```rust
let status = deploy().await?;
assert!(status.ready());
```
"#,
    );

    let mut operation = RichExtensionBlock::new("operation-log", "Release operation");
    operation.status = RichDocumentStatus::Running;
    operation.summary = Some("2 of 3 stages complete".into());
    operation.body = "build   complete\ntest    complete\npublish running".into();
    document.append_extension(operation);

    let view = RichDocumentView::new(document)
        .theme(theme)
        .on_link(|destination| println!("Application URL policy received: {destination}"));
    let root = Surface::window(ScrollView::vertical(view).retain_content_layer())
        .theme(theme)
        .padding(Insets::all(24.0))
        .fill();

    App::new()
        .window(Window::new("SUI Rich Document").root(root))
        .run()
}
