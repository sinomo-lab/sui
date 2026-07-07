use std::{
    fmt,
    rc::Rc,
    time::{Duration, Instant},
};

use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, ToggleState, Transform,
    Vector, WakeEvent, WidgetId,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild, Widget,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_text::{
    FontFeature, FontWeight, TextAlign, TextDocument, TextLayoutRequest, TextMeasurement,
    TextStyle, TextWrap,
};

use crate::{
    DefaultTheme, MotionScalar, ThemeTextToken,
    controls::{IconGlyph, draw_icon_glyph},
    text_align::{paint_aligned_text, vertically_centered_text_rect_y},
};

pub struct ListItem {
    label: String,
    detail: Option<String>,
    semantic_name: Option<String>,
    semantic_description: Option<String>,
    trailing: Option<String>,
    leading_icon: Option<IconGlyph>,
    leading_text: Option<String>,
    leading_color: Option<Color>,
    accent: Option<Color>,
    disabled: bool,
    activate_with_child: bool,
    content: Option<SingleChild>,
}

impl ListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            semantic_name: None,
            semantic_description: None,
            trailing: None,
            leading_icon: None,
            leading_text: None,
            leading_color: None,
            accent: None,
            disabled: false,
            activate_with_child: false,
            content: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn subtitle(self, subtitle: impl Into<String>) -> Self {
        self.detail(subtitle)
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.semantic_description = Some(description.into());
        self
    }

    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    pub fn leading_icon(mut self, icon: IconGlyph) -> Self {
        self.leading_icon = Some(icon);
        self.leading_text = None;
        self
    }

    pub fn leading_text(mut self, text: impl Into<String>) -> Self {
        self.leading_text = Some(text.into());
        self.leading_icon = None;
        self
    }

    pub fn leading_color(mut self, color: Color) -> Self {
        self.leading_color = Some(color);
        self
    }

    pub fn accent(mut self, accent: Color) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn activate_with_child(mut self) -> Self {
        self.activate_with_child = true;
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.content = Some(SingleChild::new(child));
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn has_child(&self) -> bool {
        self.content.is_some()
    }

    fn can_activate_from_row(&self) -> bool {
        !self.disabled && (self.content.is_none() || self.activate_with_child)
    }
}

pub struct ListView {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    items: Vec<ListItem>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hover_motion: IndexedInteractionMotion<usize>,
    press_motion: IndexedInteractionMotion<usize>,
    focus_animation: AnimatedScalar,
    row_height: Option<f32>,
    scroll_y: f32,
    row_heights: Vec<f32>,
    row_offsets: Vec<f32>,
    content_height: f32,
    on_change: Option<ListViewChange>,
    on_change_with_ctx: Option<ListViewContextChange>,
}

impl ListView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            items: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            hover_motion: IndexedInteractionMotion::new(),
            press_motion: IndexedInteractionMotion::new(),
            focus_animation: AnimatedScalar::new(0.0),
            row_height: None,
            scroll_y: 0.0,
            row_heights: Vec::new(),
            row_offsets: Vec::new(),
            content_height: 0.0,
            on_change: None,
            on_change_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn item(mut self, item: ListItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = ListItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = Some(row_height.max(0.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String, &mut EventCtx) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    fn sync_selected(&mut self) {
        if self.selected_reader.is_some() {
            self.selected = self.current_selected();
        } else if self
            .selected
            .is_some_and(|selected| selected >= self.items.len())
        {
            self.selected = None;
        }
    }

    fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.items.len())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_row_height(&self) -> f32 {
        let theme = self.resolved_theme();
        let base = self.row_height.unwrap_or(theme.metrics.list_row_height);
        if self.items.iter().any(|item| item.detail.is_some()) {
            base.max(two_line_row_height(
                theme.body_text_style().line_height,
                caption_style(&theme).line_height,
            ))
        } else {
            base
        }
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.resolved_theme().metrics.data_viewport_padding)
    }

    fn measured_content_height(&self) -> f32 {
        if self.row_heights.len() == self.items.len() {
            self.content_height
        } else {
            self.items.len() as f32 * self.resolved_row_height()
        }
    }

    fn clamp_scroll(&self, viewport_height: f32, scroll_y: f32) -> f32 {
        let max_scroll = (self.measured_content_height() - viewport_height).max(0.0);
        scroll_y.clamp(0.0, max_scroll)
    }

    fn row_metrics(&self, index: usize) -> Option<(f32, f32)> {
        self.row_offsets
            .get(index)
            .zip(self.row_heights.get(index))
            .map(|(offset, height)| (*offset, *height))
            .or_else(|| {
                (index < self.items.len()).then(|| {
                    let row_height = self.resolved_row_height();
                    (index as f32 * row_height, row_height)
                })
            })
    }

    fn visible_row_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let viewport = self.viewport_rect(bounds);
        let (top, row_height) = self.row_metrics(index)?;
        let y = viewport.y() + top - self.scroll_y;
        Rect::new(viewport.x(), y, viewport.width(), row_height)
            .intersection(viewport)
            .filter(|rect| !rect.is_empty())
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let viewport = self.viewport_rect(bounds);
        if !viewport.contains(position) {
            return None;
        }

        let y = position.y - viewport.y() + self.scroll_y;
        (0..self.items.len()).find(|index| {
            self.row_metrics(*index)
                .is_some_and(|(top, height)| y >= top && y < top + height)
        })
    }

    fn ensure_visible(&mut self, viewport_height: f32, index: usize) {
        let Some((top, row_height)) = self.row_metrics(index) else {
            return;
        };
        let bottom = top + row_height;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if bottom > self.scroll_y + viewport_height {
            self.scroll_y = bottom - viewport_height;
        }
        self.scroll_y = self.clamp_scroll(viewport_height, self.scroll_y);
    }

    fn row_blocks_parent_activation(&self, index: usize) -> bool {
        self.items
            .get(index)
            .is_some_and(|item| item.has_child() && !item.activate_with_child)
    }

    fn row_activation_waits_for_child_target(&self, index: usize) -> bool {
        self.items
            .get(index)
            .is_some_and(|item| item.has_child() && item.activate_with_child)
    }

    fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        if item.disabled {
            return;
        }
        let label = item.label.clone();

        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, label.clone());
        }
        if let Some(on_change) = &mut self.on_change_with_ctx {
            on_change(index, label, ctx);
        }
    }

    fn move_selection(&mut self, delta: isize, viewport_height: f32, ctx: &mut EventCtx) {
        if self.items.is_empty() {
            return;
        }

        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.items.len() as isize - 1) as usize;
        self.activate(next, ctx);
        self.ensure_visible(viewport_height, next);
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }

        self.hovered = hovered;
        let theme = self.resolved_theme();
        self.hover_motion.set_hover_target(hovered, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        let theme = self.resolved_theme();
        self.press_motion.set_press_target(pressed, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (hover_changed, hover_active) = self.hover_motion.advance(time);
        let (press_changed, press_active) = self.press_motion.advance(time);
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);

        if hover_changed || press_changed || focus_changed {
            ctx.request_paint();
        }
        if hover_active || press_active || focus_active {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for ListView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_selected();
        let viewport = self.viewport_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && viewport.contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                let next = self.clamp_scroll(viewport.height(), self.scroll_y - delta.y);
                if (next - self.scroll_y).abs() > f32::EPSILON {
                    self.scroll_y = next;
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && viewport.contains(pointer.position) =>
            {
                let row = self.row_at_position(ctx.bounds(), pointer.position);
                if row.is_some_and(|index| self.row_blocks_parent_activation(index)) {
                    return;
                }
                if ctx.phase() == EventPhase::Capture
                    && row.is_some_and(|index| self.row_activation_waits_for_child_target(index))
                {
                    return;
                }
                self.set_hovered(row, ctx);
                self.set_pressed(row, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self.pressed.is_none() {
                    return;
                }
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(pressed, hovered)| pressed == hovered)
                    .map(|(index, _)| index)
                {
                    if !self.row_blocks_parent_activation(index) {
                        self.activate(index, ctx);
                    }
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowUp" => self.move_selection(-1, viewport.height(), ctx),
                    "ArrowDown" => self.move_selection(1, viewport.height(), ctx),
                    "Home" => {
                        if !self.items.is_empty() {
                            self.activate(0, ctx);
                            self.ensure_visible(viewport.height(), 0);
                        }
                    }
                    "End" => {
                        if !self.items.is_empty() {
                            let last = self.items.len() - 1;
                            self.activate(last, ctx);
                            self.ensure_visible(viewport.height(), last);
                        }
                    }
                    "PageUp" => {
                        let next = self.clamp_scroll(
                            viewport.height(),
                            self.scroll_y - viewport.height() * 0.85,
                        );
                        if (next - self.scroll_y).abs() > f32::EPSILON {
                            self.scroll_y = next;
                        }
                    }
                    "PageDown" => {
                        let next = self.clamp_scroll(
                            viewport.height(),
                            self.scroll_y + viewport.height() * 0.85,
                        );
                        if (next - self.scroll_y).abs() > f32::EPSILON {
                            self.scroll_y = next;
                        }
                    }
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_selected();
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let text_style = theme.body_text_style();
        let detail_style = caption_style(&theme);
        let base_row_height = self.resolved_row_height();
        let child_max_width = if constraints.max.width.is_finite() {
            (constraints.max.width
                - metrics.data_viewport_padding.left
                - metrics.data_viewport_padding.right
                - metrics.data_row_padding.left
                - metrics.data_row_padding.right)
                .max(0.0)
        } else {
            260.0
        };
        let child_constraints =
            Constraints::new(Size::ZERO, Size::new(child_max_width, f32::INFINITY));
        let explicit_row_height = self.row_height.is_some();
        let mut content_width: f32 = 220.0;
        let mut content_height = 0.0;
        self.row_offsets.clear();
        self.row_heights.clear();

        for item in &mut self.items {
            self.row_offsets.push(content_height);
            let (row_width, row_height) = if let Some(content) = &mut item.content {
                let content_constraints = if explicit_row_height {
                    let child_height = (base_row_height
                        - metrics.data_row_padding.top
                        - metrics.data_row_padding.bottom)
                        .max(0.0);
                    Constraints::new(
                        Size::new(0.0, child_height),
                        Size::new(child_max_width, child_height),
                    )
                } else {
                    child_constraints
                };
                let child_size = content.measure(ctx, content_constraints);
                (
                    (child_size.width
                        + metrics.data_row_padding.left
                        + metrics.data_row_padding.right)
                        .max(220.0),
                    if explicit_row_height {
                        base_row_height
                    } else {
                        (child_size.height
                            + metrics.data_row_padding.top
                            + metrics.data_row_padding.bottom)
                            .max(base_row_height)
                    },
                )
            } else {
                let label = measure_text(ctx, &item.label, &text_style).width;
                let detail = item
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                let leading = measure_list_item_leading_width(ctx, item, &text_style, &theme);
                let trailing = item
                    .trailing
                    .as_deref()
                    .map(|trailing| measure_text(ctx, trailing, &detail_style).width)
                    .unwrap_or(0.0);
                let trailing_gap = if trailing > 0.0 {
                    metrics.data_row_trailing_gap
                } else {
                    0.0
                };
                (
                    metrics.data_row_padding.left
                        + leading
                        + label.max(detail)
                        + trailing_gap
                        + trailing
                        + metrics.data_row_padding.right,
                    base_row_height,
                )
            };
            content_width = content_width.max(row_width);
            content_height += row_height;
            self.row_heights.push(row_height);
        }

        self.content_height = content_height;
        let desired = Size::new(
            content_width
                + metrics.data_viewport_padding.left
                + metrics.data_viewport_padding.right,
            self.measured_content_height()
                + metrics.data_viewport_padding.top
                + metrics.data_viewport_padding.bottom,
        );
        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                desired.width
            },
            desired.height,
        ));

        self.scroll_y = self.clamp_scroll(
            self.viewport_rect(Rect::from_origin_size(Point::ZERO, size))
                .height(),
            self.scroll_y,
        );
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_selected();
        let theme = self.resolved_theme();
        let viewport = self.viewport_rect(bounds);
        for index in 0..self.items.len() {
            let Some((top, row_height)) = self.row_metrics(index) else {
                continue;
            };
            let Some(content) = self.items[index].content.as_mut() else {
                continue;
            };
            let row_y = viewport.y() + top - self.scroll_y;
            if row_y + row_height < viewport.y() || row_y > viewport.max_y() {
                content.arrange(ctx, Rect::from_origin_size(Point::ZERO, Size::ZERO));
                continue;
            }
            let child_size = content.child().measured_size();
            content.arrange(
                ctx,
                Rect::from_origin_size(
                    Point::new(
                        viewport.x() + theme.metrics.data_row_padding.left,
                        row_y + theme.metrics.data_row_padding.top,
                    ),
                    Size::new(
                        (viewport.width()
                            - theme.metrics.data_row_padding.left
                            - theme.metrics.data_row_padding.right)
                            .max(0.0),
                        child_size.height,
                    ),
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let viewport = self.viewport_rect(ctx.bounds());
        let label_style = theme.body_text_style();
        let detail_style = caption_style(&theme);

        draw_surface(ctx, ctx.bounds(), &theme, self.focus_animation.value);
        ctx.push_clip_rect(viewport);

        for index in 0..self.items.len() {
            let Some((top, row_height)) = self.row_metrics(index) else {
                continue;
            };
            let y = viewport.y() + top - self.scroll_y;
            if y + row_height < viewport.y() || y > viewport.max_y() {
                continue;
            }
            let row = Rect::new(viewport.x(), y, viewport.width(), row_height);
            let selected = self.current_selected() == Some(index);
            let hover_amount = self.hover_motion.amount_for(&index);
            let press_amount = self.press_motion.amount_for(&index);

            if selected
                || hover_amount > AnimatedScalar::EPSILON
                || press_amount > AnimatedScalar::EPSILON
            {
                if let Some(highlight) = row_highlight_rect(row, viewport) {
                    ctx.fill_rect(
                        highlight,
                        data_row_state_fill(&theme, selected, hover_amount, press_amount),
                    );
                }
            }

            let Some(item) = self.items.get(index) else {
                continue;
            };
            if let Some(accent) = item.accent {
                ctx.fill_rect(
                    Rect::new(row.x() + 4.0, row.y() + 5.0, 3.0, row.height() - 10.0),
                    accent,
                );
            }

            if let Some(content) = &item.content {
                content.paint(ctx);
                continue;
            }

            let mut text_x = row.x() + metrics.data_row_padding.left;
            let leading_color = item.leading_color.unwrap_or_else(|| {
                if item.disabled {
                    palette.placeholder
                } else {
                    palette.text_muted
                }
            });
            if let Some(icon) = item.leading_icon {
                let side = metrics
                    .data_row_icon_size
                    .min((row.height() - 8.0).max(0.0))
                    .max(0.0);
                let icon_rect = Rect::new(
                    text_x,
                    row.y() + ((row.height() - side) * 0.5).max(0.0),
                    side,
                    side,
                );
                draw_icon_glyph(ctx, icon, icon_rect, leading_color);
                text_x += side + metrics.data_row_icon_gap;
            } else if let Some(leading) = &item.leading_text {
                let leading_style = TextStyle {
                    color: leading_color,
                    ..label_style.clone()
                };
                let leading_measurement = paint_text_measurement(ctx, leading, &leading_style);
                let leading_slot =
                    Rect::new(text_x, row.y(), leading_measurement.width, row.height());
                paint_aligned_text(
                    ctx,
                    leading_slot,
                    leading,
                    &leading_style,
                    leading_style.line_height,
                    0.0,
                );
                text_x += leading_measurement.width + metrics.data_row_icon_gap;
            }

            let trailing_measurement = item
                .trailing
                .as_deref()
                .map(|trailing| paint_text_measurement(ctx, trailing, &detail_style));
            let trailing_width = trailing_measurement
                .map(|measurement| (measurement.width + 8.0).min(row.width() * 0.42))
                .unwrap_or(0.0);
            let trailing_rect = item.trailing.as_ref().map(|_| {
                Rect::new(
                    row.max_x() - metrics.data_row_padding.right - trailing_width,
                    row.y(),
                    trailing_width,
                    row.height(),
                )
            });
            let text_right = trailing_rect
                .map(|rect| rect.x() - metrics.data_row_trailing_gap)
                .unwrap_or(row.max_x() - metrics.data_row_padding.right);
            let text_bounds = Rect::new(
                text_x,
                row.y(),
                (text_right - text_x).max(0.0),
                row.height(),
            );
            let label_measurement = paint_text_measurement(ctx, &item.label, &label_style);
            let detail_measurement = item
                .detail
                .as_deref()
                .map(|detail| paint_text_measurement(ctx, detail, &detail_style));
            let (label_rect, detail_rect) = row_text_rects(
                ctx,
                text_bounds,
                label_measurement,
                label_style.line_height,
                detail_measurement,
                item.detail.as_ref().map(|_| detail_style.line_height),
            );
            ctx.push_clip_rect(label_rect);
            ctx.draw_text(
                label_rect,
                item.label.clone(),
                if item.disabled {
                    theme.text_style(palette.placeholder)
                } else {
                    label_style.clone()
                },
            );
            ctx.pop_clip();
            if let Some(detail) = &item.detail {
                let detail_rect = detail_rect.unwrap_or(text_bounds);
                ctx.push_clip_rect(detail_rect);
                ctx.draw_text(detail_rect, detail.clone(), detail_style.clone());
                ctx.pop_clip();
            }
            if let (Some(trailing), Some(rect)) = (&item.trailing, trailing_rect) {
                let style = detail_style.clone();
                ctx.push_clip_rect(rect);
                paint_aligned_text(ctx, rect, trailing, &style, style.line_height, 1.0);
                ctx.pop_clip();
            }
        }

        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = self
            .current_selected()
            .and_then(|index| self.items.get(index))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (index, item) in self.items.iter().enumerate() {
            if let Some(bounds) = self.visible_row_rect(ctx.bounds(), index) {
                let mut row = SemanticsNode::new(
                    list_view_row_id(ctx.widget_id(), index),
                    SemanticsRole::ListItem,
                    bounds,
                );
                row.parent = Some(ctx.widget_id());
                row.name = Some(
                    item.semantic_name
                        .clone()
                        .unwrap_or_else(|| item.label.clone()),
                );
                row.description = item
                    .semantic_description
                    .clone()
                    .or_else(|| item.detail.clone());
                row.value = Some(SemanticsValue::Text(
                    item.detail.clone().unwrap_or_else(|| item.label.clone()),
                ));
                row.state.disabled = item.disabled;
                row.state.hovered = self.hovered == Some(index);
                row.state.selected = self.current_selected() == Some(index);
                if item.can_activate_from_row() {
                    row.actions = vec![SemanticsAction::Activate];
                }
                ctx.push(row);
            }

            if let Some(content) = &item.content {
                content.semantics(ctx);
            }
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for item in &self.items {
            if let Some(content) = &item.content {
                content.visit_children(visitor);
            }
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for item in &mut self.items {
            if let Some(content) = &mut item.content {
                content.visit_children_mut(visitor);
            }
        }
    }
}

#[derive(Clone)]
pub struct LayerListItem {
    label: String,
    detail: Option<String>,
    detail_reader: Option<Rc<dyn Fn() -> String>>,
    thumbnail: Option<Color>,
    visible: bool,
    visible_reader: Option<Rc<dyn Fn() -> bool>>,
    locked: bool,
    locked_reader: Option<Rc<dyn Fn() -> bool>>,
    disabled: bool,
}

impl fmt::Debug for LayerListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayerListItem")
            .field("label", &self.label)
            .field("detail", &self.detail)
            .field("thumbnail", &self.thumbnail)
            .field("visible", &self.visible)
            .field("locked", &self.locked)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl PartialEq for LayerListItem {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
            && self.detail == other.detail
            && self.thumbnail == other.thumbnail
            && self.visible == other.visible
            && self.locked == other.locked
            && self.disabled == other.disabled
    }
}

impl LayerListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            detail_reader: None,
            thumbnail: None,
            visible: true,
            visible_reader: None,
            locked: false,
            locked_reader: None,
            disabled: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self.detail_reader = None;
        self
    }

    pub fn detail_when<F>(mut self, detail: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.detail_reader = Some(Rc::new(detail));
        self
    }

    pub fn thumbnail(mut self, color: Color) -> Self {
        self.thumbnail = Some(color);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self.visible_reader = None;
        self
    }

    pub fn visible_when<F>(mut self, visible: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.visible_reader = Some(Rc::new(visible));
        self
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self.locked_reader = None;
        self
    }

    pub fn locked_when<F>(mut self, locked: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.locked_reader = Some(Rc::new(locked));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    fn current_detail(&self) -> Option<String> {
        self.detail_reader
            .as_ref()
            .map(|reader| reader())
            .or_else(|| self.detail.clone())
    }

    fn current_visible(&self) -> bool {
        self.visible_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.visible)
    }

    fn current_locked(&self) -> bool {
        self.locked_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.locked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerListHit {
    Row(usize),
    Visibility(usize),
    Lock(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerListReorderChange {
    pub item: usize,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Copy)]
struct LayerListReorderPress {
    pointer_id: u64,
    start_position: Point,
    row: usize,
    drag_offset_y: f32,
}

#[derive(Debug, Clone, Copy)]
struct LayerListReorderDrag {
    pointer_id: u64,
    row: usize,
    target: usize,
    position: Point,
    drag_offset_y: f32,
}

type LayerListReorderCallback = Box<dyn FnMut(&mut EventCtx, LayerListReorderChange)>;

pub struct LayerList {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    layers: Vec<LayerListItem>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<LayerListHit>,
    pressed: Option<LayerListHit>,
    reorder_press: Option<LayerListReorderPress>,
    reorder_drag: Option<LayerListReorderDrag>,
    hover_motion: IndexedInteractionMotion<LayerListHit>,
    press_motion: IndexedInteractionMotion<LayerListHit>,
    focus_animation: AnimatedScalar,
    row_height: Option<f32>,
    drag_threshold: f32,
    on_select: Option<Box<dyn FnMut(usize, String)>>,
    on_select_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String)>>,
    on_visibility_change: Option<Box<dyn FnMut(usize, bool)>>,
    on_visibility_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, bool)>>,
    on_lock_change: Option<Box<dyn FnMut(usize, bool)>>,
    on_lock_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, bool)>>,
    on_reorder: Option<LayerListReorderCallback>,
}

impl LayerList {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            layers: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            reorder_press: None,
            reorder_drag: None,
            hover_motion: IndexedInteractionMotion::new(),
            press_motion: IndexedInteractionMotion::new(),
            focus_animation: AnimatedScalar::new(0.0),
            row_height: None,
            drag_threshold: 4.0,
            on_select: None,
            on_select_with_ctx: None,
            on_visibility_change: None,
            on_visibility_change_with_ctx: None,
            on_lock_change: None,
            on_lock_change_with_ctx: None,
            on_reorder: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn layer(mut self, layer: LayerListItem) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn layers<I>(mut self, layers: I) -> Self
    where
        I: IntoIterator<Item = LayerListItem>,
    {
        self.layers.extend(layers);
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = Some(row_height.max(0.0));
        self
    }

    pub fn drag_threshold(mut self, threshold: f32) -> Self {
        self.drag_threshold = threshold.max(0.0);
        self
    }

    pub fn on_select<F>(mut self, on_select: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_select = Some(Box::new(on_select));
        self
    }

    pub fn on_select_with_ctx<F>(mut self, on_select: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, String) + 'static,
    {
        self.on_select_with_ctx = Some(Box::new(on_select));
        self
    }

    pub fn on_visibility_change<F>(mut self, on_visibility_change: F) -> Self
    where
        F: FnMut(usize, bool) + 'static,
    {
        self.on_visibility_change = Some(Box::new(on_visibility_change));
        self
    }

    pub fn on_visibility_change_with_ctx<F>(mut self, on_visibility_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, bool) + 'static,
    {
        self.on_visibility_change_with_ctx = Some(Box::new(on_visibility_change));
        self
    }

    pub fn on_lock_change<F>(mut self, on_lock_change: F) -> Self
    where
        F: FnMut(usize, bool) + 'static,
    {
        self.on_lock_change = Some(Box::new(on_lock_change));
        self
    }

    pub fn on_lock_change_with_ctx<F>(mut self, on_lock_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, bool) + 'static,
    {
        self.on_lock_change_with_ctx = Some(Box::new(on_lock_change));
        self
    }

    pub fn on_reorder<F>(mut self, on_reorder: F) -> Self
    where
        F: FnMut(LayerListReorderChange) + 'static,
    {
        let mut on_reorder = on_reorder;
        self.on_reorder = Some(Box::new(move |_, change| on_reorder(change)));
        self
    }

    pub fn on_reorder_with_ctx<F>(mut self, on_reorder: F) -> Self
    where
        F: FnMut(&mut EventCtx, LayerListReorderChange) + 'static,
    {
        self.on_reorder = Some(Box::new(on_reorder));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    fn sync_selected(&mut self) {
        if self.selected_reader.is_some() {
            self.selected = self.current_selected();
        } else if self
            .selected
            .is_some_and(|selected| selected >= self.layers.len())
        {
            self.selected = None;
        }
    }

    fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.layers.len())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.resolved_theme().metrics.data_viewport_padding)
    }

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .unwrap_or(self.resolved_theme().metrics.layer_row_height)
    }

    fn row_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.layers.len() {
            return None;
        }

        let viewport = self.viewport_rect(bounds);
        let row_height = self.resolved_row_height();
        let y = viewport.y() + index as f32 * row_height;
        Rect::new(viewport.x(), y, viewport.width(), row_height)
            .intersection(viewport)
            .filter(|rect| !rect.is_empty())
    }

    fn visibility_rect(&self, row: Rect) -> Rect {
        let size = self
            .resolved_theme()
            .metrics
            .layer_action_size
            .min(row.height())
            .max(18.0);
        Rect::new(
            row.x() + self.resolved_theme().metrics.data_row_padding.left.min(8.0),
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn thumbnail_rect(&self, row: Rect) -> Rect {
        let theme = self.resolved_theme();
        let action = self.visibility_rect(row);
        let size = theme
            .metrics
            .layer_thumbnail_size
            .min((row.height() - 8.0).max(0.0))
            .max(18.0);
        Rect::new(
            action.max_x() + theme.metrics.data_row_icon_gap,
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn lock_rect(&self, row: Rect) -> Rect {
        let theme = self.resolved_theme();
        let size = theme.metrics.layer_action_size.min(row.height()).max(18.0);
        Rect::new(
            row.max_x() - size - theme.metrics.data_row_padding.right.min(8.0),
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn text_rect(&self, row: Rect) -> Rect {
        let thumb = self.thumbnail_rect(row);
        let lock = self.lock_rect(row);
        let theme = self.resolved_theme();
        Rect::new(
            thumb.max_x() + theme.metrics.data_row_icon_gap,
            row.y(),
            (lock.x() - thumb.max_x() - theme.metrics.data_row_trailing_gap).max(0.0),
            row.height(),
        )
    }

    fn hit_at(&self, bounds: Rect, position: Point) -> Option<LayerListHit> {
        let viewport = self.viewport_rect(bounds);
        if !viewport.contains(position) {
            return None;
        }

        (0..self.layers.len()).find_map(|index| {
            let row = self.row_rect(bounds, index)?;
            if self.visibility_rect(row).contains(position) {
                return Some(LayerListHit::Visibility(index));
            }
            if self.lock_rect(row).contains(position) {
                return Some(LayerListHit::Lock(index));
            }
            row.contains(position).then_some(LayerListHit::Row(index))
        })
    }

    fn reorder_enabled(&self) -> bool {
        self.on_reorder.is_some() && self.layers.len() > 1
    }

    fn reorder_press_at(
        &self,
        bounds: Rect,
        pointer_id: u64,
        position: Point,
    ) -> Option<LayerListReorderPress> {
        if !self.reorder_enabled() {
            return None;
        }
        let LayerListHit::Row(row) = self.hit_at(bounds, position)? else {
            return None;
        };
        let layer = self.layers.get(row)?;
        if layer.disabled {
            return None;
        }
        let rect = self.row_rect(bounds, row)?;
        Some(LayerListReorderPress {
            pointer_id,
            start_position: position,
            row,
            drag_offset_y: position.y - rect.y(),
        })
    }

    fn insertion_index_at(&self, bounds: Rect, position: Point) -> usize {
        let viewport = self.viewport_rect(bounds);
        let row_height = self.resolved_row_height();
        if row_height <= 0.0 {
            return 0;
        }
        let local_y = position.y - viewport.y();
        for index in 0..self.layers.len() {
            let midpoint = index as f32 * row_height + row_height * 0.5;
            if local_y < midpoint {
                return index;
            }
        }
        self.layers.len()
    }

    fn reorder_target_at(&self, bounds: Rect, row: usize, position: Point) -> usize {
        if self.layers.is_empty() {
            return 0;
        }
        let insertion = self.insertion_index_at(bounds, position);
        let target = if insertion > row {
            insertion.saturating_sub(1)
        } else {
            insertion
        };
        target.min(self.layers.len().saturating_sub(1))
    }

    fn start_reorder_drag(
        &mut self,
        ctx: &mut EventCtx,
        press: LayerListReorderPress,
        position: Point,
    ) {
        let target = self.reorder_target_at(ctx.bounds(), press.row, position);
        self.reorder_drag = Some(LayerListReorderDrag {
            pointer_id: press.pointer_id,
            row: press.row,
            target,
            position,
            drag_offset_y: press.drag_offset_y,
        });
        self.pressed = None;
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn update_reorder_drag(&mut self, ctx: &mut EventCtx, position: Point) {
        let Some(drag) = self.reorder_drag else {
            return;
        };
        let target = self.reorder_target_at(ctx.bounds(), drag.row, position);
        self.reorder_drag = Some(LayerListReorderDrag {
            target,
            position,
            ..drag
        });
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn remap_index_after_reorder(index: usize, from: usize, to: usize) -> usize {
        if index == from {
            to
        } else if from < to && index > from && index <= to {
            index - 1
        } else if to < from && index >= to && index < from {
            index + 1
        } else {
            index
        }
    }

    fn finish_reorder_drag(&mut self, ctx: &mut EventCtx) {
        let Some(drag) = self.reorder_drag.take() else {
            return;
        };
        self.reorder_press = None;
        self.pressed = None;

        let from = drag.row;
        let to = drag.target.min(self.layers.len().saturating_sub(1));
        if from >= self.layers.len() || from == to {
            ctx.request_paint();
            ctx.request_semantics();
            return;
        }

        let item = self.layers.remove(from);
        self.layers.insert(to, item);
        if self.selected_reader.is_none() {
            self.selected = self
                .selected
                .map(|selected| Self::remap_index_after_reorder(selected, from, to));
        }
        if let Some(callback) = &mut self.on_reorder {
            callback(
                ctx,
                LayerListReorderChange {
                    item: from,
                    from,
                    to,
                },
            );
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn cancel_reorder_drag(&mut self, ctx: &mut EventCtx) {
        if self.reorder_press.is_some() || self.reorder_drag.is_some() {
            self.reorder_press = None;
            self.reorder_drag = None;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn reorder_marker_y(&self, bounds: Rect) -> Option<f32> {
        let drag = self.reorder_drag?;
        let target = drag.target.min(self.layers.len().saturating_sub(1));
        let row = self.row_rect(bounds, target)?;
        if target > drag.row {
            Some(row.max_y())
        } else {
            Some(row.y())
        }
    }

    fn select(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(layer) = self.layers.get(index) else {
            return;
        };
        if layer.disabled {
            return;
        }
        let label = layer.label.clone();

        self.selected = Some(index);
        if let Some(on_select) = &mut self.on_select {
            on_select(index, label.clone());
        }
        if let Some(on_select) = &mut self.on_select_with_ctx {
            on_select(ctx, index, label);
        }
    }

    fn toggle_visibility(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        if layer.disabled {
            return;
        }

        layer.visible = !layer.current_visible();
        if let Some(on_visibility_change) = &mut self.on_visibility_change {
            on_visibility_change(index, layer.visible);
        }
        if let Some(on_visibility_change) = &mut self.on_visibility_change_with_ctx {
            on_visibility_change(ctx, index, layer.visible);
        }
    }

    fn toggle_lock(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        if layer.disabled {
            return;
        }

        layer.locked = !layer.current_locked();
        if let Some(on_lock_change) = &mut self.on_lock_change {
            on_lock_change(index, layer.locked);
        }
        if let Some(on_lock_change) = &mut self.on_lock_change_with_ctx {
            on_lock_change(ctx, index, layer.locked);
        }
    }

    fn move_selection(&mut self, ctx: &mut EventCtx, delta: isize) {
        if self.layers.is_empty() {
            return;
        }

        let current = self.current_selected().unwrap_or(0) as isize;
        let last = self.layers.len() as isize - 1;
        self.select(ctx, (current + delta).clamp(0, last) as usize);
    }

    fn set_hovered(&mut self, hovered: Option<LayerListHit>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }

        self.hovered = hovered;
        let theme = self.resolved_theme();
        self.hover_motion.set_hover_target(hovered, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<LayerListHit>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        let theme = self.resolved_theme();
        self.press_motion.set_press_target(pressed, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (hover_changed, hover_active) = self.hover_motion.advance(time);
        let (press_changed, press_active) = self.press_motion.advance(time);
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);

        if hover_changed || press_changed || focus_changed {
            ctx.request_paint();
        }
        if hover_active || press_active || focus_active {
            ctx.request_animation_frame();
        }
    }

    fn paint_row(
        &self,
        ctx: &mut PaintCtx,
        viewport: Rect,
        index: usize,
        row: Rect,
        theme: &DefaultTheme,
        label_style: &TextStyle,
        detail_style: &TextStyle,
    ) {
        let Some(layer) = self.layers.get(index) else {
            return;
        };
        let palette = theme.palette;
        let visible = layer.current_visible();
        let locked = layer.current_locked();
        let detail = layer.current_detail();
        let selected = self.current_selected() == Some(index);
        let row_hit = LayerListHit::Row(index);
        let row_hover_amount = self.hover_motion.amount_for(&row_hit);
        let row_press_amount = self.press_motion.amount_for(&row_hit);
        if selected
            || row_hover_amount > AnimatedScalar::EPSILON
            || row_press_amount > AnimatedScalar::EPSILON
        {
            if let Some(highlight) = row_highlight_rect(row, viewport) {
                ctx.fill_rect(
                    highlight,
                    data_row_state_fill(theme, selected, row_hover_amount, row_press_amount),
                );
            }
        }

        let visibility_hit = LayerListHit::Visibility(index);
        paint_layer_visibility_button(
            ctx,
            self.visibility_rect(row),
            theme,
            visible,
            self.hover_motion.amount_for(&visibility_hit),
            self.press_motion.amount_for(&visibility_hit),
        );
        let lock_hit = LayerListHit::Lock(index);
        paint_layer_lock_button(
            ctx,
            self.lock_rect(row),
            theme,
            locked,
            self.hover_motion.amount_for(&lock_hit),
            self.press_motion.amount_for(&lock_hit),
        );
        paint_layer_thumbnail(
            ctx,
            self.thumbnail_rect(row),
            theme,
            layer.thumbnail.unwrap_or(palette.control_hover),
            visible,
        );

        let text_rect = self.text_rect(row);
        let label_measurement = paint_text_measurement(ctx, &layer.label, label_style);
        let detail_text = detail
            .as_deref()
            .unwrap_or(if visible { "Visible" } else { "Hidden" });
        let detail_measurement = paint_text_measurement(ctx, detail_text, detail_style);
        let (label_rect, detail_rect) = row_text_rects(
            ctx,
            text_rect,
            label_measurement,
            label_style.line_height,
            Some(detail_measurement),
            Some(detail_style.line_height),
        );
        let text_alpha = if visible { 1.0 } else { 0.58 };
        ctx.draw_text(
            label_rect,
            layer.label.clone(),
            TextStyle {
                color: label_style.color.with_alpha(text_alpha),
                ..label_style.clone()
            },
        );
        if let Some(detail_rect) = detail_rect {
            ctx.draw_text(
                detail_rect,
                detail_text.to_string(),
                TextStyle {
                    color: detail_style.color.with_alpha(text_alpha),
                    ..detail_style.clone()
                },
            );
        }
    }
}

impl Widget for LayerList {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_selected();

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if self
                    .reorder_drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id)
                {
                    self.update_reorder_drag(ctx, pointer.position);
                    ctx.set_handled();
                    return;
                }
                if self
                    .reorder_press
                    .is_some_and(|press| press.pointer_id == pointer.pointer_id)
                {
                    let press = self.reorder_press.unwrap();
                    let delta = pointer.position - press.start_position;
                    let distance_sq = delta.x * delta.x + delta.y * delta.y;
                    if distance_sq >= self.drag_threshold * self.drag_threshold {
                        self.start_reorder_drag(ctx, press, pointer.position);
                        ctx.set_handled();
                        return;
                    }
                }
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                self.reorder_press =
                    self.reorder_press_at(ctx.bounds(), pointer.pointer_id, pointer.position);
                if self.pressed.is_some() {
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self
                    .reorder_drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id)
                {
                    self.finish_reorder_drag(ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                    return;
                }
                if self
                    .reorder_press
                    .is_some_and(|press| press.pointer_id == pointer.pointer_id)
                {
                    self.reorder_press = None;
                }
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                if let Some(hit) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(hit, _)| hit)
                {
                    match hit {
                        LayerListHit::Row(index) => self.select(ctx, index),
                        LayerListHit::Visibility(index) => self.toggle_visibility(ctx, index),
                        LayerListHit::Lock(index) => self.toggle_lock(ctx, index),
                    }
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self
                    .reorder_drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id)
                    || self
                        .reorder_press
                        .is_some_and(|press| press.pointer_id == pointer.pointer_id)
                {
                    self.cancel_reorder_drag(ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                    return;
                }
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowUp" => self.move_selection(ctx, -1),
                    "ArrowDown" => self.move_selection(ctx, 1),
                    "Home" => {
                        if !self.layers.is_empty() {
                            self.select(ctx, 0);
                        }
                    }
                    "End" => {
                        if !self.layers.is_empty() {
                            self.select(ctx, self.layers.len() - 1);
                        }
                    }
                    " " => {
                        if let Some(index) = self.current_selected() {
                            self.toggle_visibility(ctx, index);
                        }
                    }
                    "l" | "L" => {
                        if let Some(index) = self.current_selected() {
                            self.toggle_lock(ctx, index);
                        }
                    }
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_selected();
        let theme = self.resolved_theme();
        let text_style = theme.body_text_style();
        let detail_style = caption_style(&theme);
        let mut width: f32 = 260.0;
        for layer in &self.layers {
            let detail_width = layer
                .detail
                .as_deref()
                .map(|detail| measure_text(ctx, detail, &detail_style).width)
                .unwrap_or(0.0)
                .min(80.0);
            width = width
                .max(124.0 + measure_text(ctx, &layer.label, &text_style).width + detail_width);
        }
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                width + 16.0
            },
            self.layers.len().max(1) as f32 * self.resolved_row_height()
                + theme.metrics.data_viewport_padding.top
                + theme.metrics.data_viewport_padding.bottom,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let viewport = self.viewport_rect(ctx.bounds());
        let label_style = theme.body_text_style();
        let detail_style = caption_style(&theme);
        let active_row = self.reorder_drag.map(|drag| drag.row);

        draw_surface(ctx, ctx.bounds(), &theme, self.focus_animation.value);
        ctx.push_clip_rect(viewport);

        for index in 0..self.layers.len() {
            if active_row == Some(index) {
                continue;
            }
            let Some(row) = self.row_rect(ctx.bounds(), index) else {
                continue;
            };
            self.paint_row(
                ctx,
                viewport,
                index,
                row,
                &theme,
                &label_style,
                &detail_style,
            );
        }

        if let Some(marker_y) = self.reorder_marker_y(ctx.bounds()) {
            let marker = Rect::new(
                viewport.x() + 6.0,
                (marker_y - 1.0).clamp(viewport.y(), viewport.max_y() - 2.0),
                (viewport.width() - 12.0).max(0.0),
                2.0,
            );
            ctx.fill(Path::rounded_rect(marker, 1.0), theme.palette.border_focus);
        }

        if let Some(drag) = self.reorder_drag {
            if let Some(row) = self.row_rect(ctx.bounds(), drag.row) {
                let y = drag.position.y - drag.drag_offset_y;
                ctx.push_transform(Transform::translation(0.0, y - row.y()));
                self.paint_row(
                    ctx,
                    viewport,
                    drag.row,
                    row,
                    &theme,
                    &label_style,
                    &detail_style,
                );
                ctx.pop_transform();
            }
        }

        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = self
            .current_selected()
            .and_then(|index| self.layers.get(index))
            .map(|layer| SemanticsValue::Text(layer.label.clone()));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (index, layer) in self.layers.iter().enumerate() {
            let Some(row) = self.row_rect(ctx.bounds(), index) else {
                continue;
            };
            let visible = layer.current_visible();
            let locked = layer.current_locked();
            let detail = layer.current_detail();
            let row_id = layer_list_row_id(ctx.widget_id(), index);
            let mut row_node = SemanticsNode::new(row_id, SemanticsRole::ListItem, row);
            row_node.parent = Some(ctx.widget_id());
            row_node.name = Some(layer.label.clone());
            row_node.description = detail.clone();
            row_node.value = Some(SemanticsValue::Text(format!(
                "{}; {}; {}",
                detail.as_deref().unwrap_or("Layer"),
                if visible { "Visible" } else { "Hidden" },
                if locked { "Locked" } else { "Unlocked" }
            )));
            row_node.state.disabled = layer.disabled;
            row_node.state.hovered = self.hovered == Some(LayerListHit::Row(index));
            row_node.state.selected = self.current_selected() == Some(index);
            if !layer.disabled {
                row_node.actions = vec![SemanticsAction::Activate];
            }
            ctx.push(row_node);

            let mut visibility = SemanticsNode::new(
                layer_list_visibility_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                self.visibility_rect(row),
            );
            visibility.parent = Some(row_id);
            visibility.name = Some(if visible {
                format!("Hide {} layer", layer.label)
            } else {
                format!("Show {} layer", layer.label)
            });
            visibility.value = Some(SemanticsValue::Text(if visible {
                "Visible".to_string()
            } else {
                "Hidden".to_string()
            }));
            visibility.state.disabled = layer.disabled;
            visibility.state.hovered = self.hovered == Some(LayerListHit::Visibility(index));
            visibility.state.checked = Some(if visible {
                ToggleState::Checked
            } else {
                ToggleState::Unchecked
            });
            if !layer.disabled {
                visibility.actions = vec![SemanticsAction::Activate];
            }
            ctx.push(visibility);

            let mut lock = SemanticsNode::new(
                layer_list_lock_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                self.lock_rect(row),
            );
            lock.parent = Some(row_id);
            lock.name = Some(if locked {
                format!("Unlock {} layer", layer.label)
            } else {
                format!("Lock {} layer", layer.label)
            });
            lock.value = Some(SemanticsValue::Text(if locked {
                "Locked".to_string()
            } else {
                "Unlocked".to_string()
            }));
            lock.state.disabled = layer.disabled;
            lock.state.hovered = self.hovered == Some(LayerListHit::Lock(index));
            lock.state.checked = Some(if locked {
                ToggleState::Checked
            } else {
                ToggleState::Unchecked
            });
            if !layer.disabled {
                lock.actions = vec![SemanticsAction::Activate];
            }
            ctx.push(lock);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.layers.is_empty()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeItem {
    label: String,
    detail: Option<String>,
    children: Vec<TreeItem>,
    expanded: bool,
    disabled: bool,
}

impl TreeItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            children: Vec::new(),
            expanded: false,
            disabled: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn with_child(mut self, child: TreeItem) -> Self {
        self.children.push(child);
        self
    }

    pub fn children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = TreeItem>,
    {
        self.children.extend(children);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

pub struct TreeView {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    items: Vec<TreeItem>,
    selected: Option<Vec<usize>>,
    hovered: Option<Vec<usize>>,
    pressed: Option<Vec<usize>>,
    hover_motion: IndexedInteractionMotion<Vec<usize>>,
    press_motion: IndexedInteractionMotion<Vec<usize>>,
    focus_animation: AnimatedScalar,
    row_height: Option<f32>,
    scroll_y: f32,
    on_change: Option<Box<dyn FnMut(Vec<usize>, String)>>,
}

impl TreeView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            items: Vec::new(),
            selected: None,
            hovered: None,
            pressed: None,
            hover_motion: IndexedInteractionMotion::new(),
            press_motion: IndexedInteractionMotion::new(),
            focus_animation: AnimatedScalar::new(0.0),
            row_height: None,
            scroll_y: 0.0,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn item(mut self, item: TreeItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = TreeItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = Some(row_height.max(0.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(Vec<usize>, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_row_height(&self) -> f32 {
        let theme = self.resolved_theme();
        let base = self.row_height.unwrap_or(theme.metrics.tree_row_height);
        if self.visible_rows().iter().any(|row| row.detail.is_some()) {
            base.max(two_line_row_height(
                theme.body_text_style().line_height,
                caption_style(&theme).line_height,
            ))
        } else {
            base
        }
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.resolved_theme().metrics.data_viewport_padding)
    }

    fn visible_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        let mut path = Vec::new();
        flatten_tree(&self.items, 0, &mut path, &mut rows);
        rows
    }

    fn content_height(&self) -> f32 {
        self.visible_rows().len() as f32 * self.resolved_row_height()
    }

    fn clamp_scroll(&self, viewport_height: f32, scroll_y: f32) -> f32 {
        let max_scroll = (self.content_height() - viewport_height).max(0.0);
        scroll_y.clamp(0.0, max_scroll)
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<TreeRow> {
        let viewport = self.viewport_rect(bounds);
        if !viewport.contains(position) {
            return None;
        }

        let row_height = self.resolved_row_height();
        let y = position.y - viewport.y() + self.scroll_y;
        let index = (y / row_height).floor() as usize;
        self.visible_rows().into_iter().nth(index)
    }

    fn select_path(&mut self, path: &[usize]) {
        let Some(item) = tree_item(&self.items, path) else {
            return;
        };
        if item.disabled {
            return;
        }
        self.selected = Some(path.to_vec());
        if let Some(on_change) = &mut self.on_change {
            on_change(path.to_vec(), item.label.clone());
        }
    }

    fn toggle_path(&mut self, path: &[usize]) -> bool {
        let Some(item) = tree_item_mut(&mut self.items, path) else {
            return false;
        };
        if item.children.is_empty() {
            return false;
        }
        item.expanded = !item.expanded;
        true
    }

    fn ensure_visible(&mut self, viewport_height: f32, path: &[usize]) {
        let rows = self.visible_rows();
        let Some(index) = rows.iter().position(|row| row.path == path) else {
            return;
        };
        let row_height = self.resolved_row_height();
        let top = index as f32 * row_height;
        let bottom = top + row_height;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if bottom > self.scroll_y + viewport_height {
            self.scroll_y = bottom - viewport_height;
        }
        self.scroll_y = self.clamp_scroll(viewport_height, self.scroll_y);
    }

    fn set_hovered(&mut self, hovered: Option<Vec<usize>>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }

        self.hovered = hovered.clone();
        let theme = self.resolved_theme();
        self.hover_motion.set_hover_target(hovered, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<Vec<usize>>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed.clone();
        let theme = self.resolved_theme();
        self.press_motion.set_press_target(pressed, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (hover_changed, hover_active) = self.hover_motion.advance(time);
        let (press_changed, press_active) = self.press_motion.advance(time);
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);

        if hover_changed || press_changed || focus_changed {
            ctx.request_paint();
        }
        if hover_active || press_active || focus_active {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for TreeView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let viewport = self.viewport_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .map(|row| row.path);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && viewport.contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                let next = self.clamp_scroll(viewport.height(), self.scroll_y - delta.y);
                if (next - self.scroll_y).abs() > f32::EPSILON {
                    self.scroll_y = next;
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && viewport.contains(pointer.position) =>
            {
                let pressed = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .map(|row| row.path);
                self.set_hovered(pressed.clone(), ctx);
                self.set_pressed(pressed, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered_row = self.row_at_position(ctx.bounds(), pointer.position);
                if let Some(row) = hovered_row
                    .as_ref()
                    .filter(|row| self.pressed.as_deref() == Some(row.path.as_slice()))
                {
                    let row_height = self.resolved_row_height();
                    let viewport_rect = self.viewport_rect(ctx.bounds());
                    let index = self
                        .visible_rows()
                        .iter()
                        .position(|candidate| candidate.path == row.path)
                        .unwrap_or(0);
                    let row_rect = Rect::new(
                        viewport_rect.x(),
                        viewport_rect.y() + (index as f32 * row_height) - self.scroll_y,
                        viewport_rect.width(),
                        row_height,
                    );
                    if disclosure_rect(&self.resolved_theme(), row_rect, row.depth)
                        .contains(pointer.position)
                    {
                        if self.toggle_path(&row.path) {
                            ctx.request_measure();
                        }
                    } else {
                        self.select_path(&row.path);
                    }
                }
                self.set_hovered(hovered_row.map(|row| row.path), ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let rows = self.visible_rows();
                if rows.is_empty() {
                    return;
                }

                let current = self
                    .selected
                    .as_ref()
                    .and_then(|selected| rows.iter().position(|row| &row.path == selected))
                    .unwrap_or(0);

                match key.key.as_str() {
                    "ArrowUp" => {
                        let next = current.saturating_sub(1);
                        self.select_path(&rows[next].path);
                        self.ensure_visible(viewport.height(), &rows[next].path);
                    }
                    "ArrowDown" => {
                        let next = (current + 1).min(rows.len() - 1);
                        self.select_path(&rows[next].path);
                        self.ensure_visible(viewport.height(), &rows[next].path);
                    }
                    "ArrowRight" => {
                        let row = &rows[current];
                        if row.has_children && !row.expanded {
                            if self.toggle_path(&row.path) {
                                ctx.request_measure();
                            }
                        } else if row.has_children {
                            let mut child = row.path.clone();
                            child.push(0);
                            self.select_path(&child);
                            self.ensure_visible(viewport.height(), &child);
                        }
                    }
                    "ArrowLeft" => {
                        let row = &rows[current];
                        if row.has_children && row.expanded {
                            if self.toggle_path(&row.path) {
                                ctx.request_measure();
                            }
                        } else if !row.path.is_empty() {
                            let mut parent = row.path.clone();
                            parent.pop();
                            self.select_path(&parent);
                            self.ensure_visible(viewport.height(), &parent);
                        }
                    }
                    "Home" => {
                        self.select_path(&rows[0].path);
                        self.ensure_visible(viewport.height(), &rows[0].path);
                    }
                    "End" => {
                        let last = rows.len() - 1;
                        self.select_path(&rows[last].path);
                        self.ensure_visible(viewport.height(), &rows[last].path);
                    }
                    _ => return,
                }

                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let label_style = theme.body_text_style();
        let detail_style = caption_style(&theme);
        let width = self
            .visible_rows()
            .iter()
            .map(|row| {
                let label_start = tree_label_offset(&theme, row.depth);
                let label = measure_text(ctx, &row.label, &label_style).width;
                let detail = row
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                label_start + label.max(detail) + theme.metrics.data_row_padding.right
            })
            .fold(220.0, f32::max);
        let desired = Size::new(
            width
                + theme.metrics.data_viewport_padding.left
                + theme.metrics.data_viewport_padding.right,
            self.content_height()
                + theme.metrics.data_viewport_padding.top
                + theme.metrics.data_viewport_padding.bottom,
        );
        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                desired.width
            },
            desired.height,
        ));
        self.scroll_y = self.clamp_scroll(
            self.viewport_rect(Rect::from_origin_size(Point::ZERO, size))
                .height(),
            self.scroll_y,
        );
        size
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let viewport = self.viewport_rect(ctx.bounds());
        let row_height = self.resolved_row_height();
        let rows = self.visible_rows();

        draw_surface(ctx, ctx.bounds(), &theme, self.focus_animation.value);
        ctx.push_clip_rect(viewport);

        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + viewport.height()) / row_height).ceil() as usize + 1)
            .min(rows.len());

        for index in start..end {
            let row = &rows[index];
            let y = viewport.y() + (index as f32 * row_height) - self.scroll_y;
            let row_rect = Rect::new(viewport.x(), y, viewport.width(), row_height);
            let selected = self.selected.as_deref() == Some(row.path.as_slice());
            let hover_amount = self
                .hover_motion
                .amount_for_by(|path| path.as_slice() == row.path.as_slice());
            let press_amount = self
                .press_motion
                .amount_for_by(|path| path.as_slice() == row.path.as_slice());

            if selected
                || hover_amount > AnimatedScalar::EPSILON
                || press_amount > AnimatedScalar::EPSILON
            {
                if let Some(highlight) = row_highlight_rect(row_rect, viewport) {
                    ctx.fill_rect(
                        highlight,
                        data_row_state_fill(&theme, selected, hover_amount, press_amount),
                    );
                }
            }

            if row.has_children {
                ctx.fill(
                    disclosure_path(disclosure_rect(&theme, row_rect, row.depth), row.expanded),
                    if selected {
                        palette.text_muted
                    } else {
                        palette.placeholder
                    },
                );
            }

            let label_x = row_rect.x() + tree_label_offset(&theme, row.depth);
            let text_bounds = Rect::new(
                label_x,
                row_rect.y(),
                (row_rect.max_x() - label_x - 8.0).max(0.0),
                row_rect.height(),
            );
            let detail_style = caption_style(&theme);
            let label_style = theme.body_text_style();
            let label_measurement = paint_text_measurement(ctx, &row.label, &label_style);
            let detail_measurement = row
                .detail
                .as_deref()
                .map(|detail| paint_text_measurement(ctx, detail, &detail_style));
            let (label_rect, detail_rect) = row_text_rects(
                ctx,
                text_bounds,
                label_measurement,
                label_style.line_height,
                detail_measurement,
                row.detail.as_ref().map(|_| detail_style.line_height),
            );
            ctx.draw_text(
                label_rect,
                row.label.clone(),
                if row.disabled {
                    theme.text_style(palette.placeholder)
                } else {
                    label_style
                },
            );
            if let Some(detail) = &row.detail {
                ctx.draw_text(
                    detail_rect.unwrap_or(text_bounds),
                    detail.clone(),
                    detail_style,
                );
            }
        }

        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tree, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = self
            .selected
            .as_ref()
            .and_then(|path| tree_item(&self.items, path))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::SetValue,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableColumnAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    title: String,
    width: Option<f32>,
    min_width: f32,
    alignment: TableColumnAlignment,
    numeric: bool,
}

impl TableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: None,
            min_width: 96.0,
            alignment: TableColumnAlignment::Start,
            numeric: false,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(40.0));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width.max(40.0);
        self
    }

    pub fn alignment(mut self, alignment: TableColumnAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn numeric(mut self) -> Self {
        self.alignment = TableColumnAlignment::End;
        self.numeric = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    cells: Vec<String>,
}

impl TableRow {
    pub fn new<I, S>(cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }
}

pub type DataGrid = Table;

pub struct Table {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    name_reader: Option<Box<dyn Fn() -> String>>,
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hover_motion: IndexedInteractionMotion<usize>,
    press_motion: IndexedInteractionMotion<usize>,
    focus_animation: AnimatedScalar,
    row_height: Option<f32>,
    header_height: Option<f32>,
    scroll_y: f32,
    column_widths: Vec<f32>,
    on_change: Option<Box<dyn FnMut(usize)>>,
}

impl Table {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            name_reader: None,
            columns: Vec::new(),
            rows: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            hover_motion: IndexedInteractionMotion::new(),
            press_motion: IndexedInteractionMotion::new(),
            focus_animation: AnimatedScalar::new(0.0),
            row_height: None,
            header_height: None,
            scroll_y: 0.0,
            column_widths: Vec::new(),
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn name_when<F>(mut self, name: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.name_reader = Some(Box::new(name));
        self
    }

    pub fn column(mut self, column: TableColumn) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = TableColumn>,
    {
        self.columns.extend(columns);
        self
    }

    pub fn row(mut self, row: TableRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn rows<I>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = TableRow>,
    {
        self.rows.extend(rows);
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn name(&self) -> String {
        self.name_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.name.clone())
    }

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .unwrap_or(self.resolved_theme().metrics.table_row_height)
    }

    fn resolved_header_height(&self) -> f32 {
        self.header_height
            .unwrap_or(self.resolved_theme().metrics.table_header_height)
    }

    fn body_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.data_viewport_padding;
        let gap = theme.metrics.select_menu_gap;
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top + self.resolved_header_height() + gap,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom - self.resolved_header_height() - gap)
                .max(0.0),
        )
    }

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.resolved_row_height()
    }

    fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.rows.len())
    }

    fn clamp_scroll(&self, viewport_height: f32, scroll_y: f32) -> f32 {
        let max_scroll = (self.content_height() - viewport_height).max(0.0);
        scroll_y.clamp(0.0, max_scroll)
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let body = self.body_rect(bounds);
        if !body.contains(position) {
            return None;
        }
        let y = position.y - body.y() + self.scroll_y;
        let index = (y / self.resolved_row_height()).floor() as usize;
        (index < self.rows.len()).then_some(index)
    }

    fn resolve_column_widths(&mut self, ctx: &mut MeasureCtx, available_width: f32) {
        let theme = self.resolved_theme();
        let header_style = theme.text_style(theme.palette.placeholder);
        let body_style = theme.body_text_style();
        let numeric_style = numeric_text_style(body_style.clone());
        self.column_widths = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let measured_title = measure_text(ctx, &column.title, &header_style).width;
                let cell_style = if column.numeric {
                    &numeric_style
                } else {
                    &body_style
                };
                let measured_cells = self
                    .rows
                    .iter()
                    .filter_map(|row| row.cells.get(index))
                    .map(|cell| measure_text(ctx, cell, cell_style).width)
                    .fold(0.0, f32::max);
                column.width.unwrap_or(
                    (measured_title.max(measured_cells) + (theme.metrics.table_cell_padding * 2.0))
                        .max(column.min_width),
                )
            })
            .collect();

        if available_width <= 0.0 || self.column_widths.is_empty() {
            return;
        }

        let total = self.column_widths.iter().sum::<f32>();
        if total < available_width {
            let extra = (available_width - total) / self.column_widths.len() as f32;
            for width in &mut self.column_widths {
                *width += extra;
            }
        }
    }

    fn activate(&mut self, index: usize) {
        if index >= self.rows.len() {
            return;
        }
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index);
        }
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }

        self.hovered = hovered;
        let theme = self.resolved_theme();
        self.hover_motion.set_hover_target(hovered, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        let theme = self.resolved_theme();
        self.press_motion.set_press_target(pressed, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (hover_changed, hover_active) = self.hover_motion.advance(time);
        let (press_changed, press_active) = self.press_motion.advance(time);
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);

        if hover_changed || press_changed || focus_changed {
            ctx.request_paint();
        }
        if hover_active || press_active || focus_active {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for Table {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let body = self.body_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll && body.contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                let next = self.clamp_scroll(body.height(), self.scroll_y - delta.y);
                if (next - self.scroll_y).abs() > f32::EPSILON {
                    self.scroll_y = next;
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && body.contains(pointer.position) =>
            {
                let pressed = self.row_at_position(ctx.bounds(), pointer.position);
                self.set_hovered(pressed, ctx);
                self.set_pressed(pressed, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(pressed, hovered)| pressed == hovered)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.rows.is_empty() {
                    return;
                }

                let current = self.current_selected().unwrap_or(0);
                match key.key.as_str() {
                    "ArrowUp" => self.activate(current.saturating_sub(1)),
                    "ArrowDown" => self.activate((current + 1).min(self.rows.len() - 1)),
                    "Home" => self.activate(0),
                    "End" => self.activate(self.rows.len() - 1),
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let desired_width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            540.0
        };
        let theme = self.resolved_theme();
        let padding = theme.metrics.data_viewport_padding;
        self.resolve_column_widths(ctx, (desired_width - padding.left - padding.right).max(0.0));
        let desired_height = padding.top
            + self.resolved_header_height()
            + theme.metrics.select_menu_gap
            + self.content_height()
            + padding.bottom;
        let size = constraints.clamp(Size::new(desired_width, desired_height));
        self.scroll_y = self.clamp_scroll(
            self.body_rect(Rect::from_origin_size(Point::ZERO, size))
                .height(),
            self.scroll_y,
        );
        size
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let header_style = theme.text_style(palette.placeholder);
        let body_style = theme.body_text_style();
        let numeric_body_style = numeric_text_style(body_style.clone());
        let body = self.body_rect(ctx.bounds());
        let padding = metrics.data_viewport_padding;
        let header = Rect::new(
            ctx.bounds().x() + padding.left,
            ctx.bounds().y() + padding.top,
            (ctx.bounds().width() - padding.left - padding.right).max(0.0),
            self.resolved_header_height(),
        );
        let row_height = self.resolved_row_height();

        draw_surface(ctx, ctx.bounds(), &theme, self.focus_animation.value);
        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            palette.control,
        );

        let mut x = header.x();
        for (index, column) in self.columns.iter().enumerate() {
            let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
            let cell = Rect::new(x, header.y(), width, header.height());
            if index > 0 {
                let separator_inset = metrics
                    .table_header_separator_inset
                    .min(cell.height() * 0.5);
                ctx.stroke_rect(
                    Rect::new(
                        cell.x(),
                        cell.y() + separator_inset,
                        metrics.table_separator_width,
                        (cell.height() - (separator_inset * 2.0)).max(0.0),
                    ),
                    palette.border,
                    sui_scene::StrokeStyle::new(metrics.table_separator_width.max(1.0)),
                );
            }
            draw_aligned_text(
                ctx,
                horizontal_inset_rect(cell, metrics.table_cell_padding),
                &column.title,
                &header_style,
                column.alignment,
            );
            x += width;
        }

        ctx.push_clip_rect(body);
        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + body.height()) / row_height).ceil() as usize + 1)
            .min(self.rows.len());

        for row_index in start..end {
            let row_y = body.y() + (row_index as f32 * row_height) - self.scroll_y;
            let row_rect = Rect::new(body.x(), row_y, body.width(), row_height);
            let selected = self.current_selected() == Some(row_index);
            let hover_amount = self.hover_motion.amount_for(&row_index);
            let press_amount = self.press_motion.amount_for(&row_index);
            let background = if row_index % 2 == 0 {
                palette.surface.with_alpha(0.88)
            } else {
                palette.surface_raised
            };
            ctx.fill_rect(row_rect, background);
            if selected
                || hover_amount > AnimatedScalar::EPSILON
                || press_amount > AnimatedScalar::EPSILON
            {
                ctx.fill_rect(
                    row_rect,
                    data_row_state_fill(&theme, selected, hover_amount, press_amount),
                );
            }
            ctx.stroke_rect(
                row_rect,
                palette.border.with_alpha(metrics.table_row_border_opacity),
                sui_scene::StrokeStyle::new(metrics.table_separator_width.max(1.0)),
            );

            let mut cell_x = row_rect.x();
            for (column_index, column) in self.columns.iter().enumerate() {
                let width = *self
                    .column_widths
                    .get(column_index)
                    .unwrap_or(&column.min_width);
                let cell_rect = Rect::new(cell_x, row_rect.y(), width, row_rect.height());
                if let Some(value) = self.rows[row_index].cells.get(column_index) {
                    let style = if column.numeric {
                        numeric_body_style.clone()
                    } else {
                        body_style.clone()
                    };
                    draw_aligned_text(
                        ctx,
                        horizontal_inset_rect(cell_rect, metrics.table_cell_padding),
                        value,
                        &style,
                        column.alignment,
                    );
                }
                cell_x += width;
            }
        }

        ctx.pop_clip();
        draw_vertical_scroll_thumb(
            ctx,
            &theme,
            body,
            self.content_height(),
            self.scroll_y,
            palette.border_hover,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Table, ctx.bounds());
        node.name = Some(self.name());
        node.state.focused = ctx.is_focused();
        node.value = self
            .current_selected()
            .and_then(|row| self.rows.get(row))
            .and_then(|row| row.cells.first())
            .cloned()
            .map(SemanticsValue::Text);
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualTableSortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualTableColumn {
    title: String,
    width: Option<f32>,
    min_width: f32,
    alignment: TableColumnAlignment,
    sort_direction: Option<VirtualTableSortDirection>,
}

impl VirtualTableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: None,
            min_width: 96.0,
            alignment: TableColumnAlignment::Start,
            sort_direction: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(32.0));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width.max(32.0);
        self
    }

    pub fn alignment(mut self, alignment: TableColumnAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn sort_direction(mut self, direction: Option<VirtualTableSortDirection>) -> Self {
        self.sort_direction = direction;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualTableRowActivationKind {
    Single,
    Double,
}

pub struct VirtualTableRowContext<'a> {
    pub row_index: usize,
    pub row_rect: Rect,
    pub column_rects: &'a [Rect],
    pub selected: bool,
    pub hovered: bool,
    pub pressed: bool,
}

type ListViewChange = Box<dyn FnMut(usize, String)>;
type ListViewContextChange = Box<dyn FnMut(usize, String, &mut EventCtx)>;
type VirtualTableRowPainter = Box<dyn for<'a> Fn(&mut PaintCtx, &VirtualTableRowContext<'a>)>;
type VirtualTableRowName = Box<dyn Fn(usize) -> String>;
type VirtualTableRowDescription = Box<dyn Fn(usize) -> String>;
type VirtualTableCellActivation =
    Box<dyn FnMut(usize, usize, VirtualTableRowActivationKind) -> bool>;

pub struct VirtualTable {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    columns: Vec<VirtualTableColumn>,
    row_count: usize,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered_row: Option<usize>,
    pressed_row: Option<usize>,
    pressed_header: Option<usize>,
    row_height: Option<f32>,
    header_height: Option<f32>,
    scroll_y: f32,
    column_widths: Vec<f32>,
    focus_animation: AnimatedScalar,
    row_painter: Option<VirtualTableRowPainter>,
    row_name: Option<VirtualTableRowName>,
    row_description: Option<VirtualTableRowDescription>,
    on_row_activate: Option<Box<dyn FnMut(usize, VirtualTableRowActivationKind)>>,
    on_cell_activate: Option<VirtualTableCellActivation>,
    on_header_activate: Option<Box<dyn FnMut(usize)>>,
    on_near_end: Option<Box<dyn FnMut()>>,
    last_click: Option<(usize, Instant)>,
}

impl VirtualTable {
    const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);

    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            columns: Vec::new(),
            row_count: 0,
            selected: None,
            selected_reader: None,
            hovered_row: None,
            pressed_row: None,
            pressed_header: None,
            row_height: None,
            header_height: None,
            scroll_y: 0.0,
            column_widths: Vec::new(),
            focus_animation: AnimatedScalar::new(0.0),
            row_painter: None,
            row_name: None,
            row_description: None,
            on_row_activate: None,
            on_cell_activate: None,
            on_header_activate: None,
            on_near_end: None,
            last_click: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn columns<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = VirtualTableColumn>,
    {
        self.columns.extend(columns);
        self
    }

    pub fn row_count(mut self, row_count: usize) -> Self {
        self.row_count = row_count;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = Some(row_height.max(1.0));
        self
    }

    pub fn header_height(mut self, header_height: f32) -> Self {
        self.header_height = Some(header_height.max(1.0));
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn row_painter<F>(mut self, painter: F) -> Self
    where
        F: for<'a> Fn(&mut PaintCtx, &VirtualTableRowContext<'a>) + 'static,
    {
        self.row_painter = Some(Box::new(painter));
        self
    }

    pub fn row_name<F>(mut self, name: F) -> Self
    where
        F: Fn(usize) -> String + 'static,
    {
        self.row_name = Some(Box::new(name));
        self
    }

    pub fn row_description<F>(mut self, description: F) -> Self
    where
        F: Fn(usize) -> String + 'static,
    {
        self.row_description = Some(Box::new(description));
        self
    }

    pub fn on_row_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, VirtualTableRowActivationKind) + 'static,
    {
        self.on_row_activate = Some(Box::new(on_activate));
        self
    }

    pub fn on_cell_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, usize, VirtualTableRowActivationKind) -> bool + 'static,
    {
        self.on_cell_activate = Some(Box::new(on_activate));
        self
    }

    pub fn on_header_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize) + 'static,
    {
        self.on_header_activate = Some(Box::new(on_activate));
        self
    }

    pub fn on_near_end<F>(mut self, on_near_end: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_near_end = Some(Box::new(on_near_end));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .and_then(|selected| selected())
            .or(self.selected)
            .filter(|index| *index < self.row_count)
    }

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .unwrap_or(self.resolved_theme().metrics.table_row_height)
    }

    fn resolved_header_height(&self) -> f32 {
        self.header_height
            .unwrap_or(self.resolved_theme().metrics.table_header_height)
    }

    fn body_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.data_viewport_padding;
        let gap = theme.metrics.select_menu_gap;
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top + self.resolved_header_height() + gap,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom - self.resolved_header_height() - gap)
                .max(0.0),
        )
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.data_viewport_padding;
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top,
            (bounds.width() - padding.left - padding.right).max(0.0),
            self.resolved_header_height(),
        )
    }

    fn content_height(&self) -> f32 {
        self.row_count as f32 * self.resolved_row_height()
    }

    fn clamp_scroll(&self, viewport_height: f32, scroll_y: f32) -> f32 {
        let max_scroll = (self.content_height() - viewport_height).max(0.0);
        scroll_y.clamp(0.0, max_scroll)
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let body = self.body_rect(bounds);
        if !body.contains(position) {
            return None;
        }
        let y = position.y - body.y() + self.scroll_y;
        let index = (y / self.resolved_row_height()).floor() as usize;
        (index < self.row_count).then_some(index)
    }

    fn body_column_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let body = self.body_rect(bounds);
        if !body.contains(position) {
            return None;
        }
        let mut x = body.x();
        for (index, column) in self.columns.iter().enumerate() {
            let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
            if Rect::new(x, body.y(), width, body.height()).contains(position) {
                return Some(index);
            }
            x += width;
        }
        None
    }

    fn header_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let header = self.header_rect(bounds);
        if !header.contains(position) {
            return None;
        }
        let mut x = header.x();
        for (index, column) in self.columns.iter().enumerate() {
            let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
            let cell = Rect::new(x, header.y(), width, header.height());
            if cell.contains(position) {
                return Some(index);
            }
            x += width;
        }
        None
    }

    fn resolve_column_widths(&mut self, ctx: &mut MeasureCtx, available_width: f32) {
        let theme = self.resolved_theme();
        let header_style = theme.text_style(theme.palette.placeholder);
        self.column_widths = self
            .columns
            .iter()
            .map(|column| {
                column.width.unwrap_or(
                    (measure_text(ctx, &column.title, &header_style).width
                        + (theme.metrics.table_cell_padding * 2.0))
                        .max(column.min_width),
                )
            })
            .collect();

        if available_width <= 0.0 || self.column_widths.is_empty() {
            return;
        }

        let total = self.column_widths.iter().sum::<f32>();
        if total < available_width {
            let extra = (available_width - total) / self.column_widths.len() as f32;
            for width in &mut self.column_widths {
                *width += extra;
            }
        }
    }

    fn column_rects(&self, row_rect: Rect) -> Vec<Rect> {
        let mut x = row_rect.x();
        self.columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
                let rect = Rect::new(x, row_rect.y(), width, row_rect.height());
                x += width;
                rect
            })
            .collect()
    }

    fn row_activation_kind(&mut self, row_index: usize) -> VirtualTableRowActivationKind {
        let now = Instant::now();
        let double_click = self.last_click.take().is_some_and(|(index, instant)| {
            index == row_index && now.duration_since(instant) <= Self::DOUBLE_CLICK_WINDOW
        });
        if double_click {
            VirtualTableRowActivationKind::Double
        } else {
            self.last_click = Some((row_index, now));
            VirtualTableRowActivationKind::Single
        }
    }

    fn activate_row(&mut self, row_index: usize, column_index: Option<usize>) {
        if row_index >= self.row_count {
            return;
        }
        self.selected = Some(row_index);
        let kind = self.row_activation_kind(row_index);
        if let Some(column_index) = column_index {
            if let Some(on_activate) = &mut self.on_cell_activate {
                if on_activate(row_index, column_index, kind) {
                    return;
                }
            }
        }
        if let Some(on_activate) = &mut self.on_row_activate {
            on_activate(row_index, kind);
        }
    }

    fn maybe_notify_near_end(&mut self, viewport_height: f32) {
        let remaining = (self.content_height() - viewport_height - self.scroll_y).max(0.0);
        if remaining <= self.resolved_row_height() * 12.0 {
            if let Some(on_near_end) = &mut self.on_near_end {
                on_near_end();
            }
        }
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);
        if focus_changed {
            ctx.request_paint();
        }
        if focus_active {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for VirtualTable {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let bounds = ctx.bounds();
        let body = self.body_rect(bounds);

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.row_at_position(bounds, pointer.position);
                if self.hovered_row != hovered {
                    self.hovered_row = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll && body.contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                let next = self.clamp_scroll(body.height(), self.scroll_y - delta.y);
                if (next - self.scroll_y).abs() > f32::EPSILON {
                    self.scroll_y = next;
                    self.maybe_notify_near_end(body.height());
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(header) = self.header_at_position(bounds, pointer.position) {
                    self.pressed_header = Some(header);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                } else if let Some(row) = self.row_at_position(bounds, pointer.position) {
                    self.pressed_row = Some(row);
                    self.hovered_row = Some(row);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let header = self.header_at_position(bounds, pointer.position);
                if let Some(index) = self
                    .pressed_header
                    .zip(header)
                    .filter(|(pressed, hovered)| pressed == hovered)
                    .map(|(index, _)| index)
                {
                    if let Some(on_activate) = &mut self.on_header_activate {
                        on_activate(index);
                    }
                } else {
                    let hovered = self.row_at_position(bounds, pointer.position);
                    if let Some(index) = self
                        .pressed_row
                        .zip(hovered)
                        .filter(|(pressed, hovered)| pressed == hovered)
                        .map(|(index, _)| index)
                    {
                        let column = self.body_column_at_position(bounds, pointer.position);
                        self.activate_row(index, column);
                    }
                    self.hovered_row = hovered;
                }
                self.pressed_row = None;
                self.pressed_header = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered_row.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_row.take().is_some() || self.pressed_header.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
                if self.hovered_row.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.row_count == 0 {
                    return;
                }
                let current = self.resolved_selected().unwrap_or(0);
                match key.key.as_str() {
                    "ArrowUp" => self.activate_row(current.saturating_sub(1), None),
                    "ArrowDown" => self.activate_row((current + 1).min(self.row_count - 1), None),
                    "Home" => self.activate_row(0, None),
                    "End" => self.activate_row(self.row_count - 1, None),
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let desired_width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            540.0
        };
        let desired_height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            let theme = self.resolved_theme();
            let padding = theme.metrics.data_viewport_padding;
            (padding.top
                + self.resolved_header_height()
                + theme.metrics.select_menu_gap
                + self.content_height()
                + padding.bottom)
                .min(420.0)
        };
        let theme = self.resolved_theme();
        let padding = theme.metrics.data_viewport_padding;
        self.resolve_column_widths(ctx, (desired_width - padding.left - padding.right).max(0.0));
        let size = constraints.clamp(Size::new(desired_width, desired_height));
        self.scroll_y = self.clamp_scroll(
            self.body_rect(Rect::from_origin_size(Point::ZERO, size))
                .height(),
            self.scroll_y,
        );
        size
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let header_style = theme.text_style(palette.placeholder);
        let bounds = ctx.bounds();
        let body = self.body_rect(bounds);
        let header = self.header_rect(bounds);
        let row_height = self.resolved_row_height();
        let selected = self.resolved_selected();

        draw_surface(ctx, bounds, &theme, self.focus_animation.value);
        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            palette.control,
        );

        let mut x = header.x();
        for (index, column) in self.columns.iter().enumerate() {
            let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
            let cell = Rect::new(x, header.y(), width, header.height());
            if index > 0 {
                let separator_inset = metrics
                    .table_header_separator_inset
                    .min(cell.height() * 0.5);
                ctx.stroke_rect(
                    Rect::new(
                        cell.x(),
                        cell.y() + separator_inset,
                        metrics.table_separator_width,
                        (cell.height() - (separator_inset * 2.0)).max(0.0),
                    ),
                    palette.border,
                    sui_scene::StrokeStyle::new(metrics.table_separator_width.max(1.0)),
                );
            }
            let title = match column.sort_direction {
                Some(VirtualTableSortDirection::Ascending) => format!("{} ^", column.title),
                Some(VirtualTableSortDirection::Descending) => format!("{} v", column.title),
                None => column.title.clone(),
            };
            draw_aligned_text(
                ctx,
                horizontal_inset_rect(cell, metrics.table_cell_padding),
                &title,
                &header_style,
                column.alignment,
            );
            x += width;
        }

        ctx.push_clip_rect(body);
        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + body.height()) / row_height).ceil() as usize + 1)
            .min(self.row_count);

        for row_index in start..end {
            let row_y = body.y() + (row_index as f32 * row_height) - self.scroll_y;
            let row_rect = Rect::new(body.x(), row_y, body.width(), row_height);
            let row_selected = selected == Some(row_index);
            let row_hovered = self.hovered_row == Some(row_index);
            let row_pressed = self.pressed_row == Some(row_index);
            let background = if row_selected {
                data_row_state_fill(&theme, true, 0.0, 0.0)
            } else if row_pressed || row_hovered {
                data_row_state_fill(
                    &theme,
                    false,
                    row_hovered as u8 as f32,
                    row_pressed as u8 as f32,
                )
            } else {
                Color::TRANSPARENT
            };
            if background.alpha > 0.0 {
                ctx.fill(
                    rounded_rect_path(row_rect, metrics.corner_radius),
                    background,
                );
            }
            let columns = self.column_rects(row_rect);
            if let Some(painter) = &self.row_painter {
                let row = VirtualTableRowContext {
                    row_index,
                    row_rect,
                    column_rects: &columns,
                    selected: row_selected,
                    hovered: row_hovered,
                    pressed: row_pressed,
                };
                painter(ctx, &row);
            }
        }
        ctx.pop_clip();

        draw_vertical_scroll_thumb(
            ctx,
            &theme,
            body,
            self.content_height(),
            self.scroll_y,
            palette.placeholder,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Table, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(format!("{} rows", self.row_count)));
        let columns = self
            .columns
            .iter()
            .filter_map(|column| {
                let title = column.title.trim();
                (!title.is_empty()).then(|| title.to_string())
            })
            .collect::<Vec<_>>();
        if !columns.is_empty() {
            node.description = Some(format!("Columns: {}", columns.join(", ")));
        }
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        let body = self.body_rect(ctx.bounds());
        let row_height = self.resolved_row_height();
        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + body.height()) / row_height).ceil() as usize + 1)
            .min(self.row_count);
        let selected = self.resolved_selected();
        for row_index in start..end {
            let row_y = body.y() + (row_index as f32 * row_height) - self.scroll_y;
            let row_rect = Rect::new(body.x(), row_y, body.width(), row_height);
            let mut row = SemanticsNode::new(
                WidgetId::new(
                    ctx.widget_id()
                        .get()
                        .wrapping_mul(67)
                        .wrapping_add(row_index as u64),
                ),
                SemanticsRole::ListItem,
                row_rect,
            );
            row.parent = Some(ctx.widget_id());
            row.name = self.row_name.as_ref().map(|name| name(row_index));
            row.description = self
                .row_description
                .as_ref()
                .map(|description| description(row_index));
            row.state.selected = selected == Some(row_index);
            row.state.hovered = self.hovered_row == Some(row_index);
            row.actions = vec![SemanticsAction::Activate];
            ctx.push(row);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbItem {
    label: String,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

pub type PathBar = Breadcrumb;

pub struct Breadcrumb {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    name_reader: Option<Box<dyn Fn() -> String>>,
    items: Vec<BreadcrumbItem>,
    current: usize,
    current_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    focused_index: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hover_motion: IndexedInteractionMotion<usize>,
    press_motion: IndexedInteractionMotion<usize>,
    focus_animation: AnimatedScalar,
    measured_widths: Vec<f32>,
    content_width: f32,
    scroll_x: f32,
    auto_scroll_to_end: bool,
    drag_anchor: Option<(Point, f32)>,
    drag_moved: bool,
    on_activate: Option<Box<dyn FnMut(usize, String)>>,
}

impl Breadcrumb {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            name_reader: None,
            items: Vec::new(),
            current: 0,
            current_reader: None,
            focused_index: 0,
            hovered: None,
            pressed: None,
            hover_motion: IndexedInteractionMotion::new(),
            press_motion: IndexedInteractionMotion::new(),
            focus_animation: AnimatedScalar::new(0.0),
            measured_widths: Vec::new(),
            content_width: 0.0,
            scroll_x: 0.0,
            auto_scroll_to_end: true,
            drag_anchor: None,
            drag_moved: false,
            on_activate: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = BreadcrumbItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn current(mut self, current: usize) -> Self {
        self.current = current;
        self.current_reader = None;
        self.focused_index = current;
        self
    }

    pub fn name_when<F>(mut self, name: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.name_reader = Some(Box::new(name));
        self
    }

    pub fn current_when<F>(mut self, current: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.current_reader = Some(Box::new(current));
        self
    }

    pub fn on_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_activate = Some(Box::new(on_activate));
        self
    }

    fn normalized_current(&self) -> usize {
        let current = self
            .current_reader
            .as_ref()
            .and_then(|reader| reader())
            .unwrap_or(self.current);
        if self.items.is_empty() {
            0
        } else {
            current.min(self.items.len() - 1)
        }
    }

    fn name(&self) -> String {
        self.name_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.name.clone())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn activate(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }
        self.current = index;
        self.focused_index = index;
        if let Some(on_activate) = &mut self.on_activate {
            on_activate(index, self.items[index].label.clone());
        }
    }

    fn max_scroll_x(&self, bounds: Rect) -> f32 {
        (self.content_width - bounds.width()).max(0.0)
    }

    fn content_overflows(&self, bounds: Rect) -> bool {
        self.max_scroll_x(bounds) > 0.5
    }

    fn sync_scroll_to_bounds(&mut self, bounds: Rect) {
        let max_scroll = self.max_scroll_x(bounds);
        self.scroll_x = if self.auto_scroll_to_end {
            max_scroll
        } else {
            self.scroll_x.clamp(0.0, max_scroll)
        };
    }

    fn set_scroll_x(&mut self, bounds: Rect, next_scroll_x: f32) -> bool {
        let max_scroll = self.max_scroll_x(bounds);
        let next_scroll_x = next_scroll_x.clamp(0.0, max_scroll);
        if (self.scroll_x - next_scroll_x).abs() <= 0.5 {
            return false;
        }

        self.scroll_x = next_scroll_x;
        self.auto_scroll_to_end = false;
        true
    }

    fn wheel_scroll_delta(delta: Vector) -> f32 {
        if delta.x.abs() > 0.0 {
            -delta.x
        } else {
            -delta.y
        }
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let widths = &self.measured_widths;
        if index >= widths.len() {
            return None;
        }
        let theme = self.resolved_theme();
        let padding = theme.metrics.breadcrumb_item_padding;
        let gap = theme.metrics.breadcrumb_gap;
        let mut x = bounds.x() + padding.left - self.scroll_x;
        for (current, width) in widths.iter().enumerate() {
            let rect = Rect::new(
                x,
                bounds.y() + padding.top,
                *width,
                (bounds.height() - padding.top - padding.bottom).max(0.0),
            );
            if current == index {
                return Some(rect);
            }
            x += *width + gap;
        }
        None
    }

    fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }

        (0..self.items.len()).find(|index| {
            self.item_rect(bounds, *index)
                .is_some_and(|rect| rect.contains(position))
        })
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }

        self.hovered = hovered;
        let theme = self.resolved_theme();
        self.hover_motion.set_hover_target(hovered, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        let theme = self.resolved_theme();
        self.press_motion.set_press_target(pressed, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let (hover_changed, hover_active) = self.hover_motion.advance(time);
        let (press_changed, press_active) = self.press_motion.advance(time);
        let (focus_changed, focus_active) = advance_scalar(&mut self.focus_animation, time);

        if hover_changed || press_changed || focus_changed {
            ctx.request_paint();
        }
        if hover_active || press_active || focus_active {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for Breadcrumb {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if pointer.buttons.contains(PointerButton::Primary) {
                    if let Some((anchor_position, anchor_scroll_x)) = self.drag_anchor {
                        let bounds = ctx.bounds();
                        if self.content_overflows(bounds) {
                            let next_scroll_x =
                                anchor_scroll_x - (pointer.position.x - anchor_position.x);
                            if self.set_scroll_x(bounds, next_scroll_x) {
                                self.drag_moved = true;
                                self.set_pressed(None, ctx);
                                ctx.request_paint();
                                ctx.request_semantics();
                                ctx.set_handled();
                            }

                            if (pointer.position.x - anchor_position.x).abs() > 3.0 {
                                self.drag_moved = true;
                            }
                        }
                    }
                }

                let hovered = self.item_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Scroll => {
                let bounds = ctx.bounds();
                if bounds.contains(pointer.position) && self.content_overflows(bounds) {
                    let delta = pointer
                        .scroll_delta
                        .map(scroll_delta_to_offset)
                        .unwrap_or(pointer.delta);
                    if self.set_scroll_x(bounds, self.scroll_x + Self::wheel_scroll_delta(delta)) {
                        let hovered = self.item_at(bounds, pointer.position);
                        self.set_hovered(hovered, ctx);
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let pressed = self.item_at(ctx.bounds(), pointer.position);
                self.drag_anchor = Some((pointer.position, self.scroll_x));
                self.drag_moved = false;
                self.set_hovered(pressed, ctx);
                self.set_pressed(pressed, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                if !self.drag_moved {
                    if let Some(index) = self
                        .pressed
                        .zip(hovered)
                        .filter(|(pressed, hovered)| pressed == hovered)
                        .map(|(index, _)| index)
                    {
                        self.activate(index);
                    }
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                self.drag_anchor = None;
                self.drag_moved = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.drag_anchor.is_none() {
                    self.set_hovered(None, ctx);
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() || self.drag_anchor.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    self.drag_anchor = None;
                    self.drag_moved = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.items.is_empty() {
                    return;
                }
                match key.key.as_str() {
                    "ArrowLeft" => {
                        self.focused_index = self.focused_index.saturating_sub(1);
                    }
                    "ArrowRight" => {
                        self.focused_index = (self.focused_index + 1).min(self.items.len() - 1);
                    }
                    "Enter" | " " => self.activate(self.focused_index),
                    "Home" => self.focused_index = 0,
                    "End" => self.focused_index = self.items.len() - 1,
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let text_style = theme.body_text_style();
        self.measured_widths = self
            .items
            .iter()
            .map(|item| {
                measure_text(ctx, &item.label, &text_style).width
                    + theme.metrics.breadcrumb_item_padding.left
                    + theme.metrics.breadcrumb_item_padding.right
            })
            .collect();
        let desired_width = self.measured_widths.iter().sum::<f32>()
            + (self.items.len().saturating_sub(1) as f32 * theme.metrics.breadcrumb_gap)
            + theme.metrics.breadcrumb_item_padding.left
            + theme.metrics.breadcrumb_item_padding.right;
        if (self.content_width - desired_width).abs() > 0.5 {
            self.content_width = desired_width;
            self.auto_scroll_to_end = true;
        }
        constraints.clamp(Size::new(
            desired_width.max(180.0),
            theme.metrics.breadcrumb_height,
        ))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_scroll_to_bounds(bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        draw_surface(ctx, bounds, &theme, self.focus_animation.value);

        ctx.push_clip_rect(bounds);
        for (index, item) in self.items.iter().enumerate() {
            let Some(rect) = self.item_rect(bounds, index) else {
                continue;
            };
            if rect.max_x() < bounds.x() || rect.x() > bounds.max_x() {
                continue;
            }

            let current = self.normalized_current() == index;
            let focused = ctx.is_focused() && self.focused_index == index;
            let hover_amount = self.hover_motion.amount_for(&index);
            let press_amount = self.press_motion.amount_for(&index);

            if current
                || focused
                || hover_amount > AnimatedScalar::EPSILON
                || press_amount > AnimatedScalar::EPSILON
            {
                ctx.fill(
                    rounded_rect_path(rect, theme.metrics.corner_radius),
                    data_row_state_fill(&theme, current || focused, hover_amount, press_amount),
                );
            }

            let style = if current {
                theme.body_text_style()
            } else {
                theme.body_text_style()
            };
            draw_aligned_text(
                ctx,
                horizontal_inset_rect(rect, 8.0),
                &item.label,
                &style,
                TableColumnAlignment::Start,
            );

            if index + 1 < self.items.len() {
                let separator = chevron_path(Rect::new(
                    rect.max_x()
                        + ((theme.metrics.breadcrumb_gap
                            - theme.metrics.breadcrumb_separator_size)
                            * 0.5)
                            .max(0.0),
                    rect.y() + ((rect.height() - theme.metrics.breadcrumb_separator_size) * 0.5),
                    theme.metrics.breadcrumb_separator_size,
                    theme.metrics.breadcrumb_separator_size,
                ));
                ctx.stroke(
                    separator,
                    palette.placeholder.with_alpha(0.9),
                    sui_scene::StrokeStyle::new(1.5),
                );
            }
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Breadcrumb, ctx.bounds());
        node.name = Some(self.name());
        node.state.focused = ctx.is_focused();
        node.value = self
            .items
            .get(self.normalized_current())
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TreeRow {
    path: Vec<usize>,
    depth: usize,
    label: String,
    detail: Option<String>,
    has_children: bool,
    expanded: bool,
    disabled: bool,
}

fn flatten_tree(items: &[TreeItem], depth: usize, path: &mut Vec<usize>, rows: &mut Vec<TreeRow>) {
    for (index, item) in items.iter().enumerate() {
        path.push(index);
        rows.push(TreeRow {
            path: path.clone(),
            depth,
            label: item.label.clone(),
            detail: item.detail.clone(),
            has_children: !item.children.is_empty(),
            expanded: item.expanded,
            disabled: item.disabled,
        });
        if item.expanded {
            flatten_tree(&item.children, depth + 1, path, rows);
        }
        path.pop();
    }
}

fn tree_item<'a>(items: &'a [TreeItem], path: &[usize]) -> Option<&'a TreeItem> {
    let (first, rest) = path.split_first()?;
    let item = items.get(*first)?;
    if rest.is_empty() {
        Some(item)
    } else {
        tree_item(&item.children, rest)
    }
}

fn tree_item_mut<'a>(items: &'a mut [TreeItem], path: &[usize]) -> Option<&'a mut TreeItem> {
    let (first, rest) = path.split_first()?;
    let item = items.get_mut(*first)?;
    if rest.is_empty() {
        Some(item)
    } else {
        tree_item_mut(&mut item.children, rest)
    }
}

fn disclosure_rect(theme: &DefaultTheme, row: Rect, depth: usize) -> Rect {
    let metrics = theme.metrics;
    Rect::new(
        row.x() + metrics.data_row_padding.left + depth as f32 * metrics.tree_indent,
        row.y() + ((row.height() - metrics.tree_disclosure_size) * 0.5),
        metrics.tree_disclosure_size,
        metrics.tree_disclosure_size,
    )
}

fn tree_label_offset(theme: &DefaultTheme, depth: usize) -> f32 {
    theme.metrics.data_row_padding.left
        + depth as f32 * theme.metrics.tree_indent
        + theme.metrics.tree_disclosure_size
        + theme.metrics.tree_disclosure_gap
}

fn disclosure_path(rect: Rect, expanded: bool) -> Path {
    let mut builder = PathBuilder::new();
    if expanded {
        builder
            .move_to(Point::new(rect.x(), rect.y() + 2.0))
            .line_to(Point::new(rect.max_x(), rect.y() + 2.0))
            .line_to(Point::new(
                rect.x() + (rect.width() * 0.5),
                rect.max_y() - 2.0,
            ))
            .close();
    } else {
        builder
            .move_to(Point::new(rect.x() + 2.0, rect.y()))
            .line_to(Point::new(
                rect.max_x() - 2.0,
                rect.y() + (rect.height() * 0.5),
            ))
            .line_to(Point::new(rect.x() + 2.0, rect.max_y()))
            .close();
    }
    builder.build()
}

fn chevron_path(rect: Rect) -> Path {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(rect.x(), rect.y()))
        .line_to(Point::new(rect.max_x(), rect.y() + (rect.height() * 0.5)))
        .line_to(Point::new(rect.x(), rect.max_y()));
    builder.build()
}

fn draw_surface(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, _focus_progress: f32) {
    let palette = theme.palette;
    let metrics = theme.metrics;
    ctx.fill(
        rounded_rect_path(rect, metrics.corner_radius),
        palette.surface,
    );
    ctx.stroke(
        rounded_rect_path(rect, metrics.corner_radius),
        palette.border,
        sui_scene::StrokeStyle::new(metrics.border_width.max(1.0)),
    );
}

fn draw_vertical_scroll_thumb(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    viewport: Rect,
    content_height: f32,
    scroll_y: f32,
    color: Color,
) {
    if content_height <= viewport.height() || viewport.height() <= 0.0 {
        return;
    }

    let ratio = (viewport.height() / content_height).clamp(0.08, 1.0);
    let metrics = theme.metrics;
    let thumb_height = (viewport.height() * ratio).max(metrics.data_scroll_thumb_min_length);
    let max_scroll = (content_height - viewport.height()).max(1.0);
    let thumb_y = viewport.y() + ((viewport.height() - thumb_height) * (scroll_y / max_scroll));
    let thumb_width = metrics
        .data_scroll_thumb_width
        .min(viewport.width())
        .max(0.0);
    let thumb_inset = metrics
        .data_scroll_thumb_inset
        .min((viewport.width() - thumb_width).max(0.0));
    ctx.fill(
        rounded_rect_path(
            Rect::new(
                viewport.max_x() - thumb_inset - thumb_width,
                thumb_y,
                thumb_width,
                thumb_height,
            ),
            metrics.data_scroll_thumb_radius,
        ),
        color.with_alpha(metrics.data_scroll_thumb_opacity),
    );
}

fn draw_aligned_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    alignment: TableColumnAlignment,
) {
    let horizontal_alignment = match alignment {
        TableColumnAlignment::Start => 0.0,
        TableColumnAlignment::Center => 0.5,
        TableColumnAlignment::End => 1.0,
    };
    ctx.push_clip_rect(rect);
    paint_aligned_text(
        ctx,
        rect,
        text,
        style,
        style.line_height,
        horizontal_alignment,
    );
    ctx.pop_clip();
}

const TWO_LINE_ROW_TEXT_GAP: f32 = 2.0;

fn row_text_rects(
    ctx: &PaintCtx,
    rect: Rect,
    primary_measurement: TextMeasurement,
    primary_line_height: f32,
    secondary_measurement: Option<TextMeasurement>,
    secondary_line_height: Option<f32>,
) -> (Rect, Option<Rect>) {
    match secondary_line_height {
        Some(secondary_line_height) => {
            let secondary_measurement = secondary_measurement.unwrap_or(primary_measurement);
            let primary_height = primary_line_height.max(primary_measurement.height);
            let secondary_height = secondary_line_height.max(secondary_measurement.height);
            let total_height = primary_height + secondary_height + TWO_LINE_ROW_TEXT_GAP;
            let top = rect.y() + ((rect.height() - total_height) * 0.5).max(0.0);
            let primary_rect = Rect::new(rect.x(), top, rect.width(), primary_height);
            let secondary_rect = Rect::new(
                rect.x(),
                top + primary_height + TWO_LINE_ROW_TEXT_GAP,
                rect.width(),
                secondary_height,
            );
            (
                Rect::new(
                    primary_rect.x(),
                    vertically_centered_text_rect_y(
                        ctx,
                        primary_rect,
                        primary_measurement,
                        primary_height,
                    ),
                    primary_rect.width(),
                    primary_rect.height(),
                ),
                Some(Rect::new(
                    secondary_rect.x(),
                    vertically_centered_text_rect_y(
                        ctx,
                        secondary_rect,
                        secondary_measurement,
                        secondary_height,
                    ),
                    secondary_rect.width(),
                    secondary_rect.height(),
                )),
            )
        }
        None => {
            let height = primary_line_height
                .max(primary_measurement.height)
                .min(rect.height());
            let y = vertically_centered_text_rect_y(ctx, rect, primary_measurement, height);
            (Rect::new(rect.x(), y, rect.width(), height), None)
        }
    }
}

fn two_line_row_height(primary_line_height: f32, secondary_line_height: f32) -> f32 {
    primary_line_height + secondary_line_height + TWO_LINE_ROW_TEXT_GAP
}

fn horizontal_inset_rect(rect: Rect, inset: f32) -> Rect {
    Rect::new(
        rect.x() + inset,
        rect.y(),
        (rect.width() - (inset * 2.0)).max(0.0),
        rect.height(),
    )
}

fn estimate_text_width(text: &str, style: &TextStyle) -> f32 {
    text.chars().count() as f32 * style.font_size * 0.56
}

fn numeric_text_style(mut style: TextStyle) -> TextStyle {
    style.features.enable(FontFeature::TABULAR_FIGURES);
    style
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeadingLabelCellPaint {
    pub icon: Option<IconGlyph>,
    pub icon_color: Option<Color>,
    pub label_color: Option<Color>,
    pub label_weight: FontWeight,
    pub padding_left: f32,
    pub padding_right: f32,
    pub icon_slot_side: Option<f32>,
    pub icon_inset: f32,
    pub gap: Option<f32>,
}

impl LeadingLabelCellPaint {
    pub const fn new() -> Self {
        Self {
            icon: None,
            icon_color: None,
            label_color: None,
            label_weight: FontWeight::NORMAL,
            padding_left: 6.0,
            padding_right: 8.0,
            icon_slot_side: None,
            icon_inset: 2.0,
            gap: None,
        }
    }

    pub const fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub const fn icon_color(mut self, color: Color) -> Self {
        self.icon_color = Some(color);
        self
    }

    pub const fn label_color(mut self, color: Color) -> Self {
        self.label_color = Some(color);
        self
    }

    pub const fn label_weight(mut self, weight: FontWeight) -> Self {
        self.label_weight = weight;
        self
    }

    pub const fn padding(mut self, left: f32, right: f32) -> Self {
        self.padding_left = left;
        self.padding_right = right;
        self
    }

    pub const fn icon_slot_side(mut self, side: f32) -> Self {
        self.icon_slot_side = Some(side);
        self
    }

    pub const fn icon_inset(mut self, inset: f32) -> Self {
        self.icon_inset = inset;
        self
    }

    pub const fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap);
        self
    }
}

impl Default for LeadingLabelCellPaint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCellPaint {
    pub color: Option<Color>,
    pub weight: FontWeight,
    pub padding_left: f32,
    pub padding_right: f32,
    pub alignment: TableColumnAlignment,
    pub numeric: bool,
    pub text_token: Option<ThemeTextToken>,
}

impl TextCellPaint {
    pub const fn new() -> Self {
        Self {
            color: None,
            weight: FontWeight::NORMAL,
            padding_left: 8.0,
            padding_right: 8.0,
            alignment: TableColumnAlignment::Start,
            numeric: false,
            text_token: None,
        }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub const fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub const fn padding(mut self, left: f32, right: f32) -> Self {
        self.padding_left = left;
        self.padding_right = right;
        self
    }

    pub const fn alignment(mut self, alignment: TableColumnAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub const fn numeric(mut self) -> Self {
        self.numeric = true;
        self
    }

    pub const fn text_token(mut self, token: ThemeTextToken) -> Self {
        self.text_token = Some(token);
        self
    }
}

impl Default for TextCellPaint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextBlockPaint {
    pub color: Option<Color>,
    pub weight: FontWeight,
    pub padding: Insets,
    pub text_token: Option<ThemeTextToken>,
    pub wrap: TextWrap,
    pub alignment: TextAlign,
}

impl TextBlockPaint {
    pub const fn new() -> Self {
        Self {
            color: None,
            weight: FontWeight::NORMAL,
            padding: Insets::ZERO,
            text_token: None,
            wrap: TextWrap::Word,
            alignment: TextAlign::Start,
        }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub const fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub const fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub const fn text_token(mut self, token: ThemeTextToken) -> Self {
        self.text_token = Some(token);
        self
    }

    pub const fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub const fn alignment(mut self, alignment: TextAlign) -> Self {
        self.alignment = alignment;
        self
    }
}

impl Default for TextBlockPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_leading_label_cell(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    style: LeadingLabelCellPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let mut text_x = rect.x() + style.padding_left.max(0.0);
    if let Some(icon) = style.icon {
        let slot_side = style
            .icon_slot_side
            .unwrap_or(theme.metrics.data_row_icon_size)
            .min((rect.height() - 6.0).max(0.0))
            .max(0.0);
        let icon_rect = Rect::new(
            text_x,
            rect.y() + ((rect.height() - slot_side) * 0.5).max(0.0),
            slot_side,
            slot_side,
        );
        let icon_inset = style.icon_inset.max(0.0).min(slot_side * 0.45);
        draw_icon_glyph(
            ctx,
            icon,
            icon_rect.inflate(-icon_inset, -icon_inset),
            style.icon_color.unwrap_or(theme.palette.text_muted),
        );
        text_x = icon_rect.max_x()
            + style
                .gap
                .unwrap_or(theme.metrics.data_row_icon_gap)
                .max(0.0);
    }

    let label_rect = Rect::new(
        text_x,
        rect.y(),
        (rect.max_x() - text_x - style.padding_right.max(0.0)).max(0.0),
        rect.height(),
    );
    if label_rect.width() <= 0.0 {
        return;
    }

    let mut label_style = TextStyle {
        font_size: theme.text.sm.size.max(1.0),
        line_height: theme.text.sm.line_height.max(1.0),
        color: style.label_color.unwrap_or(theme.palette.text),
        ..theme.body_text_style()
    };
    label_style.weight = style.label_weight;
    let measurement = paint_text_measurement(ctx, label, &label_style);
    let text_rect = vertically_centered_text_rect_y(
        ctx,
        label_rect,
        measurement,
        label_style.line_height.max(measurement.height),
    );
    let text_rect = Rect::new(
        label_rect.x(),
        text_rect,
        label_rect.width(),
        label_rect.height(),
    );

    ctx.push_clip_rect(label_rect);
    paint_aligned_text(
        ctx,
        text_rect,
        label,
        &label_style,
        label_style.line_height,
        0.0,
    );
    ctx.pop_clip();
}

pub fn paint_text_block(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    text: &str,
    style: TextBlockPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let content = Rect::new(
        rect.x() + style.padding.left.max(0.0),
        rect.y() + style.padding.top.max(0.0),
        (rect.width() - style.padding.left.max(0.0) - style.padding.right.max(0.0)).max(0.0),
        (rect.height() - style.padding.top.max(0.0) - style.padding.bottom.max(0.0)).max(0.0),
    );
    if content.width() <= 0.0 || content.height() <= 0.0 {
        return;
    }

    let token = style.text_token.unwrap_or(theme.text.sm);
    let mut text_style = TextStyle {
        font_size: token.size.max(1.0),
        line_height: token.line_height.max(1.0),
        color: style.color.unwrap_or(theme.palette.text),
        ..theme.body_text_style()
    };
    text_style.weight = style.weight;

    let color = text_style.color;
    let mut layout_style = text_style.clone();
    layout_style.color = Color::WHITE;
    let mut document = TextDocument::from_plain_text(text.to_string(), layout_style);
    for paragraph in &mut document.paragraphs {
        paragraph.style.align = style.alignment;
        paragraph.style.wrap = style.wrap;
    }

    ctx.push_clip_rect(content);
    if let Ok(layout) = ctx.layout_text_document(TextLayoutRequest::new(document).with_box_size(
        Size::new(content.width().max(1.0), content.height().max(1.0)),
    )) {
        ctx.draw_text_layout_with_color(content.origin, &layout, color);
    } else {
        ctx.draw_text(content, text.to_string(), text_style);
    }
    ctx.pop_clip();
}

pub fn paint_text_cell(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    text: &str,
    style: TextCellPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let left = style.padding_left.max(0.0);
    let right = style.padding_right.max(0.0);
    let content = Rect::new(
        rect.x() + left,
        rect.y(),
        (rect.width() - left - right).max(0.0),
        rect.height(),
    );
    if content.width() <= 0.0 {
        return;
    }

    let token = style.text_token.unwrap_or(theme.text.sm);
    let mut text_style = TextStyle {
        font_size: token.size.max(1.0),
        line_height: token.line_height.max(1.0),
        color: style.color.unwrap_or(theme.palette.text),
        ..theme.body_text_style()
    };
    text_style.weight = style.weight;
    if style.numeric {
        text_style = numeric_text_style(text_style);
    }

    let measurement = paint_text_measurement(ctx, text, &text_style);
    let text_y = vertically_centered_text_rect_y(
        ctx,
        content,
        measurement,
        text_style.line_height.max(measurement.height),
    );
    let text_rect = Rect::new(content.x(), text_y, content.width(), content.height());

    ctx.push_clip_rect(content);
    paint_aligned_text(
        ctx,
        text_rect,
        text,
        &text_style,
        text_style.line_height,
        match style.alignment {
            TableColumnAlignment::Start => 0.0,
            TableColumnAlignment::Center => 0.5,
            TableColumnAlignment::End => 1.0,
        },
    );
    ctx.pop_clip();
}

fn measure_list_item_leading_width(
    ctx: &mut MeasureCtx,
    item: &ListItem,
    style: &TextStyle,
    theme: &DefaultTheme,
) -> f32 {
    if item.leading_icon.is_some() {
        return theme.metrics.data_row_icon_size + theme.metrics.data_row_icon_gap;
    }
    item.leading_text
        .as_deref()
        .map(|text| measure_text(ctx, text, style).width + theme.metrics.data_row_icon_gap)
        .unwrap_or(0.0)
}

fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.layout()
        .measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: estimate_text_width(text, style),
            height: style.line_height,
            bounds: Rect::ZERO,
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}

fn paint_text_measurement(ctx: &PaintCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: estimate_text_width(text, style),
            height: style.line_height,
            bounds: Rect::ZERO,
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}

fn caption_style(theme: &DefaultTheme) -> TextStyle {
    TextStyle {
        font_size: theme.text.xs.size.max(1.0),
        line_height: theme.text.xs.line_height.max(1.0),
        color: theme.palette.placeholder,
        ..theme.body_text_style()
    }
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    let mut builder = PathBuilder::new();
    builder.push_rounded_rect(rect, radius);
    builder.build()
}

fn row_highlight_rect(row: Rect, viewport: Rect) -> Option<Rect> {
    row.intersection(viewport)
        .map(|visible| inset_rect(visible, Insets::all(1.0)))
        .filter(|rect| !rect.is_empty())
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

fn set_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
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

fn advance_scalar(animation: &mut AnimatedScalar, time: f64) -> (bool, bool) {
    let previous = animation.value;
    let active = animation.advance(time);
    (animation.changed_since(previous), active)
}

#[derive(Debug, Clone, PartialEq)]
struct IndexedInteractionMotion<T>
where
    T: Clone + PartialEq,
{
    visual: Option<T>,
    animation: AnimatedScalar,
}

impl<T> IndexedInteractionMotion<T>
where
    T: Clone + PartialEq,
{
    const fn new() -> Self {
        Self {
            visual: None,
            animation: AnimatedScalar::new(0.0),
        }
    }

    fn set_hover_target(
        &mut self,
        target: Option<T>,
        theme: &DefaultTheme,
        ctx: &mut EventCtx,
    ) -> bool {
        self.set_target(target, theme, ctx, |animation, target, theme, ctx| {
            set_hover_animation_target(animation, target, theme, ctx)
        })
    }

    fn set_press_target(
        &mut self,
        target: Option<T>,
        theme: &DefaultTheme,
        ctx: &mut EventCtx,
    ) -> bool {
        self.set_target(target, theme, ctx, |animation, target, theme, ctx| {
            set_press_animation_target(animation, target, theme, ctx)
        })
    }

    fn set_target(
        &mut self,
        target: Option<T>,
        theme: &DefaultTheme,
        ctx: &mut EventCtx,
        set_animation: impl FnOnce(&mut AnimatedScalar, f32, &DefaultTheme, &mut EventCtx) -> bool,
    ) -> bool {
        match target {
            Some(target) => {
                if self.visual.as_ref() != Some(&target) {
                    self.visual = Some(target);
                    self.animation = AnimatedScalar::new(0.0);
                }
                set_animation(&mut self.animation, 1.0, theme, ctx)
            }
            None => {
                let changed = set_animation(&mut self.animation, 0.0, theme, ctx);
                if !changed && !self.animation.is_presented() {
                    self.visual = None;
                }
                changed
            }
        }
    }

    fn amount_for(&self, key: &T) -> f32 {
        self.amount_for_by(|visual| visual == key)
    }

    fn amount_for_by(&self, matches: impl FnOnce(&T) -> bool) -> f32 {
        self.visual
            .as_ref()
            .filter(|visual| matches(visual))
            .map(|_| self.animation.value)
            .unwrap_or(0.0)
    }

    fn advance(&mut self, time: f64) -> (bool, bool) {
        let previous = self.animation.value;
        let active = self.animation.advance(time);
        let changed = self.animation.changed_since(previous);
        if !self.animation.is_presented() {
            self.visual = None;
        }
        (changed, active)
    }
}

fn data_row_state_fill(
    theme: &DefaultTheme,
    selected: bool,
    hover_amount: f32,
    press_amount: f32,
) -> Color {
    let palette = theme.palette;
    let interaction = theme.interaction;
    if selected {
        mix_color(
            palette.selection,
            palette.accent,
            interaction.selected_blend * 0.18,
        )
    } else {
        let overlay = theme.surfaces.hover;
        let hover_alpha = overlay.alpha * interaction.hover_blend * hover_amount.clamp(0.0, 1.0);
        let press_alpha =
            overlay.alpha * 1.75 * interaction.pressed_blend * press_amount.clamp(0.0, 1.0);
        let alpha = hover_alpha.max(press_alpha).min(0.18);
        if alpha > AnimatedScalar::EPSILON {
            overlay.with_alpha(alpha)
        } else {
            Color::TRANSPARENT
        }
    }
}

fn list_view_row_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 5_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(359)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn layer_list_row_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 6_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(383)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn layer_list_visibility_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 7_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(389)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn layer_list_lock_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 4_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn paint_layer_visibility_button(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    visible: bool,
    hover_amount: f32,
    press_amount: f32,
) {
    let palette = theme.palette;
    if hover_amount > AnimatedScalar::EPSILON || press_amount > AnimatedScalar::EPSILON {
        ctx.fill(
            rounded_rect_path(rect, theme.metrics.corner_radius.min(rect.height() * 0.35)),
            data_row_state_fill(theme, false, hover_amount, press_amount),
        );
    }

    let metrics = theme.metrics;
    let icon = inset_rect(rect, Insets::all(metrics.layer_action_icon_inset));
    let color = if visible {
        palette.accent
    } else {
        palette.placeholder
    };
    ctx.stroke(
        layer_visibility_eye_path(icon),
        color,
        sui_scene::StrokeStyle::new(metrics.layer_visibility_stroke_width),
    );
    if visible {
        ctx.fill(
            Path::circle(
                Point::new(
                    icon.x() + icon.width() * 0.5,
                    icon.y() + icon.height() * 0.5,
                ),
                icon.width().min(icon.height()) * 0.17,
            ),
            color,
        );
    } else {
        ctx.stroke(
            line_path(
                Point::new(
                    icon.x() + icon.width() * 0.1,
                    icon.max_y() - icon.height() * 0.05,
                ),
                Point::new(
                    icon.max_x() - icon.width() * 0.1,
                    icon.y() + icon.height() * 0.05,
                ),
            ),
            color,
            sui_scene::StrokeStyle::new(metrics.layer_visibility_slash_stroke_width),
        );
    }
}

fn paint_layer_lock_button(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    locked: bool,
    hover_amount: f32,
    press_amount: f32,
) {
    let palette = theme.palette;
    if hover_amount > AnimatedScalar::EPSILON || press_amount > AnimatedScalar::EPSILON {
        ctx.fill(
            rounded_rect_path(rect, theme.metrics.corner_radius.min(rect.height() * 0.35)),
            data_row_state_fill(theme, false, hover_amount, press_amount),
        );
    }

    let metrics = theme.metrics;
    draw_icon_glyph(
        ctx,
        if locked {
            IconGlyph::Lock
        } else {
            IconGlyph::Unlock
        },
        inset_rect(rect, Insets::all(metrics.layer_lock_icon_inset)),
        if locked {
            palette.accent
        } else {
            palette.placeholder
        },
    );
}

fn paint_layer_thumbnail(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    color: Color,
    visible: bool,
) {
    let palette = theme.palette;
    let metrics = theme.metrics;
    let radius = metrics.layer_thumbnail_radius;
    ctx.fill(rounded_rect_path(rect, radius), palette.control_hover);
    let fill = inset_rect(rect, Insets::all(metrics.layer_thumbnail_inset));
    ctx.fill(
        rounded_rect_path(
            fill,
            (radius - metrics.layer_thumbnail_inset * 0.5).max(0.0),
        ),
        if visible {
            color
        } else {
            color.with_alpha(color.alpha * metrics.layer_thumbnail_disabled_opacity)
        },
    );
    ctx.stroke(
        rounded_rect_path(rect, radius),
        palette.border.with_alpha(if visible {
            1.0
        } else {
            metrics.layer_thumbnail_disabled_border_opacity
        }),
        sui_scene::StrokeStyle::new(metrics.border_width.max(1.0)),
    );
}

fn layer_visibility_eye_path(rect: Rect) -> Path {
    let mut builder = PathBuilder::new();
    let cx = rect.x() + rect.width() * 0.5;
    let cy = rect.y() + rect.height() * 0.5;
    builder
        .move_to(Point::new(rect.x(), cy))
        .cubic_to(
            Point::new(rect.x() + rect.width() * 0.22, rect.y()),
            Point::new(rect.x() + rect.width() * 0.78, rect.y()),
            Point::new(rect.max_x(), cy),
        )
        .cubic_to(
            Point::new(rect.x() + rect.width() * 0.78, rect.max_y()),
            Point::new(rect.x() + rect.width() * 0.22, rect.max_y()),
            Point::new(rect.x(), cy),
        )
        .close();
    let _ = (cx, cy);
    builder.build()
}

fn line_path(from: Point, to: Point) -> Path {
    let mut builder = PathBuilder::new();
    builder.move_to(from).line_to(to);
    builder.build()
}

fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

fn scroll_delta_to_offset(delta: sui_core::ScrollDelta) -> Vector {
    match delta {
        sui_core::ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        sui_core::ScrollDelta::Pixels(delta) => delta,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Breadcrumb, BreadcrumbItem, DefaultTheme, LayerList, LayerListItem, LayerListReorderChange,
        LeadingLabelCellPaint, ListItem, ListView, Table, TableColumn, TableColumnAlignment,
        TableRow, TextBlockPaint, TextCellPaint, TreeItem, TreeView, VirtualTable,
        VirtualTableColumn,
    };
    use crate::{Button, Label, ScrollView, SizedBox, Stack, ThemeTextToken};
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons,
        PointerEvent, PointerEventKind, PointerKind, Rect, Result, ScrollDelta, SemanticsAction,
        SemanticsRole, SemanticsValue, Size, ToggleState, Vector, WidgetId, WindowEvent,
    };
    use sui_layout::{Constraints, Padding as Insets};
    use sui_runtime::{Application, PaintCtx, RenderOutput, Runtime, Widget, WindowBuilder};
    use sui_scene::{Brush, SceneCommand};
    use sui_text::{FontFeature, FontRegistry, FontWeight, TextSystem};

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Data widgets").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn render<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        runtime.render(window_id).expect("render should succeed")
    }

    #[test]
    fn density_modes_resize_data_widgets() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(
                ListView::new("Layers")
                    .theme(compact)
                    .items([ListItem::new("Paint"), ListItem::new("Ink")])
            )
            .frame
            .viewport
            .height
                < render(
                    ListView::new("Layers")
                        .theme(touch)
                        .items([ListItem::new("Paint"), ListItem::new("Ink")])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                LayerList::new("Layers")
                    .theme(compact)
                    .layers([LayerListItem::new("Paint"), LayerListItem::new("Ink")])
            )
            .frame
            .viewport
            .height
                < render(
                    LayerList::new("Layers")
                        .theme(touch)
                        .layers([LayerListItem::new("Paint"), LayerListItem::new("Ink")])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                TreeView::new("Scene")
                    .theme(compact)
                    .items([TreeItem::new("Canvas"), TreeItem::new("Lighting")])
            )
            .frame
            .viewport
            .height
                < render(
                    TreeView::new("Scene")
                        .theme(touch)
                        .items([TreeItem::new("Canvas"), TreeItem::new("Lighting")])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                Table::new("Objects")
                    .theme(compact)
                    .columns([TableColumn::new("Name")])
                    .rows([TableRow::new(["Canvas"]), TableRow::new(["Lighting"])])
            )
            .frame
            .viewport
            .height
                < render(
                    Table::new("Objects")
                        .theme(touch)
                        .columns([TableColumn::new("Name")])
                        .rows([TableRow::new(["Canvas"]), TableRow::new(["Lighting"])])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                Breadcrumb::new("Path")
                    .theme(compact)
                    .items([BreadcrumbItem::new("Scene"), BreadcrumbItem::new("Layers")])
            )
            .frame
            .viewport
            .height
                < render(
                    Breadcrumb::new("Path")
                        .theme(touch)
                        .items([BreadcrumbItem::new("Scene"), BreadcrumbItem::new("Layers")])
                )
                .frame
                .viewport
                .height
        );
    }

    fn text_rects_for(output: &RenderOutput, text: &str) -> Vec<Rect> {
        let mut rects = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawText(run) if run.text == text => rects.push(run.rect),
                SceneCommand::DrawShapedText(run) => {
                    if let Some(layout) = run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .filter(|layout| layout.text() == text)
                    {
                        rects.push(shaped_text_run_rect(run.origin, layout));
                    }
                }
                _ => {}
            });

        rects
    }

    fn shaped_text_run_rect(origin: Point, layout: &sui_text::TextLayout) -> Rect {
        let measurement = layout.measurement();
        let bounds = measurement.bounds;
        let width = if bounds.width().is_finite() && bounds.width() > 0.0 {
            bounds.width()
        } else {
            measurement.width
        };
        Rect::new(
            origin.x + bounds.x(),
            origin.y + ((layout.box_size().height - measurement.height).max(0.0) * 0.5),
            width,
            layout.style().line_height.max(measurement.height),
        )
    }

    fn text_runs_for(output: &RenderOutput, text: &str) -> Vec<sui_text::TextRun> {
        let mut runs = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawText(run) if run.text == text => runs.push(run.clone()),
                SceneCommand::DrawShapedText(run) => {
                    if let Some(layout) = run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .filter(|layout| layout.text() == text)
                    {
                        let mut style = layout.style().clone();
                        if let Some(color) = run.color_override {
                            style.color = color;
                        }
                        runs.push(sui_text::TextRun {
                            rect: shaped_text_run_rect(run.origin, layout),
                            text: layout.text().to_string(),
                            style,
                        });
                    }
                }
                _ => {}
            });

        runs
    }

    fn draw_clip_rects_for(output: &RenderOutput, text: &str) -> Vec<Rect> {
        let mut clips = Vec::new();
        let mut stack = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::PushClip { rect } => stack.push(*rect),
                SceneCommand::PopClip => {
                    stack.pop();
                }
                SceneCommand::DrawText(run) if run.text == text => {
                    if let Some(rect) = stack.last() {
                        clips.push(*rect);
                    }
                }
                SceneCommand::DrawShapedText(run) => {
                    if let Some(layout) = run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .filter(|layout| layout.text() == text)
                    {
                        if !layout.text().is_empty() {
                            if let Some(rect) = stack.last() {
                                clips.push(*rect);
                            }
                        }
                    }
                }
                _ => {}
            });

        clips
    }

    fn selected_highlight_rects(output: &RenderOutput) -> Vec<Rect> {
        let theme = DefaultTheme::default();
        let selected_brush = Brush::Solid(super::data_row_state_fill(&theme, true, 0.0, 0.0));
        let mut rects = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillPath { path, brush } if *brush == selected_brush => {
                    rects.push(path.bounds());
                }
                SceneCommand::FillRect { rect, brush } if *brush == selected_brush => {
                    rects.push(*rect);
                }
                _ => {}
            });
        rects
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

    fn solid_stroke_widths(output: &RenderOutput) -> Vec<f32> {
        let mut widths = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect { stroke, .. }
                | SceneCommand::StrokePath { stroke, .. } => {
                    widths.push(stroke.width);
                }
                _ => {}
            });
        widths
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

    fn solid_stroke_rects(output: &RenderOutput) -> Vec<Rect> {
        let mut rects = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect { rect, .. } => rects.push(*rect),
                _ => {}
            });
        rects
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
    }

    fn rect_center(rect: Rect) -> Point {
        Point::new(
            rect.x() + rect.width() * 0.5,
            rect.y() + rect.height() * 0.5,
        )
    }

    fn text_run_visual_center(run: &sui_text::TextRun) -> f32 {
        let layout = TextSystem::new()
            .shape_text(
                run.text.clone(),
                Size::new(f32::INFINITY, run.rect.height().max(1.0)),
                run.style.clone(),
                &FontRegistry::new(),
            )
            .expect("text run should shape");
        let line = layout.lines().first().expect("text run should have a line");
        run.rect.y() + line.baseline + optical_visual_center(layout.measurement())
    }

    fn text_visual_center_for(output: &RenderOutput, text: &str) -> f32 {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::DrawText(run) if run.text == text => {
                    Some(text_run_visual_center(run))
                }
                SceneCommand::DrawShapedText(run) => {
                    let layout = run.resolve(output.frame.text_layout_registry.as_ref())?;
                    if layout.text() != text {
                        return None;
                    }
                    let line = layout.lines().first().expect("text run should have a line");
                    Some(run.origin.y + line.baseline + optical_visual_center(layout.measurement()))
                }
                _ => None,
            })
            .expect("text draw command present")
    }

    fn assert_two_line_row_text_matches_slots(
        output: &RenderOutput,
        label: &str,
        detail: &str,
        row: Rect,
    ) {
        let label = text_runs_for(output, label)
            .into_iter()
            .next()
            .expect("row label draw command present");
        let detail = text_runs_for(output, detail)
            .into_iter()
            .next()
            .expect("row detail draw command present");
        let total_height =
            label.style.line_height + detail.style.line_height + super::TWO_LINE_ROW_TEXT_GAP;
        let top = row.y() + ((row.height() - total_height) * 0.5).max(0.0);
        let label_slot_center = top + (label.style.line_height * 0.5);
        let detail_slot_center = top
            + label.style.line_height
            + super::TWO_LINE_ROW_TEXT_GAP
            + (detail.style.line_height * 0.5);

        assert!((text_run_visual_center(&label) - label_slot_center).abs() < 0.75);
        assert!((text_run_visual_center(&detail) - detail_slot_center).abs() < 0.75);
    }

    fn assert_text_run_uses_token(run: &sui_text::TextRun, token: ThemeTextToken) {
        assert!(
            (run.style.font_size - token.size).abs() < 0.001,
            "text '{}' used font size {}, expected token size {}",
            run.text,
            run.style.font_size,
            token.size
        );
        assert!(
            (run.style.line_height - token.line_height).abs() < 0.001,
            "text '{}' used line height {}, expected token line height {}",
            run.text,
            run.style.line_height,
            token.line_height
        );
    }

    fn vertical_scroll_thumb_rects(output: &RenderOutput) -> Vec<Rect> {
        let mut rects = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillPath { path, .. } = command {
                let bounds = path.bounds();
                if (bounds.width() - 4.0).abs() <= f32::EPSILON && bounds.height() >= 28.0 {
                    rects.push(bounds);
                }
            }
        });
        rects
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

    fn wheel_scroll(position: Point, delta: Vector) -> Event {
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, position);
        scroll.scroll_delta = Some(ScrollDelta::Pixels(delta));
        Event::Pointer(scroll)
    }

    fn handle_ready_events(runtime: &mut Runtime) -> Result<usize> {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window, event) in ready {
            runtime.handle_event(ready_window, event)?;
        }
        Ok(count)
    }

    fn assert_focus_surface_keeps_chrome_neutral<W>(root: W, position: Point) -> Result<()>
    where
        W: Widget + 'static,
    {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;

        let focused = runtime.render(window_id)?;
        let focused_strokes = solid_stroke_colors(&focused);
        assert!(
            !focused_strokes.contains(&theme.palette.focus_ring),
            "focused data containers should not paint a focus ring; strokes={focused_strokes:?}"
        );
        assert_eq!(
            focused_strokes.first().copied(),
            Some(theme.palette.border),
            "focused data containers should keep their surface border neutral; strokes={focused_strokes:?}"
        );

        Ok(())
    }

    fn assert_pointer_hover_and_press_use_theme_motion(
        runtime: &mut Runtime,
        window_id: sui_core::WindowId,
        position: Point,
        theme: &DefaultTheme,
    ) -> Result<()> {
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::data_row_state_fill(theme, false, 1.0, 0.0);
        let expected_press = super::data_row_state_fill(theme, false, 0.0, 1.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, position, false),
        )?;
        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(runtime)?, 1);
        let settled_hover = runtime.render(window_id)?;
        assert!(
            solid_fill_colors(&settled_hover).contains(&expected_hover),
            "hover fill should settle to the theme interaction color"
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(runtime)?, 1);
        let settled_press = runtime.render(window_id)?;
        assert!(
            solid_fill_colors(&settled_press).contains(&expected_press),
            "press fill should settle to the theme interaction color"
        );

        Ok(())
    }

    #[test]
    fn data_row_hover_and_press_fills_are_translucent_overlays() {
        let theme = DefaultTheme::default();
        let hover = super::data_row_state_fill(&theme, false, 1.0, 0.0);
        let press = super::data_row_state_fill(&theme, false, 0.0, 1.0);

        assert_eq!(
            super::data_row_state_fill(&theme, false, 0.0, 0.0),
            Color::TRANSPARENT
        );
        assert!(hover.alpha > 0.0 && hover.alpha < 1.0);
        assert!(press.alpha > hover.alpha && press.alpha < 1.0);
        assert_ne!(hover, theme.palette.control);
        assert_ne!(hover, theme.palette.control_hover);

        let dark_hover = super::data_row_state_fill(&DefaultTheme::dark(), false, 1.0, 0.0);
        assert!(dark_hover.alpha > 0.0 && dark_hover.alpha < 1.0);
    }

    #[test]
    fn data_focus_surfaces_keep_chrome_neutral() -> Result<()> {
        assert_focus_surface_keeps_chrome_neutral(
            SizedBox::new().width(260.0).height(120.0).with_child(
                ListView::new("Assets").items([ListItem::new("First"), ListItem::new("Second")]),
            ),
            Point::new(24.0, 24.0),
        )?;

        assert_focus_surface_keeps_chrome_neutral(
            SizedBox::new().width(280.0).height(120.0).with_child(
                LayerList::new("Layers")
                    .layers([LayerListItem::new("Paint"), LayerListItem::new("Ink")]),
            ),
            Point::new(24.0, 24.0),
        )?;

        assert_focus_surface_keeps_chrome_neutral(
            SizedBox::new().width(260.0).height(120.0).with_child(
                TreeView::new("Scene").items([TreeItem::new("Canvas"), TreeItem::new("Lighting")]),
            ),
            Point::new(24.0, 24.0),
        )?;

        assert_focus_surface_keeps_chrome_neutral(
            SizedBox::new().width(280.0).height(140.0).with_child(
                Table::new("Objects")
                    .columns([TableColumn::new("Name")])
                    .rows([TableRow::new(["Canvas"]), TableRow::new(["Lighting"])]),
            ),
            Point::new(24.0, 58.0),
        )?;

        assert_focus_surface_keeps_chrome_neutral(
            Breadcrumb::new("Path").items([
                BreadcrumbItem::new("Scene"),
                BreadcrumbItem::new("Layers"),
                BreadcrumbItem::new("Ink"),
            ]),
            Point::new(24.0, 18.0),
        )
    }

    #[test]
    fn list_view_row_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(SizedBox::new().width(260.0).height(140.0).with_child(
                ListView::new("Assets").theme(theme).items([
                    ListItem::new("First"),
                    ListItem::new("Second"),
                    ListItem::new("Third"),
                ]),
            ));

        let output = runtime.render(window_id)?;
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Second")
            })
            .expect("second row semantics present");

        assert_pointer_hover_and_press_use_theme_motion(
            &mut runtime,
            window_id,
            rect_center(row.bounds),
            &theme,
        )
    }

    #[test]
    fn layer_list_action_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(280.0).height(120.0).with_child(
                LayerList::new("Layers")
                    .theme(theme)
                    .layers([LayerListItem::new("Paint"), LayerListItem::new("Ink")]),
            ),
        );

        let output = runtime.render(window_id)?;
        let visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paint layer")
            })
            .expect("visibility button semantics present");

        assert_pointer_hover_and_press_use_theme_motion(
            &mut runtime,
            window_id,
            rect_center(visibility.bounds),
            &theme,
        )
    }

    #[test]
    fn layer_list_drag_reorders_rows_without_activating_buttons() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(280.0).height(168.0).with_child(
                LayerList::new("Layers")
                    .layers([
                        LayerListItem::new("Paint"),
                        LayerListItem::new("Paper"),
                        LayerListItem::new("Ink"),
                    ])
                    .on_reorder(move |change| captured.borrow_mut().push(change)),
            ),
        );
        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer semantics present");
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer semantics present");

        let start = rect_center(paint.bounds);
        let end = Point::new(start.x, paper.bounds.max_y() + 4.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        assert_eq!(
            changes.borrow().as_slice(),
            &[LayerListReorderChange {
                item: 0,
                from: 0,
                to: 1
            }]
        );

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer semantics present after reorder");
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer semantics present after reorder");
        assert!(paper.bounds.y() < paint.bounds.y());
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paint layer")
                    && node.value == Some(SemanticsValue::Text("Visible".to_string()))
            }),
            "visibility button should remain a normal button after enabling row reorder"
        );
        Ok(())
    }

    #[test]
    fn table_row_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let position = Point::new(
            theme.metrics.data_viewport_padding.left + 24.0,
            theme.metrics.data_viewport_padding.top
                + theme.metrics.table_header_height
                + theme.metrics.select_menu_gap
                + theme.metrics.table_row_height * 0.5,
        );
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(360.0).height(180.0).with_child(
                Table::new("Objects")
                    .theme(theme)
                    .columns([TableColumn::new("Name"), TableColumn::new("State")])
                    .rows([
                        TableRow::new(["Canvas", "Visible"]),
                        TableRow::new(["Lighting", "Locked"]),
                    ]),
            ),
        );

        let _ = runtime.render(window_id)?;
        assert_pointer_hover_and_press_use_theme_motion(&mut runtime, window_id, position, &theme)
    }

    #[test]
    fn table_row_hover_keeps_row_base_under_overlay() -> Result<()> {
        let theme = DefaultTheme::default();
        let position = Point::new(
            theme.metrics.data_viewport_padding.left + 24.0,
            theme.metrics.data_viewport_padding.top
                + theme.metrics.table_header_height
                + theme.metrics.select_menu_gap
                + theme.metrics.table_row_height * 0.5,
        );
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(360.0).height(180.0).with_child(
                Table::new("Objects")
                    .theme(theme)
                    .columns([TableColumn::new("Name"), TableColumn::new("State")])
                    .rows([
                        TableRow::new(["Canvas", "Visible"]),
                        TableRow::new(["Lighting", "Locked"]),
                    ]),
            ),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, position, false),
        )?;
        runtime.tick(theme.motion.hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let hovered = runtime.render(window_id)?;
        let fills = solid_fill_colors(&hovered);

        assert!(fills.contains(&theme.palette.surface.with_alpha(0.88)));
        assert!(fills.contains(&super::data_row_state_fill(&theme, false, 1.0, 0.0)));
        assert!(
            !fills.contains(&theme.palette.control_hover),
            "hovered table rows should paint a translucent overlay, not an opaque control fill"
        );
        Ok(())
    }

    #[test]
    fn breadcrumb_item_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(Breadcrumb::new("Path").theme(theme).items([
            BreadcrumbItem::new("Workspace"),
            BreadcrumbItem::new("Project"),
            BreadcrumbItem::new("Scene"),
        ]));

        let output = runtime.render(window_id)?;
        let project_label = text_rects_for(&output, "Project")
            .into_iter()
            .next()
            .expect("project label should render");

        assert_pointer_hover_and_press_use_theme_motion(
            &mut runtime,
            window_id,
            rect_center(project_label),
            &theme,
        )
    }

    #[test]
    fn breadcrumb_overflow_clips_and_defaults_to_trailing_items() {
        let output = render(SizedBox::new().width(180.0).with_child(
            Breadcrumb::new("Path").items([
                BreadcrumbItem::new("SIFS"),
                BreadcrumbItem::new("sinomo"),
                BreadcrumbItem::new("nodes"),
                BreadcrumbItem::new("node 01kvbbd...3fsrx"),
                BreadcrumbItem::new("attachments"),
            ]),
        ));

        assert!(
            output.frame.scene.commands().iter().any(|command| matches!(
                command,
                SceneCommand::PushClip { rect }
                    if (rect.width() - 180.0).abs() < 0.01
                        && (rect.height() - output.frame.viewport.height).abs() < 0.01
            )),
            "overflowing breadcrumb content should be clipped to the widget bounds"
        );

        let trailing_label = text_rects_for(&output, "attachments")
            .into_iter()
            .next()
            .expect("trailing breadcrumb item should render");
        assert!(
            trailing_label.x() >= -0.01 && trailing_label.max_x() <= 180.01,
            "default overflow position should keep the last item visible: {trailing_label:?}"
        );
    }

    #[test]
    fn breadcrumb_wheel_scrolls_overflowing_content_leftward() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(SizedBox::new().width(180.0).with_child(
            Breadcrumb::new("Path").items([
                BreadcrumbItem::new("SIFS"),
                BreadcrumbItem::new("sinomo"),
                BreadcrumbItem::new("nodes"),
                BreadcrumbItem::new("node 01kvbbd...3fsrx"),
                BreadcrumbItem::new("attachments"),
            ]),
        ));

        let before = runtime.render(window_id)?;
        let before_x = text_rects_for(&before, "attachments")[0].x();

        runtime.handle_event(
            window_id,
            wheel_scroll(Point::new(90.0, 18.0), Vector::new(0.0, 24.0)),
        )?;

        let after = runtime.render(window_id)?;
        let after_x = text_rects_for(&after, "attachments")[0].x();
        assert!(
            after_x > before_x,
            "wheel scrolling upward should reveal earlier breadcrumb items by moving content right"
        );
        Ok(())
    }

    #[test]
    fn breadcrumb_drag_scrolls_overflowing_content_leftward() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(SizedBox::new().width(180.0).with_child(
            Breadcrumb::new("Path").items([
                BreadcrumbItem::new("SIFS"),
                BreadcrumbItem::new("sinomo"),
                BreadcrumbItem::new("nodes"),
                BreadcrumbItem::new("node 01kvbbd...3fsrx"),
                BreadcrumbItem::new("attachments"),
            ]),
        ));

        let before = runtime.render(window_id)?;
        let before_x = text_rects_for(&before, "attachments")[0].x();

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(90.0, 18.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(126.0, 18.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(126.0, 18.0), false),
        )?;

        let after = runtime.render(window_id)?;
        let after_x = text_rects_for(&after, "attachments")[0].x();
        assert!(
            after_x > before_x,
            "dragging right should reveal earlier breadcrumb items by moving content right"
        );
        Ok(())
    }

    #[test]
    fn list_view_click_selects_row_and_updates_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ListView::new("Assets")
                .items([
                    ListItem::new("First"),
                    ListItem::new("Second"),
                    ListItem::new("Third"),
                ])
                .on_change(move |index, label| on_change.borrow_mut().push((index, label))),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(44.0, 44.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(44.0, 44.0), false),
        )?;

        assert_eq!(changes.borrow().as_slice(), &[(1, "Second".to_string())]);

        let output = runtime.render(window_id)?;
        let list = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("list semantics present");
        assert_eq!(list.value, Some(SemanticsValue::Text("Second".to_string())));
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Second")
            })
            .expect("selected row semantics present");
        assert_eq!(row.parent, Some(list.id));
        assert!(row.state.selected);
        assert!(row.actions.contains(&SemanticsAction::Activate));
        Ok(())
    }

    #[test]
    fn list_view_change_callback_can_request_relayout() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let requests = Rc::new(RefCell::new(0_usize));
        let on_change = Rc::clone(&changes);
        let requested = Rc::clone(&requests);
        let (mut runtime, window_id) = build_runtime(
            ListView::new("Assets")
                .items([ListItem::new("First"), ListItem::new("Second")])
                .on_change_with_ctx(move |index, label, ctx| {
                    on_change.borrow_mut().push((index, label));
                    *requested.borrow_mut() += 1;
                    ctx.request_measure();
                    ctx.request_arrange();
                    ctx.request_paint();
                    ctx.request_semantics();
                }),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(44.0, 44.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(44.0, 44.0), false),
        )?;

        assert_eq!(changes.borrow().as_slice(), &[(1, "Second".to_string())]);
        assert_eq!(*requests.borrow(), 1);
        Ok(())
    }

    #[test]
    fn list_view_exposes_visible_row_semantics() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(260.0).height(132.0).with_child(
                ListView::new("Layers")
                    .items([
                        ListItem::new("Paint").detail("Normal / 100%"),
                        ListItem::new("Paper").detail("Background"),
                        ListItem::new("Locked").detail("Read only").disabled(),
                    ])
                    .selected(0),
            ),
        );

        let output = runtime.render(window_id)?;
        let list = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("list semantics present");
        let rows = output
            .semantics
            .iter()
            .filter(|node| node.role == SemanticsRole::ListItem)
            .collect::<Vec<_>>();

        assert_eq!(rows.len(), 3);
        let paint = rows
            .iter()
            .find(|node| node.name.as_deref() == Some("Paint"))
            .expect("paint row semantics present");
        assert_eq!(paint.parent, Some(list.id));
        assert_eq!(paint.description.as_deref(), Some("Normal / 100%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text("Normal / 100%".to_string()))
        );
        assert!(paint.state.selected);
        assert!(!paint.state.disabled);
        assert!(paint.actions.contains(&SemanticsAction::Activate));

        let paper = rows
            .iter()
            .find(|node| node.name.as_deref() == Some("Paper"))
            .expect("paper row semantics present");
        assert_eq!(paper.parent, Some(list.id));
        assert_eq!(paper.description.as_deref(), Some("Background"));
        assert_eq!(
            paper.value,
            Some(SemanticsValue::Text("Background".to_string()))
        );
        assert!(!paper.state.selected);

        let locked = rows
            .iter()
            .find(|node| node.name.as_deref() == Some("Locked"))
            .expect("locked row semantics present");
        assert!(locked.state.disabled);
        assert!(locked.actions.is_empty());
        Ok(())
    }

    #[test]
    fn list_view_selected_when_reads_external_selection() -> Result<()> {
        let selected = Rc::new(RefCell::new(0_usize));
        let selected_reader = Rc::clone(&selected);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(260.0).height(132.0).with_child(
                ListView::new("Layers")
                    .items([
                        ListItem::new("Paint").detail("Normal / 100%"),
                        ListItem::new("Paper").detail("Background"),
                    ])
                    .selected_when(move || Some(*selected_reader.borrow())),
            ),
        );

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint row semantics present");
        assert!(paint.state.selected);

        *selected.borrow_mut() = 1;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(260.0, 132.0))),
        )?;
        let output = runtime.render(window_id)?;
        let list = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("list semantics present");
        assert_eq!(list.value, Some(SemanticsValue::Text("Paper".to_string())));
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper row semantics present");
        assert!(paper.state.selected);
        Ok(())
    }

    #[test]
    fn list_view_row_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(402);
        let mut ids = (0..8)
            .map(|index| super::list_view_row_id(parent, index).get())
            .collect::<Vec<_>>();

        assert!(ids.iter().all(|id| *id <= ((1_u64 << 53) - 1)));
        assert!(ids.iter().all(|id| *id != parent.get()));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn layer_list_exposes_visibility_semantics() {
        let output = render(
            SizedBox::new().width(280.0).height(112.0).with_child(
                LayerList::new("Layers")
                    .layers([
                        LayerListItem::new("Paint")
                            .detail("Normal / 100%")
                            .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                        LayerListItem::new("Paper")
                            .detail("Background")
                            .thumbnail(Color::rgba(0.89, 0.91, 0.94, 1.0)),
                    ])
                    .selected(0),
            ),
        );

        let list = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("layer list semantics present");
        assert_eq!(list.name.as_deref(), Some("Layers"));
        assert_eq!(list.value, Some(SemanticsValue::Text("Paint".to_string())));

        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row semantics present");
        assert_eq!(paint.parent, Some(list.id));
        assert_eq!(paint.description.as_deref(), Some("Normal / 100%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Normal / 100%; Visible; Unlocked".to_string()
            ))
        );
        assert!(paint.state.selected);

        let visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paint layer")
            })
            .expect("paint layer visibility control semantics present");
        assert_eq!(visibility.parent, Some(paint.id));
        assert_eq!(
            visibility.value,
            Some(SemanticsValue::Text("Visible".to_string()))
        );
        assert_eq!(visibility.state.checked, Some(ToggleState::Checked));
        assert!(visibility.actions.contains(&SemanticsAction::Activate));

        let lock = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Lock Paint layer")
            })
            .expect("paint layer lock control semantics present");
        assert_eq!(lock.parent, Some(paint.id));
        assert_eq!(
            lock.value,
            Some(SemanticsValue::Text("Unlocked".to_string()))
        );
        assert_eq!(lock.state.checked, Some(ToggleState::Unchecked));
        assert!(lock.actions.contains(&SemanticsAction::Activate));
    }

    #[test]
    fn layer_list_chrome_uses_theme_metrics() {
        let mut theme = DefaultTheme::default();
        theme.metrics.layer_visibility_stroke_width = 2.75;
        theme.metrics.layer_visibility_slash_stroke_width = 3.25;
        theme.metrics.layer_thumbnail_disabled_opacity = 0.21;
        theme.metrics.layer_thumbnail_disabled_border_opacity = 0.33;
        let thumbnail = Color::rgba(0.30, 0.50, 0.70, 1.0);

        let output = render(
            SizedBox::new().width(280.0).height(64.0).with_child(
                LayerList::new("Layers")
                    .theme(theme)
                    .layers([LayerListItem::new("Paper")
                        .thumbnail(thumbnail)
                        .visible(false)]),
            ),
        );
        let fills = solid_fill_colors(&output);
        let stroke_widths = solid_stroke_widths(&output);
        let stroke_colors = solid_stroke_colors(&output);

        assert!(fills.contains(&thumbnail.with_alpha(0.21)));
        assert!(fills.contains(&theme.palette.control_hover));
        assert!(stroke_colors.contains(&theme.palette.border.with_alpha(0.33)));
        assert!(
            stroke_widths
                .iter()
                .any(|width| (*width - 2.75).abs() < f32::EPSILON)
        );
        assert!(
            stroke_widths
                .iter()
                .any(|width| (*width - 3.25).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn layer_list_label_and_detail_visual_centers_match_row_slots() {
        let output = render(
            SizedBox::new().width(280.0).height(64.0).with_child(
                LayerList::new("Layers").layer(
                    LayerListItem::new("Paint")
                        .detail("Normal / 100%")
                        .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                ),
            ),
        );
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("layer row semantics present")
            .bounds;

        assert_two_line_row_text_matches_slots(&output, "Paint", "Normal / 100%", row);
    }

    #[test]
    fn layer_list_item_dynamic_detail_and_visibility_update_semantics() -> Result<()> {
        let detail = Rc::new(RefCell::new("Normal / 100%".to_string()));
        let detail_reader = Rc::clone(&detail);
        let visible = Rc::new(RefCell::new(true));
        let visible_reader = Rc::clone(&visible);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(280.0).height(112.0).with_child(
                LayerList::new("Layers")
                    .layers([
                        LayerListItem::new("Paint")
                            .detail_when(move || detail_reader.borrow().clone())
                            .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0))
                            .visible_when(move || *visible_reader.borrow()),
                        LayerListItem::new("Paper")
                            .detail("Background")
                            .thumbnail(Color::rgba(0.89, 0.91, 0.94, 1.0)),
                    ])
                    .selected(0),
            ),
        );

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row semantics present");
        assert_eq!(paint.description.as_deref(), Some("Normal / 100%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Normal / 100%; Visible; Unlocked".to_string()
            ))
        );

        *detail.borrow_mut() = "Normal / 50%".to_string();
        *visible.borrow_mut() = false;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(280.0, 112.0))),
        )?;
        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row semantics present");
        assert_eq!(paint.description.as_deref(), Some("Normal / 50%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Normal / 50%; Hidden; Unlocked".to_string()
            ))
        );

        let visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Show Paint layer")
            })
            .expect("paint layer visibility control should update its label");
        assert_eq!(
            visibility.value,
            Some(SemanticsValue::Text("Hidden".to_string()))
        );
        assert_eq!(visibility.state.checked, Some(ToggleState::Unchecked));
        Ok(())
    }

    #[test]
    fn layer_list_visibility_button_toggles_without_selecting_row() -> Result<()> {
        let selections = Rc::new(RefCell::new(Vec::new()));
        let on_select = Rc::clone(&selections);
        let visibility_changes = Rc::new(RefCell::new(Vec::new()));
        let on_visibility_change = Rc::clone(&visibility_changes);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(280.0).height(112.0).with_child(
                LayerList::new("Layers")
                    .layers([
                        LayerListItem::new("Paint")
                            .detail("Normal / 100%")
                            .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                        LayerListItem::new("Paper")
                            .detail("Background")
                            .thumbnail(Color::rgba(0.89, 0.91, 0.94, 1.0)),
                    ])
                    .selected(0)
                    .on_select(move |index, label| {
                        on_select.borrow_mut().push((index, label));
                    })
                    .on_visibility_change(move |index, visible| {
                        on_visibility_change.borrow_mut().push((index, visible));
                    }),
            ),
        );

        let output = runtime.render(window_id)?;
        let visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paper layer")
            })
            .expect("paper layer visibility control should exist");
        let position = Point::new(
            visibility.bounds.x() + (visibility.bounds.width() * 0.5),
            visibility.bounds.y() + (visibility.bounds.height() * 0.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert!(selections.borrow().is_empty());
        assert_eq!(visibility_changes.borrow().as_slice(), &[(1, false)]);

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert!(paint.state.selected);

        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should still exist");
        assert!(!paper.state.selected);
        assert_eq!(
            paper.value,
            Some(SemanticsValue::Text(
                "Background; Hidden; Unlocked".to_string()
            ))
        );

        let visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Show Paper layer")
            })
            .expect("paper layer visibility control should update its label");
        assert_eq!(
            visibility.value,
            Some(SemanticsValue::Text("Hidden".to_string()))
        );
        assert_eq!(visibility.state.checked, Some(ToggleState::Unchecked));
        Ok(())
    }

    #[test]
    fn layer_list_lock_button_toggles_without_selecting_row() -> Result<()> {
        let selections = Rc::new(RefCell::new(Vec::new()));
        let on_select = Rc::clone(&selections);
        let lock_changes = Rc::new(RefCell::new(Vec::new()));
        let on_lock_change = Rc::clone(&lock_changes);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(280.0).height(112.0).with_child(
                LayerList::new("Layers")
                    .layers([
                        LayerListItem::new("Paint")
                            .detail("Normal / 100%")
                            .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                        LayerListItem::new("Paper")
                            .detail("Background")
                            .thumbnail(Color::rgba(0.89, 0.91, 0.94, 1.0)),
                    ])
                    .selected(0)
                    .on_select(move |index, label| {
                        on_select.borrow_mut().push((index, label));
                    })
                    .on_lock_change(move |index, locked| {
                        on_lock_change.borrow_mut().push((index, locked));
                    }),
            ),
        );

        let output = runtime.render(window_id)?;
        let lock = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Lock Paper layer")
            })
            .expect("paper layer lock control should exist");
        let position = Point::new(
            lock.bounds.x() + (lock.bounds.width() * 0.5),
            lock.bounds.y() + (lock.bounds.height() * 0.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert!(selections.borrow().is_empty());
        assert_eq!(lock_changes.borrow().as_slice(), &[(1, true)]);

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert!(paint.state.selected);

        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should still exist");
        assert!(!paper.state.selected);
        assert_eq!(
            paper.value,
            Some(SemanticsValue::Text(
                "Background; Visible; Locked".to_string()
            ))
        );

        let lock = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Unlock Paper layer")
            })
            .expect("paper layer lock control should update its label");
        assert_eq!(lock.value, Some(SemanticsValue::Text("Locked".to_string())));
        assert_eq!(lock.state.checked, Some(ToggleState::Checked));
        Ok(())
    }

    #[test]
    fn layer_list_row_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(402);
        let mut ids = (0..8)
            .flat_map(|index| {
                [
                    super::layer_list_row_id(parent, index).get(),
                    super::layer_list_visibility_id(parent, index).get(),
                    super::layer_list_lock_id(parent, index).get(),
                ]
            })
            .collect::<Vec<_>>();

        assert!(ids.iter().all(|id| *id <= ((1_u64 << 53) - 1)));
        assert!(ids.iter().all(|id| *id != parent.get()));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 24);
    }

    #[test]
    fn list_item_child_widget_receives_pointer_events() -> Result<()> {
        let presses = Rc::new(RefCell::new(0));
        let on_press = Rc::clone(&presses);
        let row = Stack::horizontal()
            .spacing(8.0)
            .with_child(SizedBox::new().width(96.0).with_child(Label::new("Asset")))
            .with_child(Button::new("Open").on_press(move || *on_press.borrow_mut() += 1));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(260.0)
                .height(80.0)
                .with_child(ListView::new("Actions").item(ListItem::new("Asset").with_child(row))),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(136.0, 28.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(136.0, 28.0), false),
        )?;

        assert_eq!(*presses.borrow(), 1);
        let output = runtime.render(window_id)?;
        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Open"))
        );
        Ok(())
    }

    #[test]
    fn list_item_child_row_can_opt_into_row_activation() -> Result<()> {
        let row_changes = Rc::new(RefCell::new(Vec::new()));
        let button_presses = Rc::new(RefCell::new(0));
        let on_row = Rc::clone(&row_changes);
        let on_button = Rc::clone(&button_presses);
        let row = Stack::horizontal()
            .spacing(8.0)
            .with_child(SizedBox::new().width(96.0).with_child(Label::new("Asset")))
            .with_child(Button::new("Open").on_press(move || *on_button.borrow_mut() += 1));
        let item = ListItem::new("Asset")
            .semantic_name("Asset row")
            .description("Selectable asset row")
            .activate_with_child()
            .with_child(row);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(260.0).height(80.0).with_child(
                ListView::new("Actions")
                    .item(item)
                    .on_change(move |index, label| {
                        on_row.borrow_mut().push((index, label));
                    }),
            ),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(36.0, 28.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(36.0, 28.0), false),
        )?;

        assert_eq!(row_changes.borrow().as_slice(), &[(0, "Asset".to_string())]);
        assert_eq!(*button_presses.borrow(), 0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(136.0, 28.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(136.0, 28.0), false),
        )?;

        assert_eq!(
            row_changes.borrow().as_slice(),
            &[(0, "Asset".to_string())],
            "nested button clicks should not also activate the row"
        );
        assert_eq!(*button_presses.borrow(), 1);

        let output = runtime.render(window_id)?;
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Asset row")
            })
            .expect("custom child row should expose list-item semantics");
        assert_eq!(row.description.as_deref(), Some("Selectable asset row"));
        assert!(row.actions.contains(&SemanticsAction::Activate));
        Ok(())
    }

    #[test]
    fn list_item_child_respects_explicit_row_height() -> Result<()> {
        let row_changes = Rc::new(RefCell::new(Vec::new()));
        let on_row = Rc::clone(&row_changes);
        let item = ListItem::new("Tall asset")
            .activate_with_child()
            .with_child(
                SizedBox::new()
                    .height(180.0)
                    .with_child(Label::new("Tall custom row")),
            );
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(260.0).height(96.0).with_child(
                ListView::new("Assets")
                    .row_height(36.0)
                    .item(item)
                    .on_change(move |index, label| {
                        on_row.borrow_mut().push((index, label));
                    }),
            ),
        );

        let output = runtime.render(window_id)?;
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Tall asset")
            })
            .expect("fixed-height child row should expose list-item semantics");
        assert!(
            (row.bounds.height() - 36.0).abs() <= 0.5,
            "explicit row height should cap oversized child content, got {:?}",
            row.bounds
        );

        let position = Point::new(
            row.bounds.x() + row.bounds.width() * 0.5,
            row.bounds.y() + row.bounds.height() * 0.5,
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert_eq!(
            row_changes.borrow().as_slice(),
            &[(0, "Tall asset".to_string())]
        );
        Ok(())
    }

    #[test]
    fn tree_view_keyboard_expands_and_selects_child() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TreeView::new("Scene")
                .item(
                    TreeItem::new("Root")
                        .with_child(TreeItem::new("Child A"))
                        .with_child(TreeItem::new("Child B")),
                )
                .on_change(move |path, label| on_change.borrow_mut().push((path, label))),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(48.0, 26.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(48.0, 26.0), false),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
        )?;

        assert!(changes.borrow().iter().any(|(_, label)| label == "Child A"));

        let output = runtime.render(window_id)?;
        let tree = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Tree)
            .expect("tree semantics present");
        assert_eq!(
            tree.value,
            Some(SemanticsValue::Text("Child A".to_string()))
        );
        Ok(())
    }

    #[test]
    fn table_keyboard_selects_next_row() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            Table::new("Materials")
                .columns([TableColumn::new("Name"), TableColumn::new("Passes")])
                .rows([TableRow::new(["Glass", "3"]), TableRow::new(["Water", "4"])])
                .on_change(move |index| on_change.borrow_mut().push(index)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(60.0, 72.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(60.0, 72.0), false),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
        )?;

        assert!(changes.borrow().contains(&1));

        let output = runtime.render(window_id)?;
        let table = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Table)
            .expect("table semantics present");
        assert_eq!(table.value, Some(SemanticsValue::Text("Water".to_string())));
        Ok(())
    }

    #[test]
    fn virtual_table_semantics_include_columns_and_visible_rows() {
        let output = render(
            SizedBox::new().width(360.0).height(120.0).with_child(
                VirtualTable::new("Files")
                    .columns([
                        VirtualTableColumn::new("Name").min_width(160.0),
                        VirtualTableColumn::new("Kind").width(80.0),
                    ])
                    .row_count(100)
                    .row_height(20.0)
                    .header_height(24.0)
                    .row_name(|index| format!("Row {index}"))
                    .row_description(|index| format!("File row {index}")),
            ),
        );

        let table = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Table)
            .expect("virtual table semantics present");
        assert_eq!(table.name.as_deref(), Some("Files"));
        assert_eq!(
            table.value,
            Some(SemanticsValue::Text("100 rows".to_string()))
        );
        assert_eq!(table.description.as_deref(), Some("Columns: Name, Kind"));

        let rows = output
            .semantics
            .iter()
            .filter(|node| node.role == SemanticsRole::ListItem)
            .collect::<Vec<_>>();
        assert!(!rows.is_empty());
        assert!(
            rows.len() < 100,
            "virtual table should expose only visible rows, got {}",
            rows.len()
        );
        assert_eq!(rows[0].name.as_deref(), Some("Row 0"));
        assert_eq!(rows[0].description.as_deref(), Some("File row 0"));
    }

    #[test]
    fn table_numeric_column_uses_tabular_figures_and_shared_right_edge() {
        let output = render(
            SizedBox::new().width(360.0).height(140.0).with_child(
                Table::new("Materials")
                    .columns([
                        TableColumn::new("Name"),
                        TableColumn::new("Passes").numeric(),
                    ])
                    .rows([
                        TableRow::new(["Glass", "3"]),
                        TableRow::new(["Water", "128"]),
                    ]),
            ),
        );
        let short = text_runs_for(&output, "3")
            .into_iter()
            .find(|run| {
                run.style
                    .features
                    .iter()
                    .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES)
            })
            .expect("short numeric cell should be tabular");
        let long = text_runs_for(&output, "128")
            .into_iter()
            .next()
            .expect("long numeric cell should render");

        assert!(
            long.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((short.rect.max_x() - long.rect.max_x()).abs() < 0.75);
    }

    #[test]
    fn selected_table_cells_preserve_body_metrics_and_numeric_alignment() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 13.0;
        theme.typography.body_line_height = 21.0;
        let output = render(
            SizedBox::new().width(360.0).height(140.0).with_child(
                Table::new("Materials")
                    .theme(theme)
                    .columns([
                        TableColumn::new("Name"),
                        TableColumn::new("Passes").numeric(),
                    ])
                    .rows([
                        TableRow::new(["Glass", "8642"]),
                        TableRow::new(["Water", "7"]),
                    ])
                    .selected(0),
            ),
        );
        let selected_label = text_runs_for(&output, "Glass")
            .into_iter()
            .next()
            .expect("selected text cell should render");
        let selected_number = text_runs_for(&output, "8642")
            .into_iter()
            .next()
            .expect("selected numeric cell should render");
        let unselected_number = text_runs_for(&output, "7")
            .into_iter()
            .next()
            .expect("unselected numeric cell should render");

        assert_eq!(selected_label.style.color, theme.palette.text);
        assert_eq!(
            selected_label.style.font_size,
            theme.typography.body_font_size
        );
        assert_eq!(
            selected_label.style.line_height,
            theme.typography.body_line_height
        );
        assert_eq!(selected_number.style.color, theme.palette.text);
        assert_eq!(
            selected_number.style.font_size,
            theme.typography.body_font_size
        );
        assert_eq!(
            selected_number.style.line_height,
            theme.typography.body_line_height
        );
        assert!(
            selected_number
                .style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((selected_number.rect.max_x() - unselected_number.rect.max_x()).abs() < 0.75);
    }

    #[test]
    fn collection_and_path_widgets_theme_when_paints_dark_tokens() {
        let theme = DefaultTheme::dark();
        let list = render(
            SizedBox::new().width(320.0).height(120.0).with_child(
                ListView::new("Assets")
                    .theme_when(move || theme)
                    .item(ListItem::new("Hero texture").detail("2048 x 2048 RGBA")),
            ),
        );
        assert_eq!(
            text_runs_for(&list, "Hero texture")[0].style.color,
            theme.palette.text
        );
        assert!(solid_fill_colors(&list).contains(&theme.palette.surface));

        let theme = DefaultTheme::dark();
        let tree = render(
            SizedBox::new().width(320.0).height(120.0).with_child(
                TreeView::new("Scene")
                    .theme_when(move || theme)
                    .item(TreeItem::new("Environment").detail("Visible")),
            ),
        );
        assert_eq!(
            text_runs_for(&tree, "Environment")[0].style.color,
            theme.palette.text
        );
        assert!(solid_fill_colors(&tree).contains(&theme.palette.surface));

        let theme = DefaultTheme::dark();
        let table = render(
            SizedBox::new().width(320.0).height(140.0).with_child(
                Table::new("Materials")
                    .theme_when(move || theme)
                    .columns([TableColumn::new("Name")])
                    .rows([TableRow::new(["Glass"])]),
            ),
        );
        let table_fills = solid_fill_colors(&table);
        assert!(table_fills.contains(&theme.palette.surface));
        assert!(table_fills.contains(&theme.palette.control));
        assert!(
            !table_fills.contains(&Color::rgba(0.95, 0.965, 0.985, 1.0)),
            "dark table header should not use the old hardcoded light fill"
        );

        let theme = DefaultTheme::dark();
        let breadcrumb = render(Breadcrumb::new("Path").theme_when(move || theme).items([
            BreadcrumbItem::new("Workspace"),
            BreadcrumbItem::new("Project"),
        ]));
        assert_eq!(
            text_runs_for(&breadcrumb, "Workspace")[0].style.color,
            theme.palette.text
        );
        assert!(solid_fill_colors(&breadcrumb).contains(&theme.palette.surface));
    }

    #[test]
    fn table_chrome_uses_theme_metrics() {
        let mut theme = DefaultTheme::default();
        theme.metrics.table_header_separator_inset = 7.0;
        theme.metrics.table_separator_width = 2.75;
        theme.metrics.table_row_border_opacity = 0.29;
        theme.metrics.data_scroll_thumb_width = 5.5;
        theme.metrics.data_scroll_thumb_inset = 9.0;
        theme.metrics.data_scroll_thumb_radius = 2.75;
        theme.metrics.data_scroll_thumb_min_length = 35.0;
        theme.metrics.data_scroll_thumb_opacity = 0.41;
        let rows = (0..10).map(|index| TableRow::new([format!("Row {index}"), format!("{index}")]));

        let output = render(
            SizedBox::new().width(320.0).height(120.0).with_child(
                Table::new("Materials")
                    .theme(theme)
                    .columns([
                        TableColumn::new("Name").width(160.0),
                        TableColumn::new("Passes").width(80.0),
                    ])
                    .rows(rows),
            ),
        );

        let expected_separator_height =
            theme.metrics.table_header_height - (theme.metrics.table_header_separator_inset * 2.0);
        assert!(solid_stroke_rects(&output).iter().any(|rect| {
            (rect.width() - theme.metrics.table_separator_width).abs() < 0.01
                && (rect.height() - expected_separator_height).abs() < 0.01
        }));
        assert!(
            solid_stroke_widths(&output)
                .iter()
                .any(|width| { (*width - theme.metrics.table_separator_width).abs() < 0.01 })
        );
        assert!(
            solid_stroke_colors(&output).contains(
                &theme
                    .palette
                    .border
                    .with_alpha(theme.metrics.table_row_border_opacity)
            )
        );
        assert!(
            solid_fill_colors(&output).contains(
                &theme
                    .palette
                    .border_hover
                    .with_alpha(theme.metrics.data_scroll_thumb_opacity)
            )
        );
    }

    #[test]
    fn list_view_detail_text_does_not_overlap_primary_label() {
        let output = render(SizedBox::new().width(320.0).height(120.0).with_child(
            ListView::new("Assets").item(ListItem::new("Hero texture").detail("2048 x 2048 RGBA")),
        ));

        let label = text_rects_for(&output, "Hero texture")[0];
        let detail = text_rects_for(&output, "2048 x 2048 RGBA")[0];

        assert!(label.max_y() <= detail.y());
    }

    #[test]
    fn list_view_label_and_detail_visual_centers_match_row_slots() {
        let theme = DefaultTheme::default();
        let output = render(SizedBox::new().width(320.0).height(72.0).with_child(
            ListView::new("Assets").item(ListItem::new("Hero texture").detail("2048 x 2048 RGBA")),
        ));
        let label = text_runs_for(&output, "Hero texture")
            .into_iter()
            .next()
            .expect("list row label draw command present");
        let detail = text_runs_for(&output, "2048 x 2048 RGBA")
            .into_iter()
            .next()
            .expect("list row detail draw command present");
        let viewport_padding = theme.metrics.data_viewport_padding;
        let row_height = theme
            .metrics
            .list_row_height
            .max(super::two_line_row_height(
                label.style.line_height,
                detail.style.line_height,
            ));
        let row = Rect::new(
            viewport_padding.left,
            viewport_padding.top,
            (output.frame.viewport.width - viewport_padding.left - viewport_padding.right).max(0.0),
            row_height,
        );

        assert_two_line_row_text_matches_slots(&output, "Hero texture", "2048 x 2048 RGBA", row);
    }

    #[test]
    fn data_detail_text_styles_follow_theme_xs_token() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 10.5,
            line_height: 17.5,
        };
        theme.sync_derived_fields();

        let list = render(
            SizedBox::new().width(320.0).height(72.0).with_child(
                ListView::new("Assets")
                    .theme(theme)
                    .item(ListItem::new("Hero texture").detail("2048 x 2048 RGBA")),
            ),
        );
        assert_text_run_uses_token(
            &text_runs_for(&list, "2048 x 2048 RGBA")
                .into_iter()
                .next()
                .expect("list detail should render"),
            theme.text.xs,
        );

        let layer = render(
            SizedBox::new().width(280.0).height(72.0).with_child(
                LayerList::new("Layers").theme(theme).layer(
                    LayerListItem::new("Paint")
                        .detail("Normal / 100%")
                        .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                ),
            ),
        );
        assert_text_run_uses_token(
            &text_runs_for(&layer, "Normal / 100%")
                .into_iter()
                .next()
                .expect("layer detail should render"),
            theme.text.xs,
        );

        let tree = render(
            SizedBox::new().width(320.0).height(72.0).with_child(
                TreeView::new("Scene")
                    .theme(theme)
                    .item(TreeItem::new("Environment").detail("Visible")),
            ),
        );
        assert_text_run_uses_token(
            &text_runs_for(&tree, "Visible")
                .into_iter()
                .next()
                .expect("tree detail should render"),
            theme.text.xs,
        );
    }

    #[test]
    fn list_view_long_label_clips_to_single_line() {
        let title = "What tools are available to you when the session title is very long";
        let output = render(
            SizedBox::new()
                .width(170.0)
                .height(56.0)
                .with_child(ListView::new("Sessions").item(ListItem::new(title))),
        );

        let run = text_runs_for(&output, title)
            .into_iter()
            .next()
            .expect("long title should be drawn");
        let clips = draw_clip_rects_for(&output, title);

        assert_eq!(clips.len(), 1);
        assert!(
            clips[0].height() <= run.style.line_height + 0.5,
            "long single-line labels should clip wrapped overflow to one line; clip={:?}, line_height={}",
            clips[0],
            run.style.line_height
        );
    }

    #[test]
    fn tree_view_detail_text_does_not_overlap_primary_label() {
        let output = render(SizedBox::new().width(320.0).height(120.0).with_child(
            TreeView::new("Scene").item(TreeItem::new("Environment").detail("Visible")),
        ));

        let label = text_rects_for(&output, "Environment")[0];
        let detail = text_rects_for(&output, "Visible")[0];

        assert!(label.max_y() <= detail.y());
    }

    #[test]
    fn list_and_tree_detail_text_stays_grouped_with_primary_label() {
        let list = render(SizedBox::new().width(320.0).height(120.0).with_child(
            ListView::new("Assets").item(ListItem::new("Hero texture").detail("2048 x 2048 RGBA")),
        ));
        let tree = render(SizedBox::new().width(320.0).height(120.0).with_child(
            TreeView::new("Scene").item(TreeItem::new("Environment").detail("Visible")),
        ));

        for (output, label_text, detail_text) in [
            (&list, "Hero texture", "2048 x 2048 RGBA"),
            (&tree, "Environment", "Visible"),
        ] {
            let label = text_rects_for(output, label_text)[0];
            let detail = text_rects_for(output, detail_text)[0];
            let gap = detail.y() - label.max_y();

            assert!(
                (0.0..=2.5).contains(&gap),
                "expected {label_text:?} and {detail_text:?} to read as one row; gap was {gap}"
            );
        }
    }

    #[test]
    fn tree_view_label_and_detail_visual_centers_match_row_slots() {
        let theme = DefaultTheme::default();
        let output = render(SizedBox::new().width(320.0).height(72.0).with_child(
            TreeView::new("Scene").item(TreeItem::new("Environment").detail("Visible")),
        ));
        let label = text_runs_for(&output, "Environment")
            .into_iter()
            .next()
            .expect("tree row label draw command present");
        let detail = text_runs_for(&output, "Visible")
            .into_iter()
            .next()
            .expect("tree row detail draw command present");
        let viewport_padding = theme.metrics.data_viewport_padding;
        let row_height = theme
            .metrics
            .tree_row_height
            .max(super::two_line_row_height(
                label.style.line_height,
                detail.style.line_height,
            ));
        let row = Rect::new(
            viewport_padding.left,
            viewport_padding.top,
            (output.frame.viewport.width - viewport_padding.left - viewport_padding.right).max(0.0),
            row_height,
        );

        assert_two_line_row_text_matches_slots(&output, "Environment", "Visible", row);
    }

    #[test]
    fn list_view_does_not_paint_internal_scroll_thumb() {
        let output = render(
            SizedBox::new().width(320.0).height(100.0).with_child(
                ListView::new("Assets")
                    .items([
                        ListItem::new("Hero texture").detail("2048 x 2048 RGBA"),
                        ListItem::new("UI icon sheet").detail("Tagged for export"),
                        ListItem::new("Archive cache").detail("Read only"),
                        ListItem::new("Normals atlas").detail("Streaming mip chain"),
                    ])
                    .selected(1),
            ),
        );

        assert!(vertical_scroll_thumb_rects(&output).is_empty());
    }

    #[test]
    fn list_view_scrolls_overflowing_rows_without_thumb() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(SizedBox::new().width(320.0).height(100.0).with_child(
                ListView::new("Assets").items([
                    ListItem::new("Hero texture").detail("2048 x 2048 RGBA"),
                    ListItem::new("UI icon sheet").detail("Tagged for export"),
                    ListItem::new("Archive cache").detail("Read only"),
                    ListItem::new("Normals atlas").detail("Streaming mip chain"),
                ]),
            ));

        let before = runtime.render(window_id)?;
        let before_y = text_rects_for(&before, "Hero texture")[0].y();

        runtime.handle_event(
            window_id,
            wheel_scroll(Point::new(60.0, 60.0), Vector::new(0.0, -24.0)),
        )?;
        let after = runtime.render(window_id)?;
        let after_y = text_rects_for(&after, "Hero texture")[0].y();

        assert!(after_y < before_y);
        assert!(vertical_scroll_thumb_rects(&after).is_empty());
        Ok(())
    }

    #[test]
    fn list_view_clips_scrolled_selection_highlight_to_viewport() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(320.0).height(100.0).with_child(
                ListView::new("Assets")
                    .theme(theme)
                    .items([
                        ListItem::new("Hero texture").detail("2048 x 2048 RGBA"),
                        ListItem::new("UI icon sheet").detail("Tagged for export"),
                        ListItem::new("Archive cache").detail("Read only"),
                        ListItem::new("Normals atlas").detail("Streaming mip chain"),
                    ])
                    .selected(0),
            ),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            wheel_scroll(Point::new(60.0, 60.0), Vector::new(0.0, -24.0)),
        )?;
        let output = runtime.render(window_id)?;
        let highlight = selected_highlight_rects(&output)
            .first()
            .copied()
            .expect("selected list row highlight should be painted");

        assert!(highlight.y() >= theme.metrics.data_viewport_padding.top);
        assert!(highlight.max_y() <= 100.0 - theme.metrics.data_viewport_padding.bottom);
        Ok(())
    }

    #[test]
    fn tree_view_does_not_paint_internal_scroll_thumb() {
        let output = render(
            SizedBox::new().width(320.0).height(120.0).with_child(
                TreeView::new("Scene").item(
                    TreeItem::new("Environment")
                        .with_child(TreeItem::new("Sky dome").detail("Visible"))
                        .with_child(TreeItem::new("Fog volume").detail("Animated"))
                        .with_child(TreeItem::new("Characters").detail("Selected")),
                ),
            ),
        );

        assert!(vertical_scroll_thumb_rects(&output).is_empty());
    }

    #[test]
    fn tree_view_scrolls_overflowing_rows_without_thumb() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(320.0).height(120.0).with_child(
                TreeView::new("Scene").item(
                    TreeItem::new("Scene")
                        .expanded(true)
                        .with_child(TreeItem::new("Environment").detail("Visible"))
                        .with_child(TreeItem::new("Sky dome").detail("Visible"))
                        .with_child(TreeItem::new("Fog volume").detail("Animated"))
                        .with_child(TreeItem::new("Pilot").detail("Selected")),
                ),
            ),
        );

        let before = runtime.render(window_id)?;
        let before_y = text_rects_for(&before, "Scene")[0].y();

        runtime.handle_event(
            window_id,
            wheel_scroll(Point::new(60.0, 60.0), Vector::new(0.0, -24.0)),
        )?;
        let after = runtime.render(window_id)?;
        let after_y = text_rects_for(&after, "Scene")[0].y();

        assert!(after_y < before_y);
        assert!(vertical_scroll_thumb_rects(&after).is_empty());
        Ok(())
    }

    #[test]
    fn tree_view_clips_scrolled_selection_highlight_to_viewport() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(320.0).height(120.0).with_child(
                TreeView::new("Scene").item(
                    TreeItem::new("Scene")
                        .expanded(true)
                        .with_child(TreeItem::new("Environment").detail("Visible"))
                        .with_child(TreeItem::new("Sky dome").detail("Visible"))
                        .with_child(TreeItem::new("Fog volume").detail("Animated"))
                        .with_child(TreeItem::new("Pilot").detail("Selected")),
                ),
            ),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(60.0, 24.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(60.0, 24.0), false),
        )?;
        runtime.handle_event(
            window_id,
            wheel_scroll(Point::new(60.0, 60.0), Vector::new(0.0, -24.0)),
        )?;
        let output = runtime.render(window_id)?;
        let highlight = selected_highlight_rects(&output)
            .first()
            .copied()
            .expect("selected tree row highlight should be painted");

        assert!(highlight.y() >= theme.metrics.data_viewport_padding.top);
        assert!(highlight.max_y() <= 120.0 - theme.metrics.data_viewport_padding.bottom);
        Ok(())
    }

    #[test]
    fn tree_view_disclosure_does_not_overlap_primary_label() {
        let output = render(
            SizedBox::new().width(320.0).height(120.0).with_child(
                TreeView::new("Scene").item(
                    TreeItem::new("Environment")
                        .with_child(TreeItem::new("Sky dome").detail("Visible")),
                ),
            ),
        );

        let label = text_rects_for(&output, "Environment")[0];
        let mut disclosure_bounds = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillPath { path, .. } = command {
                let bounds = path.bounds();
                if bounds.width() <= 12.0 && bounds.height() <= 12.0 {
                    disclosure_bounds.push(bounds);
                }
            }
        });

        let disclosure = disclosure_bounds
            .first()
            .expect("tree disclosure should be painted");
        assert!(
            disclosure.max_x() + DefaultTheme::default().metrics.tree_disclosure_gap <= label.x()
        );
    }

    #[test]
    fn table_text_rect_uses_full_line_height() {
        let output = render(
            SizedBox::new().width(320.0).height(140.0).with_child(
                Table::new("Materials")
                    .columns([TableColumn::new("Name")])
                    .rows([TableRow::new(["Glass"])]),
            ),
        );

        let header = text_rects_for(&output, "Name")[0];
        let cell = text_rects_for(&output, "Glass")[0];
        let line_height = DefaultTheme::default().body_text_style().line_height;

        assert!(header.height() >= line_height);
        assert!(cell.height() >= line_height);
    }

    #[test]
    fn table_aligned_text_preserves_tall_measurements_and_cell_alignment() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.table_header_height = 44.0;
        theme.metrics.table_row_height = 48.0;

        let output = render(
            SizedBox::new().width(360.0).height(136.0).with_child(
                Table::new("Materials")
                    .theme(theme)
                    .columns([
                        TableColumn::new("Name")
                            .width(180.0)
                            .alignment(TableColumnAlignment::Center),
                        TableColumn::new("Passes").width(120.0).numeric(),
                    ])
                    .rows([TableRow::new(["Glass", "128"])]),
            ),
        );
        let header = text_runs_for(&output, "Name")
            .into_iter()
            .next()
            .expect("centered table header should render");
        let header_clip = draw_clip_rects_for(&output, "Name")
            .into_iter()
            .next()
            .expect("centered table header should be clipped to its cell");
        let cell = text_runs_for(&output, "Glass")
            .into_iter()
            .next()
            .expect("table cell should render");
        let cell_clip = draw_clip_rects_for(&output, "Glass")
            .into_iter()
            .next()
            .expect("table cell should be clipped to its cell");
        let numeric = text_runs_for(&output, "128")
            .into_iter()
            .next()
            .expect("numeric table cell should render");
        let numeric_clip = draw_clip_rects_for(&output, "128")
            .into_iter()
            .next()
            .expect("numeric table cell should be clipped to its cell");
        let measured_height = |run: &sui_text::TextRun| {
            TextSystem::new()
                .shape_text_run(run, &FontRegistry::new())
                .expect("table text should shape")
                .measurement()
                .height
        };

        assert!(header.rect.height() >= measured_height(&header) - 0.01);
        assert!(cell.rect.height() >= measured_height(&cell) - 0.01);
        assert!(numeric.rect.height() >= measured_height(&numeric) - 0.01);
        assert!(
            (rect_center(header.rect).x - rect_center(header_clip).x).abs() < 0.75,
            "centered table header should align to cell center: text={:?}, cell={:?}",
            header.rect,
            header_clip
        );
        assert!(
            (numeric.rect.max_x() - numeric_clip.max_x()).abs() < 0.75,
            "numeric table cell should align to trailing edge: text={:?}, cell={:?}",
            numeric.rect,
            numeric_clip
        );
        assert!(
            (text_visual_center_for(&output, "Name") - rect_center(header_clip).y).abs() < 0.75,
            "centered table header should be optically centered in its cell"
        );
        assert!(
            (text_visual_center_for(&output, "Glass") - rect_center(cell_clip).y).abs() < 0.75,
            "table body cell should be optically centered in its cell"
        );
        assert!(
            (text_visual_center_for(&output, "128") - rect_center(numeric_clip).y).abs() < 0.75,
            "numeric table body cell should be optically centered in its cell"
        );
    }

    #[test]
    fn non_scrollable_table_allows_wheel_to_bubble_to_parent_scroll_view() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(220.0, 120.0))
                .with_child(ScrollView::vertical(
                    Stack::vertical()
                        .with_child(SizedBox::new().width(220.0).height(80.0))
                        .with_child(
                            SizedBox::new().width(220.0).height(120.0).with_child(
                                Table::new("Materials")
                                    .columns([
                                        TableColumn::new("Name"),
                                        TableColumn::new("Passes").width(80.0),
                                    ])
                                    .rows([
                                        TableRow::new(["Glass", "3"]),
                                        TableRow::new(["Water", "4"]),
                                    ]),
                            ),
                        )
                        .with_child(SizedBox::new().width(220.0).height(160.0)),
                )),
        );

        let _ = runtime.render(window_id)?;

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(60.0, 90.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -24.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll))?;
        let _ = runtime.render(window_id)?;

        let graph = runtime.widget_graph(window_id)?;
        let outer_content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 220.0 && node.bounds.height() == 360.0)
            .expect("outer scroll content present");

        assert_eq!(outer_content.bounds.y(), -24.0);
        Ok(())
    }

    #[test]
    fn breadcrumb_keyboard_activates_last_segment() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_activate = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            Breadcrumb::new("Path")
                .items([
                    BreadcrumbItem::new("Workspace"),
                    BreadcrumbItem::new("Project"),
                    BreadcrumbItem::new("Scene"),
                ])
                .current(0)
                .on_activate(move |index, label| on_activate.borrow_mut().push((index, label))),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(28.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(28.0, 20.0), false),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("End", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;

        assert_eq!(changes.borrow().last(), Some(&(2, "Scene".to_string())));

        let output = runtime.render(window_id)?;
        let breadcrumb = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Breadcrumb)
            .expect("breadcrumb semantics present");
        assert_eq!(
            breadcrumb.value,
            Some(SemanticsValue::Text("Scene".to_string()))
        );
        Ok(())
    }

    #[test]
    fn breadcrumb_paints_segment_labels_with_line_height() {
        let output = render(Breadcrumb::new("Path").items([
            BreadcrumbItem::new("Workspace"),
            BreadcrumbItem::new("Project"),
            BreadcrumbItem::new("Scene"),
        ]));

        let run = text_runs_for(&output, "Workspace")
            .into_iter()
            .next()
            .expect("breadcrumb label draw command present");
        let theme = DefaultTheme::default();
        let line_height = theme.body_text_style().line_height;
        let available_height = (theme.metrics.breadcrumb_height
            - theme.metrics.breadcrumb_item_padding.top
            - theme.metrics.breadcrumb_item_padding.bottom)
            .max(0.0);
        let layout = TextSystem::new()
            .shape_text_run(&run, &FontRegistry::new())
            .expect("breadcrumb label should shape");
        let line = layout.lines().first().expect("breadcrumb line present");
        let actual_visual_center =
            run.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let slot_center = theme.metrics.breadcrumb_item_padding.top + (available_height * 0.5);

        assert!((run.rect.height() - line_height.min(available_height)).abs() < 0.001);
        assert!(run.rect.width() > 0.0);
        assert!(
            (actual_visual_center - slot_center).abs() < 0.75,
            "breadcrumb label visual center {actual_visual_center} did not match slot center {slot_center}; text rect {:?}",
            run.rect
        );
    }

    #[test]
    fn breadcrumb_label_stays_within_vertical_item_bounds() {
        let output = render(Breadcrumb::new("Path").items([BreadcrumbItem::new("Workspace")]));
        let label = text_rects_for(&output, "Workspace")[0];

        assert!(label.y() >= -0.01);
        assert!(label.max_y() <= output.frame.viewport.height + 0.01);
    }

    #[test]
    fn list_row_label_visual_center_matches_row_center() {
        let output = render(ListView::new("Assets").item(ListItem::new("Hero texture")));
        let run = text_runs_for(&output, "Hero texture")
            .into_iter()
            .next()
            .expect("list row label draw command present");
        let layout = TextSystem::new()
            .shape_text_run(&run, &FontRegistry::new())
            .expect("list row label should shape");
        let line = layout.lines().first().expect("list row line present");
        let actual_visual_center =
            run.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let row_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    struct LeadingLabelCellFixture {
        theme: DefaultTheme,
    }

    impl Widget for LeadingLabelCellFixture {
        fn measure(
            &mut self,
            _ctx: &mut sui_runtime::MeasureCtx,
            constraints: Constraints,
        ) -> Size {
            constraints.clamp(Size::new(132.0, 32.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            super::paint_leading_label_cell(
                ctx,
                &self.theme,
                ctx.bounds(),
                "artifact.txt",
                LeadingLabelCellPaint::new()
                    .icon(crate::IconGlyph::File)
                    .icon_color(self.theme.palette.text_muted)
                    .label_color(self.theme.palette.text)
                    .label_weight(FontWeight::MEDIUM)
                    .icon_slot_side(20.0)
                    .icon_inset(2.0)
                    .gap(8.0)
                    .padding(6.0, 8.0),
            );
        }
    }

    #[test]
    fn leading_label_cell_uses_compact_token_and_clips_to_cell() {
        let theme = DefaultTheme::default();
        let output = render(LeadingLabelCellFixture { theme });
        let label = text_runs_for(&output, "artifact.txt")
            .into_iter()
            .next()
            .expect("leading-label cell text should paint");

        assert_text_run_uses_token(&label, theme.text.sm);
        assert_eq!(label.style.weight, FontWeight::MEDIUM);
        assert!(label.rect.x() >= 34.0);
        assert!(
            label.rect.max_x() <= output.frame.viewport.width + 0.75,
            "label should stay within the cell bounds: {:?}",
            label.rect
        );
    }

    struct TextCellFixture {
        theme: DefaultTheme,
    }

    impl Widget for TextCellFixture {
        fn measure(
            &mut self,
            _ctx: &mut sui_runtime::MeasureCtx,
            constraints: Constraints,
        ) -> Size {
            constraints.clamp(Size::new(160.0, 32.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            super::paint_text_cell(
                ctx,
                &self.theme,
                ctx.bounds(),
                "128",
                TextCellPaint::new()
                    .text_token(self.theme.text.xs)
                    .color(self.theme.palette.text_muted)
                    .weight(FontWeight::MEDIUM)
                    .padding(10.0, 18.0)
                    .alignment(TableColumnAlignment::End)
                    .numeric(),
            );
        }
    }

    #[test]
    fn text_cell_paint_uses_alignment_numeric_token_and_clipping() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 10.5,
            line_height: 17.5,
        };
        theme.sync_derived_fields();
        let output = render(TextCellFixture { theme });
        let run = text_runs_for(&output, "128")
            .into_iter()
            .next()
            .expect("text-cell text should paint");
        let clip = draw_clip_rects_for(&output, "128")
            .into_iter()
            .next()
            .expect("text-cell text should clip to padded content");
        let content_right = output.frame.viewport.width - 18.0;

        assert_text_run_uses_token(&run, theme.text.xs);
        assert_eq!(run.style.color, theme.palette.text_muted);
        assert_eq!(run.style.weight, FontWeight::MEDIUM);
        assert!(
            run.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1),
            "numeric text cells should opt into tabular figures"
        );
        assert!((run.rect.max_x() - content_right).abs() < 0.75);
        assert!((clip.x() - 10.0).abs() < 0.01);
        assert!((clip.max_x() - content_right).abs() < 0.01);
    }

    struct TextBlockFixture {
        theme: DefaultTheme,
        text: String,
    }

    impl Widget for TextBlockFixture {
        fn measure(
            &mut self,
            _ctx: &mut sui_runtime::MeasureCtx,
            constraints: Constraints,
        ) -> Size {
            constraints.clamp(Size::new(178.0, 76.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            super::paint_text_block(
                ctx,
                &self.theme,
                ctx.bounds(),
                &self.text,
                TextBlockPaint::new()
                    .text_token(self.theme.text.sm)
                    .color(self.theme.palette.text_muted)
                    .weight(FontWeight::MEDIUM)
                    .padding(Insets {
                        left: 12.0,
                        top: 7.0,
                        right: 16.0,
                        bottom: 9.0,
                    }),
            );
        }
    }

    #[test]
    fn text_block_paint_wraps_top_aligns_and_clips_to_padding() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 12.0,
            line_height: 18.0,
        };
        theme.sync_derived_fields();
        let text =
            "This preview line wraps through SUI text layout.\nSecond paragraph stays compact."
                .to_string();
        let output = render(TextBlockFixture {
            theme,
            text: text.clone(),
        });
        let (origin, layout, color) = output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| {
                let SceneCommand::DrawShapedText(run) = command else {
                    return None;
                };
                let layout = run.resolve(output.frame.text_layout_registry.as_ref())?;
                (layout.text() == text).then_some((run.origin, layout.clone(), run.color_override))
            })
            .expect("text block should paint as a shaped text layout");
        let clip = draw_clip_rects_for(&output, &text)
            .into_iter()
            .next()
            .expect("text block should clip to padded content");

        assert_eq!(color, Some(theme.palette.text_muted));
        assert!((origin.x - 12.0).abs() < 0.01);
        assert!((origin.y - 7.0).abs() < 0.01);
        assert!((layout.box_size().width - 150.0).abs() < 0.01);
        assert!((layout.box_size().height - 60.0).abs() < 0.01);
        assert!(layout.lines().len() >= 2, "text block should wrap");
        assert_text_run_uses_token(
            &sui_text::TextRun {
                rect: Rect::ZERO,
                text,
                style: layout.style().clone(),
            },
            theme.text.sm,
        );
        assert_eq!(layout.style().weight, FontWeight::MEDIUM);
        assert!((clip.x() - 12.0).abs() < 0.01);
        assert!((clip.y() - 7.0).abs() < 0.01);
        assert!((clip.width() - 150.0).abs() < 0.01);
        assert!((clip.height() - 60.0).abs() < 0.01);
    }

    #[test]
    fn list_row_text_preserves_tall_measurement_in_compact_line_box() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.list_row_height = 52.0;

        let output = render(
            SizedBox::new().width(320.0).height(72.0).with_child(
                ListView::new("Assets")
                    .theme(theme)
                    .item(ListItem::new("Glass")),
            ),
        );
        let run = text_runs_for(&output, "Glass")
            .into_iter()
            .next()
            .expect("list row label draw command present");
        let layout = TextSystem::new()
            .shape_text_run(&run, &FontRegistry::new())
            .expect("list row label should shape");
        let row_center =
            theme.metrics.data_viewport_padding.top + theme.metrics.list_row_height * 0.5;

        assert!(
            run.rect.height() >= layout.measurement().height - 0.01,
            "list row text rect should preserve measured glyph height: rect={:?}, measurement={:?}",
            run.rect,
            layout.measurement()
        );
        assert!(
            run.rect.height() > run.style.line_height,
            "test theme should exercise measured-height preservation"
        );
        assert!(
            (text_run_visual_center(&run) - row_center).abs() < 0.75,
            "list row label visual center should remain row-centered"
        );
    }

    #[test]
    fn list_row_leading_and_trailing_text_align_to_row_center_and_edge() {
        let theme = DefaultTheme::default();
        let output = render(
            SizedBox::new().width(260.0).with_child(
                ListView::new("Assets").item(
                    ListItem::new("Hero texture")
                        .leading_text("A")
                        .trailing("42"),
                ),
            ),
        );
        let leading = text_runs_for(&output, "A")
            .into_iter()
            .next()
            .expect("leading text draw command present");
        let trailing = text_runs_for(&output, "42")
            .into_iter()
            .next()
            .expect("trailing text draw command present");
        let text_system = TextSystem::new();
        let leading_layout = text_system
            .shape_text_run(&leading, &FontRegistry::new())
            .expect("leading text should shape");
        let trailing_layout = text_system
            .shape_text_run(&trailing, &FontRegistry::new())
            .expect("trailing text should shape");
        let leading_line = leading_layout
            .lines()
            .first()
            .expect("leading line present");
        let trailing_line = trailing_layout
            .lines()
            .first()
            .expect("trailing line present");
        let leading_visual_center = leading.rect.y()
            + leading_line.baseline
            + optical_visual_center(leading_layout.measurement());
        let trailing_visual_center = trailing.rect.y()
            + trailing_line.baseline
            + optical_visual_center(trailing_layout.measurement());
        let row_center = output.frame.viewport.height * 0.5;
        let trailing_edge = output.frame.viewport.width
            - theme.metrics.data_viewport_padding.right
            - theme.metrics.data_row_padding.right;

        assert!((leading_visual_center - row_center).abs() < 0.75);
        assert!((trailing_visual_center - row_center).abs() < 0.75);
        assert!(
            (trailing.rect.max_x() - trailing_edge).abs() < 0.75,
            "trailing text max_x {} did not align to content edge {trailing_edge}",
            trailing.rect.max_x()
        );
    }

    #[test]
    fn list_row_leading_and_trailing_text_preserve_tall_measurements() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.text.xs = ThemeTextToken {
            size: 26.0,
            line_height: 10.0,
        };
        theme.metrics.list_row_height = 64.0;

        let output = render(
            SizedBox::new().width(320.0).height(88.0).with_child(
                ListView::new("Assets").theme(theme).item(
                    ListItem::new("Hero texture")
                        .leading_text("A")
                        .trailing("42"),
                ),
            ),
        );
        let row = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ListItem)
            .expect("list row semantics present")
            .bounds;
        let leading = text_runs_for(&output, "A")
            .into_iter()
            .next()
            .expect("leading text draw command present");
        let trailing = text_runs_for(&output, "42")
            .into_iter()
            .next()
            .expect("trailing text draw command present");
        let leading_layout = TextSystem::new()
            .shape_text_run(&leading, &FontRegistry::new())
            .expect("leading text should shape");
        let trailing_layout = TextSystem::new()
            .shape_text_run(&trailing, &FontRegistry::new())
            .expect("trailing text should shape");
        let trailing_clip = draw_clip_rects_for(&output, "42")
            .into_iter()
            .next()
            .expect("trailing text should be clipped to reserved slot");
        let row_center = row.y() + (row.height() * 0.5);
        let trailing_edge = row.max_x() - theme.metrics.data_row_padding.right;

        assert_eq!(leading.style.font_size, 28.0);
        assert_eq!(leading.style.line_height, 12.0);
        assert_text_run_uses_token(&trailing, theme.text.xs);
        assert!(leading.rect.height() >= leading_layout.measurement().height - 0.01);
        assert!(trailing.rect.height() >= trailing_layout.measurement().height - 0.01);
        assert!(leading.rect.height() > leading.style.line_height);
        assert!(trailing.rect.height() > trailing.style.line_height);
        assert!((text_run_visual_center(&leading) - row_center).abs() < 0.75);
        assert!((text_run_visual_center(&trailing) - row_center).abs() < 0.75);
        assert!((trailing.rect.max_x() - trailing_edge).abs() < 0.75);
        assert!((trailing_clip.max_x() - trailing_edge).abs() < 0.75);
    }

    #[test]
    fn selected_list_row_trailing_text_preserves_caption_metrics() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 10.5,
            line_height: 17.5,
        };
        theme.sync_derived_fields();
        let output = render(
            SizedBox::new().width(260.0).with_child(
                ListView::new("Assets")
                    .theme(theme)
                    .selected(0)
                    .item(ListItem::new("Hero texture").trailing("42")),
            ),
        );
        let trailing = text_runs_for(&output, "42")
            .into_iter()
            .next()
            .expect("selected trailing text draw command present");
        let trailing_layout = TextSystem::new()
            .shape_text_run(&trailing, &FontRegistry::new())
            .expect("selected trailing text should shape");
        let trailing_line = trailing_layout
            .lines()
            .first()
            .expect("selected trailing line present");
        let trailing_visual_center = trailing.rect.y()
            + trailing_line.baseline
            + optical_visual_center(trailing_layout.measurement());
        let row_center = output.frame.viewport.height * 0.5;
        let trailing_edge = output.frame.viewport.width
            - theme.metrics.data_viewport_padding.right
            - theme.metrics.data_row_padding.right;

        assert_text_run_uses_token(&trailing, theme.text.xs);
        assert_eq!(trailing.style.color, theme.palette.placeholder);
        assert!((trailing_visual_center - row_center).abs() < 0.75);
        assert!(
            (trailing.rect.max_x() - trailing_edge).abs() < 0.75,
            "selected trailing text max_x {} did not align to content edge {trailing_edge}",
            trailing.rect.max_x()
        );
    }
}
