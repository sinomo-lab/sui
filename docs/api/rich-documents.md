# Rich Documents

[Previous: virtual collections](virtual-collections.md) ·
[API guide](README.md) ·
[Next: input and text editing](input-and-editing.md)

Use `RichDocumentModel` and `RichDocumentView` for Markdown, streamed
transcripts, read-only reports, and mixed text/structured results. The API is a
general document layer: it does not define chat messages, roles, agents, tool
schemas, or application persistence.

```rust,no_run
use sui::prelude::*;

let document = RichDocumentModel::from_markdown(
    "# Run report\n\nWaiting for output…",
);
let view_state = RichDocumentViewState::new();

let view = RichDocumentView::new(document.clone())
    .state(view_state.clone())
    .on_link(|destination| {
        // Route through the application's URL policy.
        println!("open {destination}");
    });

// A streaming producer can retain its clone and append fragments later.
document.append_markdown("\n\n```text\nready\n```");
# let _ = view;
```

The model is an `Observable<u64>`. `RichDocumentView` observes a structural
selector plus the signal for each realized block, keeps one keyed `WidgetPod`
per `RichBlockId`, and invalidates only changed blocks when the key sequence is
stable. A streaming append reparses the final mutable Markdown block while
retaining the completed prefix. This preserves widget identity, persistent
text-layout handles, structured-block expansion, and selection anchors across
normal append-only updates.

## Model and streaming contract

`RichDocumentModel` supports three update paths:

- `set_markdown` replaces the Markdown source, reconciles block IDs by source
  origin and block kind, and places existing structured blocks after the new
  Markdown segment.
- `append_markdown` appends a streaming fragment, reparsing only the previous
  tail block and the new fragment. `last_update` reports the reparsed byte
  range, reused prefix length, changed IDs, and whether the edit was
  append-only. If a structured block is currently last, the fragment begins a
  new independent Markdown segment after it; subsequent fragments continue
  that segment. This preserves Markdown → result → Markdown ordering.
- `append_attachment`, `append_extension`, and `update_structured` manage
  application-provided blocks after the Markdown portion of the document.

The built-in parser preserves paragraphs, headings, block quotes, ordered and
unordered lists, task-list state, fenced code blocks, thematic breaks, links,
inline code, emphasis, strong text, strike metadata, and images. Source ranges
are UTF-8 byte ranges into the Markdown string.

`snapshot` is a read-only diagnostic/export view. Its block vector is the
authoritative mixed-content order; its Markdown string concatenates the
Markdown segments. Do not mutate a snapshot and expect the view to update;
write through the model methods.

## Selection and actions

`RichDocumentView` coordinates character selection across block boundaries.
Dragging from a heading through paragraphs, lists, or code produces one ordered
selection. `Control/Command+A`, `Control/Command+C`, `TextCommand::SelectAll`,
and `TextCommand::Copy` operate on the whole document. The selected plain text
is also available from `RichDocumentViewState::selected_text`.

Links and images are inert until the application supplies `on_link` or
`on_image`; SUI never opens an external URL by itself. Both pointer activation
and accessibility activation use the same callbacks.

Markdown image sources are strings rather than network requests. Use
`image_resolver` to map a `RichInlineImage` to an `ImageHandle` already
registered with the SUI runtime:

```rust,no_run
# use sui::prelude::*;
# let document = RichDocumentModel::new();
# let cached_image: Option<ImageHandle> = None;
let view = RichDocumentView::new(document).image_resolver(move |image| {
    (image.source == "asset:plot").then_some(cached_image).flatten()
});
# let _ = view;
```

Until resolution succeeds, the renderer displays the image's alt text and
retains image semantics and activation. Attachments similarly carry portable
metadata; `on_attachment` lets the application present, save, preview, or
otherwise act on them.

## Code blocks and highlighting

The default code renderer provides a language header, a pointer- and
accessibility-invokable copy action, horizontal clipping and scrolling, and a
lightweight `BasicSyntaxHighlighter`. Horizontal wheel deltas scroll directly;
Shift plus a vertical wheel delta provides the desktop fallback.

Replace highlighting without replacing the code renderer:

```rust,no_run
# use sui::prelude::*;
# let document = RichDocumentModel::new();
let view = RichDocumentView::new(document).syntax_highlighter(
    |_language: Option<&str>, source: &str| {
        // Return non-overlapping UTF-8 byte ranges from a parser, LSP, syntect,
        // tree-sitter, or another application-owned service.
        if source.starts_with("fn") {
            vec![RichSyntaxSpan {
                range: 0..2,
                kind: RichSyntaxTokenKind::Keyword,
            }]
        } else {
            Vec::new()
        }
    },
);
# let _ = view;
```

The highlighter is synchronous and should return cached or inexpensive data.
Run expensive parsing outside the widget lifecycle and expose its current
result through a lightweight implementation.

## Structured and extensible blocks

`RichExtensionBlock` is the fallback representation for tool calls, diffs,
logs, operation results, and similar structured regions. It carries a renderer
key, title, summary, body, status, metadata, and initial expansion policy. The
default renderer exposes it as an expandable status region.

```rust,no_run
# use sui::prelude::*;
# let document = RichDocumentModel::new();
let mut call = RichExtensionBlock::new("tool-call", "Search files");
call.status = RichDocumentStatus::Running;
call.summary = Some("Searching workspace".into());
let id = document.append_extension(call);

let mut completed = RichExtensionBlock::new("tool-call", "Search files");
completed.status = RichDocumentStatus::Success;
completed.summary = Some("3 matches".into());
completed.body = "src/lib.rs\nsrc/state.rs\ntests/state.rs".into();
document.update_structured(id, RichDocumentBlockKind::Extension(completed));
```

Register a specialized retained widget when the fallback is insufficient:

```rust,no_run
# use sui::prelude::*;
# struct ToolCallBlock;
# impl ToolCallBlock {
#   fn new(_block: Signal<RichDocumentBlock>, _state: RichDocumentViewState) -> Self { Self }
# }
# impl Widget for ToolCallBlock {}
let renderers = RichDocumentRendererRegistry::new();
renderers.register("extension:tool-call", |context| {
    ToolCallBlock::new(context.block, context.state)
});

# let document = RichDocumentModel::new();
let view = RichDocumentView::new(document).renderer_registry(renderers);
# let _ = view;
```

The registry closure runs when a block identity first appears or its renderer
key changes. Payload changes update `context.block` without replacing the
widget. A custom renderer should call `ctx.observe(&block)` from `measure` (or
the corresponding `observe_with` method) to declare its invalidation needs.
Keep editor or disclosure state in the retained widget or the supplied
`RichDocumentViewState`, not in the renderer closure.

## Accessibility

The view emits a document root and semantic nodes for paragraphs, headings,
lists and list items, code, links, images, attachments, and status regions.
Task items expose checked state; running and pending extension blocks expose
busy state; expandable blocks expose Expand/Collapse; code exposes Copy.

These roles are mapped on desktop, web, and TUI surfaces. Custom renderers own
the semantics of their replacement widget and should keep the extension title,
status, actions, and expanded state discoverable.
