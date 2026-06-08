use std::{fmt, rc::Rc};

use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, ToggleState, Vector,
    WidgetId,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild, Widget,
    WidgetPodMutVisitor, WidgetPodVisitor, window_render_options,
};
use sui_text::{TextMeasurement, TextStyle};

use crate::{
    DefaultTheme,
    controls::{IconGlyph, draw_icon_glyph},
};

const LIST_ROW_LEFT_PADDING: f32 = 14.0;
const LIST_ROW_RIGHT_PADDING: f32 = 10.0;
const LIST_ROW_LEADING_ICON_SIZE: f32 = 14.0;
const LIST_ROW_LEADING_GAP: f32 = 8.0;
const LIST_ROW_TRAILING_GAP: f32 = 12.0;

pub struct ListItem {
    label: String,
    detail: Option<String>,
    trailing: Option<String>,
    leading_icon: Option<IconGlyph>,
    leading_text: Option<String>,
    leading_color: Option<Color>,
    accent: Option<Color>,
    disabled: bool,
    content: Option<SingleChild>,
}

impl ListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            trailing: None,
            leading_icon: None,
            leading_text: None,
            leading_color: None,
            accent: None,
            disabled: false,
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
    row_height: f32,
    scroll_y: f32,
    row_heights: Vec<f32>,
    row_offsets: Vec<f32>,
    content_height: f32,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
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
            row_height: 28.0,
            scroll_y: 0.0,
            row_heights: Vec::new(),
            row_offsets: Vec::new(),
            content_height: 0.0,
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
        self.row_height = row_height.max(24.0);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
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
        let base = self
            .row_height
            .max((theme.metrics.min_height + 4.0).max(28.0));
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
        inset_rect(bounds, Insets::all(8.0))
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

    fn row_has_child(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(ListItem::has_child)
    }

    fn activate(&mut self, index: usize) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        if item.disabled {
            return;
        }

        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, item.label.clone());
        }
    }

    fn move_selection(&mut self, delta: isize, viewport_height: f32) {
        if self.items.is_empty() {
            return;
        }

        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.items.len() as isize - 1) as usize;
        self.activate(next);
        self.ensure_visible(viewport_height, next);
    }
}

impl Widget for ListView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_selected();
        let viewport = self.viewport_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
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
                if row.is_some_and(|index| self.row_has_child(index)) {
                    return;
                }
                self.pressed = row;
                self.hovered = self.pressed;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
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
                    if !self.row_has_child(index) {
                        self.activate(index);
                    }
                }
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowUp" => self.move_selection(-1, viewport.height()),
                    "ArrowDown" => self.move_selection(1, viewport.height()),
                    "Home" => {
                        if !self.items.is_empty() {
                            self.activate(0);
                            self.ensure_visible(viewport.height(), 0);
                        }
                    }
                    "End" => {
                        if !self.items.is_empty() {
                            let last = self.items.len() - 1;
                            self.activate(last);
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
        let text_style = theme.body_text_style();
        let detail_style = caption_style(&theme);
        let base_row_height = self.resolved_row_height();
        let child_max_width = if constraints.max.width.is_finite() {
            (constraints.max.width - 36.0).max(0.0)
        } else {
            260.0
        };
        let child_constraints =
            Constraints::new(Size::ZERO, Size::new(child_max_width, f32::INFINITY));
        let mut content_width: f32 = 220.0;
        let mut content_height = 0.0;
        self.row_offsets.clear();
        self.row_heights.clear();

        for item in &mut self.items {
            self.row_offsets.push(content_height);
            let (row_width, row_height) = if let Some(content) = &mut item.content {
                let child_size = content.measure(ctx, child_constraints);
                (
                    (child_size.width + 20.0).max(220.0),
                    (child_size.height + 12.0).max(base_row_height),
                )
            } else {
                let label = measure_text(ctx, &item.label, &text_style).width;
                let detail = item
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                let leading = measure_list_item_leading_width(ctx, item, &text_style);
                let trailing = item
                    .trailing
                    .as_deref()
                    .map(|trailing| measure_text(ctx, trailing, &detail_style).width)
                    .unwrap_or(0.0);
                let trailing_gap = if trailing > 0.0 {
                    LIST_ROW_TRAILING_GAP
                } else {
                    0.0
                };
                (
                    LIST_ROW_LEFT_PADDING
                        + leading
                        + label.max(detail)
                        + trailing_gap
                        + trailing
                        + LIST_ROW_RIGHT_PADDING,
                    base_row_height,
                )
            };
            content_width = content_width.max(row_width);
            content_height += row_height;
            self.row_heights.push(row_height);
        }

        self.content_height = content_height;
        let desired = Size::new(content_width + 16.0, self.measured_content_height() + 16.0);
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
                    Point::new(viewport.x() + 10.0, row_y + 6.0),
                    Size::new((viewport.width() - 20.0).max(0.0), child_size.height),
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let viewport = self.viewport_rect(ctx.bounds());
        let label_style = theme.body_text_style();
        let detail_style = caption_style(&theme);

        draw_surface(ctx, ctx.bounds(), &theme, ctx.is_focused());
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
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);

            if selected || hovered || pressed {
                if let Some(highlight) = row_highlight_rect(row, viewport) {
                    ctx.fill_rect(
                        highlight,
                        if selected {
                            palette.selection
                        } else if pressed {
                            palette.control_active
                        } else {
                            palette.control_hover
                        },
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

            let mut text_x = row.x() + LIST_ROW_LEFT_PADDING;
            let leading_color = item.leading_color.unwrap_or_else(|| {
                if item.disabled {
                    palette.placeholder
                } else if selected {
                    palette.border_focus
                } else {
                    palette.text_muted
                }
            });
            if let Some(icon) = item.leading_icon {
                let side = LIST_ROW_LEADING_ICON_SIZE
                    .min((row.height() - 8.0).max(0.0))
                    .max(0.0);
                let icon_rect = Rect::new(
                    text_x,
                    row.y() + ((row.height() - side) * 0.5).max(0.0),
                    side,
                    side,
                );
                draw_icon_glyph(ctx, icon, icon_rect, leading_color);
                text_x += side + LIST_ROW_LEADING_GAP;
            } else if let Some(leading) = &item.leading_text {
                let leading_style = TextStyle {
                    color: leading_color,
                    ..label_style.clone()
                };
                let leading_measurement = paint_text_measurement(ctx, leading, &leading_style);
                let leading_rect = Rect::new(
                    text_x,
                    vertically_centered_text_rect_y(
                        ctx,
                        row,
                        leading_measurement,
                        leading_style.line_height,
                    ),
                    leading_measurement.width,
                    leading_style.line_height,
                );
                ctx.draw_text(leading_rect, leading.clone(), leading_style);
                text_x += leading_measurement.width + LIST_ROW_LEADING_GAP;
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
                    row.max_x() - LIST_ROW_RIGHT_PADDING - trailing_width,
                    row.y(),
                    trailing_width,
                    row.height(),
                )
            });
            let text_right = trailing_rect
                .map(|rect| rect.x() - LIST_ROW_TRAILING_GAP)
                .unwrap_or(row.max_x() - LIST_ROW_RIGHT_PADDING);
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
                } else if selected {
                    theme.text_style(palette.border_focus)
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
            if let (Some(trailing), Some(rect), Some(measurement)) =
                (&item.trailing, trailing_rect, trailing_measurement)
            {
                let style = if selected {
                    theme.text_style(palette.border_focus)
                } else {
                    detail_style.clone()
                };
                let trailing_text_rect = Rect::new(
                    rect.x(),
                    vertically_centered_text_rect_y(ctx, rect, measurement, style.line_height),
                    rect.width(),
                    style.line_height,
                );
                ctx.push_clip_rect(trailing_text_rect);
                ctx.draw_text(trailing_text_rect, trailing.clone(), style);
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
                row.name = Some(item.label.clone());
                row.description = item.detail.clone();
                row.value = Some(SemanticsValue::Text(
                    item.detail.clone().unwrap_or_else(|| item.label.clone()),
                ));
                row.state.disabled = item.disabled;
                row.state.hovered = self.hovered == Some(index);
                row.state.selected = self.current_selected() == Some(index);
                if !item.disabled && item.content.is_none() {
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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

pub struct LayerList {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    layers: Vec<LayerListItem>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<LayerListHit>,
    pressed: Option<LayerListHit>,
    row_height: f32,
    on_select: Option<Box<dyn FnMut(usize, String)>>,
    on_select_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String)>>,
    on_visibility_change: Option<Box<dyn FnMut(usize, bool)>>,
    on_visibility_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, bool)>>,
    on_lock_change: Option<Box<dyn FnMut(usize, bool)>>,
    on_lock_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, bool)>>,
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
            row_height: 46.0,
            on_select: None,
            on_select_with_ctx: None,
            on_visibility_change: None,
            on_visibility_change_with_ctx: None,
            on_lock_change: None,
            on_lock_change_with_ctx: None,
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
        self.row_height = row_height.max(40.0);
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
        inset_rect(bounds, Insets::all(8.0))
    }

    fn row_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.layers.len() {
            return None;
        }

        let viewport = self.viewport_rect(bounds);
        let y = viewport.y() + index as f32 * self.row_height;
        Rect::new(viewport.x(), y, viewport.width(), self.row_height)
            .intersection(viewport)
            .filter(|rect| !rect.is_empty())
    }

    fn visibility_rect(&self, row: Rect) -> Rect {
        let size = 26.0_f32.min(row.height()).max(18.0);
        Rect::new(
            row.x() + 4.0,
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn thumbnail_rect(&self, row: Rect) -> Rect {
        let size = (row.height() - 14.0).clamp(22.0, 34.0);
        Rect::new(
            row.x() + 36.0,
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn lock_rect(&self, row: Rect) -> Rect {
        let size = 26.0_f32.min(row.height()).max(18.0);
        Rect::new(
            row.max_x() - size - 4.0,
            row.y() + ((row.height() - size) * 0.5),
            size,
            size,
        )
    }

    fn text_rect(&self, row: Rect) -> Rect {
        let thumb = self.thumbnail_rect(row);
        let lock = self.lock_rect(row);
        Rect::new(
            thumb.max_x() + 8.0,
            row.y(),
            (lock.x() - thumb.max_x() - 12.0).max(0.0),
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
}

impl Widget for LayerList {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_selected();

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
                if self.pressed.is_some() {
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
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
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
            self.layers.len().max(1) as f32 * self.row_height + 16.0,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let viewport = self.viewport_rect(ctx.bounds());
        let label_style = theme.body_text_style();
        let detail_style = caption_style(&theme);

        draw_surface(ctx, ctx.bounds(), &theme, ctx.is_focused());
        ctx.push_clip_rect(viewport);

        for (index, layer) in self.layers.iter().enumerate() {
            let Some(row) = self.row_rect(ctx.bounds(), index) else {
                continue;
            };
            let visible = layer.current_visible();
            let locked = layer.current_locked();
            let detail = layer.current_detail();
            let selected = self.current_selected() == Some(index);
            let row_hovered = self.hovered == Some(LayerListHit::Row(index));
            let row_pressed = self.pressed == Some(LayerListHit::Row(index));
            if selected || row_hovered || row_pressed {
                if let Some(highlight) = row_highlight_rect(row, viewport) {
                    ctx.fill_rect(
                        highlight,
                        if selected {
                            palette.selection
                        } else if row_pressed {
                            palette.control_active
                        } else {
                            palette.control_hover
                        },
                    );
                }
            }

            paint_layer_visibility_button(
                ctx,
                self.visibility_rect(row),
                &theme,
                visible,
                self.hovered == Some(LayerListHit::Visibility(index)),
                self.pressed == Some(LayerListHit::Visibility(index)),
            );
            paint_layer_lock_button(
                ctx,
                self.lock_rect(row),
                &theme,
                locked,
                self.hovered == Some(LayerListHit::Lock(index)),
                self.pressed == Some(LayerListHit::Lock(index)),
            );
            paint_layer_thumbnail(
                ctx,
                self.thumbnail_rect(row),
                &theme,
                layer.thumbnail.unwrap_or(palette.control_hover),
                visible,
            );

            let text_rect = self.text_rect(row);
            let label_measurement = paint_text_measurement(ctx, &layer.label, &label_style);
            let detail_text =
                detail
                    .as_deref()
                    .unwrap_or(if visible { "Visible" } else { "Hidden" });
            let detail_measurement = paint_text_measurement(ctx, detail_text, &detail_style);
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
                if selected {
                    theme.text_style(palette.border_focus.with_alpha(text_alpha))
                } else {
                    TextStyle {
                        color: label_style.color.with_alpha(text_alpha),
                        ..label_style.clone()
                    }
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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
    row_height: f32,
    scroll_y: f32,
    on_change: Option<Box<dyn FnMut(Vec<usize>, String)>>,
}

const TREE_DISCLOSURE_LEFT: f32 = 8.0;
const TREE_DISCLOSURE_SIZE: f32 = 12.0;
const TREE_DISCLOSURE_LABEL_GAP: f32 = 6.0;
const TREE_DEPTH_INDENT: f32 = 18.0;
const TREE_ROW_RIGHT_PADDING: f32 = 8.0;

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
            row_height: 30.0,
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
        self.row_height = row_height.max(24.0);
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
        let base = self
            .row_height
            .max((theme.metrics.min_height + 4.0).max(28.0));
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
        inset_rect(bounds, Insets::all(8.0))
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
}

impl Widget for TreeView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let viewport = self.viewport_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .map(|row| row.path);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
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
                self.pressed = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .map(|row| row.path);
                self.hovered = self.pressed.clone();
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
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
                    if disclosure_rect(row_rect, row.depth).contains(pointer.position) {
                        if self.toggle_path(&row.path) {
                            ctx.request_measure();
                        }
                    } else {
                        self.select_path(&row.path);
                    }
                }
                self.hovered = hovered_row.map(|row| row.path);
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
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
                let label_start = tree_label_offset(row.depth);
                let label = measure_text(ctx, &row.label, &label_style).width;
                let detail = row
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                label_start + label.max(detail) + TREE_ROW_RIGHT_PADDING
            })
            .fold(220.0, f32::max);
        let desired = Size::new(width + 16.0, self.content_height() + 16.0);
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

        draw_surface(ctx, ctx.bounds(), &theme, ctx.is_focused());
        ctx.push_clip_rect(viewport);

        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + viewport.height()) / row_height).ceil() as usize + 1)
            .min(rows.len());

        for index in start..end {
            let row = &rows[index];
            let y = viewport.y() + (index as f32 * row_height) - self.scroll_y;
            let row_rect = Rect::new(viewport.x(), y, viewport.width(), row_height);
            let selected = self.selected.as_deref() == Some(row.path.as_slice());
            let hovered = self.hovered.as_deref() == Some(row.path.as_slice());
            let pressed = self.pressed.as_deref() == Some(row.path.as_slice());

            if selected || hovered || pressed {
                if let Some(highlight) = row_highlight_rect(row_rect, viewport) {
                    ctx.fill_rect(
                        highlight,
                        if selected {
                            palette.selection
                        } else if pressed {
                            palette.control_active
                        } else {
                            palette.control_hover
                        },
                    );
                }
            }

            if row.has_children {
                ctx.fill(
                    disclosure_path(disclosure_rect(row_rect, row.depth), row.expanded),
                    if selected {
                        palette.border_focus
                    } else {
                        palette.placeholder
                    },
                );
            }

            let label_x = row_rect.x() + tree_label_offset(row.depth);
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
                } else if selected {
                    theme.text_style(palette.border_focus)
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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
}

impl TableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: None,
            min_width: 96.0,
            alignment: TableColumnAlignment::Start,
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
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    selected: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    row_height: f32,
    header_height: f32,
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
            columns: Vec::new(),
            rows: Vec::new(),
            selected: None,
            hovered: None,
            pressed: None,
            row_height: 28.0,
            header_height: 30.0,
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

    fn resolved_row_height(&self) -> f32 {
        let theme = self.resolved_theme();
        self.row_height
            .max((theme.metrics.min_height + 2.0).max(26.0))
    }

    fn resolved_header_height(&self) -> f32 {
        let theme = self.resolved_theme();
        self.header_height
            .max((theme.metrics.min_height + 4.0).max(28.0))
    }

    fn body_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + 8.0,
            bounds.y() + self.resolved_header_height() + 4.0,
            (bounds.width() - 16.0).max(0.0),
            (bounds.height() - self.resolved_header_height() - 12.0).max(0.0),
        )
    }

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.resolved_row_height()
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
        self.column_widths = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let measured_title = measure_text(ctx, &column.title, &header_style).width;
                let measured_cells = self
                    .rows
                    .iter()
                    .filter_map(|row| row.cells.get(index))
                    .map(|cell| measure_text(ctx, cell, &body_style).width)
                    .fold(0.0, f32::max);
                column
                    .width
                    .unwrap_or((measured_title.max(measured_cells) + 26.0).max(column.min_width))
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
}

impl Widget for Table {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let body = self.body_rect(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.row_at_position(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
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
                self.pressed = self.row_at_position(ctx.bounds(), pointer.position);
                self.hovered = self.pressed;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
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
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.rows.is_empty() {
                    return;
                }

                let current = self.selected.unwrap_or(0);
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
        self.resolve_column_widths(ctx, (desired_width - 20.0).max(0.0));
        let desired_height = self.resolved_header_height() + self.content_height() + 12.0;
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
        let header_style = theme.text_style(palette.placeholder);
        let body_style = theme.body_text_style();
        let selected_body_style = theme.text_style(palette.border_focus);
        let body = self.body_rect(ctx.bounds());
        let header = Rect::new(
            ctx.bounds().x() + 8.0,
            ctx.bounds().y() + 8.0,
            (ctx.bounds().width() - 16.0).max(0.0),
            self.resolved_header_height(),
        );
        let row_height = self.resolved_row_height();

        draw_surface(ctx, ctx.bounds(), &theme, ctx.is_focused());
        ctx.fill(rounded_rect_path(header, 6.0), palette.control);

        let mut x = header.x();
        for (index, column) in self.columns.iter().enumerate() {
            let width = *self.column_widths.get(index).unwrap_or(&column.min_width);
            let cell = Rect::new(x, header.y(), width, header.height());
            if index > 0 {
                ctx.stroke_rect(
                    Rect::new(cell.x(), cell.y() + 4.0, 1.0, cell.height() - 8.0),
                    palette.border,
                    sui_scene::StrokeStyle::new(1.0),
                );
            }
            draw_aligned_text(
                ctx,
                horizontal_inset_rect(cell, 8.0),
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
            let selected = self.selected == Some(row_index);
            let hovered = self.hovered == Some(row_index);
            let pressed = self.pressed == Some(row_index);
            let background = if selected {
                palette.selection
            } else if pressed {
                palette.control_active
            } else if hovered {
                palette.control_hover
            } else if row_index % 2 == 0 {
                palette.surface.with_alpha(0.88)
            } else {
                palette.surface_raised
            };
            ctx.fill_rect(row_rect, background);
            ctx.stroke_rect(
                row_rect,
                palette.border.with_alpha(0.55),
                sui_scene::StrokeStyle::new(1.0),
            );

            let mut cell_x = row_rect.x();
            for (column_index, column) in self.columns.iter().enumerate() {
                let width = *self
                    .column_widths
                    .get(column_index)
                    .unwrap_or(&column.min_width);
                let cell_rect = Rect::new(cell_x, row_rect.y(), width, row_rect.height());
                if let Some(value) = self.rows[row_index].cells.get(column_index) {
                    draw_aligned_text(
                        ctx,
                        horizontal_inset_rect(cell_rect, 8.0),
                        value,
                        &if selected {
                            selected_body_style.clone()
                        } else {
                            body_style.clone()
                        },
                        column.alignment,
                    );
                }
                cell_x += width;
            }
        }

        ctx.pop_clip();
        draw_vertical_scroll_thumb(
            ctx,
            body,
            self.content_height(),
            self.scroll_y,
            palette.border_hover,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Table, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = self
            .selected
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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
    items: Vec<BreadcrumbItem>,
    current: usize,
    focused_index: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    measured_widths: Vec<f32>,
    on_activate: Option<Box<dyn FnMut(usize, String)>>,
}

impl Breadcrumb {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            items: Vec::new(),
            current: 0,
            focused_index: 0,
            hovered: None,
            pressed: None,
            measured_widths: Vec::new(),
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
        self.focused_index = current;
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
        if self.items.is_empty() {
            0
        } else {
            self.current.min(self.items.len() - 1)
        }
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

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let widths = &self.measured_widths;
        if index >= widths.len() {
            return None;
        }
        let vertical_padding = 4.0;
        let mut x = bounds.x() + 10.0;
        for (current, width) in widths.iter().enumerate() {
            let rect = Rect::new(
                x,
                bounds.y() + vertical_padding,
                *width,
                (bounds.height() - vertical_padding * 2.0).max(0.0),
            );
            if current == index {
                return Some(rect);
            }
            x += *width + 20.0;
        }
        None
    }

    fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        (0..self.items.len()).find(|index| {
            self.item_rect(bounds, *index)
                .is_some_and(|rect| rect.contains(position))
        })
    }
}

impl Widget for Breadcrumb {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed = self.item_at(ctx.bounds(), pointer.position);
                self.hovered = self.pressed;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(pressed, hovered)| pressed == hovered)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                }
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
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
            .map(|item| measure_text(ctx, &item.label, &text_style).width + 22.0)
            .collect();
        let desired_width = self.measured_widths.iter().sum::<f32>()
            + (self.items.len().saturating_sub(1) as f32 * 20.0)
            + 20.0;
        constraints.clamp(Size::new(
            desired_width.max(180.0),
            theme.metrics.min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        draw_surface(ctx, ctx.bounds(), &theme, ctx.is_focused());

        for (index, item) in self.items.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let current = self.normalized_current() == index;
            let focused = ctx.is_focused() && self.focused_index == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);

            if current || hovered || pressed || focused {
                ctx.fill(
                    rounded_rect_path(rect, theme.metrics.corner_radius),
                    if current {
                        palette.selection
                    } else if pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
                    },
                );
            }

            let style = if current {
                theme.text_style(palette.border_focus)
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
                    rect.max_x() + 6.0,
                    rect.y() + ((rect.height() - 10.0) * 0.5),
                    10.0,
                    10.0,
                ));
                ctx.stroke(
                    separator,
                    palette.placeholder.with_alpha(0.9),
                    sui_scene::StrokeStyle::new(1.5),
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Breadcrumb, ctx.bounds());
        node.name = Some(self.name.clone());
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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

fn disclosure_rect(row: Rect, depth: usize) -> Rect {
    Rect::new(
        row.x() + TREE_DISCLOSURE_LEFT + depth as f32 * TREE_DEPTH_INDENT,
        row.y() + ((row.height() - TREE_DISCLOSURE_SIZE) * 0.5),
        TREE_DISCLOSURE_SIZE,
        TREE_DISCLOSURE_SIZE,
    )
}

fn tree_label_offset(depth: usize) -> f32 {
    TREE_DISCLOSURE_LEFT
        + depth as f32 * TREE_DEPTH_INDENT
        + TREE_DISCLOSURE_SIZE
        + TREE_DISCLOSURE_LABEL_GAP
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

fn draw_surface(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, focused: bool) {
    let palette = theme.palette;
    ctx.fill(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        palette.surface,
    );
    ctx.stroke(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        if focused {
            palette.border_focus
        } else {
            palette.border
        },
        sui_scene::StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );
}

fn draw_vertical_scroll_thumb(
    ctx: &mut PaintCtx,
    viewport: Rect,
    content_height: f32,
    scroll_y: f32,
    color: Color,
) {
    if content_height <= viewport.height() || viewport.height() <= 0.0 {
        return;
    }

    let ratio = (viewport.height() / content_height).clamp(0.08, 1.0);
    let thumb_height = (viewport.height() * ratio).max(28.0);
    let max_scroll = (content_height - viewport.height()).max(1.0);
    let thumb_y = viewport.y() + ((viewport.height() - thumb_height) * (scroll_y / max_scroll));
    ctx.fill(
        rounded_rect_path(
            Rect::new(viewport.max_x() - 6.0, thumb_y, 4.0, thumb_height),
            2.0,
        ),
        color.with_alpha(0.75),
    );
}

fn draw_aligned_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    alignment: TableColumnAlignment,
) {
    let measurement = paint_text_measurement(ctx, text, style);
    let estimated = estimate_text_width(text, style);
    let width = rect.width().max(estimated);
    let x = match alignment {
        TableColumnAlignment::Start => rect.x(),
        TableColumnAlignment::Center => rect.x() + ((rect.width() - estimated) * 0.5).max(0.0),
        TableColumnAlignment::End => rect.max_x() - estimated.max(0.0),
    };
    let height = style.line_height.min(rect.height());
    let y = vertically_centered_text_rect_y(ctx, rect, measurement, height);
    ctx.draw_text(
        Rect::new(x, y, width, height),
        text.to_string(),
        style.clone(),
    );
}

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
            let total_height = primary_line_height + secondary_line_height + 2.0;
            let top = rect.y() + ((rect.height() - total_height) * 0.5).max(0.0);
            let primary_rect = Rect::new(rect.x(), top, rect.width(), primary_line_height);
            let secondary_rect = Rect::new(
                rect.x(),
                top + primary_line_height + 2.0,
                rect.width(),
                secondary_line_height,
            );
            (
                Rect::new(
                    primary_rect.x(),
                    vertically_centered_text_rect_y(
                        ctx,
                        primary_rect,
                        primary_measurement,
                        primary_line_height,
                    ),
                    primary_rect.width(),
                    primary_rect.height(),
                ),
                Some(Rect::new(
                    secondary_rect.x(),
                    vertically_centered_text_rect_y(
                        ctx,
                        secondary_rect,
                        secondary_measurement.unwrap_or(primary_measurement),
                        secondary_line_height,
                    ),
                    secondary_rect.width(),
                    secondary_rect.height(),
                )),
            )
        }
        None => {
            let height = primary_line_height.min(rect.height());
            let y = vertically_centered_text_rect_y(ctx, rect, primary_measurement, height);
            (Rect::new(rect.x(), y, rect.width(), height), None)
        }
    }
}

fn two_line_row_height(primary_line_height: f32, secondary_line_height: f32) -> f32 {
    primary_line_height + secondary_line_height + 6.0
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

fn measure_list_item_leading_width(
    ctx: &mut MeasureCtx,
    item: &ListItem,
    style: &TextStyle,
) -> f32 {
    if item.leading_icon.is_some() {
        return LIST_ROW_LEADING_ICON_SIZE + LIST_ROW_LEADING_GAP;
    }
    item.leading_text
        .as_deref()
        .map(|text| measure_text(ctx, text, style).width + LIST_ROW_LEADING_GAP)
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

fn vertically_centered_text_rect_y(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: TextMeasurement,
    height: f32,
) -> f32 {
    let optical_centering = window_render_options(ctx.window_id())
        .map(|options| options.optical_vertical_text_alignment_enabled)
        .unwrap_or(true);
    let top = if optical_centering {
        -measurement.cap_height.unwrap_or(measurement.ascent)
    } else {
        -measurement.ascent
    };
    let bottom = if optical_centering {
        measurement.descent * 0.5
    } else {
        measurement.descent
    };
    let visual_center = (top + bottom) * 0.5;
    let baseline = rect.y() + (rect.height() * 0.5) - visual_center;
    let leading_above = ((height - (measurement.ascent + measurement.descent)).max(0.0)) * 0.5;
    baseline - measurement.ascent - leading_above
}

fn caption_style(theme: &DefaultTheme) -> TextStyle {
    TextStyle {
        font_size: (theme.typography.body_font_size - 1.0).max(11.0),
        line_height: (theme.typography.body_line_height - 2.0).max(14.0),
        color: theme.palette.placeholder,
        ..TextStyle::default()
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
    hovered: bool,
    pressed: bool,
) {
    let palette = theme.palette;
    if hovered || pressed {
        ctx.fill(
            rounded_rect_path(rect, theme.metrics.corner_radius.min(rect.height() * 0.35)),
            if pressed {
                palette.control_active
            } else {
                palette.control_hover
            },
        );
    }

    let icon = inset_rect(rect, Insets::all(5.0));
    let color = if visible {
        palette.border_focus
    } else {
        palette.placeholder
    };
    ctx.stroke(
        layer_visibility_eye_path(icon),
        color,
        sui_scene::StrokeStyle::new(1.4),
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
            sui_scene::StrokeStyle::new(1.6),
        );
    }
}

fn paint_layer_lock_button(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    locked: bool,
    hovered: bool,
    pressed: bool,
) {
    let palette = theme.palette;
    if hovered || pressed {
        ctx.fill(
            rounded_rect_path(rect, theme.metrics.corner_radius.min(rect.height() * 0.35)),
            if pressed {
                palette.control_active
            } else {
                palette.control_hover
            },
        );
    }

    draw_icon_glyph(
        ctx,
        if locked {
            IconGlyph::Lock
        } else {
            IconGlyph::Unlock
        },
        inset_rect(rect, Insets::all(4.0)),
        if locked {
            palette.border_focus
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
    let radius = theme.metrics.corner_radius.min(4.0);
    ctx.fill(rounded_rect_path(rect, radius), palette.control_hover);
    let fill = inset_rect(rect, Insets::all(2.0));
    ctx.fill(
        rounded_rect_path(fill, (radius - 1.0).max(0.0)),
        if visible {
            color
        } else {
            color.with_alpha(color.alpha * 0.36)
        },
    );
    ctx.stroke(
        rounded_rect_path(rect, radius),
        palette.border.with_alpha(if visible { 1.0 } else { 0.55 }),
        sui_scene::StrokeStyle::new(1.0),
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
        Breadcrumb, BreadcrumbItem, DefaultTheme, LayerList, LayerListItem, ListItem, ListView,
        TREE_DISCLOSURE_LABEL_GAP, Table, TableColumn, TableRow, TreeItem, TreeView,
    };
    use crate::{Button, Label, ScrollView, SizedBox, Stack};
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons,
        PointerEvent, PointerEventKind, PointerKind, Rect, Result, ScrollDelta, SemanticsAction,
        SemanticsRole, SemanticsValue, Size, ToggleState, Vector, WidgetId, WindowEvent,
    };
    use sui_runtime::{Application, RenderOutput, Runtime, Widget, WindowBuilder};
    use sui_scene::{Brush, SceneCommand};
    use sui_text::{FontRegistry, TextSystem};

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
                        rects.push(Rect::new(
                            run.origin.x,
                            run.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ));
                    }
                }
                _ => {}
            });

        rects
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
                        runs.push(sui_text::TextRun {
                            rect: Rect::new(
                                run.origin.x,
                                run.origin.y,
                                layout.box_size().width,
                                layout.box_size().height,
                            ),
                            text: layout.text().to_string(),
                            style: layout.style().clone(),
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
        let selected_brush = Brush::Solid(DefaultTheme::default().palette.selection);
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

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
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
            theme.palette.border_focus
        );
        assert!(solid_fill_colors(&breadcrumb).contains(&theme.palette.surface));
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
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(320.0).height(100.0).with_child(
                ListView::new("Assets")
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

        assert!(highlight.y() >= 8.0);
        assert!(highlight.max_y() <= 100.0 - 8.0);
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

        assert!(highlight.y() >= 8.0);
        assert!(highlight.max_y() <= 120.0 - 8.0);
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
        assert!(disclosure.max_x() + TREE_DISCLOSURE_LABEL_GAP <= label.x());
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

        let label = text_rects_for(&output, "Workspace")[0];
        let theme = DefaultTheme::default();
        let line_height = theme.body_text_style().line_height;
        let available_height = (theme.metrics.min_height - 8.0).max(0.0);

        assert!((label.height() - line_height.min(available_height)).abs() < 0.001);
        assert!(label.width() > 0.0);
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
}
