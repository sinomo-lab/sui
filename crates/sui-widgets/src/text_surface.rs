use std::ops::Range;

use sui_core::{
    Color, Event, ImeEvent, KeyState, KeyboardEvent, Path, Point, PointerButton, PointerEventKind,
    Rect, ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector,
    WindowEvent,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::{LayerCompositionMode, StrokeStyle};
use sui_text::{
    PersistentTextLayout, TextCursor, TextDirection, TextSelection, TextStyle, TextWrap,
};

use crate::{DefaultTheme, ThemeColorScheme};

struct CompositionState {
    text: String,
    replacement_range: Range<usize>,
}

pub struct TextSurface {
    theme: Box<DefaultTheme>,
    name: String,
    value: String,
    composition: Option<CompositionState>,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    wrap: TextWrap,
    direction: TextDirection,
    selection: TextSelection,
    preferred_x: Option<f32>,
    hovered: bool,
    dragging_selection: bool,
    scroll_x: f32,
    scroll_y: f32,
    layout: Option<PersistentTextLayout>,
    line_layouts: Vec<PersistentTextLayout>,
    line_offsets: Vec<usize>,
    line_lengths: Vec<usize>,
    on_change: Option<Box<dyn FnMut(String)>>,
}

impl TextSurface {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            value: String::new(),
            composition: None,
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            wrap: TextWrap::NoWrap,
            direction: TextDirection::Auto,
            selection: TextSelection::new(TextCursor::new(0), TextCursor::new(0)),
            preferred_x: None,
            hovered: false,
            dragging_selection: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
            layout: None,
            line_layouts: Vec::new(),
            line_offsets: Vec::new(),
            line_lengths: Vec::new(),
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
            self.scroll_x = 0.0;
        }
        self
    }

    pub fn direction(mut self, direction: TextDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.set_value(value);
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        let cursor = TextCursor::new(self.value.len());
        self.selection = TextSelection::new(cursor, cursor);
        self.preferred_x = None;
        self.composition = None;
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

    fn selection_is_collapsed(&self) -> bool {
        self.selection.anchor.utf8_offset == self.selection.focus.utf8_offset
    }

    fn selection_range(&self) -> Range<usize> {
        let start = self.selection.anchor.utf8_offset.min(self.value.len());
        let end = self.selection.focus.utf8_offset.min(self.value.len());
        if start <= end { start..end } else { end..start }
    }

    fn display_text(&self) -> String {
        let Some(composition) = &self.composition else {
            return self.value.clone();
        };

        let mut text = self.value.clone();
        let range = composition.replacement_range.start.min(text.len())
            ..composition.replacement_range.end.min(text.len());
        text.replace_range(range, &composition.text);
        text
    }

    fn display_selection(&self) -> TextSelection {
        if let Some(composition) = &self.composition {
            let offset = composition.replacement_range.start + composition.text.len();
            let cursor = TextCursor::new(offset);
            TextSelection::new(cursor, cursor)
        } else {
            self.selection.clone()
        }
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
        if let Some(on_change) = &mut self.on_change {
            on_change(self.value.clone());
        }
    }

    fn clear_composition(&mut self) -> bool {
        let had_composition = self.composition.is_some();
        self.composition = None;
        had_composition
    }

    fn replace_selected_text(&mut self, replacement: &str) {
        let range = self.selection_range();
        self.value.replace_range(range.clone(), replacement);
        let offset = range.start + replacement.len();
        let cursor = TextCursor::new(offset);
        self.selection = TextSelection::new(cursor, cursor);
        self.preferred_x = None;
        self.commit_text_change();
    }

    fn backspace(&mut self) -> bool {
        if !self.selection_is_collapsed() {
            self.replace_selected_text("");
            return true;
        }

        let focus = clamp_offset_to_boundary(&self.value, self.selection.focus.utf8_offset);
        let previous = previous_boundary(&self.value, focus);
        if previous == focus {
            return false;
        }

        self.selection = TextSelection::new(TextCursor::new(previous), TextCursor::new(focus));
        self.replace_selected_text("");
        true
    }

    fn delete_forward(&mut self) -> bool {
        if !self.selection_is_collapsed() {
            self.replace_selected_text("");
            return true;
        }

        let focus = clamp_offset_to_boundary(&self.value, self.selection.focus.utf8_offset);
        let next = next_boundary(&self.value, focus);
        if next == focus {
            return false;
        }

        self.selection = TextSelection::new(TextCursor::new(focus), TextCursor::new(next));
        self.replace_selected_text("");
        true
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        self.replace_selected_text(text);
        true
    }

    fn set_collapsed_cursor(&mut self, offset: usize) {
        let offset = clamp_offset_to_boundary(&self.value, offset);
        let cursor = TextCursor::new(offset);
        self.selection = TextSelection::new(cursor, cursor);
        self.preferred_x = None;
    }

    fn set_selection(&mut self, anchor: usize, focus: usize) {
        self.selection = TextSelection::new(
            TextCursor::new(clamp_offset_to_boundary(&self.value, anchor)),
            TextCursor::new(clamp_offset_to_boundary(&self.value, focus)),
        );
    }

    fn move_horizontal(&mut self, backward: bool, extend: bool) -> bool {
        let range = self.selection_range();
        let current_focus = clamp_offset_to_boundary(&self.value, self.selection.focus.utf8_offset);
        let next = if !extend && range.start != range.end {
            if backward { range.start } else { range.end }
        } else if backward {
            previous_boundary(&self.value, current_focus)
        } else {
            next_boundary(&self.value, current_focus)
        };

        if extend {
            self.set_selection(self.selection.anchor.utf8_offset, next);
        } else {
            self.set_collapsed_cursor(next);
        }
        true
    }

    fn move_line_boundary(&mut self, to_start: bool, extend: bool) -> bool {
        let Some(layout) = self.layout.as_ref() else {
            return false;
        };

        let focus = if extend {
            self.selection.focus.utf8_offset
        } else {
            self.selection_range().end
        };
        let caret = layout.caret(TextCursor::new(focus));
        let line = &layout.lines()[caret.line_index];
        let target = if to_start {
            line.byte_range.start
        } else {
            line.byte_range.end
        };

        if extend {
            self.set_selection(self.selection.anchor.utf8_offset, target);
        } else {
            self.set_collapsed_cursor(target);
        }
        self.preferred_x = None;
        true
    }

    fn move_vertical(&mut self, delta_lines: isize, extend: bool, viewport_height: f32) -> bool {
        let Some(layout) = self.layout.as_ref() else {
            return false;
        };

        if layout.lines().is_empty() {
            return false;
        }

        let focus = if extend {
            self.selection.focus.utf8_offset
        } else {
            self.selection_range().end
        };
        let caret = layout.caret(TextCursor::new(focus));
        let preferred_x = self.preferred_x.unwrap_or(caret.rect.x());
        let line_index = caret.line_index as isize + delta_lines;
        let target_line =
            line_index.clamp(0, layout.lines().len().saturating_sub(1) as isize) as usize;
        let line = &layout.lines()[target_line];
        let target = layout.hit_test_point(Point::new(
            preferred_x,
            line.rect.y() + (line.rect.height() * 0.5),
        ));

        if extend {
            self.set_selection(self.selection.anchor.utf8_offset, target.utf8_offset);
        } else {
            self.set_collapsed_cursor(target.utf8_offset);
        }
        self.preferred_x = Some(preferred_x);

        if viewport_height > 0.0 { true } else { false }
    }

    fn select_all(&mut self) {
        self.selection = TextSelection::new(TextCursor::new(0), TextCursor::new(self.value.len()));
        self.preferred_x = None;
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
            let local_x = position.x - content.x() + self.scroll_x;
            let local_y = position.y - content.y() + self.scroll_y;
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
            position.x - content.x() + self.scroll_x,
            position.y - content.y() + self.scroll_y,
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
            let visible_top = (self.scroll_y - overdraw).max(0.0);
            let visible_bottom = self.scroll_y + viewport_height + overdraw;
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
        let visible_top = (self.scroll_y - overdraw).max(0.0);
        let visible_bottom = self.scroll_y + viewport_height + overdraw;
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
        let previous = (self.scroll_x, self.scroll_y);
        let content_size = if !self.line_layouts.is_empty() {
            self.multi_line_content_size()
        } else if let Some(layout) = self.layout.as_ref() {
            layout_content_size(layout)
        } else {
            self.scroll_x = 0.0;
            self.scroll_y = 0.0;
            return previous != (0.0, 0.0);
        };

        let max_x = if self.wrap == TextWrap::NoWrap {
            (content_size.width - viewport_size.width).max(0.0)
        } else {
            0.0
        };
        let max_y = (content_size.height - viewport_size.height).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
        previous != (self.scroll_x, self.scroll_y)
    }

    fn ensure_caret_visible(&mut self, bounds: Rect) -> bool {
        let viewport = self.content_viewport_size(bounds);
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return false;
        }

        let previous = (self.scroll_x, self.scroll_y);
        let Some(caret) = self.caret_rect_for_cursor(self.display_selection().focus) else {
            return false;
        };
        if self.wrap == TextWrap::NoWrap {
            if caret.x() < self.scroll_x {
                self.scroll_x = caret.x().max(0.0);
            } else if caret.max_x() > self.scroll_x + viewport.width {
                self.scroll_x = (caret.max_x() - viewport.width).max(0.0);
            }
        } else {
            self.scroll_x = 0.0;
        }
        if caret.y() < self.scroll_y {
            self.scroll_y = caret.y().max(0.0);
        } else if caret.max_y() > self.scroll_y + viewport.height {
            self.scroll_y = (caret.max_y() - viewport.height).max(0.0);
        }
        let _ = self.clamp_scroll_to_bounds(viewport);
        previous != (self.scroll_x, self.scroll_y)
    }

    fn scroll_by(&mut self, bounds: Rect, delta: Vector) -> bool {
        let previous = (self.scroll_x, self.scroll_y);
        if self.wrap == TextWrap::NoWrap {
            self.scroll_x += delta.x;
        }
        self.scroll_y += delta.y;
        let _ = self.clamp_scroll_to_bounds(self.content_viewport_size(bounds));
        previous != (self.scroll_x, self.scroll_y)
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
            let range = selection_sorted_range(selection, self.display_text().len());
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
                {
                    if let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position) {
                        self.set_selection(self.selection.anchor.utf8_offset, cursor.utf8_offset);
                        self.request_after_overlay_change(ctx);
                        ctx.set_handled();
                    }
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.hovered = true;
                if self.clear_composition() {
                    ctx.request_measure();
                    ctx.request_text();
                }
                if let Some(cursor) = self.point_to_cursor(ctx.bounds(), pointer.position) {
                    if pointer.modifiers.shift {
                        self.set_selection(self.selection.anchor.utf8_offset, cursor.utf8_offset);
                    } else {
                        self.set_collapsed_cursor(cursor.utf8_offset);
                    }
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
                self.composition = Some(CompositionState {
                    text: String::new(),
                    replacement_range: self.selection_range(),
                });
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionUpdate { text }) if ctx.is_focused() => {
                let range = self.selection_range();
                self.composition = Some(CompositionState {
                    text: text.clone(),
                    replacement_range: range,
                });
                ctx.request_measure();
                ctx.request_text();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                if let Some(composition) = &self.composition {
                    self.set_selection(
                        composition.replacement_range.start,
                        composition.replacement_range.end,
                    );
                }
                self.composition = None;
                if self.insert_text(text) {
                    ctx.request_measure();
                    ctx.request_text();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                if self.clear_composition() {
                    ctx.request_measure();
                    ctx.request_text();
                    ctx.request_semantics();
                }
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let mut text_changed = false;
                let mut overlay_changed = false;
                let mut semantics_changed = false;

                match key.key.as_str() {
                    "a" | "A" if key.modifiers.control => {
                        let _ = self.clear_composition();
                        self.select_all();
                        overlay_changed = true;
                    }
                    "ArrowLeft" => {
                        let had_composition = self.clear_composition();
                        overlay_changed = self.move_horizontal(true, key.modifiers.shift);
                        text_changed |= had_composition;
                    }
                    "ArrowRight" => {
                        let had_composition = self.clear_composition();
                        overlay_changed = self.move_horizontal(false, key.modifiers.shift);
                        text_changed |= had_composition;
                    }
                    "ArrowUp" => {
                        let had_composition = self.clear_composition();
                        let viewport_height = self.content_viewport_size(ctx.bounds()).height;
                        overlay_changed =
                            self.move_vertical(-1, key.modifiers.shift, viewport_height);
                        text_changed |= had_composition;
                    }
                    "ArrowDown" => {
                        let had_composition = self.clear_composition();
                        let viewport_height = self.content_viewport_size(ctx.bounds()).height;
                        overlay_changed =
                            self.move_vertical(1, key.modifiers.shift, viewport_height);
                        text_changed |= had_composition;
                    }
                    "Home" => {
                        let had_composition = self.clear_composition();
                        overlay_changed = self.move_line_boundary(true, key.modifiers.shift);
                        text_changed |= had_composition;
                    }
                    "End" => {
                        let had_composition = self.clear_composition();
                        overlay_changed = self.move_line_boundary(false, key.modifiers.shift);
                        text_changed |= had_composition;
                    }
                    "PageUp" => {
                        let had_composition = self.clear_composition();
                        let layout = self.layout.as_ref();
                        if let Some(layout) = layout {
                            let line_height = layout
                                .lines()
                                .get(layout.caret(self.selection.focus).line_index)
                                .map(|line| line.rect.height())
                                .unwrap_or(self.resolved_text_style().line_height)
                                .max(1.0);
                            let delta = (self.content_viewport_size(ctx.bounds()).height
                                / line_height)
                                .floor()
                                .max(1.0) as isize;
                            overlay_changed = self.move_vertical(
                                -delta,
                                key.modifiers.shift,
                                self.content_viewport_size(ctx.bounds()).height,
                            );
                        }
                        text_changed |= had_composition;
                    }
                    "PageDown" => {
                        let had_composition = self.clear_composition();
                        let layout = self.layout.as_ref();
                        if let Some(layout) = layout {
                            let line_height = layout
                                .lines()
                                .get(layout.caret(self.selection.focus).line_index)
                                .map(|line| line.rect.height())
                                .unwrap_or(self.resolved_text_style().line_height)
                                .max(1.0);
                            let delta = (self.content_viewport_size(ctx.bounds()).height
                                / line_height)
                                .floor()
                                .max(1.0) as isize;
                            overlay_changed = self.move_vertical(
                                delta,
                                key.modifiers.shift,
                                self.content_viewport_size(ctx.bounds()).height,
                            );
                        }
                        text_changed |= had_composition;
                    }
                    "Backspace" => {
                        let had_composition = self.clear_composition();
                        text_changed = self.backspace() || had_composition;
                        semantics_changed = text_changed;
                    }
                    "Delete" => {
                        let had_composition = self.clear_composition();
                        text_changed = self.delete_forward() || had_composition;
                        semantics_changed = text_changed;
                    }
                    "Enter" => {
                        let _ = self.clear_composition();
                        text_changed = self.insert_text("\n");
                        semantics_changed = text_changed;
                    }
                    _ if self.composition.is_none() => {
                        if let Some(text) = keyboard_text(key) {
                            text_changed = self.insert_text(text);
                            semantics_changed = text_changed;
                        }
                    }
                    _ => {}
                }

                if text_changed {
                    ctx.request_measure();
                    ctx.request_text();
                } else if overlay_changed {
                    self.request_after_overlay_change(ctx);
                }
                if semantics_changed {
                    ctx.request_semantics();
                }
                if text_changed || overlay_changed {
                    ctx.set_handled();
                }
            }
            Event::Window(WindowEvent::Focused(false)) => {
                if self.clear_composition() {
                    ctx.request_measure();
                    ctx.request_text();
                }
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
        let (line_texts, line_offsets, line_lengths) = split_lines_with_offsets(&display_text);
        let line_box_size = Size::new(
            self.layout_box_size(available_width).width,
            self.resolved_text_style().line_height.max(1.0),
        );
        let mut next_line_layouts: Vec<PersistentTextLayout> = Vec::with_capacity(line_texts.len());
        let mut line_layout_failed = false;
        for (index, line) in line_texts.iter().enumerate() {
            match ctx.shape_text_persistent(
                self.line_layouts.get(index).map(|layout| layout.handle()),
                line.clone(),
                line_box_size,
                self.resolved_text_style(),
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
        } else {
            self.line_layouts.clear();
            self.line_offsets.clear();
            self.line_lengths.clear();
        }

        let layout = ctx
            .shape_text_persistent(
                self.layout.as_ref().map(|layout| layout.handle()),
                display_text,
                self.layout_box_size(available_width),
                self.resolved_text_style(),
            )
            .ok();
        self.layout = (self.line_layouts.len() <= 1).then_some(layout).flatten();

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
            palette.surface_hover
        } else {
            palette.surface
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
        let origin = Point::new(content.x() - self.scroll_x, content.y() - self.scroll_y);
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
                }),
            );
        } else {
            for rect in selection_rects {
                ctx.fill(
                    Path::rounded_rect(rect.translate(origin.to_vector()), 3.0),
                    palette.accent.with_alpha(0.22),
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
            ctx.fill(
                Path::rounded_rect(caret, caret_width * 0.5),
                palette.accent_text,
            );
        }

        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            composition_mode: LayerCompositionMode::Scroll,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.display_text()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.clear_composition() {
            ctx.request_measure();
            ctx.request_text();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn clamp_offset_to_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn previous_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset_to_boundary(text, offset);
    if offset == 0 {
        return 0;
    }

    text[..offset]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset_to_boundary(text, offset);
    if offset >= text.len() {
        return text.len();
    }

    text[offset..]
        .chars()
        .next()
        .map(|ch| offset + ch.len_utf8())
        .unwrap_or(text.len())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    use sui_core::{
        Event, Modifiers, PointerButtons, PointerEvent, PointerKind, SemanticsValue, WindowId,
    };
    use sui_runtime::{Application, RenderOutput, Runtime, WindowBuilder};
    use sui_scene::SceneCommand;

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

    fn shaped_text_commands(output: &RenderOutput) -> Vec<sui_text::ShapedText> {
        let mut found = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command {
                found.push(text.clone());
            }
        });
        found
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
}

fn selection_sorted_range(selection: &TextSelection, text_len: usize) -> Range<usize> {
    let start = selection.anchor.utf8_offset.min(text_len);
    let end = selection.focus.utf8_offset.min(text_len);
    if start <= end { start..end } else { end..start }
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
