use std::{cell::RefCell, rc::Rc};

use pulldown_cmark::{Event as MarkdownEvent, HeadingLevel, Options, Parser, Tag, TagEnd};
use sui::{
    Event as SuiEvent, EventPhase, KeyState, TextArea, WidgetPodMutVisitor, WidgetPodVisitor,
    prelude::*,
};

use crate::app::{DevThemeReader, dev_theme_color, request_window_refresh};

pub(crate) const MARKDOWN_RENDER_DEMO_NAME: &str = "Markdown render";
pub(crate) const MARKDOWN_RENDER_SCROLL_NAME: &str = "Markdown render demo";
pub(crate) const MARKDOWN_SOURCE_EDITOR_NAME: &str = "Markdown source";

pub(crate) const MARKDOWN_RENDER_COOLDOWN_SECONDS: f64 = 0.5;

const SAMPLE_MARKDOWN: &str = r#"# SUI rich text report

The markdown renderer is intentionally small: it translates markdown events into a `TextDocument`,
then the `RichText` widget lays out and paints the styled spans.

## What the document uses

- headings with independent size and weight
- inline **strong**, _emphasis_, and `code` spans
- links such as [SUI workspace](https://example.invalid/sui) with accent color
- ordered list markers that become ordinary rich text spans
- Unicode fallback text: 你好, 日本語, 한국어, 🙂 ✅ 🎨

1. Parse markdown events.
2. Build paragraphs and spans.
3. Render the document through `RichText`.

## Source ownership

Markdown stays in the demo crate behind a feature flag. The reusable SUI layer only needs
`TextDocument`, `TextParagraph`, and `TextSpan`, so applications can supply their own parser,
syntax highlighter, or document model.
"#;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Paragraph,
    Heading(HeadingLevel),
    Item,
    CodeBlock,
}

#[derive(Clone, Default)]
struct InlineState {
    strong: usize,
    emphasis: usize,
    strikethrough: usize,
    code: usize,
    link: usize,
}

#[derive(Clone)]
struct MarkdownStyles {
    body: TextStyle,
    muted: TextStyle,
    strong: TextStyle,
    emphasis: TextStyle,
    code: TextStyle,
    link: TextStyle,
    marker: TextStyle,
    headings: [TextStyle; 6],
    gap: TextStyle,
}

impl MarkdownStyles {
    fn new(theme: DefaultTheme) -> Self {
        let mut body = theme.body_text_style();
        body.font_size = 14.0;
        body.line_height = 21.0;

        let mut muted = body.clone();
        muted.color = theme.palette.text_muted;

        let mut strong = body.clone();
        strong.weight = FontWeight::BOLD;

        let mut emphasis = body.clone();
        emphasis.style = FontStyle::Italic;

        let mut code = body.clone();
        code.color = theme.palette.warning;
        code.weight = FontWeight::SEMIBOLD;
        code.features.enable(FontFeature::TABULAR_FIGURES);

        let mut link = body.clone();
        link.color = theme.palette.accent;
        link.weight = FontWeight::SEMIBOLD;

        let mut marker = body.clone();
        marker.color = theme.palette.accent;
        marker.weight = FontWeight::SEMIBOLD;

        let mut heading1 = body.clone();
        heading1.font_size = 28.0;
        heading1.line_height = 34.0;
        heading1.weight = FontWeight::BOLD;

        let mut heading2 = body.clone();
        heading2.font_size = 21.0;
        heading2.line_height = 27.0;
        heading2.weight = FontWeight::BOLD;

        let mut heading3 = body.clone();
        heading3.font_size = 18.0;
        heading3.line_height = 24.0;
        heading3.weight = FontWeight::SEMIBOLD;

        let mut heading4 = body.clone();
        heading4.font_size = 16.0;
        heading4.line_height = 22.0;
        heading4.weight = FontWeight::SEMIBOLD;

        let heading5 = heading4.clone();
        let heading6 = heading4.clone();

        let mut gap = body.clone();
        gap.font_size = 1.0;
        gap.line_height = 8.0;
        gap.color = Color::TRANSPARENT;

        Self {
            body,
            muted,
            strong,
            emphasis,
            code,
            link,
            marker,
            headings: [heading1, heading2, heading3, heading4, heading5, heading6],
            gap,
        }
    }

    fn block_style(&self, kind: BlockKind) -> TextStyle {
        match kind {
            BlockKind::Heading(level) => self.headings[heading_index(level)].clone(),
            BlockKind::CodeBlock => self.code.clone(),
            BlockKind::Paragraph | BlockKind::Item => self.body.clone(),
        }
    }

    fn inline_style(&self, kind: BlockKind, inline: &InlineState) -> TextStyle {
        if inline.code > 0 || kind == BlockKind::CodeBlock {
            return self.code.clone();
        }
        let mut style = if inline.link > 0 {
            self.link.clone()
        } else if inline.strong > 0 {
            self.strong.clone()
        } else if inline.emphasis > 0 {
            self.emphasis.clone()
        } else if inline.strikethrough > 0 {
            self.muted.clone()
        } else {
            self.block_style(kind)
        };
        if inline.strong > 0 {
            style.weight = FontWeight::BOLD;
        }
        if inline.emphasis > 0 {
            style.style = FontStyle::Italic;
        }
        if inline.strikethrough > 0 {
            style.color = self.muted.color;
        }
        style
    }
}

#[derive(Clone, Copy)]
struct ListState {
    next: Option<u64>,
}

struct MarkdownDocumentBuilder {
    styles: MarkdownStyles,
    paragraphs: Vec<TextParagraph>,
    current_kind: Option<BlockKind>,
    current_spans: Vec<TextSpan>,
    inline: InlineState,
    lists: Vec<ListState>,
}

#[derive(Clone)]
struct MarkdownDemoState {
    inner: Rc<RefCell<MarkdownDemoStateInner>>,
}

struct MarkdownDemoStateInner {
    source: String,
    rendered_document: TextDocument,
    dirty: bool,
    cooling_down: bool,
    cooldown_timer: Option<TimerToken>,
}

impl MarkdownDemoState {
    fn new(theme: DefaultTheme) -> Self {
        Self {
            inner: Rc::new(RefCell::new(MarkdownDemoStateInner {
                source: SAMPLE_MARKDOWN.to_string(),
                rendered_document: markdown_to_document(SAMPLE_MARKDOWN, theme),
                dirty: false,
                cooling_down: false,
                cooldown_timer: None,
            })),
        }
    }

    fn source(&self) -> String {
        self.inner.borrow().source.clone()
    }

    fn rendered_document(&self) -> TextDocument {
        self.inner.borrow().rendered_document.clone()
    }

    #[cfg(test)]
    fn is_dirty(&self) -> bool {
        self.inner.borrow().dirty
    }

    #[cfg(test)]
    fn set_source(&self, source: String) {
        let mut inner = self.inner.borrow_mut();
        if inner.source != source {
            inner.source = source;
            inner.dirty = true;
        }
    }

    #[cfg(test)]
    fn apply_pending_render(&self, theme: DefaultTheme) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !inner.dirty {
            return false;
        }
        let source = inner.source.clone();
        inner.rendered_document = markdown_to_document(&source, theme);
        inner.dirty = false;
        true
    }

    fn arm_cooldown_timer(&self, ctx: &mut EventCtx) {
        let mut inner = self.inner.borrow_mut();
        if inner.cooldown_timer.is_none() {
            inner.cooldown_timer = Some(ctx.schedule_timer_after(MARKDOWN_RENDER_COOLDOWN_SECONDS));
        }
    }

    fn set_source_throttled(&self, ctx: &mut EventCtx, source: String, theme: DefaultTheme) {
        let mut should_refresh = false;
        {
            let mut inner = self.inner.borrow_mut();
            if inner.source == source {
                return;
            }

            inner.source = source;
            if inner.cooling_down {
                inner.dirty = true;
            } else {
                let source = inner.source.clone();
                inner.rendered_document = markdown_to_document(&source, theme);
                inner.dirty = false;
                inner.cooling_down = inner.cooldown_timer.is_some();
                should_refresh = true;
            }
        }

        if should_refresh {
            request_window_refresh(ctx, false);
        }
    }

    fn handle_cooldown_timer(&self, ctx: &mut EventCtx, token: TimerToken, theme: DefaultTheme) {
        let mut should_refresh = false;
        {
            let mut inner = self.inner.borrow_mut();
            if inner.cooldown_timer != Some(token) {
                return;
            }

            inner.cooldown_timer = None;
            if inner.dirty {
                let source = inner.source.clone();
                inner.rendered_document = markdown_to_document(&source, theme);
                inner.dirty = false;
                inner.cooling_down = true;
                inner.cooldown_timer =
                    Some(ctx.schedule_timer_after(MARKDOWN_RENDER_COOLDOWN_SECONDS));
                should_refresh = true;
            } else {
                inner.cooling_down = false;
            }
        }

        if should_refresh {
            request_window_refresh(ctx, false);
        }
        ctx.set_handled();
    }
}

struct MarkdownSourceEditor {
    state: MarkdownDemoState,
    theme_reader: DevThemeReader,
    editor: SingleChild,
}

impl MarkdownSourceEditor {
    fn new(state: MarkdownDemoState, theme_reader: DevThemeReader) -> Self {
        let editor_state = state.clone();
        let editor_theme_reader = Rc::clone(&theme_reader);
        let source_style = source_text_style(theme_reader());
        let editor = TextArea::new(MARKDOWN_SOURCE_EDITOR_NAME)
            .value(state.source())
            .text_style(source_style)
            .padding(Insets::all(12.0))
            .min_height(360.0)
            .on_change_with_ctx(move |ctx, value| {
                editor_state.set_source_throttled(ctx, value, editor_theme_reader());
            });

        Self {
            state,
            theme_reader,
            editor: SingleChild::new(editor),
        }
    }

    fn event_may_edit_source(event: &SuiEvent) -> bool {
        matches!(
            event,
            SuiEvent::Keyboard(key) if key.state == KeyState::Pressed
        ) || matches!(event, SuiEvent::Ime(_))
    }
}

impl Widget for MarkdownSourceEditor {
    fn event(&mut self, ctx: &mut EventCtx, event: &SuiEvent) {
        if let SuiEvent::Wake(WakeEvent::Timer { token, .. }) = event {
            self.state
                .handle_cooldown_timer(ctx, *token, (self.theme_reader)());
        } else if ctx.phase() == EventPhase::Capture && Self::event_may_edit_source(event) {
            self.state.arm_cooldown_timer(ctx);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.editor.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.editor.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.editor.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.editor.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.editor.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.editor.visit_children_mut(visitor);
    }
}

impl MarkdownDocumentBuilder {
    fn new(theme: DefaultTheme) -> Self {
        Self {
            styles: MarkdownStyles::new(theme),
            paragraphs: Vec::new(),
            current_kind: None,
            current_spans: Vec::new(),
            inline: InlineState::default(),
            lists: Vec::new(),
        }
    }

    fn start_block(&mut self, kind: BlockKind) {
        self.finish_current(true);
        self.current_kind = Some(kind);
    }

    fn start_item(&mut self) {
        self.finish_current(true);
        self.current_kind = Some(BlockKind::Item);
        let prefix = self.next_list_prefix();
        self.current_spans
            .push(TextSpan::new(prefix, self.styles.marker.clone()));
    }

    fn finish_current(&mut self, include_gap: bool) {
        let Some(kind) = self.current_kind.take() else {
            return;
        };
        if self.current_spans.is_empty() && kind != BlockKind::CodeBlock {
            return;
        }
        if self.current_spans.is_empty() {
            self.current_spans
                .push(TextSpan::new("", self.styles.code.clone()));
        }
        self.paragraphs.push(TextParagraph {
            style: TextParagraphStyle::default(),
            spans: std::mem::take(&mut self.current_spans),
        });
        if include_gap {
            self.paragraphs
                .push(TextParagraph::new("", self.gap_style(kind)));
        }
    }

    fn gap_style(&self, kind: BlockKind) -> TextStyle {
        let mut gap = self.styles.gap.clone();
        gap.line_height = match kind {
            BlockKind::Heading(HeadingLevel::H1) => 12.0,
            BlockKind::Heading(_) => 9.0,
            BlockKind::CodeBlock => 4.0,
            BlockKind::Item => 3.0,
            BlockKind::Paragraph => 8.0,
        };
        gap
    }

    fn current_kind(&self) -> BlockKind {
        self.current_kind.unwrap_or(BlockKind::Paragraph)
    }

    fn add_text(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            return;
        }
        if self.current_kind.is_none() {
            self.start_block(BlockKind::Paragraph);
        }
        let style = self.styles.inline_style(self.current_kind(), &self.inline);
        self.current_spans.push(TextSpan::new(text, style));
    }

    fn add_code_block_text(&mut self, text: &str) {
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                self.finish_current(false);
                self.current_kind = Some(BlockKind::CodeBlock);
            }
            self.add_text(line);
        }
    }

    fn next_list_prefix(&mut self) -> String {
        let indent = "  ".repeat(self.lists.len().saturating_sub(1));
        match self.lists.last_mut().and_then(|list| list.next.as_mut()) {
            Some(next) => {
                let current = *next;
                *next += 1;
                format!("{indent}{current}. ")
            }
            None => format!("{indent}- "),
        }
    }

    fn finish(mut self) -> TextDocument {
        self.finish_current(false);
        if self.paragraphs.is_empty() {
            self.paragraphs
                .push(TextParagraph::new("", self.styles.body.clone()));
        }
        TextDocument {
            paragraphs: self.paragraphs,
        }
    }
}

pub(crate) fn markdown_to_document(markdown: &str, theme: DefaultTheme) -> TextDocument {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let mut builder = MarkdownDocumentBuilder::new(theme);

    for event in Parser::new_ext(markdown, options) {
        match event {
            MarkdownEvent::Start(Tag::Paragraph) => {
                if builder.current_kind.is_none() {
                    builder.start_block(BlockKind::Paragraph);
                }
            }
            MarkdownEvent::End(TagEnd::Paragraph) => {
                if !matches!(builder.current_kind, Some(BlockKind::Item)) {
                    builder.finish_current(true);
                }
            }
            MarkdownEvent::Start(Tag::Heading { level, .. }) => {
                builder.start_block(BlockKind::Heading(level));
            }
            MarkdownEvent::End(TagEnd::Heading(_)) => builder.finish_current(true),
            MarkdownEvent::Start(Tag::List(start)) => builder.lists.push(ListState { next: start }),
            MarkdownEvent::End(TagEnd::List(_)) => {
                builder.finish_current(true);
                builder.lists.pop();
            }
            MarkdownEvent::Start(Tag::Item) => builder.start_item(),
            MarkdownEvent::End(TagEnd::Item) => builder.finish_current(true),
            MarkdownEvent::Start(Tag::CodeBlock(_)) => builder.start_block(BlockKind::CodeBlock),
            MarkdownEvent::End(TagEnd::CodeBlock) => builder.finish_current(true),
            MarkdownEvent::Start(Tag::Emphasis) => builder.inline.emphasis += 1,
            MarkdownEvent::End(TagEnd::Emphasis) => {
                builder.inline.emphasis = builder.inline.emphasis.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Strong) => builder.inline.strong += 1,
            MarkdownEvent::End(TagEnd::Strong) => {
                builder.inline.strong = builder.inline.strong.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Strikethrough) => builder.inline.strikethrough += 1,
            MarkdownEvent::End(TagEnd::Strikethrough) => {
                builder.inline.strikethrough = builder.inline.strikethrough.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Link { .. }) => builder.inline.link += 1,
            MarkdownEvent::End(TagEnd::Link) => {
                builder.inline.link = builder.inline.link.saturating_sub(1);
            }
            MarkdownEvent::Text(text) => {
                if builder.current_kind == Some(BlockKind::CodeBlock) {
                    builder.add_code_block_text(&text);
                } else {
                    builder.add_text(&text);
                }
            }
            MarkdownEvent::Code(code) => {
                builder.inline.code += 1;
                builder.add_text(&code);
                builder.inline.code = builder.inline.code.saturating_sub(1);
            }
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => builder.add_text(" "),
            MarkdownEvent::Rule => {
                builder.start_block(BlockKind::Paragraph);
                builder.add_text("-----");
                builder.finish_current(true);
            }
            _ => {}
        }
    }

    builder.finish()
}

pub(crate) fn build_markdown_render_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let state = MarkdownDemoState::new(theme_reader());
    let rendered_state = state.clone();
    let rendered = RichText::dynamic(state.rendered_document(), move || {
        rendered_state.rendered_document()
    })
    .semantic_name(MARKDOWN_RENDER_DEMO_NAME)
    .padding(Insets::all(16.0))
    .min_height(360.0);
    let source = MarkdownSourceEditor::new(state, Rc::clone(&theme_reader));

    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            18.0,
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(MARKDOWN_RENDER_DEMO_NAME)
                        .font_size(22.0)
                        .line_height(28.0)
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                )
                .with_child(
                    Flex::horizontal()
                        .gap(16.0)
                        .wrap(FlexWrap::Wrap)
                        .align_items(Alignment::Stretch)
                        .with_item(
                            markdown_panel("Source", source, Rc::clone(&theme_reader)),
                            FlexItem::new()
                                .basis_gap_aware_fraction(0.44)
                                .min_width(320.0),
                        )
                        .with_item(
                            markdown_panel("Rendered", rendered, Rc::clone(&theme_reader)),
                            FlexItem::new()
                                .basis_gap_aware_fraction(0.56)
                                .min_width(380.0),
                        ),
                ),
        ))
        .name(MARKDOWN_RENDER_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn markdown_panel<W>(title: &'static str, child: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Background::new(
        theme_reader().palette.surface_raised,
        Padding::all(
            12.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .font_size(13.0)
                        .line_height(18.0)
                        .color_when(dev_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                )
                .with_child(child),
        ),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface_raised
    }))
}

fn heading_index(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 0,
        HeadingLevel::H2 => 1,
        HeadingLevel::H3 => 2,
        HeadingLevel::H4 => 3,
        HeadingLevel::H5 => 4,
        HeadingLevel::H6 => 5,
    }
}

fn source_text_style(theme: DefaultTheme) -> TextStyle {
    let mut style = theme.body_text_style();
    style.font_size = 13.0;
    style.line_height = 18.0;
    style.color = theme.palette.text_muted;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_to_document_styles_headings_and_inline_spans() {
        let document = markdown_to_document(
            "# Title\n\nBody with **strong**, _emphasis_, `code`, and [link](https://example.invalid).",
            DefaultTheme::default(),
        );

        assert!(document.paragraphs.len() >= 3);
        assert_eq!(document.paragraphs[0].text(), "Title");
        assert!(
            document.paragraphs[0].spans[0].style.font_size
                > document.paragraphs[2].spans[0].style.font_size
        );
        let body_spans = &document.paragraphs[2].spans;
        assert!(
            body_spans
                .iter()
                .any(|span| span.text == "strong" && span.style.weight == FontWeight::BOLD)
        );
        assert!(
            body_spans
                .iter()
                .any(|span| span.text == "emphasis" && span.style.style == FontStyle::Italic)
        );
        assert!(
            body_spans
                .iter()
                .any(|span| span.text == "code" && span.style.weight == FontWeight::SEMIBOLD)
        );
        assert!(body_spans.iter().any(|span| span.text == "link"
            && span.style.color == DefaultTheme::default().palette.accent));
    }

    #[test]
    fn markdown_to_document_preserves_ordered_list_markers() {
        let document = markdown_to_document("1. Parse\n2. Render", DefaultTheme::default());
        let text = document.plain_text();

        assert!(text.contains("1. Parse"));
        assert!(text.contains("2. Render"));
    }

    #[test]
    fn markdown_state_holds_preview_until_dirty_render_applies() {
        let state = MarkdownDemoState::new(DefaultTheme::default());
        state.set_source("# First edit".to_string());
        state.set_source("# Final edit".to_string());

        assert!(state.is_dirty());
        assert!(
            state
                .rendered_document()
                .plain_text()
                .contains("SUI rich text report")
        );

        assert!(state.apply_pending_render(DefaultTheme::default()));
        assert!(!state.is_dirty());
        assert_eq!(state.rendered_document().paragraphs[0].text(), "Final edit");
        assert!(!state.apply_pending_render(DefaultTheme::default()));
    }
}
