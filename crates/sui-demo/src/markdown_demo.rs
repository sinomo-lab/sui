use std::{cell::RefCell, rc::Rc};

use sui::{
    Event as SuiEvent, EventPhase, KeyState, SemanticRegion, WidgetPodMutVisitor, WidgetPodVisitor,
    prelude::*,
};

use crate::app::{DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color};

pub(crate) const MARKDOWN_RENDER_DEMO_NAME: &str = "Rich document preview";
pub(crate) const MARKDOWN_RENDER_SCROLL_NAME: &str = "Rich document demo";
#[cfg(test)]
pub(crate) const MARKDOWN_RENDER_SCROLL_BAR_NAME: &str = "Rich document demo vertical scroll bar";
pub(crate) const MARKDOWN_SOURCE_EDITOR_NAME: &str = "Markdown source";
pub(crate) const MARKDOWN_RENDER_COOLDOWN_SECONDS: f64 = 0.5;

const MARKDOWN_PANEL_MIN_WIDTH: f32 = 320.0;
const MARKDOWN_PANEL_GAP: f32 = 16.0;

const SAMPLE_MARKDOWN: &str = r##"# SUI rich document report

`RichDocumentModel` keeps this document incremental while `RichDocumentView` retains keyed block widgets, cached layouts, and selection across the complete document.

## Production primitives

- [x] selectable text across headings, paragraphs, lists, and code
- [x] links such as the [SUI documentation](https://github.com/sinomo-lab/sui)
- [x] syntax-highlighted code with horizontal scrolling and copy actions
- [x] inline images, attachments, and expandable structured results
- [x] semantic headings, lists, code, links, attachments, and status regions

> Edit this source to exercise reconciled block identity. Streaming producers use `append_markdown` to reparse only the mutable tail.

```rust
let document = RichDocumentModel::from_markdown("# Streaming");
document.append_markdown("\n\nFirst retained block");
document.append_markdown("\n\nSecond incremental block");
```

![Renderer-neutral chart](asset:release-chart)
"##;

#[derive(Clone)]
struct MarkdownDemoState {
    inner: Rc<RefCell<MarkdownDemoStateInner>>,
}

struct MarkdownDemoStateInner {
    source: String,
    rendered_source: String,
    document: RichDocumentModel,
    dirty: bool,
    cooling_down: bool,
    cooldown_timer: Option<TimerToken>,
}

impl MarkdownDemoState {
    fn new() -> Self {
        let document = RichDocumentModel::from_markdown(SAMPLE_MARKDOWN);

        let mut attachment = RichAttachment::new("release-notes.md");
        attachment.media_type = Some("text/markdown".into());
        attachment.size_bytes = Some(4_812);
        attachment.description =
            Some("Portable attachment metadata with an application action".into());
        document.append_attachment(attachment);

        let mut operation = RichExtensionBlock::new("operation-log", "0.2.0 release checks");
        operation.status = RichDocumentStatus::Running;
        operation.summary = Some("Documentation and demo audit in progress".into());
        operation.body =
            "docs links       ready\nwidget gallery   ready\nrelease package  pending".into();
        operation.initially_expanded = true;
        operation.metadata = vec![
            ("scope".into(), "workspace".into()),
            ("renderer".into(), "fallback extension block".into()),
        ];
        document.append_extension(operation);

        Self {
            inner: Rc::new(RefCell::new(MarkdownDemoStateInner {
                source: SAMPLE_MARKDOWN.to_string(),
                rendered_source: SAMPLE_MARKDOWN.to_string(),
                document,
                dirty: false,
                cooling_down: false,
                cooldown_timer: None,
            })),
        }
    }

    fn source(&self) -> String {
        self.inner.borrow().source.clone()
    }

    fn document(&self) -> RichDocumentModel {
        self.inner.borrow().document.clone()
    }

    fn replace_rendered(inner: &mut MarkdownDemoStateInner, source: String) {
        inner.document.set_markdown(source.clone());
        inner.rendered_source = source;
    }

    #[cfg(test)]
    fn rendered_snapshot(&self) -> RichDocumentSnapshot {
        self.inner.borrow().document.snapshot()
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
    fn apply_pending_render(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !inner.dirty {
            return false;
        }
        let source = inner.source.clone();
        Self::replace_rendered(&mut inner, source);
        inner.dirty = false;
        true
    }

    fn arm_cooldown_timer(&self, ctx: &mut EventCtx) {
        let mut inner = self.inner.borrow_mut();
        if inner.cooldown_timer.is_none() {
            inner.cooldown_timer = Some(ctx.schedule_timer_after(MARKDOWN_RENDER_COOLDOWN_SECONDS));
        }
    }

    fn set_source_throttled(&self, source: String) {
        let mut inner = self.inner.borrow_mut();
        if inner.source == source {
            return;
        }

        inner.source = source;
        if inner.cooling_down {
            inner.dirty = true;
        } else {
            let source = inner.source.clone();
            Self::replace_rendered(&mut inner, source);
            inner.dirty = false;
            inner.cooling_down = inner.cooldown_timer.is_some();
        }
    }

    fn handle_cooldown_timer(&self, ctx: &mut EventCtx, token: TimerToken) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.cooldown_timer != Some(token) {
                return;
            }

            inner.cooldown_timer = None;
            if inner.dirty {
                let source = inner.source.clone();
                Self::replace_rendered(&mut inner, source);
                inner.dirty = false;
                inner.cooling_down = true;
                inner.cooldown_timer =
                    Some(ctx.schedule_timer_after(MARKDOWN_RENDER_COOLDOWN_SECONDS));
            } else {
                inner.cooling_down = false;
            }
        }

        ctx.set_handled();
    }
}

struct MarkdownSourceEditor {
    state: MarkdownDemoState,
    editor: SingleChild,
}

impl MarkdownSourceEditor {
    fn new(state: MarkdownDemoState, theme_reader: DevThemeReader) -> Self {
        let editor_state = state.clone();
        let editor = TextArea::new(MARKDOWN_SOURCE_EDITOR_NAME)
            .value(state.source())
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .padding(Insets::all(12.0))
            .min_height(440.0)
            .on_change(move |value| {
                editor_state.set_source_throttled(value);
            });

        Self {
            state,
            editor: SingleChild::new(editor),
        }
    }

    fn event_may_edit_source(event: &SuiEvent) -> bool {
        matches!(event, SuiEvent::Keyboard(key) if key.state == KeyState::Pressed)
            || matches!(event, SuiEvent::Ime(_))
    }
}

impl Widget for MarkdownSourceEditor {
    fn event(&mut self, ctx: &mut EventCtx, event: &SuiEvent) {
        if let SuiEvent::Wake(WakeEvent::Timer { token, .. }) = event {
            self.state.handle_cooldown_timer(ctx, *token);
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

pub(crate) fn build_markdown_render_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let state = MarkdownDemoState::new();
    let document = state.document();
    let view_state = RichDocumentViewState::new();
    let activity = Signal::named(
        "Rich document demo activity",
        "Select across blocks, activate a link, copy code, or open a structured result."
            .to_string(),
    );

    let view_theme_reader = Rc::clone(&theme_reader);
    let view_document = document.clone();
    let retained_view_state = view_state.clone();
    let view_activity = activity.clone();
    let rendered = RebuildOnChange::new(
        move || view_theme_reader(),
        move |theme| {
            let link_activity = view_activity.clone();
            let image_activity = view_activity.clone();
            let attachment_activity = view_activity.clone();
            WidgetPod::new(
                RichDocumentView::new(view_document.clone())
                    .state(retained_view_state.clone())
                    .theme(*theme)
                    .on_link(move |destination| {
                        link_activity.set(format!("Link routed to the application: {destination}"));
                    })
                    .on_image(move |source| {
                        image_activity.set(format!("Image action requested for {source}"));
                    })
                    .on_attachment(move |_| {
                        attachment_activity
                            .set("Attachment action routed to the application".to_string());
                    }),
            )
        },
    );
    let rendered = SemanticRegion::new(MARKDOWN_RENDER_DEMO_NAME, rendered).description(
        "Selectable incremental Markdown with code, attachment, image, and status semantics",
    );
    let source = MarkdownSourceEditor::new(state, Rc::clone(&theme_reader));

    let scroll = ScrollView::vertical(Padding::all(
        18.0,
        Stack::vertical()
            .spacing(14.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new("Rich documents")
                    .style(dev_text_style(
                        theme_reader(),
                        theme_reader().text._2xl,
                        theme_reader().palette.text,
                    ))
                    .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
            )
            .with_child(
                Label::new("")
                    .text_from(activity)
                    .color_when(dev_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
            )
            .with_child(MarkdownPanelSplit::new(
                markdown_panel("Markdown source", source, Rc::clone(&theme_reader)),
                markdown_panel("RichDocumentView", rendered, Rc::clone(&theme_reader)),
            )),
    ))
    .name(MARKDOWN_RENDER_SCROLL_NAME)
    .theme_when(clone_dev_theme_reader(&theme_reader));

    Background::new(theme_reader().palette.surface, scroll)
        .brush_when(dev_theme_color(&theme_reader, |theme| {
            theme.palette.surface
        }))
}

struct MarkdownPanelSplit {
    source: SingleChild,
    rendered: SingleChild,
}

impl MarkdownPanelSplit {
    fn new<Source, Rendered>(source: Source, rendered: Rendered) -> Self
    where
        Source: Widget + 'static,
        Rendered: Widget + 'static,
    {
        Self {
            source: SingleChild::new(source),
            rendered: SingleChild::new(rendered),
        }
    }

    fn available_width(constraints: Constraints) -> f32 {
        if constraints.max.width.is_finite() {
            constraints.max.width.max(0.0)
        } else if constraints.min.width.is_finite() && constraints.min.width > 0.0 {
            constraints.min.width
        } else {
            MARKDOWN_PANEL_MIN_WIDTH * 2.0 + MARKDOWN_PANEL_GAP
        }
    }

    fn split_width(total_width: f32) -> f32 {
        ((total_width - MARKDOWN_PANEL_GAP).max(0.0) * 0.5).max(MARKDOWN_PANEL_MIN_WIDTH)
    }

    fn wraps(total_width: f32) -> bool {
        total_width < MARKDOWN_PANEL_MIN_WIDTH * 2.0 + MARKDOWN_PANEL_GAP
    }

    fn panel_constraints(width: f32, max_height: f32) -> Constraints {
        Constraints::new(Size::new(width, 0.0), Size::new(width, max_height.max(0.0)))
    }
}

impl Widget for MarkdownPanelSplit {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let total_width = Self::available_width(constraints);
        let max_height = constraints.max.height;

        let size = if Self::wraps(total_width) {
            let panel_constraints = Self::panel_constraints(total_width, max_height);
            let source_size = self.source.measure(ctx, panel_constraints);
            let rendered_size = self.rendered.measure(ctx, panel_constraints);
            Size::new(
                total_width,
                source_size.height + MARKDOWN_PANEL_GAP + rendered_size.height,
            )
        } else {
            let panel_width = Self::split_width(total_width);
            let panel_constraints = Self::panel_constraints(panel_width, max_height);
            let source_size = self.source.measure(ctx, panel_constraints);
            let rendered_size = self.rendered.measure(ctx, panel_constraints);
            Size::new(total_width, source_size.height.max(rendered_size.height))
        };

        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if Self::wraps(bounds.width()) {
            let source_size = self.source.child().measured_size();
            let rendered_size = self.rendered.child().measured_size();
            self.source.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), bounds.width(), source_size.height),
            );
            self.rendered.arrange(
                ctx,
                Rect::new(
                    bounds.x(),
                    bounds.y() + source_size.height + MARKDOWN_PANEL_GAP,
                    bounds.width(),
                    rendered_size.height,
                ),
            );
        } else {
            let panel_width = ((bounds.width() - MARKDOWN_PANEL_GAP).max(0.0) * 0.5).max(0.0);
            let height = self
                .source
                .child()
                .measured_size()
                .height
                .max(self.rendered.child().measured_size().height)
                .min(bounds.height());
            self.source
                .arrange(ctx, Rect::new(bounds.x(), bounds.y(), panel_width, height));
            self.rendered.arrange(
                ctx,
                Rect::new(
                    bounds.x() + panel_width + MARKDOWN_PANEL_GAP,
                    bounds.y(),
                    panel_width,
                    height,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.source.paint(ctx);
        self.rendered.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.source.semantics(ctx);
        self.rendered.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.source.visit_children(visitor);
        self.rendered.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.source.visit_children_mut(visitor);
        self.rendered.visit_children_mut(visitor);
    }
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
                        .style(dev_text_style(
                            theme_reader(),
                            theme_reader().text.sm,
                            theme_reader().palette.text_muted,
                        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sui::{RenderOutput, SemanticsRole};
    use sui_scene::SceneCommand;

    fn render_markdown_demo(width: f32) -> RenderOutput {
        render_markdown_demo_with_theme(width, DefaultTheme::default())
    }

    fn render_markdown_demo_with_theme(width: f32, theme: DefaultTheme) -> RenderOutput {
        render_markdown_demo_with_size(width, 1100.0, theme)
    }

    fn render_markdown_demo_with_size(
        width: f32,
        height: f32,
        theme: DefaultTheme,
    ) -> RenderOutput {
        let theme_reader: DevThemeReader = Rc::new(move || theme);
        let root = SizedBox::new()
            .width(width)
            .height(height)
            .with_child(build_markdown_render_demo_with_theme(theme_reader));
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Rich document layout")
                    .root(root),
            )
            .build()
            .expect("rich document demo runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("rich document demo should render")
    }

    fn source_editor_bounds(output: &RenderOutput) -> Rect {
        output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(MARKDOWN_SOURCE_EDITOR_NAME)
            })
            .expect("source editor semantics present")
            .bounds
    }

    fn rendered_document_bounds(output: &RenderOutput) -> Rect {
        output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(MARKDOWN_RENDER_DEMO_NAME)
            })
            .expect("rich document region semantics present")
            .bounds
    }

    fn rendered_text_layout_width(output: &RenderOutput) -> f32 {
        let rendered = rendered_document_bounds(output);
        let mut width = None;

        output.frame.scene.visit_commands(&mut |command| {
            if width.is_some() {
                return;
            }
            let SceneCommand::DrawShapedText(text) = command else {
                return;
            };
            if text.origin.x < rendered.x() - 1.0
                || text.origin.y < rendered.y() - 1.0
                || text.origin.y > rendered.max_y() + 1.0
            {
                return;
            }
            let Some(layout) = text.resolve(output.frame.text_layout_registry.as_ref()) else {
                return;
            };
            if layout.text().contains("keeps this document incremental") {
                width = Some(layout.measurement().width);
            }
        });

        width.expect("rich document text layout should be present")
    }

    fn snapshot_text(snapshot: &RichDocumentSnapshot) -> String {
        snapshot
            .blocks
            .iter()
            .map(RichDocumentBlock::plain_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rich_document_demo_uses_markdown_and_structured_blocks() {
        let snapshot = MarkdownDemoState::new().rendered_snapshot();

        assert!(
            snapshot
                .blocks
                .iter()
                .any(|block| matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. }))
        );
        assert!(
            snapshot
                .blocks
                .iter()
                .any(|block| matches!(block.kind, RichDocumentBlockKind::Attachment(_)))
        );
        assert!(
            snapshot
                .blocks
                .iter()
                .any(|block| matches!(block.kind, RichDocumentBlockKind::Extension(_)))
        );
    }

    #[test]
    fn markdown_state_holds_preview_until_dirty_render_applies() {
        let state = MarkdownDemoState::new();
        state.set_source("# First edit".to_string());
        state.set_source("# Final edit".to_string());

        assert!(state.is_dirty());
        assert!(snapshot_text(&state.rendered_snapshot()).contains("SUI rich document report"));

        assert!(state.apply_pending_render());
        assert!(!state.is_dirty());
        assert!(snapshot_text(&state.rendered_snapshot()).contains("Final edit"));
        assert!(!state.apply_pending_render());
    }

    #[test]
    fn markdown_state_retains_structured_blocks_across_source_edits() {
        let state = MarkdownDemoState::new();
        let before = state.rendered_snapshot();
        let structured_ids = before
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.kind,
                    RichDocumentBlockKind::Attachment(_) | RichDocumentBlockKind::Extension(_)
                )
            })
            .map(|block| block.id)
            .collect::<Vec<_>>();

        state.set_source("# Replaced Markdown".to_string());
        assert!(state.apply_pending_render());
        let after = state.rendered_snapshot();
        let retained_ids = after
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.kind,
                    RichDocumentBlockKind::Attachment(_) | RichDocumentBlockKind::Extension(_)
                )
            })
            .map(|block| block.id)
            .collect::<Vec<_>>();

        assert_eq!(retained_ids, structured_ids);
    }

    #[test]
    fn markdown_demo_exposes_themed_scroll_bar() {
        let theme = DefaultTheme::touch();
        let output = render_markdown_demo_with_size(900.0, 320.0, theme);
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(MARKDOWN_RENDER_SCROLL_BAR_NAME)
            })
            .expect("rich document scroll bar semantics present");

        assert_eq!(
            scroll_bar.bounds.width(),
            theme.metrics.scroll_bar_thickness
        );
    }

    #[test]
    fn markdown_demo_panels_split_evenly_when_side_by_side() {
        let output = render_markdown_demo(900.0);
        let source = source_editor_bounds(&output);
        let rendered = rendered_document_bounds(&output);

        assert!(
            (source.width() - rendered.width()).abs() <= 1.0,
            "source and rendered panel content should be equal width: source={source:?}, rendered={rendered:?}"
        );
        assert!(
            (source.y() - rendered.y()).abs() <= 1.0,
            "source and rendered panels should remain on the same row: source={source:?}, rendered={rendered:?}"
        );
    }

    #[test]
    fn markdown_demo_wrapped_panels_fill_available_width() {
        let output = render_markdown_demo(620.0);
        let source = source_editor_bounds(&output);
        let rendered = rendered_document_bounds(&output);

        assert!(
            source.y() < rendered.y(),
            "rendered panel should wrap below source when space is tight: source={source:?}, rendered={rendered:?}"
        );
        assert!(
            (source.width() - rendered.width()).abs() <= 1.0,
            "wrapped panels should use the same full row width: source={source:?}, rendered={rendered:?}"
        );
        assert!(
            source.width() > 500.0 && rendered.width() > 500.0,
            "wrapped panel content should expand beyond the minimum column width: source={source:?}, rendered={rendered:?}"
        );
    }

    #[test]
    fn rich_document_text_wraps_to_render_panel_width() {
        let output = render_markdown_demo(620.0);
        let rendered = rendered_document_bounds(&output);
        let layout_width = rendered_text_layout_width(&output);

        assert!(
            layout_width <= rendered.width() + 2.0,
            "rich document text should wrap within its panel: layout_width={layout_width}, rendered={rendered:?}"
        );
    }
}
