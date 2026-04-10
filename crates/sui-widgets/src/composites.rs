use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, Size,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, LayerOptions, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild,
    StackSurfaceOptions, Widget, WidgetChildren, WidgetPodMutVisitor, WidgetPodVisitor,
    window_render_options,
};
use sui_scene::{LayerCachePolicy, LayerCompositionMode, StrokeStyle};
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

    fn text_color(&self, theme: &DefaultTheme) -> Color {
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
    theme: Box<DefaultTheme>,
    name: String,
    tabs: Vec<String>,
    selected: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    gap: f32,
    label_measurements: Vec<TextMeasurement>,
    widths: Vec<f32>,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl TabBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            tabs: Vec::new(),
            selected: 0,
            hovered: None,
            pressed: None,
            gap: 6.0,
            label_measurements: Vec::new(),
            widths: Vec::new(),
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let style = self.theme.body_text_style();
        self.label_measurements = self
            .tabs
            .iter()
            .map(|tab| measure_text(ctx, tab, &style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| (measurement.width + 28.0).max(96.0))
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
                centered_text_rect(
                    ctx,
                    rect,
                    Insets::all(10.0),
                    self.label_measurements.get(index).copied(),
                    if selected {
                        self.theme.text_style(palette.border_focus).line_height
                    } else {
                        self.theme.body_text_style().line_height
                    },
                ),
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
    theme: Box<DefaultTheme>,
    name: String,
    labels: Vec<String>,
    panels: WidgetChildren,
    selected: usize,
    hovered: Option<usize>,
    pressed: Option<usize>,
    label_measurements: Vec<TextMeasurement>,
    widths: Vec<f32>,
    gap: f32,
    panel_gap: f32,
    panel_frame: Rect,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl Tabs {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            labels: Vec::new(),
            panels: WidgetChildren::new(),
            selected: 0,
            hovered: None,
            pressed: None,
            label_measurements: Vec::new(),
            widths: Vec::new(),
            gap: 6.0,
            panel_gap: 12.0,
            panel_frame: Rect::ZERO,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
                        ctx.request_measure();
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
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.theme.body_text_style();
        self.label_measurements = self
            .labels
            .iter()
            .map(|label| measure_text(ctx, label, &text_style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| (measurement.width + 28.0).max(96.0))
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

        let panel_size = if let Some(panel) = self.selected_panel_mut() {
            panel.measure(ctx, panel_constraints)
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

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let header_height = self.header_height();
        let padding = Insets::all(16.0);
        let panel_gap = self.panel_gap;
        if let Some(panel) = self.selected_panel_mut() {
            let panel_size = panel.measured_size();
            panel.arrange(
                ctx,
                Rect::new(
                    bounds.x() + padding.left,
                    bounds.y() + header_height + panel_gap + padding.top,
                    panel_size.width,
                    panel_size.height,
                ),
            );
        }
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
                centered_text_rect(
                    ctx,
                    rect,
                    Insets::all(10.0),
                    self.label_measurements.get(index).copied(),
                    if selected {
                        self.theme.text_style(palette.border_focus).line_height
                    } else {
                        self.theme.body_text_style().line_height
                    },
                ),
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
    theme: Box<DefaultTheme>,
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
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            items: Vec::new(),
            highlighted: None,
            pressed: None,
            measured_width: 220.0,
            on_activate: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        (self.theme.metrics.min_height + 2.0).max(24.0)
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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
        let height = 12.0 + (self.row_height() * self.items.len() as f32);
        constraints.clamp(Size::new(
            self.measured_width,
            height.max(self.row_height() + 12.0),
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
                self.theme.text_style(item.text_color(self.theme.as_ref())),
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
    theme: Box<DefaultTheme>,
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
            theme: Box::new(DefaultTheme::default()),
            text: text.into(),
            placement: TooltipPlacement::Above,
            child: SingleChild::new(child),
            hovered: false,
            measurement: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
            ascent: self.theme.typography.body_font_size,
            descent: 0.0,
            cap_height: Some(self.theme.typography.body_font_size),
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.measurement = Some(measure_text(
            ctx,
            &self.text,
            &self.theme.placeholder_text_style(),
        ));
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
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

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            cache_policy: LayerCachePolicy::Direct,
            composition_mode: if self.hovered {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.hovered.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
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
    theme: Box<DefaultTheme>,
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
            theme: Box::new(DefaultTheme::default()),
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
        self.theme = Box::new(theme);
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
                ctx.request_measure();
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
                ctx.request_measure();
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
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
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
            let content_size = self.content.measure(ctx, content_constraints);
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

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        self.trigger
            .arrange(ctx, Rect::from_origin_size(bounds.origin, trigger_size));
        if self.open {
            let content_size = self.content.child().measured_size();
            self.content.arrange(
                ctx,
                Rect::new(
                    bounds.x() + self.padding.left,
                    bounds.y() + trigger_size.height + self.gap + self.padding.top,
                    content_size.width,
                    content_size.height,
                ),
            );
        }
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

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            cache_policy: LayerCachePolicy::Direct,
            composition_mode: if self.open {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.open.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
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
            ctx.request_measure();
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
    theme: Box<DefaultTheme>,
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
            theme: Box::new(DefaultTheme::default()),
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
        self.theme = Box::new(theme);
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
        (self.theme.metrics.min_height + 2.0).max(24.0)
    }

    fn measured_menu_width(&self, ctx: &mut MeasureCtx) -> f32 {
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
                ctx.request_measure();
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
                    ctx.request_measure();
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
                    ctx.request_measure();
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
                            ctx.request_measure();
                        }
                    }
                    "Escape" => {
                        self.open = false;
                        self.highlighted = None;
                        ctx.request_measure();
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
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
        let mut size = trigger_size;
        if self.open {
            let width = self.measured_menu_width(ctx).max(trigger_size.width);
            let height = 12.0 + (self.row_height() * self.items.len() as f32);
            self.frame_rect = Rect::new(0.0, trigger_size.height + 6.0, width, height);
            size = Size::new(
                width.max(trigger_size.width),
                trigger_size.height + 6.0 + height,
            );
        } else {
            self.frame_rect = Rect::ZERO;
        }
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.trigger.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.trigger.child().measured_size()),
        );
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
                self.theme.text_style(item.text_color(self.theme.as_ref())),
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

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            cache_policy: LayerCachePolicy::Direct,
            composition_mode: if self.open {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.open.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
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
            ctx.request_measure();
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
    theme: Box<DefaultTheme>,
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
            theme: Box::new(DefaultTheme::default()),
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
        self.theme = Box::new(theme);
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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
        for button in self.actions.as_mut_slice().iter_mut() {
            let button_size = button.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(dialog_width, self.theme.metrics.min_height + 8.0),
                ),
            );
            footer_height = footer_height.max(button_size.height);
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
        let body_size = self.body.measure(ctx, body_constraints);

        let dialog_height =
            body_top + body_size.height + footer_gap + footer_height + padding.bottom;
        let dialog_x = ((viewport.width - dialog_width) * 0.5).max(outer_margin);
        let dialog_y = ((viewport.height - dialog_height) * 0.5).max(outer_margin);
        self.dialog_frame = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        self.body_frame = Rect::new(padding.left, body_top, body_size.width, body_size.height);

        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(bounds.origin.to_vector());
        self.body.arrange(
            ctx,
            Rect::new(
                dialog.x() + self.body_frame.x(),
                dialog.y() + self.body_frame.y(),
                self.body_frame.width(),
                self.body_frame.height(),
            ),
        );

        if !self.actions.is_empty() {
            let padding = Insets::all(18.0);
            let footer_width = self
                .actions
                .as_slice()
                .iter()
                .map(|button| button.measured_size().width)
                .sum::<f32>()
                + (10.0 * self.actions.len().saturating_sub(1) as f32);
            let footer_height = self
                .actions
                .as_slice()
                .iter()
                .map(|button| button.measured_size().height)
                .fold(0.0, f32::max);
            let mut x = dialog.x() + dialog.width() - padding.right - footer_width;
            let y = dialog.y() + dialog.height() - padding.bottom - footer_height;
            for button in self.actions.as_mut_slice().iter_mut() {
                let size = button.measured_size();
                button.arrange(ctx, Rect::new(x, y, size.width, size.height));
                x += size.width + 10.0;
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(ctx.bounds().origin.to_vector());

        if self.modal {
            ctx.fill_bounds(Color::rgba(0.06, 0.08, 0.12, 0.24));
        }

        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        draw_control_frame(
            ctx,
            dialog,
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
        let text_x = dialog.x() + 18.0;
        let mut text_y = dialog.y() + 18.0;
        ctx.draw_text(
            Rect::new(text_x, text_y, dialog.width() - 36.0, 28.0),
            self.title.clone(),
            title_style,
        );
        text_y += 24.0;
        if let Some(description) = &self.description {
            ctx.draw_text(
                Rect::new(text_x, text_y + 8.0, dialog.width() - 36.0, 22.0),
                description.clone(),
                description_style,
            );
        }

        self.body.paint(ctx);
        for button in self.actions.as_slice() {
            button.paint(ctx);
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            cache_policy: LayerCachePolicy::Direct,
            composition_mode: if self.shown {
                if self.modal {
                    LayerCompositionMode::Effect
                } else {
                    LayerCompositionMode::Overlay
                }
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.shown.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(ctx.bounds().origin.to_vector());
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Dialog, dialog);
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
    theme: Box<DefaultTheme>,
    name: String,
    min: f64,
    max: f64,
    value: f64,
    show_value: bool,
}

impl ProgressBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            min: 0.0,
            max: 1.0,
            value: 0.0,
            show_value: false,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(240.0, 18.0))
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
    theme: Box<DefaultTheme>,
    name: String,
    size: f32,
    label: Option<String>,
}

impl Spinner {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            size: 20.0,
            label: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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

fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
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

fn centered_text_rect(
    ctx: &PaintCtx,
    bounds: Rect,
    padding: Insets,
    measurement: Option<TextMeasurement>,
    line_height: f32,
) -> Rect {
    let rect = Rect::new(
        bounds.x() + padding.left,
        bounds.y(),
        (bounds.width() - padding.left - padding.right).max(0.0),
        bounds.height(),
    );
    let Some(measurement) = measurement else {
        return rect;
    };

    let width = measurement.width.min(rect.width());
    let height = line_height.max(measurement.height).min(rect.height());
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

    Rect::new(
        rect.x() + ((rect.width() - width) * 0.5),
        baseline - measurement.ascent - leading_above,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::Tabs;
    use super::{Dialog, Popover, ProgressBar, Spinner, TabBar};
    use crate::FloatingStack;
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, SemanticsNode, SemanticsRole, SemanticsValue, Size, WidgetId,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, ArrangeCtx, MeasureCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx, Widget,
        WindowBuilder, WindowRenderOptions, clear_window_render_options, set_window_render_options,
    };
    use sui_scene::{LayerCachePolicy, LayerCompositionMode, SceneLayerDescriptor};

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

    fn layer_descriptor_for(
        output: &RenderOutput,
        owner: WidgetId,
    ) -> Option<SceneLayerDescriptor> {
        let mut descriptor = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.widget_id() == owner {
                descriptor = Some(layer.descriptor.clone());
            }
        });
        descriptor
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    struct PanelCounters {
        measure: usize,
        arrange: usize,
        paint: usize,
        semantics: usize,
    }

    struct SpyPanel {
        name: &'static str,
        counters: Rc<RefCell<PanelCounters>>,
    }

    impl SpyPanel {
        fn new(name: &'static str, counters: Rc<RefCell<PanelCounters>>) -> Self {
            Self { name, counters }
        }
    }

    impl Widget for SpyPanel {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            self.counters.borrow_mut().measure += 1;
            constraints.clamp(Size::new(180.0, 72.0))
        }

        fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: sui_core::Rect) {
            self.counters.borrow_mut().arrange += 1;
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counters.borrow_mut().paint += 1;
            ctx.fill_bounds(Color::rgba(0.20, 0.28, 0.38, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            self.counters.borrow_mut().semantics += 1;
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(self.name.to_string());
            ctx.push(node);
        }
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
    fn tabs_render_only_the_active_panel_after_switching() {
        let first = Rc::new(RefCell::new(PanelCounters::default()));
        let second = Rc::new(RefCell::new(PanelCounters::default()));
        let (mut runtime, window_id) = build_runtime(
            Tabs::new("Main tabs")
                .tab("First", SpyPanel::new("first-panel", Rc::clone(&first)))
                .tab("Second", SpyPanel::new("second-panel", Rc::clone(&second))),
        );

        let initial = runtime.render(window_id).unwrap();
        assert_eq!(
            *first.borrow(),
            PanelCounters {
                measure: 1,
                arrange: 1,
                paint: 1,
                semantics: 1
            }
        );
        assert_eq!(*second.borrow(), PanelCounters::default());
        assert!(
            initial
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some("first-panel"))
        );
        assert!(
            !initial
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some("second-panel"))
        );

        let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 20.0));
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .unwrap();

        let mut up = PointerEvent::new(PointerEventKind::Up, Point::new(48.0, 20.0));
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up)).unwrap();

        let first_before_switch = *first.borrow();
        let second_before_switch = *second.borrow();

        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
            )
            .unwrap();

        let after_switch = runtime.render(window_id).unwrap();
        assert_eq!(first.borrow().paint, first_before_switch.paint);
        assert_eq!(first.borrow().semantics, first_before_switch.semantics);
        assert_eq!(second.borrow().paint, second_before_switch.paint + 1);
        assert_eq!(
            second.borrow().semantics,
            second_before_switch.semantics + 1
        );
        assert!(
            !after_switch
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some("first-panel"))
        );
        assert!(
            after_switch
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some("second-panel"))
        );
    }

    #[test]
    fn tabs_center_header_labels_within_each_tab() {
        let optical = render(
            Tabs::new("Main tabs")
                .tab(
                    "A",
                    SpyPanel::new(
                        "first-panel",
                        Rc::new(RefCell::new(PanelCounters::default())),
                    ),
                )
                .tab(
                    "B",
                    SpyPanel::new(
                        "second-panel",
                        Rc::new(RefCell::new(PanelCounters::default())),
                    ),
                ),
        );
        let optical_label = optical
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                sui_scene::SceneCommand::DrawText(text) => Some(text.rect),
                _ => None,
            })
            .expect("tabs header label draw command present");

        let (mut runtime, window_id) = build_runtime(
            Tabs::new("Main tabs")
                .tab(
                    "A",
                    SpyPanel::new(
                        "first-panel",
                        Rc::new(RefCell::new(PanelCounters::default())),
                    ),
                )
                .tab(
                    "B",
                    SpyPanel::new(
                        "second-panel",
                        Rc::new(RefCell::new(PanelCounters::default())),
                    ),
                ),
        );
        set_window_render_options(
            window_id,
            WindowRenderOptions::new(true, 1.0).with_optical_vertical_text_alignment_enabled(false),
        );
        let geometric = runtime.render(window_id).unwrap();
        clear_window_render_options(window_id);
        let geometric_label = geometric
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                sui_scene::SceneCommand::DrawText(text) => Some(text.rect),
                _ => None,
            })
            .expect("geometric tabs header label draw command present");

        assert!(optical_label.x() > 10.0);
        assert!((optical_label.y() - geometric_label.y()).abs() > 0.001);
        assert!(optical_label.max_y() <= optical.frame.viewport.height);
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

    #[test]
    fn open_popover_uses_direct_overlay_layer_metadata() {
        let output = render(crate::Padding::all(
            16.0,
            Popover::new(
                "Options",
                crate::Button::new("Open"),
                crate::Label::new("Popover body"),
            )
            .open(true),
        ));

        let popover = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Popover)
            .expect("popover semantics present");
        let descriptor =
            layer_descriptor_for(&output, popover.id).expect("popover layer descriptor present");

        assert_eq!(descriptor.cache_policy, LayerCachePolicy::Direct);
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
    }

    #[test]
    fn open_popover_resolves_to_nearest_stack_host_and_tracks_owner_surface() {
        let (mut runtime, window_id) = build_runtime(
            FloatingStack::new().with_window(
                sui_core::Rect::new(24.0, 24.0, 240.0, 160.0),
                crate::Padding::all(
                    16.0,
                    Popover::new(
                        "Options",
                        crate::Button::new("Open"),
                        crate::Label::new("Popover body"),
                    )
                    .open(true),
                ),
            ),
        );

        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();
        let popover = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Popover)
            .expect("popover semantics present");
        let descriptor =
            layer_descriptor_for(&output, popover.id).expect("popover layer descriptor present");
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == popover.id)
            .expect("popover graph node present");
        let host = graph
            .stack_hosts
            .iter()
            .find(|host| host.host == graph.root)
            .expect("root stack host present");

        assert_eq!(node.stack_host, graph.root);
        assert_eq!(node.stack_surface, popover.id);
        assert_eq!(node.transient_owner_surface, Some(host.surfaces[0]));
        assert_eq!(host.surfaces.last().copied(), Some(popover.id));
        assert_eq!(descriptor.stack_host, graph.root);
        assert_eq!(descriptor.transient_owner_surface, Some(host.surfaces[0]));
        assert!(descriptor.is_stack_surface);
    }

    #[test]
    fn modal_dialog_uses_direct_effect_layer_metadata() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(640.0, 420.0))
                .with_child(Dialog::new(
                    "Confirm",
                    crate::Label::new("Apply the change?"),
                )),
        );

        let dialog = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present");
        let descriptor =
            layer_descriptor_for(&output, dialog.id).expect("dialog layer descriptor present");

        assert_eq!(descriptor.cache_policy, LayerCachePolicy::Direct);
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Effect);
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
