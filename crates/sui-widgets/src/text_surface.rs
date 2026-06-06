use std::ops::Range;

use sui_core::{
    Color, EditableTextSemantics, Event, ImeEvent, KeyState, KeyboardEvent, Path, Point,
    PointerButton, PointerEventKind, Rect, ScrollDelta, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsTextRange, SemanticsValue, Size, Vector, WindowEvent,
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
    DefaultTheme, ThemeColorScheme,
    editor::{
        EditorCommand, EditorCommandResult, EditorState, clamp_to_grapheme_boundary,
        selection_range,
    },
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
    dragging_selection: bool,
    layout: Option<PersistentTextLayout>,
    line_layouts: Vec<PersistentTextLayout>,
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

    fn display_selection(&self) -> TextSelection {
        self.editor.display_selection()
    }

    fn layout_box_size(&self, available_width: f32) -> Size {
        let style = self.resolved_text_style();
        let estimated_lines = self.display_text().lines().count().max(1) as f32;
        Size::new(
            if self.wrap == TextWrap::NoWrap {
                1_000_000.0
            } else {
                available_width.max(1.0)
            },
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

        let target = if !self.line_layouts.is_empty() {
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

        if !self.line_layouts.is_empty() {
            let Some(caret) = self.caret_rect_for_cursor(TextCursor::new(focus)) else {
                return EditorCommandResult::default();
            };
            let preferred_x = self.editor.preferred_x().unwrap_or(caret.x());
            let line_index = self.line_index_for_offset(focus) as isize + delta_lines;
            let target_line =
                line_index.clamp(0, self.line_layouts.len().saturating_sub(1) as isize) as usize;
            let Some(layout) = self.line_layouts.get(target_line) else {
                return EditorCommandResult::default();
            };
            let Some(line) = layout.lines().first() else {
                return EditorCommandResult::default();
            };
            let local = layout.hit_test_point(Point::new(
                preferred_x,
                line.rect.y() + (line.rect.height() * 0.5),
            ));
            let target = self.line_offsets[target_line]
                + local.utf8_offset.min(self.line_lengths[target_line]);
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
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn point_to_cursor(&self, bounds: Rect, position: Point) -> Option<TextCursor> {
        let content = self.content_rect(bounds);
        if !content.contains(position) {
            return None;
        }

        if !self.line_layouts.is_empty() {
            let local_x = position.x - content.x() + self.editor.scroll_x();
            let local_y = position.y - content.y() + self.editor.scroll_y();
            let line_index = self.line_index_for_y(local_y);
            let layout = self.line_layouts.get(line_index)?;
            let line = layout.lines().first()?;
            let local = layout.hit_test_point(Point::new(
                local_x,
                line.rect.y() + (line.rect.height() * 0.5),
            ));
            return Some(TextCursor::new(
                self.line_offsets[line_index]
                    + local.utf8_offset.min(self.line_lengths[line_index]),
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
        if !self.line_layouts.is_empty() {
            if viewport_height <= 0.0 {
                return 0..0;
            }

            let line_height = self.line_height().max(1.0);
            let overdraw = viewport_height * 0.5;
            let visible_top = (self.editor.scroll_y() - overdraw).max(0.0);
            let visible_bottom = self.editor.scroll_y() + viewport_height + overdraw;
            let start = (visible_top / line_height).floor() as usize;
            let end = ((visible_bottom / line_height).ceil() as usize + 1)
                .min(self.line_layouts.len())
                .max(start + usize::from(start < self.line_layouts.len()));
            return start.min(self.line_layouts.len())..end;
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
        let content_size = if !self.line_layouts.is_empty() {
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

    fn line_height(&self) -> f32 {
        self.line_layouts
            .iter()
            .map(|layout| {
                layout
                    .measurement()
                    .height
                    .max(self.resolved_text_style().line_height)
            })
            .fold(self.resolved_text_style().line_height, f32::max)
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        let offset = offset.min(self.display_text().len());
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
        ((y / line_height).floor() as usize).min(self.line_layouts.len().saturating_sub(1))
    }

    fn caret_rect_for_cursor(&self, cursor: TextCursor) -> Option<Rect> {
        if !self.line_layouts.is_empty() {
            let line_index = self.line_index_for_offset(cursor.utf8_offset);
            let layout = self.line_layouts.get(line_index)?;
            let local_offset = cursor
                .utf8_offset
                .saturating_sub(self.line_offsets[line_index])
                .min(self.line_lengths[line_index]);
            return Some(
                layout
                    .caret_rect(local_offset)
                    .translate(Vector::new(0.0, line_index as f32 * self.line_height())),
            );
        }

        self.layout.as_ref().map(|layout| layout.caret(cursor).rect)
    }

    fn selection_rects_for_display(&self, selection: &TextSelection) -> Vec<Rect> {
        if !self.line_layouts.is_empty() {
            let mut rects = Vec::new();
            let range = selection_range(selection, self.display_text().len());
            for (line_index, layout) in self.line_layouts.iter().enumerate() {
                let line_start = self.line_offsets[line_index];
                let line_end = line_start + self.line_lengths[line_index];
                let selection_start = range.start.max(line_start);
                let selection_end = range.end.min(line_end);
                if selection_start >= selection_end {
                    continue;
                }

                let local_rects = layout.selection_rects(
                    selection_start.saturating_sub(line_start)
                        ..selection_end.saturating_sub(line_start),
                );
                rects.extend(local_rects.into_iter().map(|rect| {
                    rect.translate(Vector::new(0.0, line_index as f32 * self.line_height()))
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
        Size::new(
            self.line_layouts
                .iter()
                .map(|layout| layout.measurement().width)
                .fold(0.0_f32, f32::max),
            self.line_layouts.len() as f32 * self.line_height(),
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
                self.hovered = true;
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
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                self.execute_editor_command(ctx, EditorCommand::StartComposition);
            }
            Event::Ime(ImeEvent::CompositionUpdate { text, cursor_range }) if ctx.is_focused() => {
                self.execute_editor_command(
                    ctx,
                    EditorCommand::UpdateComposition {
                        text: text.clone(),
                        cursor_range: cursor_range.clone(),
                    },
                );
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                self.execute_editor_command(ctx, EditorCommand::CommitComposition(text.clone()));
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                self.execute_editor_command(ctx, EditorCommand::EndComposition);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let command_modifier = key.modifiers.control || key.modifiers.meta;
                let result = match key.key.as_str() {
                    "a" | "A" if command_modifier => self.editor.execute(EditorCommand::SelectAll),
                    "c" | "C" if command_modifier => self.editor.execute(EditorCommand::Copy),
                    "x" | "X" if command_modifier => self.editor.execute(EditorCommand::Cut),
                    "v" | "V" if command_modifier => self
                        .editor
                        .execute(EditorCommand::Paste(self.clipboard.clone())),
                    "z" | "Z" if command_modifier && key.modifiers.shift => {
                        self.editor.execute(EditorCommand::Redo)
                    }
                    "z" | "Z" if command_modifier => self.editor.execute(EditorCommand::Undo),
                    "y" | "Y" if command_modifier => self.editor.execute(EditorCommand::Redo),
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
                    "Backspace" => self.editor.execute(EditorCommand::DeleteBackward),
                    "Delete" => self.editor.execute(EditorCommand::DeleteForward),
                    "Enter" => self
                        .editor
                        .execute(EditorCommand::InsertText("\n".to_string())),
                    _ if self.editor.composition().is_none() => {
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

        let display_text = self.display_text();
        let line_box_size = Size::new(
            self.layout_box_size(available_width).width,
            self.resolved_text_style().line_height.max(1.0),
        );
        let line_style = self.resolved_text_style();
        let mut line_layout_failed = false;

        if self.editor.composition().is_none() {
            let document = self.editor.document();
            let line_count = document.line_count();
            let can_reuse_lines = self.line_layout_revision != u64::MAX
                && self.line_layout_box_size == Some(line_box_size)
                && self.line_layout_style.as_ref() == Some(&line_style)
                && self.line_layout_style_revision == self.style_revision
                && self.line_layouts.len() == line_count
                && self.line_offsets.len() == line_count
                && self.line_lengths.len() == line_count;

            if can_reuse_lines {
                let dirty = document.dirty_line_range();
                for index in dirty {
                    if index >= line_count {
                        continue;
                    }
                    let line_range = document.line_range(index);
                    match self.shape_line_layout(
                        ctx,
                        self.line_layouts.get(index).map(|layout| layout.handle()),
                        document.line_text(index),
                        line_range.clone(),
                        line_box_size,
                        line_style.clone(),
                    ) {
                        Ok(layout) => {
                            self.line_layouts[index] = layout;
                            self.line_offsets[index] = line_range.start;
                            self.line_lengths[index] = line_range.len();
                        }
                        Err(_) => {
                            line_layout_failed = true;
                            break;
                        }
                    }
                }
            } else {
                let mut next_line_layouts: Vec<PersistentTextLayout> =
                    Vec::with_capacity(line_count);
                let mut next_line_offsets = Vec::with_capacity(line_count);
                let mut next_line_lengths = Vec::with_capacity(line_count);
                for index in 0..line_count {
                    let line_range = document.line_range(index);
                    match self.shape_line_layout(
                        ctx,
                        self.line_layouts.get(index).map(|layout| layout.handle()),
                        document.line_text(index),
                        line_range.clone(),
                        line_box_size,
                        line_style.clone(),
                    ) {
                        Ok(layout) => {
                            next_line_layouts.push(layout);
                            next_line_offsets.push(line_range.start);
                            next_line_lengths.push(line_range.len());
                        }
                        Err(_) => {
                            line_layout_failed = true;
                            break;
                        }
                    }
                }
                if !line_layout_failed {
                    self.line_layouts = next_line_layouts;
                    self.line_offsets = next_line_offsets;
                    self.line_lengths = next_line_lengths;
                }
            }

            if !line_layout_failed {
                self.line_layout_box_size = Some(line_box_size);
                self.line_layout_style = Some(line_style.clone());
                self.line_layout_revision = document.revision();
                self.line_layout_style_revision = self.style_revision;
                self.editor.clear_document_dirty();
            }
        } else {
            let (line_texts, line_offsets, line_lengths) = split_lines_with_offsets(&display_text);
            let mut next_line_layouts: Vec<PersistentTextLayout> =
                Vec::with_capacity(line_texts.len());
            for (index, line) in line_texts.iter().enumerate() {
                let line_range = line_offsets[index]..line_offsets[index] + line_lengths[index];
                match self.shape_line_layout(
                    ctx,
                    self.line_layouts.get(index).map(|layout| layout.handle()),
                    line,
                    line_range,
                    line_box_size,
                    line_style.clone(),
                ) {
                    Ok(layout) => next_line_layouts.push(layout),
                    Err(_) => {
                        line_layout_failed = true;
                        break;
                    }
                }
            }
            if !line_layout_failed {
                self.line_layouts = next_line_layouts;
                self.line_offsets = line_offsets;
                self.line_lengths = line_lengths;
                self.line_layout_box_size = Some(line_box_size);
                self.line_layout_style = Some(line_style.clone());
                self.line_layout_revision = u64::MAX;
                self.line_layout_style_revision = self.style_revision;
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

        self.layout = if self.line_layouts.len() <= 1 {
            ctx.layout()
                .shape_text_persistent(
                    self.layout.as_ref().map(|layout| layout.handle()),
                    display_text,
                    self.layout_box_size(available_width),
                    self.resolved_text_style(),
                )
                .ok()
        } else {
            None
        };

        let natural_content_size = if !self.line_layouts.is_empty() {
            self.multi_line_content_size()
        } else {
            self.layout
                .as_ref()
                .map(layout_content_size)
                .unwrap_or(Size::new(min_size.width, min_size.height))
        };
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
        let background = if ctx.is_focused() {
            palette.surface_focus
        } else if self.hovered {
            palette.control_hover
        } else {
            palette.control
        };
        let border = if ctx.is_focused() {
            palette.border_focus
        } else if self.hovered {
            palette.border_hover
        } else {
            palette.border
        };

        draw_surface_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics.border_width,
            background,
            border,
            ctx.is_focused().then_some(palette.focus_ring),
            metrics.focus_ring_width,
            metrics.focus_ring_outset,
        );

        if self.line_layouts.is_empty() && self.layout.is_none() {
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

        ctx.push_clip_rect(content);

        if selection_rects.is_empty() {
            let line_rect = Rect::new(
                content.x(),
                origin.y + current_line_index as f32 * self.line_height(),
                content.width(),
                self.line_height(),
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

        if !self.line_layouts.is_empty() {
            for line_index in line_range {
                if let Some(layout) = self.line_layouts.get(line_index) {
                    ctx.draw_persistent_text_layout(
                        Point::new(
                            origin.x,
                            origin.y + (line_index as f32 * self.line_height()),
                        ),
                        layout,
                    );
                }
            }
        } else if let Some(layout) = &self.layout {
            ctx.draw_persistent_text_layout_window(origin, layout, line_range);
        }

        if ctx.is_focused() {
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
            readonly: false,
            scroll_x: self.editor.scroll_x(),
            scroll_y: self.editor.scroll_y(),
        });
        node.actions = vec![
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
        ];
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
        ctx.request_paint();
        ctx.request_semantics();
    }
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
    use sui_runtime::{Application, RenderOutput, Runtime, WindowBuilder};
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
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command {
                found.push(text.clone());
            }
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

    #[test]
    fn text_surface_submits_only_visible_line_windows() {
        let long_text = (0..24)
            .map(|index| format!("line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(180.0, 96.0))
                .with_child(TextSurface::new("Editor").value(long_text)),
        );

        let output = runtime.render(window_id).expect("render should succeed");
        let shaped = shaped_text_commands(&output);

        assert!(
            shaped.len() < 24,
            "expected visible-line submission only, emitted {} shaped lines",
            shaped.len(),
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
        surface.line_layouts = line_layouts;
        surface.line_offsets = line_offsets;
        surface.line_lengths = line_lengths;

        let before_window = surface.visible_line_range(content.height());
        assert!(surface.scroll_by(bounds, Vector::new(0.0, 120.0)));
        let after_window = surface.visible_line_range(content.height());

        assert!(after_window.start >= before_window.start);
        assert_ne!(after_window, before_window);
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
