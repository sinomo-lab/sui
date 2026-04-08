use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_text::{TextMeasurement, TextStyle};

use crate::DefaultTheme;

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    label: String,
    detail: Option<String>,
    accent: Option<Color>,
    disabled: bool,
}

impl ListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            accent: None,
            disabled: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
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

    pub fn label(&self) -> &str {
        &self.label
    }
}

pub struct ListView {
    theme: Box<DefaultTheme>,
    name: String,
    items: Vec<ListItem>,
    selected: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    row_height: f32,
    scroll_y: f32,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl ListView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            items: Vec::new(),
            selected: None,
            hovered: None,
            pressed: None,
            row_height: 28.0,
            scroll_y: 0.0,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self.selected
    }

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .max((self.theme.metrics.min_height + 4.0).max(28.0))
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, Insets::all(8.0))
    }

    fn content_height(&self) -> f32 {
        self.items.len() as f32 * self.resolved_row_height()
    }

    fn clamp_scroll(&self, viewport_height: f32, scroll_y: f32) -> f32 {
        let max_scroll = (self.content_height() - viewport_height).max(0.0);
        scroll_y.clamp(0.0, max_scroll)
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let viewport = self.viewport_rect(bounds);
        if !viewport.contains(position) {
            return None;
        }

        let y = position.y - viewport.y() + self.scroll_y;
        let index = (y / self.resolved_row_height()).floor() as usize;
        (index < self.items.len()).then_some(index)
    }

    fn ensure_visible(&mut self, viewport_height: f32, index: usize) {
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
        let text_style = self.theme.body_text_style();
        let detail_style = caption_style(self.theme.as_ref());
        let content_width = self
            .items
            .iter()
            .map(|item| {
                let label = measure_text(ctx, &item.label, &text_style).width;
                let detail = item
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                label.max(detail) + 28.0
            })
            .fold(220.0, f32::max);
        let desired = Size::new(content_width + 16.0, self.content_height() + 16.0);
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
        let palette = self.theme.palette;
        let viewport = self.viewport_rect(ctx.bounds());
        let row_height = self.resolved_row_height();
        let label_style = self.theme.body_text_style();
        let detail_style = caption_style(self.theme.as_ref());

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
        ctx.push_clip_rect(viewport);

        let start = (self.scroll_y / row_height).floor().max(0.0) as usize;
        let end = (((self.scroll_y + viewport.height()) / row_height).ceil() as usize + 1)
            .min(self.items.len());

        for index in start..end {
            let y = viewport.y() + (index as f32 * row_height) - self.scroll_y;
            let row = Rect::new(viewport.x(), y, viewport.width(), row_height);
            let selected = self.selected == Some(index);
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);

            if selected || hovered || pressed {
                ctx.fill(
                    rounded_rect_path(inset_rect(row, Insets::all(1.0)), 6.0),
                    if selected {
                        palette.accent.with_alpha(0.14)
                    } else if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
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

            let text_x = row.x() + 14.0;
            let label_rect = Rect::new(text_x, row.y() + 5.0, row.width() - 22.0, 16.0);
            ctx.draw_text(
                label_rect,
                item.label.clone(),
                if item.disabled {
                    self.theme.text_style(palette.placeholder)
                } else if selected {
                    self.theme.text_style(palette.border_focus)
                } else {
                    label_style.clone()
                },
            );
            if let Some(detail) = &item.detail {
                ctx.draw_text(
                    Rect::new(
                        text_x,
                        row.y() + row.height() - 16.0,
                        row.width() - 22.0,
                        14.0,
                    ),
                    detail.clone(),
                    detail_style.clone(),
                );
            }
        }

        ctx.pop_clip();
        draw_vertical_scroll_thumb(
            ctx,
            viewport,
            self.content_height(),
            self.scroll_y,
            palette.border_hover,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = self
            .selected
            .and_then(|index| self.items.get(index))
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
    name: String,
    items: Vec<TreeItem>,
    selected: Option<Vec<usize>>,
    hovered: Option<Vec<usize>>,
    pressed: Option<Vec<usize>>,
    row_height: f32,
    scroll_y: f32,
    on_change: Option<Box<dyn FnMut(Vec<usize>, String)>>,
}

impl TreeView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
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

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .max((self.theme.metrics.min_height + 4.0).max(28.0))
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
        let label_style = self.theme.body_text_style();
        let detail_style = caption_style(self.theme.as_ref());
        let row_padding = 34.0;
        let width = self
            .visible_rows()
            .iter()
            .map(|row| {
                let indent = row.depth as f32 * 18.0;
                let label = measure_text(ctx, &row.label, &label_style).width;
                let detail = row
                    .detail
                    .as_deref()
                    .map(|detail| measure_text(ctx, detail, &detail_style).width)
                    .unwrap_or(0.0);
                indent + label.max(detail) + row_padding
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
        let palette = self.theme.palette;
        let viewport = self.viewport_rect(ctx.bounds());
        let row_height = self.resolved_row_height();
        let rows = self.visible_rows();

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
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
                ctx.fill(
                    rounded_rect_path(inset_rect(row_rect, Insets::all(1.0)), 6.0),
                    if selected {
                        palette.accent.with_alpha(0.14)
                    } else if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
            }

            let indent = row.depth as f32 * 16.0;
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

            let label_x = row_rect.x() + 16.0 + indent;
            ctx.draw_text(
                Rect::new(
                    label_x,
                    row_rect.y() + 5.0,
                    row_rect.width() - label_x,
                    16.0,
                ),
                row.label.clone(),
                if row.disabled {
                    self.theme.text_style(palette.placeholder)
                } else if selected {
                    self.theme.text_style(palette.border_focus)
                } else {
                    self.theme.body_text_style()
                },
            );
            if let Some(detail) = &row.detail {
                ctx.draw_text(
                    Rect::new(
                        label_x,
                        row_rect.y() + row_rect.height() - 16.0,
                        row_rect.width() - label_x,
                        14.0,
                    ),
                    detail.clone(),
                    caption_style(self.theme.as_ref()),
                );
            }
        }

        ctx.pop_clip();
        draw_vertical_scroll_thumb(
            ctx,
            viewport,
            self.content_height(),
            self.scroll_y,
            palette.border_hover,
        );
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

    fn resolved_row_height(&self) -> f32 {
        self.row_height
            .max((self.theme.metrics.min_height + 2.0).max(26.0))
    }

    fn resolved_header_height(&self) -> f32 {
        self.header_height
            .max((self.theme.metrics.min_height + 4.0).max(28.0))
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
        let header_style = self.theme.text_style(self.theme.palette.placeholder);
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
                    .map(|cell| measure_text(ctx, cell, &self.theme.body_text_style()).width)
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
                if pointer.kind == PointerEventKind::Scroll
                    && body.contains(pointer.position) =>
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
        let palette = self.theme.palette;
        let header_style = self.theme.text_style(palette.placeholder);
        let body = self.body_rect(ctx.bounds());
        let header = Rect::new(
            ctx.bounds().x() + 8.0,
            ctx.bounds().y() + 8.0,
            (ctx.bounds().width() - 16.0).max(0.0),
            self.resolved_header_height(),
        );
        let row_height = self.resolved_row_height();

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
        ctx.fill(
            rounded_rect_path(header, 6.0),
            Color::rgba(0.95, 0.965, 0.985, 1.0),
        );

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
                inset_rect(cell, Insets::all(8.0)),
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
                palette.accent.with_alpha(0.14)
            } else if pressed {
                palette.surface_pressed
            } else if hovered {
                palette.surface_hover
            } else if row_index % 2 == 0 {
                palette.surface.with_alpha(0.88)
            } else {
                Color::rgba(0.985, 0.989, 0.997, 1.0)
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
                        inset_rect(cell_rect, Insets::all(8.0)),
                        value,
                        &if selected {
                            self.theme.text_style(palette.border_focus)
                        } else {
                            self.theme.body_text_style()
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
        let mut x = bounds.x() + 10.0;
        for (current, width) in widths.iter().enumerate() {
            let rect = Rect::new(x, bounds.y() + 8.0, *width, bounds.height() - 16.0);
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
        let text_style = self.theme.body_text_style();
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
            self.theme.metrics.min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());

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
                    rounded_rect_path(rect, self.theme.metrics.corner_radius),
                    if current {
                        palette.accent.with_alpha(0.14)
                    } else if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
            }

            ctx.draw_text(
                inset_rect(rect, Insets::all(8.0)),
                item.label.clone(),
                if current {
                    self.theme.text_style(palette.border_focus)
                } else {
                    self.theme.body_text_style()
                },
            );

            if index + 1 < self.items.len() {
                let separator = chevron_path(Rect::new(
                    rect.max_x() + 6.0,
                    rect.y() + 8.0,
                    10.0,
                    rect.height() - 16.0,
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
        row.x() + 8.0 + depth as f32 * 18.0,
        row.y() + ((row.height() - 12.0) * 0.5),
        12.0,
        12.0,
    )
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
    ctx.fill(rounded_rect_path(rect, theme.metrics.corner_radius), palette.surface);
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
    let estimated = estimate_text_width(text, style);
    let width = rect.width().max(estimated);
    let x = match alignment {
        TableColumnAlignment::Start => rect.x(),
        TableColumnAlignment::Center => rect.x() + ((rect.width() - estimated) * 0.5).max(0.0),
        TableColumnAlignment::End => rect.max_x() - estimated.max(0.0),
    };
    ctx.draw_text(
        Rect::new(x, rect.y(), width, rect.height()),
        text.to_string(),
        style.clone(),
    );
}

fn estimate_text_width(text: &str, style: &TextStyle) -> f32 {
    text.chars().count() as f32 * style.font_size * 0.56
}

fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
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
        Breadcrumb, BreadcrumbItem, ListItem, ListView, Table, TableColumn, TableRow, TreeItem,
        TreeView,
    };
    use crate::{ScrollView, SizedBox, Stack};
    use sui_core::{
        Event, KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons,
        PointerEvent, PointerEventKind, PointerKind, Result, ScrollDelta, SemanticsRole,
        SemanticsValue, Size, Vector,
    };
    use sui_runtime::{Application, Runtime, Widget, WindowBuilder};

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
    fn non_scrollable_table_allows_wheel_to_bubble_to_parent_scroll_view() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(220.0, 120.0)).with_child(ScrollView::vertical(
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
}
