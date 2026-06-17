use std::ops::Range;

use sui_core::{
    Color, EditableTextSemantics, Event, ImeEvent, KeyState, KeyboardEvent, Path, Point,
    PointerButton, PointerEventKind, Rect, ScrollDelta, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsTextRange, SemanticsValue, Size, Vector, WakeEvent, WindowEvent,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{
    EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx, SemanticsCtx,
    Widget,
};
use sui_scene::{LayerCompositionMode, StrokeStyle};
use sui_text::{
    PersistentTextLayout, TextCursor, TextDirection, TextDocument, TextLayoutRequest,
    TextParagraph, TextSelection, TextSpan, TextStyle, TextWrap,
};

use crate::{
    DefaultTheme, MotionScalar, ThemeColorScheme,
    editor::{
        EditorCommand, EditorCommandResult, EditorState, clamp_to_grapheme_boundary,
        selection_range,
    },
    text_align::paint_aligned_text,
};

#[derive(Debug, Clone, PartialEq)]
pub struct TextSurfaceStyleSpan {
    pub range: Range<usize>,
    pub style: TextStyle,
}

impl TextSurfaceStyleSpan {
    pub fn new(range: Range<usize>, style: TextStyle) -> Self {
        Self { range, style }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSurfaceOverlayKind {
    Syntax,
    Diagnostic,
    SearchMatch,
    CurrentLine,
    RichTextPreview,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSurfaceStyleOverlay {
    pub range: Range<usize>,
    pub style: TextStyle,
    pub kind: TextSurfaceOverlayKind,
}

impl TextSurfaceStyleOverlay {
    pub fn new(range: Range<usize>, style: TextStyle, kind: TextSurfaceOverlayKind) -> Self {
        Self { range, style, kind }
    }
}

pub struct TextSurface {
    theme: Box<DefaultTheme>,
    name: String,
    placeholder: String,
    read_only: bool,
    editor: EditorState,
    clipboard: String,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    wrap: TextWrap,
    direction: TextDirection,
    style_spans: Vec<TextSurfaceStyleSpan>,
    style_overlays: Vec<TextSurfaceStyleOverlay>,
    style_revision: u64,
    hovered: bool,
    hover_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    dragging_selection: bool,
    layout: Option<PersistentTextLayout>,
    line_layouts: Vec<Option<PersistentTextLayout>>,
    line_offsets: Vec<usize>,
    line_lengths: Vec<usize>,
    line_layout_box_size: Option<Size>,
    line_layout_style: Option<TextStyle>,
    line_layout_revision: u64,
    line_layout_style_revision: u64,
    on_change: Option<Box<dyn FnMut(String)>>,
}

impl TextSurface {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            placeholder: String::new(),
            read_only: false,
            editor: EditorState::new(),
            clipboard: String::new(),
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            wrap: TextWrap::NoWrap,
            direction: TextDirection::Auto,
            style_spans: Vec::new(),
            style_overlays: Vec::new(),
            style_revision: 0,
            hovered: false,
            hover_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            dragging_selection: false,
            layout: None,
            line_layouts: Vec::new(),
            line_offsets: Vec::new(),
            line_lengths: Vec::new(),
            line_layout_box_size: None,
            line_layout_style: None,
            line_layout_revision: u64::MAX,
            line_layout_style_revision: u64::MAX,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width.max(0.0));
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height.max(0.0));
        self
    }

    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        if self.wrap != TextWrap::NoWrap {
            self.editor.set_scroll(0.0, self.editor.scroll_y());
        }
        self.invalidate_line_layouts();
        self
    }

    pub fn direction(mut self, direction: TextDirection) -> Self {
        self.direction = direction;
        self.invalidate_line_layouts();
        self
    }

    pub fn style_spans(mut self, spans: Vec<TextSurfaceStyleSpan>) -> Self {
        self.set_style_spans(spans);
        self
    }

    pub fn set_style_spans(&mut self, spans: Vec<TextSurfaceStyleSpan>) {
        self.style_spans = spans;
        self.bump_style_revision();
    }

    pub fn current_style_spans(&self) -> &[TextSurfaceStyleSpan] {
        &self.style_spans
    }

    pub fn style_overlays(mut self, overlays: Vec<TextSurfaceStyleOverlay>) -> Self {
        self.set_style_overlays(overlays);
        self
    }

    pub fn set_style_overlays(&mut self, overlays: Vec<TextSurfaceStyleOverlay>) {
        self.style_overlays = overlays;
        self.bump_style_revision();
    }

    pub fn current_style_overlays(&self) -> &[TextSurfaceStyleOverlay] {
        &self.style_overlays
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.set_value(value);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn current_value(&self) -> &str {
        self.editor.document().text()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.editor.set_text(value);
        self.invalidate_line_layouts();
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn display_text_style(&self) -> TextStyle {
        let mut style = self.resolved_text_style();
        if self.read_only {
            style.color = self.theme.palette.text_muted;
        }
        style
    }

    fn placeholder_text_style(&self) -> TextStyle {
        let mut style = self.resolved_text_style();
        style.color = self.theme.palette.placeholder;
        style
    }

    fn should_show_placeholder(&self) -> bool {
        self.display_text_len() == 0 && !self.placeholder.is_empty()
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.theme.metrics.text_input_padding)
    }

    fn resolved_min_size(&self) -> Size {
        Size::new(
            self.min_width
                .unwrap_or(self.theme.metrics.text_input_min_width),
            self.min_height
                .unwrap_or(self.theme.metrics.text_area_min_height),
        )
    }

    fn bump_style_revision(&mut self) {
        self.style_revision = self.style_revision.saturating_add(1);
        self.invalidate_line_layouts();
    }

    fn invalidate_line_layouts(&mut self) {
        self.layout = None;
        self.line_layouts.clear();
        self.line_offsets.clear();
        self.line_lengths.clear();
        self.line_layout_box_size = None;
        self.line_layout_style = None;
        self.line_layout_revision = u64::MAX;
        self.line_layout_style_revision = u64::MAX;
    }

    fn selection_range(&self) -> Range<usize> {
        self.editor.selection_range()
    }

    fn display_text(&self) -> String {
        self.editor.display_text()
    }

    fn display_text_len(&self) -> usize {
        if let Some(composition) = self.editor.composition() {
            self.editor
                .document()
                .len()
                .saturating_sub(composition.replacement_range.len())
                .saturating_add(composition.text.len())
        } else {
            self.editor.document().len()
        }
    }

    fn display_selection(&self) -> TextSelection {
        self.editor.display_selection()
    }

    fn layout_box_width(&self, available_width: f32) -> f32 {
        if self.wrap == TextWrap::NoWrap {
            1_000_000.0
        } else {
            available_width.max(1.0)
        }
    }

    fn layout_box_size(&self, available_width: f32) -> Size {
        let style = self.resolved_text_style();
        let estimated_lines = if self.editor.composition().is_some() {
            self.display_text().lines().count().max(1)
        } else {
            self.editor.document().line_count().max(1)
        } as f32;
        Size::new(
            self.layout_box_width(available_width),
            (estimated_lines * style.line_height).max(style.line_height),
        )
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.resolved_padding())
    }

    fn content_viewport_size(&self, bounds: Rect) -> Size {
        self.content_rect(bounds).size
    }

    fn commit_text_change(&mut self) {
        let value = self.current_value().to_string();
        if let Some(on_change) = &mut self.on_change {
            on_change(value);
        }
    }

    fn apply_editor_result(&mut self, ctx: &mut EventCtx, mut result: EditorCommandResult) {
        if let Some(text) = result.clipboard_text.take() {
            self.clipboard = text;
        }
        if result.text_changed {
            self.commit_text_change();
        }
        if result.layout_changed() {
            ctx.request_measure();
            ctx.request_text();
        } else if result.overlay_changed() {
            self.request_after_overlay_change(ctx);
        }
        if result.text_changed || result.selection_changed || result.composition_changed {
            ctx.request_semantics();
        }
        if result.handled {
            ctx.set_handled();
        }
    }

    fn execute_editor_command(
        &mut self,
        ctx: &mut EventCtx,
        command: EditorCommand,
    ) -> EditorCommandResult {
        let result = self.editor.execute(command);
        self.apply_editor_result(ctx, result.clone());
        result
    }

    fn move_line_boundary(&mut self, to_start: bool, extend: bool) -> EditorCommandResult {
        let focus = if extend {
            self.editor.selection().focus.utf8_offset
        } else {
            self.selection_range().end
        };

        let target = if self.has_line_layout_cache() {
            let line_index = self.line_index_for_offset(focus);
            if to_start {
                self.line_offsets[line_index]
            } else {
                self.line_offsets[line_index] + self.line_lengths[line_index]
            }
        } else if let Some(layout) = self.layout.as_ref() {
            let caret = layout.caret(TextCursor::new(focus));
            let line = &layout.lines()[caret.line_index];
            if to_start {
                line.byte_range.start
            } else {
                line.byte_range.end
            }
        } else {
            return self.editor.execute(if to_start {
                EditorCommand::MoveLineStart { extend }
            } else {
                EditorCommand::MoveLineEnd { extend }
            });
        };

        self.editor.execute(EditorCommand::MoveTo {
            offset: target,
            extend,
        })
    }

    fn move_vertical(
        &mut self,
        delta_lines: isize,
        extend: bool,
        _viewport_height: f32,
    ) -> EditorCommandResult {
        let focus = if extend {
            self.editor.selection().focus.utf8_offset
        } else {
            self.selection_range().end
        };

        if self.has_line_layout_cache() {
            let Some(caret) = self.caret_rect_for_cursor(TextCursor::new(focus)) else {
                return EditorCommandResult::default();
            };
            let preferred_x = self.editor.preferred_x().unwrap_or(caret.x());
            let line_index = self.line_index_for_offset(focus) as isize + delta_lines;
            let target_line =
                line_index.clamp(0, self.line_count().saturating_sub(1) as isize) as usize;
            let local_offset = if let Some(Some(layout)) = self.line_layouts.get(target_line) {
                let Some(line) = layout.lines().first() else {
                    return EditorCommandResult::default();
                };
                let local = layout.hit_test_point(Point::new(
                    preferred_x,
                    line.rect.y() + (line.rect.height() * 0.5),
                ));
                local.utf8_offset.min(self.line_lengths[target_line])
            } else {
                self.estimate_inline_offset_for_x(target_line, preferred_x)
            };
            let target = self.line_offsets[target_line] + local_offset;
            let result = self.editor.execute(EditorCommand::MoveTo {
                offset: target,
                extend,
            });
            self.editor.set_preferred_x(Some(preferred_x));
            return result;
        }

        let Some(layout) = self.layout.as_ref() else {
            let lines = delta_lines.unsigned_abs().max(1);
            return self.editor.execute(if delta_lines < 0 {
                if lines == 1 {
                    EditorCommand::MoveUp { extend }
                } else {
                    EditorCommand::PageUp { extend, lines }
                }
            } else if lines == 1 {
                EditorCommand::MoveDown { extend }
            } else {
                EditorCommand::PageDown { extend, lines }
            });
        };
        if layout.lines().is_empty() {
            return EditorCommandResult::default();
        }
        let caret = layout.caret(TextCursor::new(focus));
        let preferred_x = self.editor.preferred_x().unwrap_or(caret.rect.x());
        let line_index = caret.line_index as isize + delta_lines;
        let target_line =
            line_index.clamp(0, layout.lines().len().saturating_sub(1) as isize) as usize;
        let line = &layout.lines()[target_line];
        let target = layout.hit_test_point(Point::new(
            preferred_x,
            line.rect.y() + (line.rect.height() * 0.5),
        ));
        let result = self.editor.execute(EditorCommand::MoveTo {
            offset: target.utf8_offset,
            extend,
        });
        self.editor.set_preferred_x(Some(preferred_x));
        result
    }

    fn shape_line_layout(
        &self,
        ctx: &mut MeasureCtx,
        handle: Option<sui_text::TextLayoutHandle>,
        line_text: &str,
        line_range: Range<usize>,
        line_box_size: Size,
        base_style: TextStyle,
    ) -> sui_core::Result<PersistentTextLayout> {
        if self.style_spans.is_empty() && self.style_overlays.is_empty() {
            return ctx.layout().shape_text_persistent(
                handle,
                line_text.to_string(),
                line_box_size,
                base_style,
            );
        }

        let spans = self.line_text_spans(line_text, line_range, base_style);
        let mut paragraph = TextParagraph::from_spans(spans);
        paragraph.style.direction = self.direction;
        paragraph.style.wrap = self.wrap;
        ctx.layout().layout_document_persistent(
            handle,
            TextLayoutRequest::new(TextDocument {
                paragraphs: vec![paragraph],
            })
            .with_box_size(line_box_size),
        )
    }

    fn line_text_spans(
        &self,
        line_text: &str,
        line_range: Range<usize>,
        base_style: TextStyle,
    ) -> Vec<TextSpan> {
        let mut breaks = vec![0, line_text.len()];
        self.collect_style_breaks(&self.style_spans, &line_range, line_text, &mut breaks);
        let overlays_as_spans = self
            .style_overlays
            .iter()
            .map(|overlay| TextSurfaceStyleSpan {
                range: overlay.range.clone(),
                style: overlay.style.clone(),
            })
            .collect::<Vec<_>>();
        self.collect_style_breaks(&overlays_as_spans, &line_range, line_text, &mut breaks);

        breaks.sort_unstable();
        breaks.dedup();

        let mut spans = Vec::new();
        for pair in breaks.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if start >= end {
                continue;
            }
            let global_range = line_range.start + start..line_range.start + end;
            let mut style = base_style.clone();
            for span in &self.style_spans {
                if ranges_intersect(&global_range, &span.range) {
                    style = span.style.clone();
                }
            }
            for overlay in &self.style_overlays {
                if ranges_intersect(&global_range, &overlay.range) {
                    style = overlay.style.clone();
                }
            }
            spans.push(TextSpan::new(line_text[start..end].to_string(), style));
        }

        if spans.is_empty() {
            spans.push(TextSpan::new(String::new(), base_style));
        }
        spans
    }

    fn collect_style_breaks(
        &self,
        spans: &[TextSurfaceStyleSpan],
        line_range: &Range<usize>,
        line_text: &str,
        breaks: &mut Vec<usize>,
    ) {
        for span in spans {
            if !ranges_intersect(line_range, &span.range) {
                continue;
            }
            let local_start = span.range.start.saturating_sub(line_range.start);
            let local_end = span
                .range
                .end
                .min(line_range.end)
                .saturating_sub(line_range.start);
            breaks.push(clamp_to_grapheme_boundary(line_text, local_start));
            breaks.push(clamp_to_grapheme_boundary(line_text, local_end));
        }
    }

    fn update_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.theme.as_ref();
            self.hovered = hovered;
            set_hover_animation_target(&mut self.hover_animation, hovered as u8 as f32, theme, ctx);
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let previous_hover = self.hover_animation.value;
        let previous_focus = self.focus_animation.value;
        let animating = self.hover_animation.advance(time) | self.focus_animation.advance(time);
        let changed = self.hover_animation.changed_since(previous_hover)
            || self.focus_animation.changed_since(previous_focus);

        if changed {
            ctx.request_paint();
        }
        if animating {
            ctx.request_animation_frame();
        }
    }

    fn point_to_cursor(&self, bounds: Rect, position: Point) -> Option<TextCursor> {
        let content = self.content_rect(bounds);
        if !content.contains(position) {
            return None;
        }

        if self.has_line_layout_cache() {
            let local_x = position.x - content.x() + self.editor.scroll_x();
            let local_y = position.y - content.y() + self.editor.scroll_y();
            let line_index = self.line_index_for_y(local_y);
            let local_offset = if let Some(Some(layout)) = self.line_layouts.get(line_index) {
                let line = layout.lines().first()?;
                let local = layout.hit_test_point(Point::new(
                    local_x,
                    line.rect.y() + (line.rect.height() * 0.5),
                ));
                local.utf8_offset.min(self.line_lengths[line_index])
            } else {
                self.estimate_inline_offset_for_x(line_index, local_x)
            };
            return Some(TextCursor::new(
                self.line_offsets[line_index] + local_offset,
            ));
        }

        let layout = self.layout.as_ref()?;

        let local = Point::new(
            position.x - content.x() + self.editor.scroll_x(),
            position.y - content.y() + self.editor.scroll_y(),
        );
        Some(layout.hit_test_point(local))
    }

    fn visible_line_range(&self, viewport_height: f32) -> Range<usize> {
        if self.has_line_layout_cache() {
            if viewport_height <= 0.0 {
                return 0..0;
            }

            let line_height = self.line_height().max(1.0);
            let overdraw = viewport_height * 0.5;
            let visible_top = (self.editor.scroll_y() - overdraw).max(0.0);
            let visible_bottom = self.editor.scroll_y() + viewport_height + overdraw;
            let start = (visible_top / line_height).floor() as usize;
            let end = ((visible_bottom / line_height).ceil() as usize + 1)
                .min(self.line_count())
                .max(start + usize::from(start < self.line_count()));
            return start.min(self.line_count())..end;
        }

        let Some(layout) = self.layout.as_ref() else {
            return 0..0;
        };
        if layout.lines().is_empty() || viewport_height <= 0.0 {
            return 0..0;
        }

        let overdraw = viewport_height * 0.5;
        let visible_top = (self.editor.scroll_y() - overdraw).max(0.0);
        let visible_bottom = self.editor.scroll_y() + viewport_height + overdraw;
        let mut start = 0usize;
        while start < layout.lines().len() {
            if layout.lines()[start].rect.max_y() >= visible_top {
                break;
            }
            start += 1;
        }

        let mut end = start;
        while end < layout.lines().len() {
            if layout.lines()[end].rect.y() > visible_bottom {
                break;
            }
            end += 1;
        }

        start..end.max(start + usize::from(start < layout.lines().len()))
    }

    fn clamp_scroll_to_bounds(&mut self, viewport_size: Size) -> bool {
        let previous = (self.editor.scroll_x(), self.editor.scroll_y());
        let content_size = if self.has_line_layout_cache() {
            self.multi_line_content_size()
        } else if let Some(layout) = self.layout.as_ref() {
            layout_content_size(layout)
        } else {
            self.editor.set_scroll(0.0, 0.0);
            return previous != (0.0, 0.0);
        };

        let max_x = if self.wrap == TextWrap::NoWrap {
            (content_size.width - viewport_size.width).max(0.0)
        } else {
            0.0
        };
        let max_y = (content_size.height - viewport_size.height).max(0.0);
        self.editor.set_scroll(
            self.editor.scroll_x().clamp(0.0, max_x),
            self.editor.scroll_y().clamp(0.0, max_y),
        );
        previous != (self.editor.scroll_x(), self.editor.scroll_y())
    }

    fn ensure_caret_visible(&mut self, bounds: Rect) -> bool {
        let viewport = self.content_viewport_size(bounds);
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return false;
        }

        let previous = (self.editor.scroll_x(), self.editor.scroll_y());
        let Some(caret) = self.caret_rect_for_cursor(self.display_selection().focus) else {
            return false;
        };
        let mut scroll_x = self.editor.scroll_x();
        let mut scroll_y = self.editor.scroll_y();
        if self.wrap == TextWrap::NoWrap {
            if caret.x() < scroll_x {
                scroll_x = caret.x().max(0.0);
            } else if caret.max_x() > scroll_x + viewport.width {
                scroll_x = (caret.max_x() - viewport.width).max(0.0);
            }
        } else {
            scroll_x = 0.0;
        }
        if caret.y() < scroll_y {
            scroll_y = caret.y().max(0.0);
        } else if caret.max_y() > scroll_y + viewport.height {
            scroll_y = (caret.max_y() - viewport.height).max(0.0);
        }
        self.editor.set_scroll(scroll_x, scroll_y);
        let _ = self.clamp_scroll_to_bounds(viewport);
        previous != (self.editor.scroll_x(), self.editor.scroll_y())
    }

    fn scroll_by(&mut self, bounds: Rect, delta: Vector) -> bool {
        let previous = (self.editor.scroll_x(), self.editor.scroll_y());
        let mut scroll_x = self.editor.scroll_x();
        let mut scroll_y = self.editor.scroll_y();
        if self.wrap == TextWrap::NoWrap {
            scroll_x += delta.x;
        }
        scroll_y += delta.y;
        self.editor.set_scroll(scroll_x, scroll_y);
        let _ = self.clamp_scroll_to_bounds(self.content_viewport_size(bounds));
        previous != (self.editor.scroll_x(), self.editor.scroll_y())
    }

    fn request_after_overlay_change(&mut self, ctx: &mut EventCtx) {
        if self.ensure_caret_visible(ctx.bounds()) {
            ctx.request_text();
        } else {
            ctx.request_paint();
        }
    }

    fn line_count(&self) -> usize {
        self.line_offsets.len()
    }

    fn has_line_layout_cache(&self) -> bool {
        !self.line_offsets.is_empty()
    }

    fn line_slot_height_for_style(&self, base_style: &TextStyle) -> f32 {
        let base = base_style.line_height.max(base_style.font_size).max(1.0);
        self.style_spans
            .iter()
            .map(|span| span.style.line_height.max(span.style.font_size))
            .chain(
                self.style_overlays
                    .iter()
                    .map(|overlay| overlay.style.line_height.max(overlay.style.font_size)),
            )
            .fold(base, f32::max)
    }

    fn line_height(&self) -> f32 {
        self.line_slot_height_for_style(&self.resolved_text_style())
    }

    fn line_layout_height(&self, layout: &PersistentTextLayout, base_line_height: f32) -> f32 {
        layout.measurement().height.max(base_line_height)
    }

    fn line_layout_y_offset(
        &self,
        layout: &PersistentTextLayout,
        base_line_height: f32,
        slot_height: f32,
    ) -> f32 {
        ((slot_height - self.line_layout_height(layout, base_line_height)).max(0.0)) * 0.5
    }

    fn line_origin_y(
        &self,
        line_index: usize,
        layout: &PersistentTextLayout,
        base_line_height: f32,
        slot_height: f32,
    ) -> f32 {
        line_index as f32 * slot_height
            + self.line_layout_y_offset(layout, base_line_height, slot_height)
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        let offset = offset.min(self.display_text_len());
        if self.editor.composition().is_none() && self.has_line_layout_cache() {
            return self
                .editor
                .document()
                .line_index_for_offset(offset)
                .min(self.line_count().saturating_sub(1));
        }
        self.line_offsets
            .iter()
            .enumerate()
            .rev()
            .find(|(_, start)| **start <= offset)
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    fn line_index_for_y(&self, y: f32) -> usize {
        let line_height = self.line_height().max(1.0);
        ((y / line_height).floor() as usize).min(self.line_count().saturating_sub(1))
    }

    fn estimate_inline_offset_for_x(&self, line_index: usize, x: f32) -> usize {
        let length = self.line_lengths.get(line_index).copied().unwrap_or(0);
        if length == 0 {
            return 0;
        }

        let average_advance = (self.resolved_text_style().font_size * 0.55).max(1.0);
        ((x.max(0.0) / average_advance).round() as usize).min(length)
    }

    fn estimated_caret_rect_for_line(&self, line_index: usize, local_offset: usize) -> Rect {
        let style = self.resolved_text_style();
        let average_advance = (style.font_size * 0.55).max(1.0);
        Rect::new(
            average_advance * local_offset as f32,
            line_index as f32 * self.line_height(),
            1.0,
            style.line_height.max(1.0),
        )
    }

    fn caret_rect_for_cursor(&self, cursor: TextCursor) -> Option<Rect> {
        if self.has_line_layout_cache() {
            let line_index = self.line_index_for_offset(cursor.utf8_offset);
            let local_offset = cursor
                .utf8_offset
                .saturating_sub(self.line_offsets[line_index])
                .min(self.line_lengths[line_index]);
            let Some(Some(layout)) = self.line_layouts.get(line_index) else {
                return Some(self.estimated_caret_rect_for_line(line_index, local_offset));
            };
            let base_line_height = self.resolved_text_style().line_height;
            let slot_height = self.line_height();
            return Some(layout.caret_rect(local_offset).translate(Vector::new(
                0.0,
                self.line_origin_y(line_index, layout, base_line_height, slot_height),
            )));
        }

        self.layout.as_ref().map(|layout| layout.caret(cursor).rect)
    }

    fn selection_rects_for_display(&self, selection: &TextSelection) -> Vec<Rect> {
        if self.has_line_layout_cache() {
            let mut rects = Vec::new();
            let range = selection_range(selection, self.display_text_len());
            if range.is_empty() || self.line_count() == 0 {
                return rects;
            }

            let start_line = self.line_index_for_offset(range.start);
            let end_line = self
                .line_index_for_offset(range.end)
                .min(self.line_count().saturating_sub(1));
            let base_line_height = self.resolved_text_style().line_height;
            let slot_height = self.line_height();
            for line_index in start_line..=end_line {
                let line_start = self.line_offsets[line_index];
                let line_end = line_start + self.line_lengths[line_index];
                let selection_start = range.start.max(line_start);
                let selection_end = range.end.min(line_end);
                if selection_start >= selection_end {
                    continue;
                }

                let local_range = selection_start.saturating_sub(line_start)
                    ..selection_end.saturating_sub(line_start);
                let Some(Some(layout)) = self.line_layouts.get(line_index) else {
                    let start = self.estimated_caret_rect_for_line(line_index, local_range.start);
                    let end = self.estimated_caret_rect_for_line(line_index, local_range.end);
                    rects.push(Rect::new(
                        start.x(),
                        start.y(),
                        (end.x() - start.x()).max(1.0),
                        start.height(),
                    ));
                    continue;
                };
                let local_rects = layout.selection_rects(local_range);
                rects.extend(local_rects.into_iter().map(|rect| {
                    rect.translate(Vector::new(
                        0.0,
                        self.line_origin_y(line_index, layout, base_line_height, slot_height),
                    ))
                }));
            }
            return rects;
        }

        self.layout
            .as_ref()
            .map(|layout| layout.selection_geometry(selection).rects)
            .unwrap_or_default()
    }

    fn multi_line_content_size(&self) -> Size {
        let estimated_width = self
            .line_lengths
            .iter()
            .map(|length| *length as f32 * self.resolved_text_style().font_size * 0.55)
            .fold(0.0_f32, f32::max);
        let measured_width = self
            .line_layouts
            .iter()
            .filter_map(|layout| layout.as_ref())
            .map(|layout| layout.measurement().width)
            .fold(0.0_f32, f32::max);
        Size::new(
            estimated_width.max(measured_width),
            self.line_count() as f32 * self.line_height(),
        )
    }
}

impl Widget for TextSurface {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                self.update_hovered(hovered, ctx);
                if self.dragging_selection
                    && ctx.phase() != EventPhase::Capture
                    && pointer.buttons.contains(PointerButton::Primary)
                    && let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position)
                {
                    let anchor = self.editor.selection().anchor.utf8_offset;
                    let result = self.editor.execute(EditorCommand::SetSelection {
                        anchor,
                        focus: cursor.utf8_offset,
                    });
                    self.apply_editor_result(ctx, result);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.update_hovered(true, ctx);
                let clear_result = self.editor.execute(EditorCommand::ClearComposition);
                self.apply_editor_result(ctx, clear_result);
                if let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position) {
                    let command = if pointer.modifiers.shift {
                        EditorCommand::SetSelection {
                            anchor: self.editor.selection().anchor.utf8_offset,
                            focus: cursor.utf8_offset,
                        }
                    } else {
                        EditorCommand::MoveTo {
                            offset: cursor.utf8_offset,
                            extend: false,
                        }
                    };
                    let result = self.editor.execute(command);
                    self.apply_editor_result(ctx, result);
                }
                self.dragging_selection = true;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                self.request_after_overlay_change(ctx);
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.dragging_selection =>
            {
                self.dragging_selection = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging_selection {
                    self.dragging_selection = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                if self.scroll_by(ctx.bounds(), Vector::new(-delta.x, -delta.y)) {
                    ctx.request_text();
                    ctx.set_handled();
                }
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() && !self.read_only => {
                self.execute_editor_command(ctx, EditorCommand::StartComposition);
            }
            Event::Ime(ImeEvent::CompositionUpdate { text, cursor_range })
                if ctx.is_focused() && !self.read_only =>
            {
                self.execute_editor_command(
                    ctx,
                    EditorCommand::UpdateComposition {
                        text: text.clone(),
                        cursor_range: cursor_range.clone(),
                    },
                );
            }
            Event::Ime(ImeEvent::CompositionCommit { text })
                if ctx.is_focused() && !self.read_only =>
            {
                self.execute_editor_command(ctx, EditorCommand::CommitComposition(text.clone()));
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() && !self.read_only => {
                self.execute_editor_command(ctx, EditorCommand::EndComposition);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let command_modifier = key.modifiers.control || key.modifiers.meta;
                let result = match key.key.as_str() {
                    "a" | "A" if command_modifier => self.editor.execute(EditorCommand::SelectAll),
                    "c" | "C" if command_modifier => self.editor.execute(EditorCommand::Copy),
                    "x" | "X" if command_modifier && !self.read_only => {
                        self.editor.execute(EditorCommand::Cut)
                    }
                    "v" | "V" if command_modifier && !self.read_only => self
                        .editor
                        .execute(EditorCommand::Paste(self.clipboard.clone())),
                    "z" | "Z" if command_modifier && key.modifiers.shift && !self.read_only => {
                        self.editor.execute(EditorCommand::Redo)
                    }
                    "z" | "Z" if command_modifier && !self.read_only => {
                        self.editor.execute(EditorCommand::Undo)
                    }
                    "y" | "Y" if command_modifier && !self.read_only => {
                        self.editor.execute(EditorCommand::Redo)
                    }
                    "ArrowLeft" if command_modifier => {
                        self.editor.execute(EditorCommand::MoveWordLeft {
                            extend: key.modifiers.shift,
                        })
                    }
                    "ArrowRight" if command_modifier => {
                        self.editor.execute(EditorCommand::MoveWordRight {
                            extend: key.modifiers.shift,
                        })
                    }
                    "ArrowLeft" => self.editor.execute(EditorCommand::MoveLeft {
                        extend: key.modifiers.shift,
                    }),
                    "ArrowRight" => self.editor.execute(EditorCommand::MoveRight {
                        extend: key.modifiers.shift,
                    }),
                    "ArrowUp" => self.move_vertical(
                        -1,
                        key.modifiers.shift,
                        self.content_viewport_size(ctx.bounds()).height,
                    ),
                    "ArrowDown" => self.move_vertical(
                        1,
                        key.modifiers.shift,
                        self.content_viewport_size(ctx.bounds()).height,
                    ),
                    "Home" => self.move_line_boundary(true, key.modifiers.shift),
                    "End" => self.move_line_boundary(false, key.modifiers.shift),
                    "PageUp" => {
                        let line_height = self.line_height().max(1.0);
                        let delta = (self.content_viewport_size(ctx.bounds()).height / line_height)
                            .floor()
                            .max(1.0) as isize;
                        self.move_vertical(
                            -delta,
                            key.modifiers.shift,
                            self.content_viewport_size(ctx.bounds()).height,
                        )
                    }
                    "PageDown" => {
                        let line_height = self.line_height().max(1.0);
                        let delta = (self.content_viewport_size(ctx.bounds()).height / line_height)
                            .floor()
                            .max(1.0) as isize;
                        self.move_vertical(
                            delta,
                            key.modifiers.shift,
                            self.content_viewport_size(ctx.bounds()).height,
                        )
                    }
                    "Backspace" if !self.read_only => {
                        self.editor.execute(EditorCommand::DeleteBackward)
                    }
                    "Delete" if !self.read_only => {
                        self.editor.execute(EditorCommand::DeleteForward)
                    }
                    "Enter" if !self.read_only => self
                        .editor
                        .execute(EditorCommand::InsertText("\n".to_string())),
                    _ if !self.read_only && self.editor.composition().is_none() => {
                        if let Some(text) = keyboard_text(key) {
                            self.editor
                                .execute(EditorCommand::InsertText(text.to_string()))
                        } else {
                            self.editor.execute(EditorCommand::Noop)
                        }
                    }
                    _ => self.editor.execute(EditorCommand::Noop),
                };
                self.apply_editor_result(ctx, result);
            }
            Event::Window(WindowEvent::Focused(false)) => {
                let result = self.editor.execute(EditorCommand::ClearComposition);
                self.apply_editor_result(ctx, result);
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let padding = self.resolved_padding();
        let min_size = self.resolved_min_size();
        let available_width = if constraints.max.width.is_finite() {
            (constraints.max.width - padding.left - padding.right).max(0.0)
        } else {
            (min_size.width - padding.left - padding.right).max(0.0)
        };

        let composition_active = self.editor.composition().is_some();
        let line_style = self.display_text_style();
        let line_box_size = Size::new(
            self.layout_box_width(available_width),
            line_style.line_height.max(1.0),
        );
        let viewport_height = if constraints.max.height.is_finite() {
            (constraints.max.height - padding.top - padding.bottom).max(0.0)
        } else {
            (min_size.height - padding.top - padding.bottom).max(line_style.line_height)
        };
        let mut line_layout_failed = false;

        if !composition_active {
            let document = self.editor.document();
            let line_count = document.line_count();

            if line_count > 1 {
                let can_reuse_lines = self.line_layout_revision != u64::MAX
                    && self.line_layout_box_size == Some(line_box_size)
                    && self.line_layout_style.as_ref() == Some(&line_style)
                    && self.line_layout_style_revision == self.style_revision
                    && self.line_layouts.len() == line_count
                    && self.line_offsets.len() == line_count
                    && self.line_lengths.len() == line_count;

                if !can_reuse_lines {
                    self.line_layouts = vec![None; line_count];
                    self.line_offsets.clear();
                    self.line_lengths.clear();
                    self.line_offsets.reserve(line_count);
                    self.line_lengths.reserve(line_count);
                    for index in 0..line_count {
                        let line_range = document.line_range(index);
                        self.line_offsets.push(line_range.start);
                        self.line_lengths.push(line_range.len());
                    }
                } else {
                    for index in document.dirty_line_range() {
                        if index >= line_count {
                            continue;
                        }
                        let line_range = document.line_range(index);
                        self.line_layouts[index] = None;
                        self.line_offsets[index] = line_range.start;
                        self.line_lengths[index] = line_range.len();
                    }
                }

                let visible_lines = self.visible_line_range(viewport_height);
                let caret_line =
                    self.line_index_for_offset(self.display_selection().focus.utf8_offset);
                let mut lines_to_shape = Vec::with_capacity(visible_lines.len().saturating_add(1));
                lines_to_shape.extend(visible_lines);
                if caret_line < line_count && !lines_to_shape.contains(&caret_line) {
                    lines_to_shape.push(caret_line);
                }

                for index in lines_to_shape {
                    if self.line_layouts[index].is_some() {
                        continue;
                    }
                    let line_range = document.line_range(index);
                    match self.shape_line_layout(
                        ctx,
                        None,
                        document.line_text(index),
                        line_range,
                        line_box_size,
                        line_style.clone(),
                    ) {
                        Ok(layout) => self.line_layouts[index] = Some(layout),
                        Err(_) => {
                            line_layout_failed = true;
                            break;
                        }
                    }
                }

                if !line_layout_failed {
                    self.line_layout_box_size = Some(line_box_size);
                    self.line_layout_style = Some(line_style.clone());
                    self.line_layout_revision = document.revision();
                    self.line_layout_style_revision = self.style_revision;
                    self.layout = None;
                    self.editor.clear_document_dirty();
                }
            } else {
                self.line_layouts.clear();
                self.line_offsets.clear();
                self.line_lengths.clear();
                self.line_layout_box_size = None;
                self.line_layout_style = None;
                self.line_layout_revision = u64::MAX;
                self.line_layout_style_revision = u64::MAX;
            }
        } else {
            let display_text = self.display_text();
            let (line_texts, line_offsets, line_lengths) = split_lines_with_offsets(&display_text);
            if line_texts.len() > 1 {
                let metadata_matches = self.line_layout_box_size == Some(line_box_size)
                    && self.line_layout_style.as_ref() == Some(&line_style)
                    && self.line_layout_style_revision == self.style_revision
                    && self.line_layouts.len() == line_texts.len()
                    && self.line_offsets == line_offsets
                    && self.line_lengths == line_lengths;

                if !metadata_matches {
                    self.line_layouts = vec![None; line_texts.len()];
                    self.line_offsets = line_offsets;
                    self.line_lengths = line_lengths;
                } else {
                    self.line_layouts.fill(None);
                }

                let visible_lines = self.visible_line_range(viewport_height);
                let caret_line =
                    self.line_index_for_offset(self.display_selection().focus.utf8_offset);
                let mut lines_to_shape = Vec::with_capacity(visible_lines.len().saturating_add(1));
                lines_to_shape.extend(visible_lines);
                if caret_line < line_texts.len() && !lines_to_shape.contains(&caret_line) {
                    lines_to_shape.push(caret_line);
                }

                for index in lines_to_shape {
                    if self.line_layouts[index].is_some() {
                        continue;
                    }
                    let line_range = self.line_offsets[index]
                        ..self.line_offsets[index] + self.line_lengths[index];
                    match self.shape_line_layout(
                        ctx,
                        None,
                        &line_texts[index],
                        line_range,
                        line_box_size,
                        line_style.clone(),
                    ) {
                        Ok(layout) => self.line_layouts[index] = Some(layout),
                        Err(_) => {
                            line_layout_failed = true;
                            break;
                        }
                    }
                }

                if !line_layout_failed {
                    self.line_layout_box_size = Some(line_box_size);
                    self.line_layout_style = Some(line_style.clone());
                    self.line_layout_revision = u64::MAX;
                    self.line_layout_style_revision = self.style_revision;
                    self.layout = None;
                }
            } else {
                self.line_layouts.clear();
                self.line_offsets.clear();
                self.line_lengths.clear();
                self.line_layout_box_size = None;
                self.line_layout_style = None;
                self.line_layout_revision = u64::MAX;
                self.line_layout_style_revision = u64::MAX;
            }
        }

        if line_layout_failed {
            self.line_layouts.clear();
            self.line_offsets.clear();
            self.line_lengths.clear();
            self.line_layout_box_size = None;
            self.line_layout_style = None;
            self.line_layout_revision = u64::MAX;
            self.line_layout_style_revision = u64::MAX;
        }

        self.layout = if !self.has_line_layout_cache() {
            let display_text = self.display_text();
            let handle = self.layout.as_ref().map(|layout| layout.handle());
            let box_size = self.layout_box_size(available_width);
            if self.style_spans.is_empty() && self.style_overlays.is_empty() {
                ctx.layout()
                    .shape_text_persistent(handle, display_text, box_size, line_style)
                    .ok()
            } else {
                self.shape_line_layout(
                    ctx,
                    handle,
                    &display_text,
                    0..display_text.len(),
                    box_size,
                    line_style,
                )
                .ok()
            }
        } else {
            None
        };

        let mut natural_content_size = if self.has_line_layout_cache() {
            self.multi_line_content_size()
        } else {
            self.layout
                .as_ref()
                .map(layout_content_size)
                .unwrap_or(Size::new(min_size.width, min_size.height))
        };
        if self.should_show_placeholder() {
            let placeholder_style = self.placeholder_text_style();
            if let Ok(measurement) = ctx
                .layout()
                .measure_text(self.placeholder.clone(), placeholder_style.clone())
            {
                natural_content_size.width = natural_content_size.width.max(measurement.width);
                natural_content_size.height = natural_content_size
                    .height
                    .max(measurement.height.max(placeholder_style.line_height));
            }
        }
        let natural = Size::new(
            natural_content_size.width + padding.left + padding.right,
            natural_content_size.height + padding.top + padding.bottom,
        );

        let desired = Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural.width.max(min_size.width)
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                natural.height.max(min_size.height)
            },
        );
        let size = constraints.clamp(Size::new(
            desired.width.max(min_size.width),
            desired.height.max(min_size.height),
        ));

        let _ = self.clamp_scroll_to_bounds(
            self.content_viewport_size(Rect::from_origin_size(Point::ZERO, size)),
        );
        size
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let content = self.content_rect(ctx.bounds());
        let hover_progress = self.hover_animation.value;
        let focus_progress = self.focus_animation.value;
        let base_background = if self.read_only {
            palette.surface
        } else {
            mix_color(palette.control, palette.control_hover, hover_progress)
        };
        let background = mix_color(base_background, palette.surface_focus, focus_progress);
        let border = mix_color(
            mix_color(palette.border, palette.border_hover, hover_progress),
            palette.border_focus,
            focus_progress,
        );

        draw_surface_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics.border_width,
            background,
            border,
            (focus_progress > AnimatedScalar::EPSILON).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
            metrics.focus_ring_width,
            metrics.focus_ring_outset,
        );

        if !self.has_line_layout_cache() && self.layout.is_none() {
            return;
        }

        let display_selection = self.display_selection();
        let origin = Point::new(
            content.x() - self.editor.scroll_x(),
            content.y() - self.editor.scroll_y(),
        );
        let line_range = self.visible_line_range(content.height());
        let selection_rects = self.selection_rects_for_display(&display_selection);
        let current_caret = self.caret_rect_for_cursor(display_selection.focus);
        let current_line_index = self.line_index_for_offset(display_selection.focus.utf8_offset);
        let base_line_height = self.resolved_text_style().line_height;
        let slot_height = self.line_height();

        ctx.push_clip_rect(content);

        if selection_rects.is_empty() {
            let line_rect = Rect::new(
                content.x(),
                origin.y + current_line_index as f32 * slot_height,
                content.width(),
                slot_height,
            );
            ctx.fill(
                Path::rounded_rect(line_rect, 0.0),
                palette.accent.with_alpha(match self.theme.colors.scheme {
                    ThemeColorScheme::Light => 0.05,
                    ThemeColorScheme::Dark => 0.12,
                    ThemeColorScheme::HighContrast => 0.18,
                }),
            );
        } else {
            for rect in selection_rects {
                ctx.fill(
                    Path::rounded_rect(rect.translate(origin.to_vector()), 3.0),
                    palette.selection,
                );
            }
        }

        if self.should_show_placeholder() {
            let placeholder_style = self.placeholder_text_style();
            let placeholder_slot = Rect::new(
                origin.x,
                origin.y,
                content.width(),
                slot_height.max(placeholder_style.line_height),
            );
            paint_aligned_text(
                ctx,
                placeholder_slot,
                &self.placeholder,
                &placeholder_style,
                placeholder_style.line_height,
                0.0,
            );
        }

        if self.has_line_layout_cache() {
            for line_index in line_range {
                if let Some(Some(layout)) = self.line_layouts.get(line_index) {
                    let line_y =
                        self.line_origin_y(line_index, layout, base_line_height, slot_height);
                    ctx.draw_persistent_text_layout(
                        Point::new(origin.x, origin.y + line_y),
                        layout,
                    );
                }
            }
        } else if let Some(layout) = &self.layout {
            ctx.draw_persistent_text_layout_window(origin, layout, line_range);
        }

        if ctx.is_focused() && !self.read_only {
            let Some(current_caret) = current_caret else {
                ctx.pop_clip();
                return;
            };
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let caret = Rect::new(
                origin.x + current_caret.x(),
                origin.y + current_caret.y(),
                caret_width,
                current_caret.height().max(1.0),
            );
            ctx.set_ime_composition_rect(caret);
            ctx.fill(Path::rounded_rect(caret, caret_width * 0.5), palette.caret);
        }

        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Scroll,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        let display_text = self.display_text();
        let display_selection = self.display_selection();
        let selection = selection_range(&display_selection, display_text.len());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(display_text));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.editable_text = Some(EditableTextSemantics {
            caret_offset: display_selection.focus.utf8_offset,
            selection: SemanticsTextRange::new(selection.start, selection.end),
            multiline: true,
            readonly: self.read_only,
            scroll_x: self.editor.scroll_x(),
            scroll_y: self.editor.scroll_y(),
        });
        node.actions = if self.read_only {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetSelection,
                SemanticsAction::Copy,
            ]
        } else {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetValue,
                SemanticsAction::SetSelection,
                SemanticsAction::InsertText,
                SemanticsAction::DeleteBackward,
                SemanticsAction::DeleteForward,
                SemanticsAction::Copy,
                SemanticsAction::Cut,
                SemanticsAction::Paste,
                SemanticsAction::Undo,
                SemanticsAction::Redo,
            ]
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused {
            let result = self.editor.execute(EditorCommand::ClearComposition);
            if result.layout_changed() {
                ctx.request_measure();
                ctx.request_text();
            }
        }
        let theme = self.theme.as_ref();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

type AnimatedScalar = MotionScalar;

fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) -> bool {
    animation.set_target_event(target, duration, easing, ctx)
}

fn set_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    )
}

fn set_focus_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.focus_duration(),
        theme.motion.focus_easing(),
        ctx,
    )
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::new(
        from.space,
        from.red + (to.red - from.red) * amount,
        from.green + (to.green - from.green) * amount,
        from.blue + (to.blue - from.blue) * amount,
        from.alpha + (to.alpha - from.alpha) * amount,
    )
    .clamped()
}

fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

fn draw_surface_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    border_width: f32,
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
    focus_ring_width: f32,
    focus_ring_outset: f32,
) {
    if let Some(focus_ring) = focus_ring {
        let outset = physical_pixels(ctx, focus_ring_outset);
        ctx.stroke(
            Path::rounded_rect(bounds.inflate(outset, outset), radius + outset),
            focus_ring,
            StrokeStyle::new(physical_pixels(ctx, focus_ring_width)),
        );
    }

    let fill = Path::rounded_rect(bounds, radius);
    ctx.fill(fill, background);

    if border_width > 0.0 {
        let width = physical_pixels(ctx, border_width);
        let inset = width * 0.5;
        ctx.stroke(
            Path::rounded_rect(bounds.inflate(-inset, -inset), (radius - inset).max(0.0)),
            border,
            StrokeStyle::new(width),
        );
    }
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    ctx.dpi().physical_pixels_to_logical(value)
}

fn scroll_delta_to_offset(delta: ScrollDelta) -> Vector {
    match delta {
        ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        ScrollDelta::Pixels(delta) => delta,
    }
}

fn keyboard_text(event: &KeyboardEvent) -> Option<&str> {
    if event.state != KeyState::Pressed
        || event.is_composing
        || event.modifiers.control
        || event.modifiers.alt
        || event.modifiers.meta
    {
        return None;
    }

    event
        .text
        .as_deref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
}

fn layout_content_size(layout: &PersistentTextLayout) -> Size {
    let line_height = layout
        .lines()
        .iter()
        .map(|line| line.rect.max_y())
        .fold(0.0_f32, f32::max)
        .max(layout.measurement().height);
    Size::new(layout.measurement().width, line_height)
}

fn ranges_intersect(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    use sui_core::{
        Event, Modifiers, PointerButtons, PointerEvent, PointerKind, SemanticsValue, WindowId,
    };
    use sui_runtime::{
        Application, RenderOutput, Runtime, SceneStatisticsDetailMode, WindowBuilder,
        set_window_scene_statistics_detail_mode,
    };
    use sui_scene::{Brush, SceneCommand};

    fn build_runtime<W>(root: W) -> (Runtime, WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Editor").root(root))
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }

        Event::Pointer(PointerEvent {
            pointer_id: 1,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Primary),
            buttons,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn key_event(key: &str) -> Event {
        Event::Keyboard(KeyboardEvent::new(key, KeyState::Pressed))
    }

    fn command_key_event(key: &str) -> Event {
        let mut event = KeyboardEvent::new(key, KeyState::Pressed);
        event.modifiers.control = true;
        Event::Keyboard(event)
    }

    fn text_input_node(output: &RenderOutput) -> &SemanticsNode {
        output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text input semantics node should exist")
    }

    fn shaped_text_commands(output: &RenderOutput) -> Vec<sui_text::ShapedText> {
        let mut found = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawShapedText(text) => found.push(text.clone()),
                SceneCommand::DrawShapedTextWindow(text) => found.push(sui_text::ShapedText {
                    origin: text.origin,
                    layout_handle: text.layout_handle,
                    layout_version: text.layout_version,
                    bounds: text.bounds,
                    color_override: text.color_override,
                }),
                _ => {}
            });
        found
    }

    fn solid_fill_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    brush: Brush::Solid(color),
                    ..
                }
                | SceneCommand::FillPath {
                    brush: Brush::Solid(color),
                    ..
                } => colors.push(*color),
                _ => {}
            });
        colors
    }

    fn solid_stroke_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect {
                    brush: Brush::Solid(color),
                    ..
                }
                | SceneCommand::StrokePath {
                    brush: Brush::Solid(color),
                    ..
                } => colors.push(*color),
                _ => {}
            });
        colors
    }

    fn handle_ready_events(runtime: &mut Runtime) -> usize {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window, event) in ready {
            runtime
                .handle_event(ready_window, event)
                .expect("ready event should be handled");
        }
        count
    }

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.01,
            "expected {actual} to be within 0.01 of {expected}"
        );
    }

    #[test]
    fn text_surface_hover_and_focus_use_theme_motion() {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let focus_duration = theme.motion.focus_duration();
        let expected_hover =
            super::mix_color(theme.palette.control, theme.palette.control_hover, 1.0);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(220.0, 96.0))
                .with_child(
                    TextSurface::new("Editor")
                        .theme(theme)
                        .placeholder("Write notes"),
                ),
        );

        let _ = runtime.render(window_id).expect("render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(16.0, 16.0), false),
            )
            .expect("hover event should be handled");

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let mid_hover = runtime.render(window_id).expect("render should succeed");
        let mid_hover_background = solid_fill_colors(&mid_hover)[0];
        assert_ne!(mid_hover_background, theme.palette.control);
        assert_ne!(mid_hover_background, expected_hover);

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let settled_hover = runtime.render(window_id).expect("render should succeed");
        assert_eq!(solid_fill_colors(&settled_hover)[0], expected_hover);

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(16.0, 16.0), true),
            )
            .expect("focus event should be handled");

        runtime.tick(hover_duration + focus_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let mid_focus = runtime.render(window_id).expect("render should succeed");
        assert!(!solid_stroke_colors(&mid_focus).contains(&theme.palette.focus_ring));

        runtime.tick(hover_duration + focus_duration);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let settled_focus = runtime.render(window_id).expect("render should succeed");
        assert_eq!(
            solid_fill_colors(&settled_focus)[0],
            theme.palette.surface_focus
        );
        assert!(solid_stroke_colors(&settled_focus).contains(&theme.palette.focus_ring));
    }

    #[test]
    fn text_surface_submits_only_visible_line_windows() {
        let long_text = (0..240)
            .map(|index| format!("line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(180.0, 96.0))
                .with_child(TextSurface::new("Editor").value(long_text)),
        );
        set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);

        let output = runtime.render(window_id).expect("render should succeed");
        let shaped = shaped_text_commands(&output);
        let layout_misses = output.diagnostics.text_caches.runtime_layout.misses;

        assert!(
            shaped.len() < 24,
            "expected visible-line submission only, emitted {} shaped lines",
            shaped.len(),
        );
        assert!(
            layout_misses < 80,
            "expected viewport-aware measure, recorded {layout_misses} layout misses"
        );
        assert!(
            shaped
                .first()
                .and_then(|text| text.resolve(output.frame.text_layout_registry.as_ref()))
                .is_some_and(|layout| layout.text() == "line 00")
        );
    }

    #[test]
    fn text_surface_scroll_updates_visible_window() {
        let long_text = (0..32)
            .map(|index| format!("line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut surface = TextSurface::new("Editor").value(long_text);
        let bounds = Rect::new(0.0, 0.0, 180.0, 96.0);
        let content = surface.content_rect(bounds);
        let (line_texts, line_offsets, line_lengths) =
            split_lines_with_offsets(surface.current_value());
        let mut line_layouts = Vec::new();
        for line in &line_texts {
            line_layouts.push(
                sui_text::TextSystem::new()
                    .shape_text_persistent(
                        None,
                        line.clone(),
                        Size::new(
                            surface.layout_box_size(content.width()).width,
                            surface.resolved_text_style().line_height,
                        ),
                        surface.resolved_text_style(),
                        &sui_text::FontRegistry::new(),
                    )
                    .expect("text surface line layout should shape"),
            );
        }
        surface.line_layouts = line_layouts.into_iter().map(Some).collect();
        surface.line_offsets = line_offsets;
        surface.line_lengths = line_lengths;

        let before_window = surface.visible_line_range(content.height());
        assert!(surface.scroll_by(bounds, Vector::new(0.0, 120.0)));
        let after_window = surface.visible_line_range(content.height());

        assert!(after_window.start >= before_window.start);
        assert_ne!(after_window, before_window);
    }

    #[test]
    fn text_surface_centers_shorter_line_layouts_in_global_line_slots() {
        let mut base_style = TextStyle::new(Color::BLACK);
        base_style.font_size = 14.0;
        base_style.line_height = 18.0;
        let mut tall_style = TextStyle::new(Color::BLACK);
        tall_style.font_size = 28.0;
        tall_style.line_height = 36.0;
        let value = "tiny\nheader";
        let tall_range = "tiny\n".len()..value.len();
        let surface = TextSurface::new("Editor")
            .text_style(base_style.clone())
            .value(value)
            .style_overlays(vec![TextSurfaceStyleOverlay::new(
                tall_range.clone(),
                tall_style,
                TextSurfaceOverlayKind::Syntax,
            )]);

        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(260.0, 96.0))
                .with_child(surface),
        );
        let output = runtime.render(window_id).expect("render should succeed");
        let shaped = shaped_text_commands(&output);
        let registry = output.frame.text_layout_registry.as_ref();
        let first = shaped
            .iter()
            .find(|text| {
                text.resolve(registry)
                    .is_some_and(|layout| layout.text() == "tiny")
            })
            .expect("first line should draw");
        let second = shaped
            .iter()
            .find(|text| {
                text.resolve(registry)
                    .is_some_and(|layout| layout.text() == "header")
            })
            .expect("second line should draw");
        let first_layout = first
            .resolve(registry)
            .expect("first layout should resolve");
        let second_layout = second
            .resolve(registry)
            .expect("second layout should resolve");
        let first_height = first_layout
            .measurement()
            .height
            .max(base_style.line_height);
        let second_height = second_layout
            .measurement()
            .height
            .max(base_style.line_height);
        let slot_height = first_height.max(second_height);
        assert!(
            first_height < slot_height,
            "fixture should produce a shorter first line"
        );

        let first_offset = (slot_height - first_height) * 0.5;
        let second_offset = (slot_height - second_height) * 0.5;
        let first_row_top = second.origin.y - slot_height - second_offset;
        assert_approx_eq(first.origin.y, first_row_top + first_offset);

        let text_system = sui_text::TextSystem::new();
        let mut geometry_surface = TextSurface::new("Editor")
            .text_style(base_style.clone())
            .value(value);
        geometry_surface.line_layouts = vec![
            Some(
                text_system
                    .shape_text_persistent(
                        None,
                        "tiny",
                        Size::new(240.0, base_style.line_height),
                        base_style.clone(),
                        &sui_text::FontRegistry::new(),
                    )
                    .expect("first line should shape"),
            ),
            Some(
                text_system
                    .shape_text_persistent(
                        None,
                        "header",
                        Size::new(240.0, base_style.line_height),
                        TextStyle {
                            font_size: 28.0,
                            line_height: 36.0,
                            ..base_style.clone()
                        },
                        &sui_text::FontRegistry::new(),
                    )
                    .expect("second line should shape"),
            ),
        ];
        geometry_surface.line_offsets = vec![0, "tiny\n".len()];
        geometry_surface.line_lengths = vec!["tiny".len(), "header".len()];
        let first_geometry_layout = geometry_surface.line_layouts[0]
            .as_ref()
            .expect("first line layout should exist");
        let local_caret = first_geometry_layout.caret_rect(0);
        let local_selection = first_geometry_layout.selection_rects(0.."tiny".len());
        let base_line_height = geometry_surface.resolved_text_style().line_height;
        let slot_height = geometry_surface.line_height();
        let first_origin_y =
            geometry_surface.line_origin_y(0, first_geometry_layout, base_line_height, slot_height);
        let caret = geometry_surface
            .caret_rect_for_cursor(TextCursor::new(0))
            .expect("caret should resolve");
        let selection = geometry_surface.selection_rects_for_display(&TextSelection::new(
            TextCursor::new(0),
            TextCursor::new("tiny".len()),
        ));

        assert_approx_eq(caret.y(), first_origin_y + local_caret.y());
        assert_eq!(selection.len(), local_selection.len());
        assert_approx_eq(selection[0].y(), first_origin_y + local_selection[0].y());
    }

    #[test]
    fn text_surface_supports_editing_and_keeps_layout_handle_on_cursor_moves() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextSurface::new("Editor").on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(24.0, 24.0), true),
            )
            .expect("focus click should succeed");
        runtime
            .handle_event(
                window_id,
                Event::Ime(ImeEvent::CompositionCommit {
                    text: "hello".to_string(),
                }),
            )
            .expect("ime commit should succeed");

        let after_insert = runtime
            .render(window_id)
            .expect("render after insert should succeed");
        let inserted_line = shaped_text_commands(&after_insert)
            .into_iter()
            .next()
            .expect("text surface should draw a shaped line after insert");
        let inserted_handle = inserted_line.layout_handle;
        let inserted_version = inserted_line.layout_version;

        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowLeft", KeyState::Pressed)),
            )
            .expect("arrow key should succeed");

        let after_move = runtime
            .render(window_id)
            .expect("render after cursor move should succeed");
        let moved_line = shaped_text_commands(&after_move)
            .into_iter()
            .next()
            .expect("text surface should draw a shaped line after cursor move");

        assert_eq!(changes.borrow().last().map(String::as_str), Some("hello"));
        assert_eq!(moved_line.layout_handle, inserted_handle);
        assert_eq!(moved_line.layout_version, inserted_version);
        assert!(after_move.ime_composition_rect.is_some());
        assert_eq!(
            after_move
                .semantics
                .iter()
                .find(|node| node.role == SemanticsRole::TextInput)
                .and_then(|node| node.value.clone()),
            Some(SemanticsValue::Text("hello".to_string()))
        );
    }

    #[test]
    fn text_surface_caret_uses_theme_palette_color() {
        let mut theme = DefaultTheme::default();
        theme.palette.caret = Color::rgba(0.02, 0.18, 0.72, 1.0);
        let caret_color = theme.palette.caret;
        let accent_text = theme.palette.accent_text;
        let (mut runtime, window_id) = build_runtime(
            TextSurface::new("Editor")
                .theme(theme)
                .value("Visible caret on white"),
        );

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(24.0, 24.0), true),
            )
            .expect("focus click should succeed");
        let output = runtime
            .render(window_id)
            .expect("focused render should succeed");
        let fill_colors = solid_fill_colors(&output);

        assert!(fill_colors.iter().any(|color| *color == caret_color));
        assert!(!fill_colors.iter().any(|color| *color == accent_text));
    }

    #[test]
    fn text_surface_placeholder_uses_placeholder_style_without_editable_value() {
        let mut theme = DefaultTheme::default();
        theme.palette.placeholder = Color::rgba(0.42, 0.47, 0.55, 1.0);
        let placeholder_color = theme.palette.placeholder;
        let mut text_style = theme.body_text_style();
        text_style.font_size = 28.0;
        text_style.line_height = 12.0;

        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(280.0, 96.0))
                .with_child(
                    TextSurface::new("Editor")
                        .theme(theme)
                        .text_style(text_style.clone())
                        .placeholder("Write notes"),
                ),
        );
        let output = runtime.render(window_id).expect("render should succeed");
        let registry = output.frame.text_layout_registry.as_ref();
        let (placeholder_command, placeholder) = shaped_text_commands(&output)
            .into_iter()
            .find_map(|text| {
                let layout = text.resolve(registry)?;
                (layout.text() == "Write notes").then(|| (text, layout.clone()))
            })
            .expect("placeholder text should draw");
        let node = text_input_node(&output);

        assert_eq!(node.value, Some(SemanticsValue::Text(String::new())));
        assert_eq!(placeholder_command.color_override, Some(placeholder_color));
        assert_eq!(placeholder.style().font_size, text_style.font_size);
        assert_eq!(placeholder.style().line_height, text_style.line_height);
        assert!(placeholder.measurement().height > text_style.line_height);
    }

    #[test]
    fn text_surface_read_only_uses_muted_text_and_blocks_mutation() {
        let mut theme = DefaultTheme::default();
        theme.palette.text_muted = Color::rgba(0.36, 0.40, 0.47, 1.0);
        let muted = theme.palette.text_muted;
        let caret = theme.palette.caret;
        let mut text_style = theme.body_text_style();
        text_style.font_size = 24.0;
        text_style.line_height = 13.0;
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(280.0, 96.0))
                .with_child(
                    TextSurface::new("Editor")
                        .theme(theme)
                        .text_style(text_style.clone())
                        .value("Pinned")
                        .read_only()
                        .on_change(move |value| on_change.borrow_mut().push(value)),
                ),
        );

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(32.0, 24.0), true),
            )
            .expect("focus click should succeed");
        runtime
            .handle_event(
                window_id,
                Event::Ime(ImeEvent::CompositionCommit {
                    text: " edited".to_string(),
                }),
            )
            .expect("read-only ime commit should be ignored");
        runtime
            .handle_event(window_id, key_event("Backspace"))
            .expect("read-only backspace should be ignored");
        runtime
            .handle_event(window_id, command_key_event("x"))
            .expect("read-only cut should be ignored");
        let output = runtime.render(window_id).expect("render should succeed");
        let node = text_input_node(&output);
        let editable = node
            .editable_text
            .as_ref()
            .expect("editable text semantics should be present");
        let registry = output.frame.text_layout_registry.as_ref();
        let text = shaped_text_commands(&output)
            .into_iter()
            .find_map(|text| {
                text.resolve(registry)
                    .filter(|layout| layout.text() == "Pinned")
                    .cloned()
            })
            .expect("read-only value should draw");
        let fill_colors = solid_fill_colors(&output);

        assert_eq!(node.value, Some(SemanticsValue::Text("Pinned".to_string())));
        assert!(editable.readonly);
        assert!(node.actions.contains(&SemanticsAction::Focus));
        assert!(node.actions.contains(&SemanticsAction::SetSelection));
        assert!(node.actions.contains(&SemanticsAction::Copy));
        for action in [
            SemanticsAction::SetValue,
            SemanticsAction::InsertText,
            SemanticsAction::DeleteBackward,
            SemanticsAction::DeleteForward,
            SemanticsAction::Cut,
            SemanticsAction::Paste,
            SemanticsAction::Undo,
            SemanticsAction::Redo,
        ] {
            assert!(
                !node.actions.contains(&action),
                "read-only surface should not expose {action:?}"
            );
        }
        assert_eq!(text.style().color, muted);
        assert_eq!(text.style().font_size, text_style.font_size);
        assert_eq!(text.style().line_height, text_style.line_height);
        assert!(changes.borrow().is_empty());
        assert!(!fill_colors.iter().any(|color| *color == caret));
    }

    #[test]
    fn text_surface_shortcuts_use_editor_command_layer() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextSurface::new("Editor").on_change(move |value| on_change.borrow_mut().push(value)),
        );

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(24.0, 24.0), true),
            )
            .expect("focus click should succeed");
        runtime
            .handle_event(
                window_id,
                Event::Ime(ImeEvent::CompositionCommit {
                    text: "hello world".to_string(),
                }),
            )
            .expect("initial edit should succeed");
        runtime
            .handle_event(window_id, command_key_event("a"))
            .expect("select all should succeed");
        runtime
            .handle_event(window_id, command_key_event("x"))
            .expect("cut should succeed");
        runtime
            .handle_event(window_id, command_key_event("v"))
            .expect("paste should succeed");
        runtime
            .handle_event(window_id, command_key_event("z"))
            .expect("undo should succeed");
        runtime
            .handle_event(window_id, command_key_event("y"))
            .expect("redo should succeed");

        let output = runtime.render(window_id).expect("render should succeed");
        assert_eq!(
            text_input_node(&output).value,
            Some(SemanticsValue::Text("hello world".to_string()))
        );
        assert_eq!(
            changes.borrow().last().map(String::as_str),
            Some("hello world")
        );
    }

    #[test]
    fn text_surface_deletes_grapheme_clusters() {
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(320.0, 80.0))
                .with_child(TextSurface::new("Editor").value("a🇯🇵e\u{301}z")),
        );

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(300.0, 24.0), true),
            )
            .expect("end click should succeed");
        runtime
            .handle_event(window_id, key_event("Backspace"))
            .expect("delete z should succeed");
        runtime
            .handle_event(window_id, key_event("Backspace"))
            .expect("delete combining grapheme should succeed");
        runtime
            .handle_event(window_id, key_event("Backspace"))
            .expect("delete flag grapheme should succeed");

        let output = runtime.render(window_id).expect("render should succeed");
        assert_eq!(
            text_input_node(&output).value,
            Some(SemanticsValue::Text("a".to_string()))
        );
    }

    #[test]
    fn text_surface_ime_cursor_range_updates_editable_semantics() {
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(320.0, 80.0))
                .with_child(TextSurface::new("Editor").value("hello ")),
        );

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(300.0, 24.0), true),
            )
            .expect("end click should succeed");
        runtime
            .handle_event(window_id, Event::Ime(ImeEvent::CompositionStart))
            .expect("composition start should succeed");
        runtime
            .handle_event(
                window_id,
                Event::Ime(ImeEvent::CompositionUpdate {
                    text: "世界".to_string(),
                    cursor_range: Some(0.."世".len()),
                }),
            )
            .expect("composition update should succeed");

        let output = runtime.render(window_id).expect("render should succeed");
        let node = text_input_node(&output);
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("hello 世界".to_string()))
        );
        let editable = node
            .editable_text
            .as_ref()
            .expect("editable text semantics should be present");
        assert_eq!(editable.caret_offset, "hello 世".len());
        assert_eq!(
            editable.selection,
            SemanticsTextRange::new("hello 世".len(), "hello 世".len())
        );
    }

    #[test]
    fn text_surface_semantics_expose_editable_text_actions() {
        let (mut runtime, window_id) = build_runtime(TextSurface::new("Editor").value("one\ntwo"));
        let output = runtime.render(window_id).expect("render should succeed");
        let node = text_input_node(&output);
        let editable = node
            .editable_text
            .as_ref()
            .expect("editable text semantics should be present");

        assert!(editable.multiline);
        assert!(!editable.readonly);
        assert_eq!(editable.caret_offset, "one\ntwo".len());
        assert_eq!(
            editable.selection,
            SemanticsTextRange::new("one\ntwo".len(), "one\ntwo".len())
        );
        for action in [
            SemanticsAction::SetSelection,
            SemanticsAction::InsertText,
            SemanticsAction::DeleteBackward,
            SemanticsAction::DeleteForward,
            SemanticsAction::Copy,
            SemanticsAction::Cut,
            SemanticsAction::Paste,
            SemanticsAction::Undo,
            SemanticsAction::Redo,
        ] {
            assert!(
                node.actions.contains(&action),
                "missing editable text action {action:?}"
            );
        }
    }

    #[test]
    fn text_surface_style_overlays_do_not_mutate_source_text() {
        let syntax_style = TextStyle::new(Color::BLACK);
        let (mut runtime, window_id) = build_runtime(
            TextSurface::new("Editor")
                .value("let value = 1")
                .style_overlays(vec![TextSurfaceStyleOverlay::new(
                    0..3,
                    syntax_style.clone(),
                    TextSurfaceOverlayKind::Syntax,
                )]),
        );

        let output = runtime.render(window_id).expect("render should succeed");
        let node = text_input_node(&output);
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("let value = 1".to_string()))
        );

        let shaped = shaped_text_commands(&output);
        let layout = shaped
            .first()
            .and_then(|text| text.resolve(output.frame.text_layout_registry.as_ref()))
            .expect("styled text layout should resolve");
        assert_eq!(layout.text(), "let value = 1");
        assert!(
            layout
                .run_views()
                .any(|run| run.style.color == syntax_style.color && run.run.byte_range == (0..3)),
            "syntax overlay should style the keyword without changing the source text"
        );
    }
}

fn split_lines_with_offsets(text: &str) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut offsets = Vec::new();
    let mut lengths = Vec::new();
    let mut start = 0usize;

    for segment in text.split('\n') {
        offsets.push(start);
        lengths.push(segment.len());
        lines.push(segment.to_string());
        start += segment.len() + 1;
    }

    if lines.is_empty() {
        lines.push(String::new());
        offsets.push(0);
        lengths.push(0);
    }

    (lines, offsets, lengths)
}
