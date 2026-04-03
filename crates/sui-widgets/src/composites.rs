use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, Size,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{
    EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, SingleChild, Widget, WidgetChildren,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::StrokeStyle;
use sui_text::{TextMeasurement, TextStyle};

use crate::{Button, ControlMetrics, DefaultTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    label: String,
    shortcut: Option<String>,
    enabled: bool,
    destructive: bool,
    separator_before: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            enabled: true,
            destructive: false,
            separator_before: false,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn separator_before(mut self) -> Self {
        self.separator_before = true;
        self
    }

    fn text_color(&self, theme: DefaultTheme) -> Color {
        if !self.enabled {
            theme.palette.placeholder
        } else if self.destructive {
            Color::rgba(0.74, 0.18, 0.18, 1.0)
        } else {
            theme.palette.text
        }
    }
}

pub struct TabBar {
    theme: DefaultTheme,
    name: String,
    tabs: Vec<String>,
    selected: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    gap: f32,
    widths: Vec<f32>,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl TabBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            tabs: Vec::new(),
            selected: 0,
            hovered: None,
            pressed: None,
            gap: 6.0,
            widths: Vec::new(),
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn tab(mut self, label: impl Into<String>) -> Self {
        self.tabs.push(label.into());
        self
    }

    pub fn tabs<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tabs.extend(labels.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    pub fn current_tab(&self) -> Option<&str> {
        self.tabs
            .get(self.normalized_selected())
            .map(String::as_str)
    }

    fn normalized_selected(&self) -> usize {
        if self.tabs.is_empty() {
            0
        } else {
            self.selected.min(self.tabs.len() - 1)
        }
    }

    fn activate(&mut self, index: usize) {
        if self.tabs.is_empty() {
            return;
        }

        let index = index.min(self.tabs.len() - 1);
        if self.selected != index {
            self.selected = index;
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.tabs[index].clone());
            }
        }
    }

    fn tab_height(&self) -> f32 {
        self.theme.metrics.min_height
    }

    fn measured_widths(&self) -> &[f32] {
        &self.widths
    }

    fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() || self.measured_widths().len() != self.tabs.len() {
            return None;
        }

        let base_total =
            self.widths.iter().sum::<f32>() + (self.gap * self.tabs.len().saturating_sub(1) as f32);
        let extra_per_tab = if bounds.width() > base_total && !self.tabs.is_empty() {
            (bounds.width() - base_total) / self.tabs.len() as f32
        } else {
            0.0
        };

        let mut x = bounds.x();
        for (current, width) in self.widths.iter().enumerate() {
            let width = *width + extra_per_tab;
            let rect = Rect::new(x, bounds.y(), width, self.tab_height());
            if current == index {
                return Some(rect);
            }
            x += width + self.gap;
        }

        None
    }

    fn tab_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.tabs.iter().enumerate().find_map(|(index, _)| {
            self.tab_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn move_selection(&mut self, delta: isize) {
        if self.tabs.is_empty() {
            return;
        }

        let selected = self.normalized_selected() as isize;
        let last = self.tabs.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next);
        self.hovered = Some(next);
    }
}

impl Widget for TabBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
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
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
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
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1),
                    "Home" => self.activate(0),
                    "End" if !self.tabs.is_empty() => self.activate(self.tabs.len() - 1),
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let style = self.theme.body_text_style();
        self.widths = self
            .tabs
            .iter()
            .map(|tab| {
                let measurement = measure_text(ctx, tab, &style);
                (measurement.width + 28.0).max(96.0)
            })
            .collect();

        let width =
            self.widths.iter().sum::<f32>() + (self.gap * self.tabs.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width.max(160.0), self.tab_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;

        ctx.fill(
            rounded_rect_path(ctx.bounds(), metrics.corner_radius),
            Color::rgba(0.93, 0.95, 0.98, 1.0),
        );

        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let background = if selected {
                palette.surface
            } else if pressed {
                palette.surface_pressed
            } else if hovered {
                palette.surface_hover
            } else {
                Color::rgba(0.0, 0.0, 0.0, 0.0)
            };
            let border = if selected {
                palette.border_focus
            } else if hovered {
                palette.border_hover
            } else {
                Color::rgba(0.78, 0.83, 0.90, 0.0)
            };

            if selected || hovered || pressed {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            ctx.draw_text(
                inset_rect(rect, Insets::all(10.0)),
                tab.clone(),
                if selected {
                    self.theme.text_style(palette.border_focus)
                } else {
                    self.theme.body_text_style()
                },
            );

            if selected {
                let accent = Rect::new(
                    rect.x() + 14.0,
                    rect.max_y() - 3.0,
                    rect.width() - 28.0,
                    3.0,
                );
                ctx.fill(rounded_rect_path(accent, 1.5), palette.accent);
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TabBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .current_tab()
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
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

pub struct Tabs {
    theme: DefaultTheme,
    name: String,
    labels: Vec<String>,
    panels: WidgetChildren,
    selected: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    widths: Vec<f32>,
    gap: f32,
    panel_gap: f32,
    panel_frame: Rect,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl Tabs {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            labels: Vec::new(),
            panels: WidgetChildren::new(),
            selected: 0,
            hovered: None,
            pressed: None,
            widths: Vec::new(),
            gap: 6.0,
            panel_gap: 12.0,
            panel_frame: Rect::ZERO,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn tab<W>(mut self, label: impl Into<String>, panel: W) -> Self
    where
        W: Widget + 'static,
    {
        self.labels.push(label.into());
        self.panels.push(panel);
        self
    }

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    pub fn current_tab(&self) -> Option<&str> {
        self.labels
            .get(self.normalized_selected())
            .map(String::as_str)
    }

    fn normalized_selected(&self) -> usize {
        if self.labels.is_empty() {
            0
        } else {
            self.selected.min(self.labels.len() - 1)
        }
    }

    fn header_height(&self) -> f32 {
        self.theme.metrics.min_height
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), self.header_height())
    }

    fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.labels.len() || self.widths.len() != self.labels.len() {
            return None;
        }

        let header = self.header_rect(bounds);
        let base_total = self.widths.iter().sum::<f32>()
            + (self.gap * self.labels.len().saturating_sub(1) as f32);
        let extra_per_tab = if header.width() > base_total && !self.labels.is_empty() {
            (header.width() - base_total) / self.labels.len() as f32
        } else {
            0.0
        };

        let mut x = header.x();
        for (current, width) in self.widths.iter().enumerate() {
            let rect = Rect::new(x, header.y(), *width + extra_per_tab, header.height());
            if current == index {
                return Some(rect);
            }
            x += rect.width() + self.gap;
        }

        None
    }

    fn tab_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.labels.iter().enumerate().find_map(|(index, _)| {
            self.tab_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn select(&mut self, index: usize) {
        if self.labels.is_empty() {
            return;
        }

        let index = index.min(self.labels.len() - 1);
        if self.selected != index {
            self.selected = index;
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.labels[index].clone());
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.labels.is_empty() {
            return;
        }

        let next = (self.normalized_selected() as isize + delta)
            .clamp(0, self.labels.len() as isize - 1) as usize;
        self.hovered = Some(next);
        self.select(next);
    }

    fn selected_panel(&self) -> Option<&sui_runtime::WidgetPod> {
        self.panels.as_slice().get(self.normalized_selected())
    }

    fn selected_panel_mut(&mut self) -> Option<&mut sui_runtime::WidgetPod> {
        let index = self.normalized_selected();
        self.panels.as_mut_slice().get_mut(index)
    }
}

impl Widget for Tabs {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.header_rect(ctx.bounds()).contains(pointer.position) =>
            {
                self.hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
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
                if self.pressed.is_some() {
                    let hovered = self.tab_at(ctx.bounds(), pointer.position);
                    if let Some(index) = self
                        .pressed
                        .zip(hovered)
                        .filter(|(left, right)| left == right)
                        .map(|(index, _)| index)
                    {
                        self.select(index);
                        ctx.request_layout();
                    }
                    self.hovered = hovered;
                    self.pressed = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
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
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1),
                    "Home" => self.select(0),
                    "End" if !self.labels.is_empty() => self.select(self.labels.len() - 1),
                    _ => return,
                }
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let text_style = self.theme.body_text_style();
        self.widths = self
            .labels
            .iter()
            .map(|label| (measure_text(ctx, label, &text_style).width + 28.0).max(96.0))
            .collect();

        let header_width = self.widths.iter().sum::<f32>()
            + (self.gap * self.labels.len().saturating_sub(1) as f32);
        let available_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(header_width)
        } else {
            header_width.max(320.0)
        };
        let header_height = self.header_height();
        let padding = Insets::all(16.0);

        let panel_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (available_width - padding.left - padding.right).max(0.0),
                if constraints.max.height.is_finite() {
                    (constraints.max.height
                        - header_height
                        - self.panel_gap
                        - padding.top
                        - padding.bottom)
                        .max(0.0)
                } else {
                    f32::INFINITY
                },
            ),
        );

        let panel_origin = Point::new(padding.left, header_height + self.panel_gap + padding.top);
        let panel_size = if let Some(panel) = self.selected_panel_mut() {
            panel.layout_at(ctx, panel_constraints, panel_origin)
        } else {
            Size::new(0.0, self.theme.metrics.min_height)
        };

        let content_width = (panel_size.width + padding.left + padding.right).max(available_width);
        let content_height = panel_size.height + padding.top + padding.bottom;
        self.panel_frame = Rect::new(
            0.0,
            header_height + self.panel_gap,
            content_width,
            content_height,
        );

        constraints.clamp(Size::new(
            content_width,
            header_height + self.panel_gap + content_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let header = self.header_rect(ctx.bounds());

        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            Color::rgba(0.93, 0.95, 0.98, 1.0),
        );

        for (index, label) in self.labels.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);

            if selected || hovered || pressed {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    if selected {
                        palette.surface
                    } else if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                    if selected {
                        palette.border_focus
                    } else {
                        palette.border_hover
                    },
                );
            }

            ctx.draw_text(
                inset_rect(rect, Insets::all(10.0)),
                label.clone(),
                if selected {
                    self.theme.text_style(palette.border_focus)
                } else {
                    self.theme.body_text_style()
                },
            );
        }

        let content = self.panel_frame.translate(ctx.bounds().origin.to_vector());
        draw_control_frame(
            ctx,
            content,
            metrics.corner_radius + 2.0,
            metrics,
            palette.surface,
            palette.border,
            None,
        );
        if let Some(panel) = self.selected_panel() {
            panel.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tabs, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .current_tab()
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
        if let Some(panel) = self.selected_panel() {
            panel.semantics(ctx);
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
        if let Some(panel) = self.selected_panel() {
            visitor.visit(panel);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(panel) = self.selected_panel_mut() {
            visitor.visit(panel);
        }
    }
}

pub struct Menu {
    theme: DefaultTheme,
    name: String,
    items: Vec<MenuItem>,
    highlighted: Option<usize>,
    pressed: Option<usize>,
    measured_width: f32,
    on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
}

impl Menu {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            items: Vec::new(),
            highlighted: None,
            pressed: None,
            measured_width: 220.0,
            on_activate: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn on_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, MenuItem) + 'static,
    {
        self.on_activate = Some(Box::new(on_activate));
        self
    }

    fn row_height(&self) -> f32 {
        (self.theme.metrics.min_height - 4.0).max(32.0)
    }

    fn activate(&mut self, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if !item.enabled {
            return;
        }
        if let Some(on_activate) = &mut self.on_activate {
            on_activate(index, item);
        }
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let x = bounds.x() + 8.0;
        let y = bounds.y() + 8.0 + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (bounds.width() - 16.0).max(0.0),
            self.row_height(),
        ))
    }

    fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.items.iter().enumerate().find_map(|(index, _)| {
            self.item_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn move_highlight(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }

        let len = self.items.len() as isize;
        let start = self.highlighted.unwrap_or(0) as isize;
        let mut index = (start + delta).clamp(0, len - 1);
        while !self.items[index as usize].enabled {
            let next = (index + delta).clamp(0, len - 1);
            if next == index {
                break;
            }
            index = next;
        }
        self.highlighted = Some(index as usize);
    }
}

impl Widget for Menu {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if highlighted != self.highlighted {
                    self.highlighted = highlighted;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.highlighted = self.item_at(ctx.bounds(), pointer.position);
                self.pressed = self
                    .highlighted
                    .filter(|index| self.items.get(*index).is_some_and(|item| item.enabled));
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
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(highlighted)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                }
                self.highlighted = highlighted;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowDown" => self.move_highlight(1),
                    "ArrowUp" => self.move_highlight(-1),
                    "Home" => {
                        self.highlighted = self.items.iter().position(|item| item.enabled);
                    }
                    "End" => {
                        self.highlighted = self.items.iter().rposition(|item| item.enabled);
                    }
                    "Enter" | " " => {
                        if let Some(index) = self.highlighted {
                            self.activate(index);
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

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let label_style = self.theme.body_text_style();
        let shortcut_style = self.theme.placeholder_text_style();
        let mut width: f32 = 0.0;
        for item in &self.items {
            let label = measure_text(ctx, item.label(), &label_style).width;
            let shortcut = item
                .shortcut
                .as_ref()
                .map(|text| measure_text(ctx, text, &shortcut_style).width)
                .unwrap_or(0.0);
            width = width.max(label + shortcut + 64.0);
        }
        self.measured_width = width.max(220.0);
        let height = 16.0 + (self.row_height() * self.items.len() as f32);
        constraints.clamp(Size::new(
            self.measured_width,
            height.max(self.row_height() + 16.0),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius + 2.0,
            metrics,
            palette.surface,
            palette.border,
            ctx.is_focused().then_some(palette.focus_ring),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(row.x(), row.y() - 4.0, row.width(), 1.0);
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let pressed = self.pressed == Some(index);
            if highlighted || pressed {
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
            }

            ctx.draw_text(
                Rect::new(
                    row.x() + 12.0,
                    row.y() + 8.0,
                    row.width() - 24.0,
                    row.height() - 16.0,
                ),
                item.label.clone(),
                self.theme.text_style(item.text_color(self.theme)),
            );

            if let Some(shortcut) = &item.shortcut {
                ctx.draw_text(
                    Rect::new(
                        row.max_x() - 120.0,
                        row.y() + 8.0,
                        108.0,
                        row.height() - 16.0,
                    ),
                    shortcut.clone(),
                    self.theme.placeholder_text_style(),
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut state = SemanticsState::default();
        state.focused = ctx.is_focused();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Menu, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state = state;
        node.value = self
            .highlighted
            .and_then(|index| self.items.get(index))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::SetValue,
            SemanticsAction::Activate,
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

pub struct Tooltip {
    theme: DefaultTheme,
    text: String,
    placement: TooltipPlacement,
    child: SingleChild,
    hovered: bool,
    measurement: Option<TextMeasurement>,
}

impl Tooltip {
    pub fn new<W>(text: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: DefaultTheme::default(),
            text: text.into(),
            placement: TooltipPlacement::Above,
            child: SingleChild::new(child),
            hovered: false,
            measurement: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn placement(mut self, placement: TooltipPlacement) -> Self {
        self.placement = placement;
        self
    }

    fn bubble_rect(&self, bounds: Rect) -> Rect {
        let measurement = self.measurement.unwrap_or(TextMeasurement {
            width: 120.0,
            height: self.theme.typography.body_line_height,
            bounds: Rect::new(0.0, 0.0, 120.0, self.theme.typography.body_line_height),
        });
        let width = (measurement.width + 24.0).max(96.0);
        let height = measurement
            .height
            .max(self.theme.typography.body_line_height)
            + 18.0;
        let x = bounds.x() + ((bounds.width() - width) * 0.5);
        let y = match self.placement {
            TooltipPlacement::Above => bounds.y() - height - 10.0,
            TooltipPlacement::Below => bounds.max_y() + 10.0,
        };
        Rect::new(x, y, width, height)
    }
}

impl Widget for Tooltip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                if !self.hovered {
                    self.hovered = true;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                if self.hovered {
                    self.hovered = false;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        self.measurement = Some(measure_text(
            ctx,
            &self.text,
            &self.theme.placeholder_text_style(),
        ));
        self.child.layout_at(ctx, constraints, Point::ZERO)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        if !self.hovered {
            return;
        }

        let bubble = self.bubble_rect(ctx.bounds());
        let metrics = self.theme.metrics;
        draw_control_frame(
            ctx,
            bubble,
            metrics.corner_radius,
            metrics,
            Color::rgba(0.10, 0.14, 0.20, 0.96),
            Color::rgba(0.05, 0.08, 0.12, 1.0),
            None,
        );
        let tail = tooltip_tail(ctx.bounds(), bubble, self.placement);
        ctx.fill(tail, Color::rgba(0.10, 0.14, 0.20, 0.96));
        ctx.draw_text(
            inset_rect(bubble, Insets::all(9.0)),
            self.text.clone(),
            self.theme.text_style(Color::rgba(1.0, 1.0, 1.0, 1.0)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
        if self.hovered {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Tooltip,
                self.bubble_rect(ctx.bounds()),
            );
            node.name = Some(self.text.clone());
            ctx.push(node);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub struct Popover {
    theme: DefaultTheme,
    name: String,
    trigger: SingleChild,
    content: SingleChild,
    open: bool,
    gap: f32,
    padding: Insets,
    frame_rect: Rect,
}

impl Popover {
    pub fn new<T, C>(name: impl Into<String>, trigger: T, content: C) -> Self
    where
        T: Widget + 'static,
        C: Widget + 'static,
    {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            trigger: SingleChild::new(trigger),
            content: SingleChild::new(content),
            open: false,
            gap: 8.0,
            padding: Insets::all(14.0),
            frame_rect: Rect::ZERO,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    fn content_rect(&self) -> Rect {
        self.frame_rect
    }

    fn is_inside_open_regions(&self, position: Point) -> bool {
        self.trigger_rect().contains(position)
            || (self.open && self.content_rect().contains(position))
    }
}

impl Widget for Popover {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.trigger_rect().contains(pointer.position) =>
            {
                self.open = !self.open;
                ctx.request_focus();
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open
                    && !self.is_inside_open_regions(pointer.position) =>
            {
                self.open = false;
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && key.key == "Escape"
                    && self.open =>
            {
                self.open = false;
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let trigger_size = self
            .trigger
            .layout_at(ctx, constraints.loosen(), Point::ZERO);
        let mut size = trigger_size;
        if self.open {
            let content_constraints = Constraints::new(
                Size::ZERO,
                Size::new(
                    if constraints.max.width.is_finite() {
                        (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
                    } else {
                        f32::INFINITY
                    },
                    if constraints.max.height.is_finite() {
                        (constraints.max.height
                            - trigger_size.height
                            - self.gap
                            - self.padding.top
                            - self.padding.bottom)
                            .max(0.0)
                    } else {
                        f32::INFINITY
                    },
                ),
            );
            let content_origin = Point::new(
                self.padding.left,
                trigger_size.height + self.gap + self.padding.top,
            );
            let content_size = self
                .content
                .layout_at(ctx, content_constraints, content_origin);
            self.frame_rect = Rect::new(
                0.0,
                trigger_size.height + self.gap,
                (content_size.width + self.padding.left + self.padding.right)
                    .max(trigger_size.width),
                content_size.height + self.padding.top + self.padding.bottom,
            );
            size = Size::new(
                self.frame_rect.width().max(trigger_size.width),
                trigger_size.height + self.gap + self.frame_rect.height(),
            );
        } else {
            self.frame_rect = Rect::ZERO;
        }
        constraints.clamp(size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if !self.open {
            return;
        }

        let rect = self
            .content_rect()
            .translate(ctx.bounds().origin.to_vector());
        let metrics = self.theme.metrics;
        draw_control_frame(
            ctx,
            rect,
            metrics.corner_radius + 2.0,
            metrics,
            self.theme.palette.surface,
            self.theme.palette.border,
            Some(self.theme.palette.focus_ring),
        );
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Popover, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.open);
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
        ];
        ctx.push(node);
        self.trigger.semantics(ctx);
        if self.open {
            self.content.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.open {
            self.open = false;
            ctx.request_layout();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.open {
            self.content.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.open {
            self.content.visit_children_mut(visitor);
        }
    }
}

pub struct ContextMenu {
    theme: DefaultTheme,
    name: String,
    trigger: SingleChild,
    items: Vec<MenuItem>,
    open: bool,
    highlighted: Option<usize>,
    pressed: Option<usize>,
    frame_rect: Rect,
    on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
}

impl ContextMenu {
    pub fn new<W>(name: impl Into<String>, trigger: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            trigger: SingleChild::new(trigger),
            items: Vec::new(),
            open: false,
            highlighted: None,
            pressed: None,
            frame_rect: Rect::ZERO,
            on_activate: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn on_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, MenuItem) + 'static,
    {
        self.on_activate = Some(Box::new(on_activate));
        self
    }

    fn row_height(&self) -> f32 {
        (self.theme.metrics.min_height - 4.0).max(32.0)
    }

    fn measured_menu_width(&self, ctx: &mut LayoutCtx) -> f32 {
        let label_style = self.theme.body_text_style();
        let shortcut_style = self.theme.placeholder_text_style();
        let mut width: f32 = 220.0;
        for item in &self.items {
            let label = measure_text(ctx, item.label(), &label_style).width;
            let shortcut = item
                .shortcut
                .as_ref()
                .map(|text| measure_text(ctx, text, &shortcut_style).width)
                .unwrap_or(0.0);
            width = width.max(label + shortcut + 64.0);
        }
        width
    }

    fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() || !self.open {
            return None;
        }
        let menu = self.frame_rect.translate(bounds.origin.to_vector());
        let x = menu.x() + 8.0;
        let y = menu.y() + 8.0 + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (menu.width() - 16.0).max(0.0),
            self.row_height(),
        ))
    }

    fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.items.iter().enumerate().find_map(|(index, _)| {
            self.item_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn activate(&mut self, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if !item.enabled {
            return;
        }
        if let Some(on_activate) = &mut self.on_activate {
            on_activate(index, item);
        }
    }
}

impl Widget for ContextMenu {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move && self.open => {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if highlighted != self.highlighted {
                    self.highlighted = highlighted;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && self.trigger_rect().contains(pointer.position) =>
            {
                self.open = true;
                self.highlighted = self.items.iter().position(|item| item.enabled);
                self.pressed = None;
                ctx.request_focus();
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open =>
            {
                if let Some(index) = self.item_at(ctx.bounds(), pointer.position) {
                    self.highlighted = Some(index);
                    self.pressed = self
                        .items
                        .get(index)
                        .filter(|item| item.enabled)
                        .map(|_| index);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if !self.trigger_rect().contains(pointer.position) {
                    self.open = false;
                    self.highlighted = None;
                    ctx.request_layout();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open =>
            {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(highlighted)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                    self.open = false;
                    self.highlighted = None;
                    ctx.request_layout();
                }
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == KeyState::Pressed && self.open =>
            {
                match key.key.as_str() {
                    "ArrowDown" => {
                        let mut menu = Menu::new("temp").items(self.items.clone());
                        menu.highlighted = self.highlighted;
                        menu.move_highlight(1);
                        self.highlighted = menu.highlighted;
                    }
                    "ArrowUp" => {
                        let mut menu = Menu::new("temp").items(self.items.clone());
                        menu.highlighted = self.highlighted;
                        menu.move_highlight(-1);
                        self.highlighted = menu.highlighted;
                    }
                    "Enter" | " " => {
                        if let Some(index) = self.highlighted {
                            self.activate(index);
                            self.open = false;
                            ctx.request_layout();
                        }
                    }
                    "Escape" => {
                        self.open = false;
                        self.highlighted = None;
                        ctx.request_layout();
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

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let trigger_size = self
            .trigger
            .layout_at(ctx, constraints.loosen(), Point::ZERO);
        let mut size = trigger_size;
        if self.open {
            let width = self.measured_menu_width(ctx).max(trigger_size.width);
            let height = 16.0 + (self.row_height() * self.items.len() as f32);
            self.frame_rect = Rect::new(0.0, trigger_size.height + 8.0, width, height);
            size = Size::new(
                width.max(trigger_size.width),
                trigger_size.height + 8.0 + height,
            );
        } else {
            self.frame_rect = Rect::ZERO;
        }
        constraints.clamp(size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if !self.open {
            return;
        }

        let menu = self.frame_rect.translate(ctx.bounds().origin.to_vector());
        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        draw_control_frame(
            ctx,
            menu,
            metrics.corner_radius + 2.0,
            metrics,
            palette.surface,
            palette.border,
            Some(palette.focus_ring),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(row.x(), row.y() - 4.0, row.width(), 1.0);
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let pressed = self.pressed == Some(index);
            if highlighted || pressed {
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    if pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
            }

            ctx.draw_text(
                Rect::new(
                    row.x() + 12.0,
                    row.y() + 8.0,
                    row.width() - 24.0,
                    row.height() - 16.0,
                ),
                item.label.clone(),
                self.theme.text_style(item.text_color(self.theme)),
            );

            if let Some(shortcut) = &item.shortcut {
                ctx.draw_text(
                    Rect::new(
                        row.max_x() - 120.0,
                        row.y() + 8.0,
                        108.0,
                        row.height() - 16.0,
                    ),
                    shortcut.clone(),
                    self.theme.placeholder_text_style(),
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ContextMenu, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.open);
        node.value = self
            .highlighted
            .and_then(|index| self.items.get(index))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
            SemanticsAction::Activate,
        ];
        ctx.push(node);
        self.trigger.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.open {
            self.open = false;
            ctx.request_layout();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
    }
}

pub struct Dialog {
    theme: DefaultTheme,
    title: String,
    description: Option<String>,
    shown: bool,
    modal: bool,
    dismiss_on_scrim: bool,
    max_width: f32,
    body: SingleChild,
    actions: WidgetChildren,
    body_frame: Rect,
    dialog_frame: Rect,
    title_measurement: Option<TextMeasurement>,
    description_measurement: Option<TextMeasurement>,
    on_dismiss: Option<Box<dyn FnMut()>>,
}

impl Dialog {
    pub fn new<W>(title: impl Into<String>, body: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: DefaultTheme::default(),
            title: title.into(),
            description: None,
            shown: true,
            modal: true,
            dismiss_on_scrim: false,
            max_width: 520.0,
            body: SingleChild::new(body),
            actions: WidgetChildren::new(),
            body_frame: Rect::ZERO,
            dialog_frame: Rect::ZERO,
            title_measurement: None,
            description_measurement: None,
            on_dismiss: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn shown(mut self, shown: bool) -> Self {
        self.shown = shown;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn dismiss_on_scrim(mut self, dismiss_on_scrim: bool) -> Self {
        self.dismiss_on_scrim = dismiss_on_scrim;
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width.max(280.0);
        self
    }

    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_dismiss = Some(Box::new(on_dismiss));
        self
    }

    pub fn primary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.actions.push(
            Button::new(label.into())
                .min_width(110.0)
                .on_press(on_press),
        );
        self
    }

    pub fn secondary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.actions.push(
            Button::new(label.into())
                .min_width(110.0)
                .on_press(on_press),
        );
        self
    }

    fn dismiss(&mut self) {
        if let Some(on_dismiss) = &mut self.on_dismiss {
            on_dismiss();
        }
    }
}

impl Widget for Dialog {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.shown {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && !self
                        .dialog_frame
                        .translate(ctx.bounds().origin.to_vector())
                        .contains(pointer.position) =>
            {
                if self.dismiss_on_scrim {
                    self.dismiss();
                }
                if self.modal || self.dismiss_on_scrim {
                    ctx.set_handled();
                }
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == KeyState::Pressed && key.key == "Escape" =>
            {
                self.dismiss();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        if !self.shown {
            self.dialog_frame = Rect::ZERO;
            self.body_frame = Rect::ZERO;
            return Size::ZERO;
        }

        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                640.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                420.0
            },
        ));
        let outer_margin = 24.0;
        let padding = Insets::all(18.0);
        let title_style = TextStyle {
            font_size: 20.0,
            line_height: 24.0,
            color: self.theme.palette.text,
            ..TextStyle::default()
        };
        let description_style = self.theme.placeholder_text_style();
        self.title_measurement = Some(measure_text(ctx, &self.title, &title_style));
        self.description_measurement = self
            .description
            .as_ref()
            .map(|text| measure_text(ctx, text, &description_style));

        let dialog_width = (viewport.width - (outer_margin * 2.0))
            .min(self.max_width)
            .max(280.0);
        let mut footer_height: f32 = 0.0;
        let mut footer_width: f32 = 0.0;
        let button_y = 0.0;
        for button in self.actions.as_mut_slice().iter_mut() {
            let button_size = button.layout_at(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(dialog_width, self.theme.metrics.min_height + 8.0),
                ),
                Point::new(0.0, button_y),
            );
            footer_height = footer_height.max(button_size.height);
            footer_width += button_size.width;
        }
        if !self.actions.is_empty() {
            footer_width += 10.0 * self.actions.len().saturating_sub(1) as f32;
        }

        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height);
        let description_height = self
            .description_measurement
            .map(|measurement| measurement.height.max(description_style.line_height))
            .unwrap_or(0.0);
        let header_gap = if self.description.is_some() { 8.0 } else { 0.0 };
        let body_top = padding.top + title_height + header_gap + description_height + 14.0;
        let footer_gap = if self.actions.is_empty() { 0.0 } else { 18.0 };
        let body_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (dialog_width - padding.left - padding.right).max(0.0),
                (viewport.height
                    - outer_margin * 2.0
                    - body_top
                    - footer_gap
                    - footer_height
                    - padding.bottom)
                    .max(0.0),
            ),
        );
        let body_size =
            self.body
                .layout_at(ctx, body_constraints, Point::new(padding.left, body_top));

        let dialog_height =
            body_top + body_size.height + footer_gap + footer_height + padding.bottom;
        let dialog_x = ((viewport.width - dialog_width) * 0.5).max(outer_margin);
        let dialog_y = ((viewport.height - dialog_height) * 0.5).max(outer_margin);
        self.dialog_frame = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        self.body_frame = Rect::new(
            dialog_x + padding.left,
            dialog_y + body_top,
            body_size.width,
            body_size.height,
        );
        self.body.child_mut().set_bounds(self.body_frame);

        if !self.actions.is_empty() {
            let mut x = dialog_x + dialog_width - padding.right - footer_width;
            let y = dialog_y + dialog_height - padding.bottom - footer_height;
            for button in self.actions.as_mut_slice().iter_mut() {
                let button_bounds = button.bounds();
                button.set_bounds(Rect::new(
                    x,
                    y,
                    button_bounds.width(),
                    button_bounds.height(),
                ));
                x += button_bounds.width() + 10.0;
            }
        }

        viewport
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.shown {
            return;
        }

        if self.modal {
            ctx.fill_bounds(Color::rgba(0.06, 0.08, 0.12, 0.24));
        }

        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        draw_control_frame(
            ctx,
            self.dialog_frame,
            metrics.corner_radius + 3.0,
            metrics,
            palette.surface,
            palette.border,
            Some(palette.focus_ring),
        );

        let title_style = TextStyle {
            font_size: 20.0,
            line_height: 24.0,
            color: palette.text,
            ..TextStyle::default()
        };
        let description_style = self.theme.placeholder_text_style();
        let text_x = self.dialog_frame.x() + 18.0;
        let mut text_y = self.dialog_frame.y() + 18.0;
        ctx.draw_text(
            Rect::new(text_x, text_y, self.dialog_frame.width() - 36.0, 28.0),
            self.title.clone(),
            title_style,
        );
        text_y += 24.0;
        if let Some(description) = &self.description {
            ctx.draw_text(
                Rect::new(text_x, text_y + 8.0, self.dialog_frame.width() - 36.0, 22.0),
                description.clone(),
                description_style,
            );
        }

        self.body.paint(ctx);
        for button in self.actions.as_slice() {
            button.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.shown {
            return;
        }

        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::Dialog, self.dialog_frame);
        node.name = Some(self.title.clone());
        node.description = self.description.clone();
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.shown);
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Collapse];
        ctx.push(node);
        self.body.semantics(ctx);
        for button in self.actions.as_slice() {
            button.semantics(ctx);
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
        if self.shown {
            self.body.visit_children(visitor);
            self.actions.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.shown {
            self.body.visit_children_mut(visitor);
            self.actions.visit_children_mut(visitor);
        }
    }
}

pub type Modal = Dialog;

pub struct ProgressBar {
    theme: DefaultTheme,
    name: String,
    min: f64,
    max: f64,
    value: f64,
    show_value: bool,
}

impl ProgressBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            min: 0.0,
            max: 1.0,
            value: 0.0,
            show_value: false,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self.value = self.value.clamp(self.min, self.max);
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    pub fn show_value(mut self, show_value: bool) -> Self {
        self.show_value = show_value;
        self
    }

    fn fraction(&self) -> f32 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            0.0
        } else {
            ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        }
    }
}

impl Widget for ProgressBar {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(240.0, 22.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        draw_control_shape(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            physical_pixels(ctx, metrics.border_width),
            Color::rgba(0.91, 0.94, 0.98, 1.0),
            palette.border,
        );
        let fill = Rect::new(
            ctx.bounds().x(),
            ctx.bounds().y(),
            ctx.bounds().width() * self.fraction(),
            ctx.bounds().height(),
        );
        if fill.width() > 0.0 {
            ctx.fill(
                rounded_rect_path(fill, metrics.corner_radius),
                palette.accent,
            );
        }
        if self.show_value {
            ctx.draw_text(
                inset_rect(ctx.bounds(), Insets::all(2.0)),
                format!("{:.0}%", self.fraction() * 100.0),
                self.theme.text_style(palette.accent_text),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ProgressBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Range {
            value: self.value,
            min: self.min,
            max: self.max,
        });
        ctx.push(node);
    }
}

pub struct Spinner {
    theme: DefaultTheme,
    name: String,
    size: f32,
    label: Option<String>,
}

impl Spinner {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            size: 20.0,
            label: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(8.0);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn indicator_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y() + ((bounds.height() - self.size) * 0.5),
            self.size,
            self.size,
        )
    }
}

impl Widget for Spinner {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let label_width = self
            .label
            .as_ref()
            .map(|label| measure_text(ctx, label, &self.theme.body_text_style()).width + 12.0)
            .unwrap_or(0.0);
        constraints.clamp(Size::new(self.size + label_width, self.size.max(20.0)))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let indicator = self.indicator_rect(ctx.bounds());
        let center = rect_center(indicator);
        let radius = indicator.width().min(indicator.height()) * 0.4;
        let dot_radius = (indicator.width() * 0.09).max(1.5);
        for index in 0..10 {
            let angle = (index as f32 / 10.0) * std::f32::consts::TAU;
            let alpha = 0.22 + ((index as f32) / 10.0) * 0.72;
            let color = Color::rgba(
                self.theme.palette.accent.red,
                self.theme.palette.accent.green,
                self.theme.palette.accent.blue,
                alpha,
            );
            let dot = Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            );
            ctx.fill(Path::circle(dot, dot_radius), color);
        }

        if let Some(label) = &self.label {
            ctx.draw_text(
                Rect::new(
                    indicator.max_x() + 12.0,
                    ctx.bounds().y(),
                    ctx.bounds().width() - indicator.width() - 12.0,
                    ctx.bounds().height(),
                ),
                label.clone(),
                self.theme.body_text_style(),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::BusyIndicator, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = self.label.clone();
        node.state.busy = true;
        ctx.push(node);
    }
}

pub type BusyIndicator = Spinner;

fn measure_text(ctx: &mut LayoutCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
        })
}

fn rect_center(rect: Rect) -> Point {
    Point::new(
        rect.x() + (rect.width() * 0.5),
        rect.y() + (rect.height() * 0.5),
    )
}

fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

fn draw_control_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
) {
    if let Some(focus_ring) = focus_ring {
        let focus_ring_outset = physical_pixels(ctx, metrics.focus_ring_outset);
        ctx.stroke(
            rounded_rect_path(
                bounds.inflate(focus_ring_outset, focus_ring_outset),
                radius + focus_ring_outset,
            ),
            focus_ring,
            StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
        );
    }

    draw_control_shape(
        ctx,
        bounds,
        radius,
        physical_pixels(ctx, metrics.border_width),
        background,
        border,
    );
}

fn draw_control_shape(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    border_width: f32,
    background: Color,
    border: Color,
) {
    let shape = rounded_rect_path(bounds, radius);
    ctx.fill(shape.clone(), background);
    ctx.stroke(shape, border, StrokeStyle::new(border_width));
}

fn tooltip_tail(trigger: Rect, bubble: Rect, placement: TooltipPlacement) -> Path {
    let center_x = rect_center(trigger)
        .x
        .clamp(bubble.x() + 12.0, bubble.max_x() - 12.0);
    let mut builder = PathBuilder::new();
    match placement {
        TooltipPlacement::Above => {
            builder
                .move_to(Point::new(center_x - 6.0, bubble.max_y() - 1.0))
                .line_to(Point::new(center_x + 6.0, bubble.max_y() - 1.0))
                .line_to(Point::new(center_x, bubble.max_y() + 8.0));
        }
        TooltipPlacement::Below => {
            builder
                .move_to(Point::new(center_x - 6.0, bubble.y() + 1.0))
                .line_to(Point::new(center_x + 6.0, bubble.y() + 1.0))
                .line_to(Point::new(center_x, bubble.y() - 8.0));
        }
    }
    builder.build()
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    ctx.dpi().physical_pixels_to_logical(value)
}

#[cfg(test)]
mod tests {
    use super::{ProgressBar, Spinner, TabBar};
    use sui_core::{SemanticsRole, SemanticsValue};
    use sui_runtime::{Application, RenderOutput, Runtime, Widget, WindowBuilder};

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Composites").root(root))
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
        runtime.render(window_id).unwrap()
    }

    #[test]
    fn tab_bar_exposes_selected_value() {
        let output = render(
            TabBar::new("Main tabs")
                .tabs(["Design", "Inspect", "Export"])
                .selected(1),
        );

        let tabs = output
            .semantics
            .into_iter()
            .find(|node| node.role == SemanticsRole::TabBar)
            .expect("tab bar semantics node present");
        assert_eq!(
            tabs.value,
            Some(SemanticsValue::Text("Inspect".to_string()))
        );
    }

    #[test]
    fn progress_bar_and_spinner_publish_semantics() {
        let output = render(sui_widgets_fixture(
            ProgressBar::new("Export progress")
                .range(0.0, 100.0)
                .value(42.0),
            Spinner::new("Background work").label("Uploading textures"),
        ));

        let progress = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ProgressBar)
            .expect("progress bar node present");
        assert_eq!(
            progress.value,
            Some(SemanticsValue::Range {
                value: 42.0,
                min: 0.0,
                max: 100.0,
            })
        );
        let spinner = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::BusyIndicator)
            .expect("spinner node present");
        assert!(spinner.state.busy);
    }

    fn sui_widgets_fixture<A, B>(top: A, bottom: B) -> impl Widget
    where
        A: Widget + 'static,
        B: Widget + 'static,
    {
        crate::Stack::vertical()
            .spacing(12.0)
            .with_child(top)
            .with_child(bottom)
    }
}
