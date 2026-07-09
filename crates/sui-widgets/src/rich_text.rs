use std::ops::Range;

use sui_core::{
    Color, Event, KeyState, Point, PointerButton, PointerEventKind, Rect, SemanticsNode,
    SemanticsRole, SemanticsValue, Size,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, EventPhase, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_text::{
    PersistentTextLayout, TextCursor, TextDocument, TextLayoutRequest, TextParagraph,
    TextSelection, TextSpan, TextStyle,
};

use crate::{DefaultTheme, SelectionScope, TextCommand};

/// Maps byte ranges in rendered rich text back to the original source.
///
/// A span has a visible `display_range`, the source bytes that produced the
/// visible content (`content_source_range`), and a wider `source_range` that
/// may include invisible syntax such as Markdown emphasis markers or code
/// fences. Copying a complete visible span includes that surrounding syntax;
/// copying part of it maps into the content bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextSourceSpan {
    pub display_range: Range<usize>,
    pub source_range: Range<usize>,
    pub content_source_range: Range<usize>,
}

impl RichTextSourceSpan {
    pub fn new(
        display_range: Range<usize>,
        source_range: Range<usize>,
        content_source_range: Range<usize>,
    ) -> Self {
        Self {
            display_range,
            source_range,
            content_source_range,
        }
    }
}

/// Original source plus the mapping needed to copy source-preserving rich-text
/// selections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTextSourceMap {
    source: String,
    rendered: String,
    spans: Vec<RichTextSourceSpan>,
}

impl RichTextSourceMap {
    pub fn new(
        source: impl Into<String>,
        rendered: impl Into<String>,
        mut spans: Vec<RichTextSourceSpan>,
    ) -> Self {
        spans.sort_by_key(|span| (span.display_range.start, span.display_range.end));
        Self {
            source: source.into(),
            rendered: rendered.into(),
            spans,
        }
    }

    pub fn identity(text: impl Into<String>) -> Self {
        let text = text.into();
        let len = text.len();
        Self::new(
            text.clone(),
            text,
            vec![RichTextSourceSpan::new(0..len, 0..len, 0..len)],
        )
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn rendered(&self) -> &str {
        &self.rendered
    }

    pub fn spans(&self) -> &[RichTextSourceSpan] {
        &self.spans
    }

    /// Return the original source represented by a rendered byte range.
    pub fn copy_range(&self, range: Range<usize>) -> String {
        let range = normalized_range(range, self.rendered.len());
        if range.is_empty() {
            return String::new();
        }

        let Some(first) = self
            .spans
            .iter()
            .find(|span| ranges_overlap(&span.display_range, &range))
        else {
            return self.rendered.get(range).unwrap_or_default().to_string();
        };
        let last = self
            .spans
            .iter()
            .rev()
            .find(|span| ranges_overlap(&span.display_range, &range))
            .unwrap_or(first);

        let start = self.source_boundary(first, range.start, false);
        let end = self.source_boundary(last, range.end, true);
        self.source
            .get(start.min(end)..end.max(start))
            .unwrap_or_default()
            .to_string()
    }

    fn source_boundary(&self, span: &RichTextSourceSpan, offset: usize, end: bool) -> usize {
        let display = clamp_range(&span.display_range, self.rendered.len());
        let source = clamp_range(&span.source_range, self.source.len());
        let content = clamp_range(&span.content_source_range, self.source.len());
        if offset <= display.start {
            return if end { content.start } else { source.start };
        }
        if offset >= display.end {
            return if end { source.end } else { content.end };
        }

        let display_text = self.rendered.get(display.clone()).unwrap_or_default();
        let content_text = self.source.get(content.clone()).unwrap_or_default();
        let relative = offset.saturating_sub(display.start).min(display.len());
        let char_index = display_text
            .get(..relative)
            .map(str::chars)
            .map(Iterator::count)
            .unwrap_or(0);
        content.start + nth_char_boundary(content_text, char_index)
    }
}

fn normalized_range(range: Range<usize>, len: usize) -> Range<usize> {
    let start = range.start.min(len);
    let end = range.end.min(len);
    start.min(end)..start.max(end)
}

fn clamp_range(range: &Range<usize>, len: usize) -> Range<usize> {
    normalized_range(range.clone(), len)
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn nth_char_boundary(text: &str, index: usize) -> usize {
    text.char_indices()
        .map(|(offset, _)| offset)
        .nth(index)
        .unwrap_or(text.len())
}

pub struct RichText {
    document: TextDocument,
    document_reader: Option<Box<dyn Fn() -> TextDocument>>,
    semantic_name: Option<String>,
    padding: Insets,
    min_width: f32,
    min_height: f32,
    layout: Option<PersistentTextLayout>,
    selection_scope: Option<SelectionScope>,
    source_map: Option<RichTextSourceMap>,
    selection: TextSelection,
    selection_color: Color,
    dragging_selection: bool,
}

impl RichText {
    pub fn new(document: TextDocument) -> Self {
        Self {
            document,
            document_reader: None,
            semantic_name: None,
            padding: Insets::ZERO,
            min_width: 0.0,
            min_height: 0.0,
            layout: None,
            selection_scope: None,
            source_map: None,
            selection: TextSelection::new(TextCursor::default(), TextCursor::default()),
            selection_color: Color::rgba(0.18, 0.62, 0.86, 0.32),
            dragging_selection: false,
        }
    }

    pub fn dynamic<F>(fallback: TextDocument, reader: F) -> Self
    where
        F: Fn() -> TextDocument + 'static,
    {
        Self::new(fallback).document_when(reader)
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::from_plain_text(text, DefaultTheme::default().body_text_style())
    }

    pub fn from_plain_text(text: impl Into<String>, style: TextStyle) -> Self {
        Self::new(TextDocument::from_plain_text(text, style))
    }

    pub fn from_spans(spans: Vec<TextSpan>) -> Self {
        Self::new(TextDocument {
            paragraphs: vec![TextParagraph::from_spans(spans)],
        })
    }

    pub fn document(&self) -> &TextDocument {
        &self.document
    }

    pub fn set_document(&mut self, document: TextDocument) {
        self.document = document;
        self.document_reader = None;
        self.layout = None;
        self.source_map = None;
        self.selection = TextSelection::new(TextCursor::default(), TextCursor::default());
        self.dragging_selection = false;
    }

    pub fn document_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> TextDocument + 'static,
    {
        self.document_reader = Some(Box::new(reader));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width.max(0.0);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height.max(0.0);
        self
    }

    /// Enable character-level selection and publish it through `scope`.
    pub fn selectable(mut self, scope: SelectionScope) -> Self {
        self.selection_scope = Some(scope);
        self
    }

    /// Map copied rendered text back to an original source representation.
    pub fn source_map(mut self, source_map: RichTextSourceMap) -> Self {
        self.source_map = Some(source_map);
        self
    }

    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = color;
        self
    }

    fn current_document(&self) -> TextDocument {
        self.document_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.document.clone())
    }

    fn content_constraints(&self, constraints: Constraints) -> Constraints {
        Constraints::new(
            self.padding.inset(constraints.min),
            self.padding.inset(constraints.max),
        )
    }

    fn layout_request(
        &self,
        document: TextDocument,
        content_constraints: Constraints,
    ) -> TextLayoutRequest {
        let max_width = content_constraints.max.width;
        if max_width.is_finite() {
            TextLayoutRequest::new(document).with_box_size(Size::new(max_width.max(1.0), 1.0))
        } else {
            TextLayoutRequest::new(document)
        }
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - (self.padding.left + self.padding.right)).max(0.0),
            (bounds.height() - (self.padding.top + self.padding.bottom)).max(0.0),
        )
    }

    fn padded_size(&self, content_size: Size) -> Size {
        Size::new(
            (content_size.width + self.padding.left + self.padding.right).max(self.min_width),
            (content_size.height + self.padding.top + self.padding.bottom).max(self.min_height),
        )
    }

    fn text(&self) -> String {
        self.current_document().plain_text()
    }

    fn selection_range(&self, text_len: usize) -> Range<usize> {
        normalized_range(
            self.selection.anchor.utf8_offset..self.selection.focus.utf8_offset,
            text_len,
        )
    }

    fn selected_text(&self, text: &str) -> String {
        let range = self.selection_range(text.len());
        if range.is_empty() {
            return String::new();
        }
        self.source_map
            .as_ref()
            .map(|map| map.copy_range(range.clone()))
            .unwrap_or_else(|| text.get(range).unwrap_or_default().to_string())
    }

    fn layout_origin(&self, bounds: Rect) -> Option<Point> {
        let layout = self.layout.as_ref()?;
        let content = self.content_rect(bounds);
        let layout_bounds = layout.measurement().bounds;
        Some(Point::new(content.x() - layout_bounds.x(), content.y()))
    }

    fn point_to_cursor(&self, bounds: Rect, point: Point) -> Option<TextCursor> {
        let layout = self.layout.as_ref()?;
        let origin = self.layout_origin(bounds)?;
        Some(layout.hit_test_point(Point::new(point.x - origin.x, point.y - origin.y)))
    }

    fn publish_selection(&self, ctx: &mut EventCtx, text: &str) {
        let Some(scope) = &self.selection_scope else {
            return;
        };
        let owner = ctx.widget_id();
        let range = self.selection_range(text.len());
        let selected = self.selected_text(text);
        if range.is_empty() || selected.is_empty() {
            scope.clear_owner(owner);
        } else {
            scope.replace_text(owner, owner, range, text.len(), selected);
        }
        ctx.request_semantics();
    }

    fn set_selection(&mut self, ctx: &mut EventCtx, anchor: usize, focus: usize) {
        let text = self.text();
        let anchor = anchor.min(text.len());
        let focus = focus.min(text.len());
        let next = TextSelection::new(TextCursor::new(anchor), TextCursor::new(focus));
        if self.selection == next {
            return;
        }
        self.selection = next;
        self.publish_selection(ctx, &text);
        ctx.request_paint();
    }

    fn copy_selection(&self, ctx: &mut EventCtx) -> bool {
        let selected = self.selected_text(&self.text());
        if selected.is_empty() {
            return false;
        }
        ctx.set_clipboard_text(selected);
        true
    }
}

impl Default for RichText {
    fn default() -> Self {
        Self::plain("")
    }
}

impl Widget for RichText {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.selection_scope.is_none() {
            return;
        }
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                if let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position) {
                    let anchor = if pointer.modifiers.shift {
                        self.selection.anchor.utf8_offset
                    } else {
                        cursor.utf8_offset
                    };
                    self.set_selection(ctx, anchor, cursor.utf8_offset);
                    self.dragging_selection = true;
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self.dragging_selection
                    && ctx.phase() != EventPhase::Capture
                    && pointer.buttons.contains(PointerButton::Primary) =>
            {
                if let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position) {
                    self.set_selection(ctx, self.selection.anchor.utf8_offset, cursor.utf8_offset);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.dragging_selection =>
            {
                self.dragging_selection = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging_selection {
                    self.dragging_selection = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                ctx.request_focus();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let command = key.modifiers.control || key.modifiers.meta;
                match key.key.as_str() {
                    "a" | "A" if command => {
                        let len = self.text().len();
                        self.set_selection(ctx, 0, len);
                        ctx.set_handled();
                    }
                    "c" | "C" if command => {
                        if self.copy_selection(ctx) {
                            ctx.set_handled();
                        }
                    }
                    "Escape" => {
                        self.set_selection(ctx, 0, 0);
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            Event::Custom(custom) => {
                if let Some(command) = TextCommand::from_custom_event(custom) {
                    match command {
                        TextCommand::Copy => {
                            if self.copy_selection(ctx) {
                                ctx.set_handled();
                            }
                        }
                        TextCommand::SelectAll => {
                            let len = self.text().len();
                            self.set_selection(ctx, 0, len);
                            ctx.set_handled();
                        }
                        TextCommand::Cut | TextCommand::Paste => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let document = self.current_document();
        let content_constraints = self.content_constraints(constraints);
        let request = self.layout_request(document, content_constraints);
        let handle = self.layout.as_ref().map(|layout| layout.handle());
        self.layout = ctx
            .layout()
            .layout_document_persistent(handle, request)
            .ok();

        let content_size = self
            .layout
            .as_ref()
            .map(|layout| {
                let measurement = layout.measurement();
                Size::new(measurement.width.max(0.0), measurement.height.max(0.0))
            })
            .unwrap_or(Size::ZERO);

        constraints.clamp(self.padded_size(content_size))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(layout) = &self.layout else {
            return;
        };
        let Some(origin) = self.layout_origin(ctx.bounds()) else {
            return;
        };
        let text_len = layout.text().len();
        let selection = self.selection_range(text_len);
        if !selection.is_empty() {
            for rect in layout.selection_rects(selection) {
                ctx.fill_rect(
                    Rect::new(
                        origin.x + rect.x(),
                        origin.y + rect.y(),
                        rect.width(),
                        rect.height(),
                    ),
                    self.selection_color,
                );
            }
        }
        ctx.draw_persistent_text_layout(origin, layout);
    }

    fn accepts_focus(&self) -> bool {
        self.selection_scope.is_some()
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let text = self.current_document().plain_text();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or_else(|| text.clone()));
        if self.semantic_name.is_some() {
            node.value = Some(SemanticsValue::Text(text));
        }
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::{RichText, RichTextSourceMap, RichTextSourceSpan};
    use crate::SizedBox;
    use sui_core::{Color, SemanticsRole, SemanticsValue};
    use sui_runtime::{Application, RenderOutput, Widget, WindowBuilder};
    use sui_scene::SceneCommand;
    use sui_text::{FontStyle, FontWeight, TextDocument, TextParagraph, TextSpan, TextStyle};

    fn render<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Rich text").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).unwrap()
    }

    fn two_span_document() -> TextDocument {
        let mut strong = TextStyle::new(Color::rgba(0.84, 0.16, 0.18, 1.0));
        strong.weight = FontWeight::BOLD;
        let mut emphasis = TextStyle::new(Color::rgba(0.10, 0.38, 0.82, 1.0));
        emphasis.style = FontStyle::Italic;
        TextDocument {
            paragraphs: vec![TextParagraph::from_spans(vec![
                TextSpan::new("Warm", strong),
                TextSpan::new(" cool", emphasis),
            ])],
        }
    }

    #[test]
    fn rich_text_paints_document_spans_without_color_override() {
        let output = render(RichText::new(two_span_document()));
        let mut shaped = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command {
                shaped = Some(text.clone());
            }
        });
        let shaped = shaped.expect("rich text should emit shaped text");
        assert_eq!(shaped.color_override, None);

        let layout = shaped
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("rich text layout should resolve");
        assert_eq!(layout.text(), "Warm cool");
        assert_eq!(layout.runs().len(), 2);
        assert_eq!(layout.run_style(0).weight, FontWeight::BOLD);
        assert_eq!(layout.run_style(1).style, FontStyle::Italic);
        assert_ne!(layout.run_style(0).color, layout.run_style(1).color);
    }

    #[test]
    fn rich_text_exposes_plain_text_semantics_for_named_document() {
        let document = TextDocument {
            paragraphs: vec![
                TextParagraph::new("First", TextStyle::new(Color::WHITE)),
                TextParagraph::new("Second", TextStyle::new(Color::WHITE)),
            ],
        };
        let output = render(RichText::new(document).semantic_name("Summary"));
        let node = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text)
            .expect("rich text should expose text semantics");

        assert_eq!(node.name.as_deref(), Some("Summary"));
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("First\nSecond".to_string()))
        );
    }

    #[test]
    fn rich_text_wraps_to_parent_constraints() {
        let output = render(
            SizedBox::new()
                .width(96.0)
                .with_child(RichText::from_plain_text(
                    "alpha beta gamma delta epsilon",
                    TextStyle::new(Color::WHITE),
                )),
        );
        let mut line_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command
                && let Some(layout) = text.resolve(output.frame.text_layout_registry.as_ref())
            {
                line_count = layout.lines().len();
            }
        });

        assert!(line_count > 1, "expected constrained rich text to wrap");
    }

    #[test]
    fn source_map_copies_complete_markdown_syntax_and_partial_content() {
        let source = "Use **strong** and `code`.";
        let rendered = "Use strong and code.";
        let map = RichTextSourceMap::new(
            source,
            rendered,
            vec![
                RichTextSourceSpan::new(0..4, 0..4, 0..4),
                RichTextSourceSpan::new(4..10, 4..14, 6..12),
                RichTextSourceSpan::new(10..15, 14..19, 14..19),
                RichTextSourceSpan::new(15..19, 19..25, 20..24),
                RichTextSourceSpan::new(19..20, 25..26, 25..26),
            ],
        );

        assert_eq!(map.copy_range(4..10), "**strong**");
        assert_eq!(map.copy_range(6..9), "ron");
        assert_eq!(map.copy_range(15..19), "`code`");
        assert_eq!(map.copy_range(4..19), "**strong** and `code`");
    }
}
