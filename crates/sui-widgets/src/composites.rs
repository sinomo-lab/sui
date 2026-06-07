use std::{cell::RefCell, rc::Rc};

use sui_core::{
    Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Path,
    PathBuilder, Point, PointerButton, PointerEventKind, Rect, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsState, SemanticsValue, Size, TimerToken, Vector, WakeEvent, WidgetId,
};
use sui_layout::{Axis, Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx, SemanticsCtx,
    SingleChild, StackSurfaceOptions, Widget, WidgetChildren, WidgetPodMutVisitor,
    WidgetPodVisitor, window_render_options,
};
use sui_scene::{LayerCompositionMode, LayerProperties, StrokeStyle};
use sui_text::{FontWeight, TextMeasurement, TextStyle};

use crate::{
    Button, ControlMetrics, DefaultTheme, Easing, HdrThemeMode, IconGlyph, ResolvedEffectStyle,
    ResolvedHdrStyle, Transition, WidgetColorRole, WidgetEffectRole, WidgetLuminanceRole,
    WidgetMaterialRole,
    controls::{apply_hdr_policy_cap, cap_resolved_hdr_style, draw_icon_glyph},
    paint_theme_shadow, resolve_widget_hdr_style,
};

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

fn virtual_menu_item_id(parent: WidgetId, index: usize) -> WidgetId {
    WidgetId::new(
        (1_u64 << 63)
            | parent
                .get()
                .wrapping_mul(257)
                .wrapping_add(index as u64 + 1),
    )
}

const MENU_HORIZONTAL_PADDING: f32 = 6.0;
const MENU_VERTICAL_PADDING: f32 = 6.0;
const MENU_MIN_ROW_HEIGHT: f32 = 28.0;
const MENU_ROW_HEIGHT_REDUCTION: f32 = 6.0;

fn menu_row_height(theme: &DefaultTheme) -> f32 {
    (theme.metrics.min_height - MENU_ROW_HEIGHT_REDUCTION).max(MENU_MIN_ROW_HEIGHT)
}

fn menu_height_for_rows(row_height: f32, rows: usize) -> f32 {
    (MENU_VERTICAL_PADDING * 2.0) + (row_height * rows as f32)
}

fn menu_item_semantics_node(
    parent: WidgetId,
    index: usize,
    item: &MenuItem,
    bounds: Rect,
    highlighted: bool,
) -> SemanticsNode {
    let mut node = SemanticsNode::new(
        virtual_menu_item_id(parent, index),
        SemanticsRole::MenuItem,
        bounds,
    );
    node.parent = Some(parent);
    node.name = Some(item.label.clone());
    node.state.disabled = !item.enabled;
    node.state.selected = highlighted;
    if item.enabled {
        node.actions = vec![SemanticsAction::Activate];
    }
    node
}

const TOOLBAR_EXTENT: f32 = 52.0;
const TOOLBAR_PADDING: f32 = 8.0;
const TOOLBAR_SPACING: f32 = 8.0;

pub struct Toolbar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: Option<String>,
    extent: f32,
    padding: Insets,
    spacing: f32,
    background: Option<Color>,
    divider: bool,
    children: WidgetChildren,
}

impl Toolbar {
    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn new(axis: Axis) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            axis,
            name: None,
            extent: TOOLBAR_EXTENT,
            padding: Insets::all(TOOLBAR_PADDING),
            spacing: TOOLBAR_SPACING,
            background: None,
            divider: true,
            children: WidgetChildren::new(),
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = extent.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn divider(mut self, divider: bool) -> Self {
        self.divider = divider;
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
    }

    pub fn children(&self) -> &[sui_runtime::WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [sui_runtime::WidgetPod] {
        self.children.as_mut_slice()
    }

    fn content_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right).max(0.0),
            (bounds.height() - self.padding.top - self.padding.bottom).max(0.0),
        )
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::horizontal()
    }
}

impl Widget for Toolbar {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let content_cross = match self.axis {
            Axis::Horizontal => (self.extent - self.padding.top - self.padding.bottom).max(0.0),
            Axis::Vertical => (self.extent - self.padding.left - self.padding.right).max(0.0),
        };
        let child_constraints = match self.axis {
            Axis::Horizontal => {
                Constraints::new(Size::ZERO, Size::new(f32::INFINITY, content_cross))
            }
            Axis::Vertical => Constraints::new(Size::ZERO, Size::new(content_cross, f32::INFINITY)),
        };

        let mut main: f32 = 0.0;
        let mut cross: f32 = 0.0;
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            let child_size = child.measure(ctx, child_constraints);
            if index > 0 {
                main += self.spacing;
            }
            main += toolbar_main(self.axis, child_size);
            cross = cross.max(toolbar_cross(self.axis, child_size));
        }

        let natural = match self.axis {
            Axis::Horizontal => Size::new(
                main + self.padding.left + self.padding.right,
                self.extent
                    .max(cross + self.padding.top + self.padding.bottom),
            ),
            Axis::Vertical => Size::new(
                self.extent
                    .max(cross + self.padding.left + self.padding.right),
                main + self.padding.top + self.padding.bottom,
            ),
        };
        let filled = match self.axis {
            Axis::Horizontal => Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width
                } else {
                    natural.width
                },
                self.extent,
            ),
            Axis::Vertical => Size::new(
                self.extent,
                if constraints.max.height.is_finite() {
                    constraints.max.height
                } else {
                    natural.height
                },
            ),
        };

        constraints.clamp(filled)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let content = self.content_bounds(bounds);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let mut main_offset = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                main_offset += self.spacing;
            }

            let measured = child.measured_size();
            let remaining = (content_main - main_offset).max(0.0);
            let child_main = toolbar_main(self.axis, measured).min(remaining);
            let child_cross = toolbar_cross(self.axis, measured).min(content_cross);
            let cross_offset = ((content_cross - child_cross) * 0.5).max(0.0);
            let origin = match self.axis {
                Axis::Horizontal => {
                    Point::new(content.x() + main_offset, content.y() + cross_offset)
                }
                Axis::Vertical => Point::new(content.x() + cross_offset, content.y() + main_offset),
            };
            child.arrange(
                ctx,
                Rect::from_origin_size(origin, toolbar_size(self.axis, child_main, child_cross)),
            );
            main_offset += child_main;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        ctx.fill_bounds(self.background.unwrap_or(palette.surface));
        if self.divider {
            let divider = match self.axis {
                Axis::Horizontal => {
                    Rect::new(bounds.x(), bounds.max_y() - 1.0, bounds.width(), 1.0)
                }
                Axis::Vertical => Rect::new(bounds.max_x() - 1.0, bounds.y(), 1.0, bounds.height()),
            };
            ctx.stroke_rect(
                divider,
                palette.border.with_alpha(0.85),
                StrokeStyle::new(1.0),
            );
        }
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(name) = &self.name {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(name.clone());
            ctx.push(node);
        }
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

const COMMAND_GROUP_SPACING: f32 = 3.0;
const COMMAND_GROUP_RADIUS: f32 = 6.0;

pub struct CommandGroup {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: Option<String>,
    padding: Insets,
    spacing: f32,
    corner_radius: f32,
    background: Option<Color>,
    border: Option<Color>,
    children: WidgetChildren,
}

impl CommandGroup {
    pub fn horizontal(name: impl Into<String>) -> Self {
        Self::new(Axis::Horizontal, name)
    }

    pub fn vertical(name: impl Into<String>) -> Self {
        Self::new(Axis::Vertical, name)
    }

    pub fn new(axis: Axis, name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            axis,
            name: Some(name.into()),
            padding: Insets::all(2.0),
            spacing: COMMAND_GROUP_SPACING,
            corner_radius: COMMAND_GROUP_RADIUS,
            background: None,
            border: None,
            children: WidgetChildren::new(),
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

    pub fn unnamed(mut self) -> Self {
        self.name = None;
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, color: Color) -> Self {
        self.border = Some(color);
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
    }

    pub fn children(&self) -> &[sui_runtime::WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [sui_runtime::WidgetPod] {
        self.children.as_mut_slice()
    }

    fn content_bounds(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for CommandGroup {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        let max_height = if constraints.max.height.is_finite() {
            (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0)
        } else {
            f32::INFINITY
        };
        let child_constraints = Constraints::new(Size::ZERO, Size::new(max_width, max_height));

        let mut main: f32 = 0.0;
        let mut cross: f32 = 0.0;
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            let child_size = child.measure(ctx, child_constraints);
            if index > 0 {
                main += self.spacing;
            }
            main += toolbar_main(self.axis, child_size);
            cross = cross.max(toolbar_cross(self.axis, child_size));
        }

        let natural = match self.axis {
            Axis::Horizontal => Size::new(
                main + self.padding.left + self.padding.right,
                cross + self.padding.top + self.padding.bottom,
            ),
            Axis::Vertical => Size::new(
                cross + self.padding.left + self.padding.right,
                main + self.padding.top + self.padding.bottom,
            ),
        };
        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let content = self.content_bounds(bounds);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let mut main_offset = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                main_offset += self.spacing;
            }

            let measured = child.measured_size();
            let remaining = (content_main - main_offset).max(0.0);
            let child_main = toolbar_main(self.axis, measured).min(remaining);
            let child_cross = toolbar_cross(self.axis, measured).min(content_cross);
            let cross_offset = ((content_cross - child_cross) * 0.5).max(0.0);
            let origin = match self.axis {
                Axis::Horizontal => {
                    Point::new(content.x() + main_offset, content.y() + cross_offset)
                }
                Axis::Vertical => Point::new(content.x() + cross_offset, content.y() + main_offset),
            };
            child.arrange(
                ctx,
                Rect::from_origin_size(origin, toolbar_size(self.axis, child_main, child_cross)),
            );
            main_offset += child_main;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let radius = self
            .corner_radius
            .min(ctx.bounds().width().min(ctx.bounds().height()) * 0.5);
        let background = self.background.unwrap_or(theme.palette.surface_raised);
        let border = self.border.unwrap_or(theme.palette.border);
        let shape = rounded_rect_path(ctx.bounds(), radius);
        ctx.fill(shape.clone(), background);
        ctx.stroke(shape, border, StrokeStyle::new(physical_pixels(ctx, 1.0)));
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(name) = &self.name {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(name.clone());
            ctx.push(node);
        }
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

fn toolbar_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn toolbar_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn toolbar_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

const TOOL_PALETTE_ITEM_SIZE: f32 = 40.0;
const TOOL_PALETTE_ICON_SIZE: f32 = 20.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPaletteItem {
    icon: IconGlyph,
    label: String,
    enabled: bool,
}

impl ToolPaletteItem {
    pub fn new(icon: IconGlyph, label: impl Into<String>) -> Self {
        Self {
            icon,
            label: label.into(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

pub struct ToolPalette {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: String,
    items: Vec<ToolPaletteItem>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    extent: f32,
    padding: Insets,
    spacing: f32,
    item_size: f32,
    icon_size: f32,
    background: Option<Color>,
    divider: bool,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String)>>,
}

impl ToolPalette {
    pub fn vertical(name: impl Into<String>) -> Self {
        Self::new(Axis::Vertical, name)
    }

    pub fn horizontal(name: impl Into<String>) -> Self {
        Self::new(Axis::Horizontal, name)
    }

    pub fn new(axis: Axis, name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            axis,
            name: name.into(),
            items: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            extent: TOOLBAR_EXTENT,
            padding: Insets::all(TOOLBAR_PADDING),
            spacing: TOOLBAR_SPACING,
            item_size: TOOL_PALETTE_ITEM_SIZE,
            icon_size: TOOL_PALETTE_ICON_SIZE,
            background: None,
            divider: true,
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

    pub fn item(mut self, item: ToolPaletteItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = ToolPaletteItem>,
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

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = extent.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn item_size(mut self, item_size: f32) -> Self {
        self.item_size = item_size.max(0.0);
        self
    }

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn divider(mut self, divider: bool) -> Self {
        self.divider = divider;
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
        F: FnMut(&mut EventCtx, usize, String) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
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

    fn content_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right).max(0.0),
            (bounds.height() - self.padding.top - self.padding.bottom).max(0.0),
        )
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }

        let content = self.content_bounds(bounds);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let item_main = self.item_size.min(content_main);
        let item_cross = self.item_size.min(content_cross);
        let main_offset = index as f32 * (self.item_size + self.spacing);
        if main_offset >= content_main {
            return None;
        }
        let cross_offset = ((content_cross - item_cross) * 0.5).max(0.0);
        let origin = match self.axis {
            Axis::Horizontal => Point::new(content.x() + main_offset, content.y() + cross_offset),
            Axis::Vertical => Point::new(content.x() + cross_offset, content.y() + main_offset),
        };
        Some(Rect::from_origin_size(
            origin,
            toolbar_size(self.axis, item_main, item_cross),
        ))
    }

    fn hit_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        (0..self.items.len()).find(|index| {
            self.items[*index].enabled
                && self
                    .item_rect(bounds, *index)
                    .is_some_and(|rect| rect.contains(position))
        })
    }

    fn select(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        if !item.enabled {
            return;
        }

        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, item.label.clone());
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, index, item.label.clone());
        }
    }

    fn move_selection(&mut self, ctx: &mut EventCtx, delta: isize) {
        if self.items.is_empty() {
            return;
        }

        let start = self.current_selected().unwrap_or(0);
        let mut index = start as isize;
        let last = self.items.len() as isize - 1;
        for _ in 0..self.items.len() {
            index = (index + delta).clamp(0, last);
            if self
                .items
                .get(index as usize)
                .is_some_and(|item| item.enabled)
            {
                self.select(ctx, index as usize);
                return;
            }
            if index == 0 || index == last {
                return;
            }
        }
    }
}

impl Widget for ToolPalette {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
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
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.select(ctx, index);
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
                    "ArrowUp" if self.axis == Axis::Vertical => self.move_selection(ctx, -1),
                    "ArrowDown" if self.axis == Axis::Vertical => self.move_selection(ctx, 1),
                    "ArrowLeft" if self.axis == Axis::Horizontal => self.move_selection(ctx, -1),
                    "ArrowRight" if self.axis == Axis::Horizontal => self.move_selection(ctx, 1),
                    "Home" => {
                        if let Some(index) = self.items.iter().position(|item| item.enabled) {
                            self.select(ctx, index);
                        }
                    }
                    "End" => {
                        if let Some(index) = self.items.iter().rposition(|item| item.enabled) {
                            self.select(ctx, index);
                        }
                    }
                    "Enter" | " " => {
                        if let Some(index) = self.current_selected() {
                            self.select(ctx, index);
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

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let item_count = self.items.len();
        let main = if item_count == 0 {
            0.0
        } else {
            (self.item_size * item_count as f32) + (self.spacing * (item_count - 1) as f32)
        };
        let natural = match self.axis {
            Axis::Horizontal => {
                Size::new(main + self.padding.left + self.padding.right, self.extent)
            }
            Axis::Vertical => Size::new(self.extent, main + self.padding.top + self.padding.bottom),
        };
        let filled = match self.axis {
            Axis::Horizontal => Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width
                } else {
                    natural.width
                },
                natural.height,
            ),
            Axis::Vertical => Size::new(
                natural.width,
                if constraints.max.height.is_finite() {
                    constraints.max.height
                } else {
                    natural.height
                },
            ),
        };

        constraints.clamp(filled)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let bounds = ctx.bounds();
        ctx.fill_bounds(self.background.unwrap_or(palette.surface));
        if self.divider {
            let divider = match self.axis {
                Axis::Horizontal => {
                    Rect::new(bounds.x(), bounds.max_y() - 1.0, bounds.width(), 1.0)
                }
                Axis::Vertical => Rect::new(bounds.max_x() - 1.0, bounds.y(), 1.0, bounds.height()),
            };
            ctx.stroke_rect(
                divider,
                palette.border.with_alpha(0.85),
                StrokeStyle::new(1.0),
            );
        }

        let selected = self.current_selected();
        for (index, item) in self.items.iter().enumerate() {
            let Some(rect) = self.item_rect(bounds, index) else {
                continue;
            };
            let selected_item = selected == Some(index);
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let enabled = item.enabled;
            let base_background = if selected_item {
                palette.selection
            } else {
                palette.surface
            };
            let background = if !enabled {
                mix_color(base_background, palette.surface, 0.72).with_alpha(0.82)
            } else if pressed {
                palette.control_active
            } else if hovered {
                palette.control_hover
            } else {
                base_background
            };
            let border = if !enabled {
                palette.border.with_alpha(0.55)
            } else if ctx.is_focused() && selected_item {
                palette.border_focus
            } else if selected_item {
                palette.accent_border
            } else if hovered {
                palette.border_hover
            } else {
                palette.border
            };
            draw_control_frame(
                ctx,
                rect,
                metrics.corner_radius,
                metrics,
                background,
                border,
                (ctx.is_focused() && selected_item).then_some(palette.focus_ring),
            );
            let center = rect_center(rect);
            let side = self.icon_size.min(rect.width().min(rect.height())).max(0.0);
            let icon_rect = Rect::new(center.x - side * 0.5, center.y - side * 0.5, side, side);
            draw_icon_glyph(
                ctx,
                item.icon,
                icon_rect,
                if !enabled {
                    palette.text.with_alpha(0.38)
                } else if selected_item {
                    palette.accent
                } else {
                    palette.text
                },
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let selected = self.current_selected();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = selected
            .and_then(|index| self.items.get(index))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (index, item) in self.items.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let mut item_node = SemanticsNode::new(
                tool_palette_item_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                rect,
            );
            item_node.parent = Some(ctx.widget_id());
            item_node.name = Some(item.label.clone());
            item_node.value = Some(SemanticsValue::Text(item.label.clone()));
            item_node.state.disabled = !item.enabled;
            item_node.state.hovered = self.hovered == Some(index);
            item_node.state.selected = selected == Some(index);
            if item.enabled {
                item_node.actions = vec![SemanticsAction::Activate];
            }
            ctx.push(item_node);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.items.iter().any(|item| item.enabled)
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn tool_palette_item_id(parent: WidgetId, index: usize) -> WidgetId {
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

const ACTION_CARD_DEFAULT_WIDTH: f32 = 280.0;
const ACTION_CARD_DEFAULT_HEIGHT: f32 = 104.0;
const ACTION_CARD_PADDING: Insets = Insets {
    left: 16.0,
    top: 14.0,
    right: 14.0,
    bottom: 14.0,
};
const ACTION_CARD_ICON_BOX_SIZE: f32 = 38.0;
const ACTION_CARD_ICON_SIZE: f32 = 20.0;
const ACTION_CARD_TEXT_GAP: f32 = 5.0;
const ACTION_CARD_HOVER_ANIMATION_SECONDS: f64 = 1.0 / 8.0;
const ACTION_CARD_PRESS_ANIMATION_SECONDS: f64 = 1.0 / 12.0;

pub struct ActionCard {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    title: String,
    description: String,
    icon: Option<IconGlyph>,
    accent: Option<Color>,
    padding: Insets,
    min_width: f32,
    min_height: f32,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    title_measurement: Option<TextMeasurement>,
    description_measurement: Option<TextMeasurement>,
    enabled: bool,
    enabled_reader: Option<Box<dyn Fn() -> bool>>,
    on_press: Option<Box<dyn FnMut()>>,
    on_press_with_ctx: Option<Box<dyn FnMut(&mut EventCtx)>>,
}

impl ActionCard {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            description: description.into(),
            icon: None,
            accent: None,
            padding: ACTION_CARD_PADDING,
            min_width: ACTION_CARD_DEFAULT_WIDTH,
            min_height: ACTION_CARD_DEFAULT_HEIGHT,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            title_measurement: None,
            description_measurement: None,
            enabled: true,
            enabled_reader: None,
            on_press: None,
            on_press_with_ctx: None,
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

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    pub fn accent(mut self, accent: Color) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width.max(0.0);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height.max(0.0);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.enabled_reader = None;
        self
    }

    pub fn enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.enabled_reader = Some(Box::new(enabled));
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    pub fn on_press_with_ctx<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(&mut EventCtx) + 'static,
    {
        self.on_press_with_ctx = Some(Box::new(on_press));
        self
    }

    fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        if !self.is_enabled() {
            return;
        }
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
        if let Some(on_press) = &mut self.on_press_with_ctx {
            on_press(ctx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        set_action_card_animation_target(
            &mut self.hover_animation,
            hovered as u8 as f32,
            ACTION_CARD_HOVER_ANIMATION_SECONDS,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time) | self.press_animation.advance(time)
    }

    fn title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            font_size: 14.0,
            line_height: 18.0,
            color: theme.palette.text,
            weight: FontWeight::SEMIBOLD,
            ..theme.body_text_style()
        }
    }

    fn description_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: theme.palette.placeholder,
            ..theme.body_text_style()
        }
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }

    fn text_bounds(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let icon_extent = self
            .icon
            .map(|_| ACTION_CARD_ICON_BOX_SIZE + 12.0)
            .unwrap_or(0.0);
        let trailing = 22.0;
        Rect::new(
            content.x() + icon_extent,
            content.y(),
            (content.width() - icon_extent - trailing).max(0.0),
            content.height(),
        )
    }
}

impl Widget for ActionCard {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_enabled() {
            if self.hovered || self.pressed {
                self.hovered = false;
                self.pressed = false;
                set_action_card_animation_target(
                    &mut self.hover_animation,
                    0.0,
                    ACTION_CARD_HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_action_card_animation_target(
                    &mut self.press_animation,
                    0.0,
                    ACTION_CARD_PRESS_ANIMATION_SECONDS,
                    ctx,
                );
                ctx.request_paint();
                ctx.request_semantics();
            }
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed = true;
                self.hovered = true;
                set_action_card_animation_target(
                    &mut self.hover_animation,
                    1.0,
                    ACTION_CARD_HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_action_card_animation_target(
                    &mut self.press_animation,
                    1.0,
                    ACTION_CARD_PRESS_ANIMATION_SECONDS,
                    ctx,
                );
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_action_card_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    ACTION_CARD_HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_action_card_animation_target(
                    &mut self.press_animation,
                    0.0,
                    ACTION_CARD_PRESS_ANIMATION_SECONDS,
                    ctx,
                );
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    set_action_card_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        ACTION_CARD_HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_action_card_animation_target(
                        &mut self.press_animation,
                        0.0,
                        ACTION_CARD_PRESS_ANIMATION_SECONDS,
                        ctx,
                    );
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate(ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let title_style = self.title_style();
        let description_style = self.description_style();
        let title = measure_text(ctx, &self.title, &title_style);
        let description = measure_text(ctx, &self.description, &description_style);
        self.title_measurement = Some(title);
        self.description_measurement = Some(description);

        let icon_extent = self
            .icon
            .map(|_| ACTION_CARD_ICON_BOX_SIZE + 12.0)
            .unwrap_or(0.0);
        let text_width = title.width.max(description.width).min(320.0);
        let natural = Size::new(
            self.min_width
                .max(self.padding.left + icon_extent + text_width + 22.0 + self.padding.right),
            self.min_height.max(
                self.padding.top
                    + title.height.max(title_style.line_height)
                    + ACTION_CARD_TEXT_GAP
                    + description.height.max(description_style.line_height)
                    + self.padding.bottom,
            ),
        );
        constraints.clamp(natural)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let enabled = self.is_enabled();
        let hover = if enabled {
            self.hover_animation.value
        } else {
            0.0
        };
        let press = if enabled {
            self.press_animation.value
        } else {
            0.0
        };
        let accent = self.accent.unwrap_or(palette.accent);
        let mut background = mix_color(palette.control, palette.control_hover, hover);
        background = mix_color(background, palette.control_active, press * 0.55);
        if !enabled {
            background = mix_color(background, palette.surface, 0.68).with_alpha(0.82);
        }
        let border = if !enabled {
            palette.border.with_alpha(0.55)
        } else if ctx.is_focused() {
            palette.border_focus
        } else {
            mix_color(palette.border, palette.border_hover, hover)
        };

        // Elevation shadow behind the raised card surface, drawn before the
        // fill so the soft shadow is not clipped.
        if enabled {
            paint_theme_shadow(
                ctx,
                ctx.bounds(),
                [metrics.corner_radius; 4],
                &theme.shadows.box_shadow.md,
            );
        }

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            background,
            border,
            (ctx.is_focused() && enabled).then_some(palette.focus_ring),
        );

        let bounds = ctx.bounds();
        let content = self.content_rect(bounds);
        let accent_rail = Rect::new(bounds.x(), bounds.y() + 10.0, 3.0, bounds.height() - 20.0);
        ctx.fill(rounded_rect_path(accent_rail, 1.5), accent.with_alpha(0.78));

        if let Some(icon) = self.icon {
            let icon_box_size = ACTION_CARD_ICON_BOX_SIZE
                .min(content.width())
                .min(content.height())
                .max(0.0);
            let icon_box = Rect::new(
                content.x(),
                content.y() + ((content.height() - icon_box_size) * 0.5),
                icon_box_size,
                icon_box_size,
            );
            ctx.fill(
                rounded_rect_path(icon_box, metrics.corner_radius),
                mix_color(background, accent, 0.14),
            );
            ctx.stroke(
                rounded_rect_path(icon_box, metrics.corner_radius),
                accent.with_alpha(if enabled { 0.42 } else { 0.22 }),
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
            let icon_size = ACTION_CARD_ICON_SIZE
                .min(icon_box.width())
                .min(icon_box.height())
                .max(0.0);
            let icon_rect = Rect::new(
                icon_box.x() + ((icon_box.width() - icon_size) * 0.5),
                icon_box.y() + ((icon_box.height() - icon_size) * 0.5),
                icon_size,
                icon_size,
            );
            draw_icon_glyph(
                ctx,
                icon,
                icon_rect,
                if enabled {
                    accent
                } else {
                    palette.text.with_alpha(0.34)
                },
            );
        }

        let text_bounds = self.text_bounds(bounds);
        let title_style = self.title_style();
        let description_style = self.description_style();
        let title_height = title_style.line_height.max(
            self.title_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(title_style.line_height),
        );
        let description_height = (text_bounds.height() - title_height - ACTION_CARD_TEXT_GAP)
            .max(description_style.line_height)
            .min(description_style.line_height * 2.0);
        let text_block_height = title_height + ACTION_CARD_TEXT_GAP + description_height;
        let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
        let title_rect = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
        let description_rect = Rect::new(
            text_bounds.x(),
            title_rect.max_y() + ACTION_CARD_TEXT_GAP,
            text_bounds.width(),
            description_height,
        );
        ctx.push_clip_rect(title_rect);
        ctx.draw_text(
            title_rect,
            self.title.clone(),
            TextStyle {
                color: if enabled {
                    palette.text
                } else {
                    palette.text.with_alpha(0.45)
                },
                ..title_style
            },
        );
        ctx.pop_clip();
        ctx.push_clip_rect(description_rect);
        ctx.draw_text(
            description_rect,
            self.description.clone(),
            TextStyle {
                color: if enabled {
                    palette.placeholder
                } else {
                    palette.placeholder.with_alpha(0.45)
                },
                ..description_style
            },
        );
        ctx.pop_clip();

        let chevron_size = 16.0_f32.min(content.width()).min(content.height()).max(0.0);
        let chevron = Rect::new(
            content.max_x() - chevron_size,
            content.y() + ((content.height() - chevron_size) * 0.5),
            chevron_size,
            chevron_size,
        );
        draw_icon_glyph(
            ctx,
            IconGlyph::ChevronRight,
            chevron,
            if enabled {
                mix_color(palette.placeholder, accent, hover * 0.45).with_alpha(0.74)
            } else {
                palette.placeholder.with_alpha(0.32)
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.title.clone());
        node.description = Some(self.description.clone());
        node.value = Some(SemanticsValue::Text(self.description.clone()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered && self.is_enabled();
        node.state.disabled = !self.is_enabled();
        node.actions = if self.is_enabled() {
            vec![SemanticsAction::Focus, SemanticsAction::Activate]
        } else {
            Vec::new()
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn set_action_card_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    ctx: &mut EventCtx,
) {
    if animation.set_target(target, ctx.current_time(), duration) {
        ctx.request_animation_frame();
    }
}

const PROPERTY_ROW_LABEL_WIDTH: f32 = 112.0;
const PROPERTY_ROW_GAP: f32 = 8.0;
const PROPERTY_ROW_STACKED_GAP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRowLayout {
    Stacked,
    Inline,
}

pub struct PropertyRow {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    layout: PropertyRowLayout,
    label_width: f32,
    control_width: Option<f32>,
    gap: f32,
    label_style: Option<TextStyle>,
    child: SingleChild,
    label_measurement: Option<TextMeasurement>,
}

impl PropertyRow {
    pub fn new<W>(label: impl Into<String>, control: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            layout: PropertyRowLayout::Stacked,
            label_width: PROPERTY_ROW_LABEL_WIDTH,
            control_width: None,
            gap: PROPERTY_ROW_STACKED_GAP,
            label_style: None,
            child: SingleChild::new(control),
            label_measurement: None,
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

    pub fn layout(mut self, layout: PropertyRowLayout) -> Self {
        self.layout = layout;
        if matches!(layout, PropertyRowLayout::Inline) && self.gap == PROPERTY_ROW_STACKED_GAP {
            self.gap = PROPERTY_ROW_GAP;
        }
        self
    }

    pub fn stacked(self) -> Self {
        self.layout(PropertyRowLayout::Stacked)
    }

    pub fn inline(self) -> Self {
        self.layout(PropertyRowLayout::Inline)
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.label_width = width.max(0.0);
        self
    }

    pub fn control_width(mut self, width: f32) -> Self {
        self.control_width = Some(width.max(0.0));
        self
    }

    pub fn auto_control_width(mut self) -> Self {
        self.control_width = None;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.label_style = Some(style);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    fn resolved_label_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.label_style.clone().unwrap_or_else(|| TextStyle {
            font_size: 13.0,
            line_height: 18.0,
            color: theme.palette.text_muted,
            ..theme.body_text_style()
        })
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn label_height(&self, style: &TextStyle) -> f32 {
        self.label_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn child_constraints(&self, constraints: Constraints, label_extent: f32) -> Constraints {
        let max_width = constraints.max.width;
        let available = match self.layout {
            PropertyRowLayout::Stacked => max_width,
            PropertyRowLayout::Inline => {
                if max_width.is_finite() {
                    (max_width - label_extent - self.gap).max(0.0)
                } else {
                    f32::INFINITY
                }
            }
        };
        let width = self
            .control_width
            .map(|width| width.min(available).max(0.0));

        match width {
            Some(width) => Constraints::new(
                Size::new(width, 0.0),
                Size::new(width, constraints.max.height),
            ),
            None => Constraints::new(Size::ZERO, Size::new(available, constraints.max.height)),
        }
    }

    fn child_width_for_bounds(&self, bounds: Rect, label_extent: f32) -> f32 {
        let available = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => (bounds.width() - label_extent - self.gap).max(0.0),
        };
        self.control_width
            .unwrap_or(available)
            .min(available)
            .max(0.0)
    }
}

impl Widget for PropertyRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let label_style = self.resolved_label_style();
        let label_measurement = measure_text(ctx, &self.label, &label_style);
        self.label_measurement = Some(label_measurement);
        let label_height = self.label_height(&label_style);
        let label_extent = match self.layout {
            PropertyRowLayout::Stacked => label_measurement.width,
            PropertyRowLayout::Inline => self.label_width.max(label_measurement.width),
        };
        let child_size = self
            .child
            .measure(ctx, self.child_constraints(constraints, label_extent));
        let natural = match self.layout {
            PropertyRowLayout::Stacked => Size::new(
                label_measurement.width.max(child_size.width),
                label_height + self.gap + child_size.height,
            ),
            PropertyRowLayout::Inline => Size::new(
                label_extent + self.gap + child_size.width,
                label_height.max(child_size.height),
            ),
        };

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let label_width = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => self.label_width.min(bounds.width()).max(0.0),
        };
        let child_measured = self.child.child().measured_size();
        let child_width = self.child_width_for_bounds(bounds, label_width);
        let child_height = child_measured.height.min(bounds.height()).max(0.0);

        let child_bounds = match self.layout {
            PropertyRowLayout::Stacked => Rect::new(
                bounds.x(),
                bounds.y() + label_height + self.gap,
                child_width,
                child_height.min((bounds.height() - label_height - self.gap).max(0.0)),
            ),
            PropertyRowLayout::Inline => Rect::new(
                bounds.x() + label_width + self.gap,
                bounds.y() + ((bounds.height() - child_height) * 0.5).max(0.0),
                child_width,
                child_height,
            ),
        };
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let bounds = ctx.bounds();
        let label_rect = match self.layout {
            PropertyRowLayout::Stacked => {
                Rect::new(bounds.x(), bounds.y(), bounds.width(), label_height)
            }
            PropertyRowLayout::Inline => Rect::new(
                bounds.x(),
                bounds.y() + ((bounds.height() - label_height) * 0.5).max(0.0),
                self.label_width.min(bounds.width()).max(0.0),
                label_height,
            ),
        };
        ctx.push_clip_rect(label_rect);
        ctx.draw_text(label_rect, self.label.clone(), label_style);
        ctx.pop_clip();
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut row = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        row.name = Some(self.label.clone());
        ctx.push(row);

        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let label_bounds = match self.layout {
            PropertyRowLayout::Stacked => Rect::new(
                ctx.bounds().x(),
                ctx.bounds().y(),
                ctx.bounds().width(),
                label_height,
            ),
            PropertyRowLayout::Inline => Rect::new(
                ctx.bounds().x(),
                ctx.bounds().y() + ((ctx.bounds().height() - label_height) * 0.5).max(0.0),
                self.label_width.min(ctx.bounds().width()).max(0.0),
                label_height,
            ),
        };
        let mut label = SemanticsNode::new(
            property_row_label_id(ctx.widget_id()),
            SemanticsRole::Text,
            label_bounds,
        );
        label.parent = Some(ctx.widget_id());
        label.name = Some(self.label.clone());
        label.value = Some(SemanticsValue::Text(self.label.clone()));
        ctx.push(label);

        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn property_row_label_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 1_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(271).wrapping_add(1) & LOW_MASK))
}

const PANEL_SECTION_GAP: f32 = 8.0;

pub struct PanelSection {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    title: String,
    gap: f32,
    action_gap: f32,
    title_style: Option<TextStyle>,
    header_action: Option<SingleChild>,
    child: SingleChild,
    title_measurement: Option<TextMeasurement>,
    collapsible: bool,
    expanded: bool,
    hovered_header: bool,
    pressed_header: bool,
}

impl PanelSection {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            gap: PANEL_SECTION_GAP,
            action_gap: 6.0,
            title_style: None,
            header_action: None,
            child: SingleChild::new(child),
            title_measurement: None,
            collapsible: false,
            expanded: true,
            hovered_header: false,
            pressed_header: false,
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

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn title_style(mut self, style: TextStyle) -> Self {
        self.title_style = Some(style);
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    pub fn action_gap(mut self, gap: f32) -> Self {
        self.action_gap = gap.max(0.0);
        self
    }

    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn collapsed(mut self) -> Self {
        self.expanded = false;
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.title_style.clone().unwrap_or_else(|| TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: theme.palette.text_muted,
            ..theme.body_text_style()
        })
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn header_height(&self, title_style: &TextStyle) -> f32 {
        let action_height = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().height)
            .unwrap_or(0.0);
        self.title_height(title_style).max(action_height)
    }

    fn is_expanded(&self) -> bool {
        !self.collapsible || self.expanded
    }

    fn disclosure_width(&self) -> f32 {
        if self.collapsible { 16.0 } else { 0.0 }
    }

    fn title_rect(&self, bounds: Rect, header_height: f32, title_height: f32) -> Rect {
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.action_gap)
            .unwrap_or(0.0)
            .min(bounds.width());
        let disclosure_width = self.disclosure_width();
        Rect::new(
            bounds.x() + disclosure_width,
            bounds.y() + ((header_height - title_height) * 0.5).max(0.0),
            (bounds.width() - action_width - disclosure_width).max(0.0),
            title_height,
        )
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let title_style = self.resolved_title_style();
        let header_height = self.header_height(&title_style);
        Rect::new(bounds.x(), bounds.y(), bounds.width(), header_height)
    }

    fn header_hit_rect(&self, bounds: Rect) -> Rect {
        let header = self.header_rect(bounds);
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.action_gap)
            .unwrap_or(0.0)
            .min(header.width());
        Rect::new(
            header.x(),
            header.y(),
            (header.width() - action_width).max(0.0),
            header.height(),
        )
    }

    fn toggle(&mut self, ctx: &mut EventCtx) {
        if !self.collapsible {
            return;
        }

        self.expanded = !self.expanded;
        self.pressed_header = false;
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for PanelSection {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.collapsible {
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self
                    .header_hit_rect(ctx.bounds())
                    .contains(pointer.position);
                if hovered != self.hovered_header {
                    self.hovered_header = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .header_hit_rect(ctx.bounds())
                        .contains(pointer.position) =>
            {
                self.hovered_header = true;
                self.pressed_header = true;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pressed_header =>
            {
                let hovered = self
                    .header_hit_rect(ctx.bounds())
                    .contains(pointer.position);
                if hovered {
                    self.toggle(ctx);
                }
                self.hovered_header = hovered;
                self.pressed_header = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered_header {
                    self.hovered_header = false;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_header || self.hovered_header {
                    self.hovered_header = false;
                    self.pressed_header = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "Enter" | " " => {
                        self.toggle(ctx);
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let title_style = self.resolved_title_style();
        let title_measurement = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title_measurement);
        let action_size = self
            .header_action
            .as_mut()
            .map(|action| {
                action.measure(
                    ctx,
                    Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY)),
                )
            })
            .unwrap_or(Size::ZERO);
        let header_height = self.title_height(&title_style).max(action_size.height);
        let child_size = if self.is_expanded() {
            self.child.measure(ctx, constraints)
        } else {
            Size::ZERO
        };
        let header_width = if self.header_action.is_some() {
            self.disclosure_width() + title_measurement.width + self.action_gap + action_size.width
        } else {
            self.disclosure_width() + title_measurement.width
        };
        let natural = Size::new(
            header_width.max(child_size.width),
            header_height
                + if self.is_expanded() && child_size.height > 0.0 {
                    self.gap + child_size.height
                } else {
                    0.0
                },
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let title_style = self.resolved_title_style();
        let header_height = self.header_height(&title_style);
        if let Some(action) = &mut self.header_action {
            let action_size = action.child().measured_size();
            action.arrange(
                ctx,
                Rect::new(
                    bounds.max_x() - action_size.width.min(bounds.width()),
                    bounds.y() + ((header_height - action_size.height) * 0.5).max(0.0),
                    action_size.width.min(bounds.width()).max(0.0),
                    action_size.height,
                ),
            );
        }
        let child_size = if self.is_expanded() {
            self.child.child().measured_size()
        } else {
            Size::ZERO
        };
        let child_height = if self.is_expanded() {
            child_size
                .height
                .min((bounds.height() - header_height - self.gap).max(0.0))
        } else {
            0.0
        };
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + header_height + self.gap,
                bounds.width().min(child_size.width).max(0.0),
                child_height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header_height = self.header_height(&title_style);
        let title_rect = self.title_rect(ctx.bounds(), header_height, title_height);
        if self.collapsible {
            let header_hit = self.header_hit_rect(ctx.bounds());
            let header_fill = if self.pressed_header {
                theme.palette.accent.with_alpha(0.10)
            } else if self.hovered_header {
                theme.palette.accent.with_alpha(0.06)
            } else {
                theme.palette.surface.with_alpha(0.001)
            };
            ctx.fill(rounded_rect_path(header_hit, 4.0), header_fill);
            paint_panel_section_disclosure(
                ctx,
                self.header_rect(ctx.bounds()),
                self.expanded,
                self.hovered_header,
                self.pressed_header,
                &theme,
            );
        }
        ctx.push_clip_rect(title_rect);
        ctx.draw_text(title_rect, self.title.clone(), title_style);
        ctx.pop_clip();
        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        if self.is_expanded() {
            self.child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut section = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        section.name = Some(self.title.clone());
        section.state.focused = ctx.is_focused();
        section.state.hovered = self.hovered_header;
        if self.collapsible {
            section.state.expanded = Some(self.expanded);
            section.actions = vec![
                SemanticsAction::Focus,
                SemanticsAction::Expand,
                SemanticsAction::Collapse,
            ];
        }
        ctx.push(section);

        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header_height = self.header_height(&title_style);
        let mut title = SemanticsNode::new(
            panel_section_title_id(ctx.widget_id()),
            SemanticsRole::Text,
            self.title_rect(ctx.bounds(), header_height, title_height),
        );
        title.parent = Some(ctx.widget_id());
        title.name = Some(self.title.clone());
        title.value = Some(SemanticsValue::Text(self.title.clone()));
        ctx.push(title);

        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        if self.is_expanded() {
            self.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.header_action {
            action.visit_children(visitor);
        }
        if self.is_expanded() {
            self.child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.header_action {
            action.visit_children_mut(visitor);
        }
        if self.is_expanded() {
            self.child.visit_children_mut(visitor);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.collapsible
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        if self.collapsible {
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

fn panel_section_title_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 3_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(431).wrapping_add(7) & LOW_MASK))
}

fn paint_panel_section_disclosure(
    ctx: &mut PaintCtx,
    header: Rect,
    expanded: bool,
    hovered: bool,
    pressed: bool,
    theme: &DefaultTheme,
) {
    let palette = theme.palette;
    let center = Point::new(header.x() + 7.0, header.y() + header.height() * 0.5);
    let color = if pressed {
        palette.accent
    } else if hovered {
        palette.text
    } else {
        palette.text.with_alpha(0.68)
    };
    let mut builder = PathBuilder::new();
    if expanded {
        builder
            .move_to(Point::new(center.x - 4.0, center.y - 2.0))
            .line_to(Point::new(center.x + 4.0, center.y - 2.0))
            .line_to(Point::new(center.x, center.y + 3.5));
    } else {
        builder
            .move_to(Point::new(center.x - 2.0, center.y - 4.0))
            .line_to(Point::new(center.x + 3.5, center.y))
            .line_to(Point::new(center.x - 2.0, center.y + 4.0));
    }
    ctx.fill(builder.build(), color);
}

const DOCK_PANEL_HEADER_HEIGHT: f32 = 34.0;
const DOCK_PANEL_HORIZONTAL_PADDING: f32 = 10.0;
const DOCK_PANEL_VERTICAL_PADDING: f32 = 8.0;

pub struct DockPanel {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: Option<String>,
    title: String,
    header_height: f32,
    padding: Insets,
    background: Option<Color>,
    header_background: Option<Color>,
    child: SingleChild,
    title_measurement: Option<TextMeasurement>,
}

impl DockPanel {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            title: title.into(),
            header_height: DOCK_PANEL_HEADER_HEIGHT,
            padding: Insets {
                left: DOCK_PANEL_HORIZONTAL_PADDING,
                top: DOCK_PANEL_VERTICAL_PADDING,
                right: DOCK_PANEL_HORIZONTAL_PADDING,
                bottom: DOCK_PANEL_VERTICAL_PADDING,
            },
            background: None,
            header_background: None,
            child: SingleChild::new(child),
            title_measurement: None,
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn header_background(mut self, color: Color) -> Self {
        self.header_background = Some(color);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            font_size: 13.0,
            line_height: 18.0,
            color: theme.palette.text,
            ..theme.body_text_style()
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), self.header_height)
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(
            Rect::new(
                bounds.x(),
                bounds.y() + self.header_height,
                bounds.width(),
                (bounds.height() - self.header_height).max(0.0),
            ),
            self.padding,
        )
    }

    fn child_constraints(&self, constraints: Constraints) -> Constraints {
        let width = if constraints.max.width.is_finite() {
            (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        let height = if constraints.max.height.is_finite() {
            (constraints.max.height - self.header_height - self.padding.top - self.padding.bottom)
                .max(0.0)
        } else {
            f32::INFINITY
        };
        Constraints::new(Size::ZERO, Size::new(width, height))
    }
}

impl Widget for DockPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let title_style = self.resolved_title_style();
        let title_measurement = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title_measurement);
        let child_size = self.child.measure(ctx, self.child_constraints(constraints));
        let natural = Size::new(
            (title_measurement.width + 2.0 * DOCK_PANEL_HORIZONTAL_PADDING)
                .max(child_size.width + self.padding.left + self.padding.right),
            self.header_height + self.padding.top + child_size.height + self.padding.bottom,
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let header = self.header_rect(bounds);
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let title_rect = Rect::new(
            header.x() + DOCK_PANEL_HORIZONTAL_PADDING,
            header.y() + ((header.height() - title_height) * 0.5).max(0.0),
            (header.width() - 2.0 * DOCK_PANEL_HORIZONTAL_PADDING).max(0.0),
            title_height,
        );
        let divider_height = physical_pixels(ctx, 1.0);

        ctx.fill_rect(bounds, self.background.unwrap_or(palette.surface));
        ctx.fill_rect(
            header,
            self.header_background
                .unwrap_or_else(|| palette.surface_raised.with_alpha(0.72)),
        );
        ctx.fill_rect(
            Rect::new(
                header.x(),
                header.max_y() - divider_height,
                header.width(),
                divider_height,
            ),
            palette.border,
        );
        ctx.push_clip_rect(title_rect);
        ctx.draw_text(title_rect, self.title.clone(), title_style);
        ctx.pop_clip();

        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut panel = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        panel.name = Some(self.name.clone().unwrap_or_else(|| self.title.clone()));
        ctx.push(panel);

        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header = self.header_rect(ctx.bounds());
        let mut title = SemanticsNode::new(
            dock_panel_title_id(ctx.widget_id()),
            SemanticsRole::Text,
            Rect::new(
                header.x() + DOCK_PANEL_HORIZONTAL_PADDING,
                header.y() + ((header.height() - title_height) * 0.5).max(0.0),
                (header.width() - 2.0 * DOCK_PANEL_HORIZONTAL_PADDING).max(0.0),
                title_height,
            ),
        );
        title.parent = Some(ctx.widget_id());
        title.name = Some(self.title.clone());
        title.value = Some(SemanticsValue::Text(self.title.clone()));
        ctx.push(title);

        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn dock_panel_title_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 5_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(467).wrapping_add(11) & LOW_MASK))
}

const PRESET_STRIP_ITEM_HEIGHT: f32 = 28.0;
const PRESET_STRIP_ITEM_MIN_WIDTH: f32 = 44.0;
const PRESET_STRIP_ITEM_HORIZONTAL_PADDING: f32 = 12.0;
const PRESET_STRIP_GAP: f32 = 6.0;

pub struct PresetStrip {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    presets: Vec<String>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    item_width: Option<f32>,
    item_height: f32,
    gap: f32,
    label_measurements: Vec<TextMeasurement>,
    item_widths: Vec<f32>,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl PresetStrip {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            presets: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            item_width: None,
            item_height: PRESET_STRIP_ITEM_HEIGHT,
            gap: PRESET_STRIP_GAP,
            label_measurements: Vec::new(),
            item_widths: Vec::new(),
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

    pub fn preset(mut self, preset: impl Into<String>) -> Self {
        self.presets.push(preset.into());
        self
    }

    pub fn presets<I, S>(mut self, presets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.presets.extend(presets.into_iter().map(Into::into));
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

    pub fn item_width(mut self, width: f32) -> Self {
        self.item_width = Some(width.max(0.0));
        self
    }

    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = height.max(20.0);
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

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.presets.len())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.presets.len() || self.item_widths.len() != self.presets.len() {
            return None;
        }

        let mut x = bounds.x();
        for (current, width) in self.item_widths.iter().enumerate() {
            let available = (bounds.max_x() - x).max(0.0);
            let rect = Rect::new(x, bounds.y(), width.min(available), self.item_height);
            if current == index {
                return (!rect.is_empty()).then_some(rect);
            }
            x += *width + self.gap;
        }

        None
    }

    fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.presets.iter().enumerate().find_map(|(index, _)| {
            self.item_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn activate(&mut self, index: usize) {
        if self.presets.is_empty() {
            return;
        }

        let index = index.min(self.presets.len() - 1);
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, self.presets[index].clone());
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.presets.is_empty() {
            return;
        }

        let current = self.current_selected().unwrap_or(0) as isize;
        let last = self.presets.len() as isize - 1;
        let next = (current + delta).clamp(0, last) as usize;
        self.hovered = Some(next);
        self.activate(next);
    }

    fn selected_text(&self) -> Option<String> {
        self.current_selected()
            .and_then(|index| self.presets.get(index).cloned())
    }
}

impl Widget for PresetStrip {
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
                self.hovered = self.item_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
                if self.hovered.is_some() {
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
                let hovered = self.item_at(ctx.bounds(), pointer.position);
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
                    "End" if !self.presets.is_empty() => self.activate(self.presets.len() - 1),
                    "Enter" | " " => {
                        if let Some(selected) = self.current_selected().or(Some(0)) {
                            self.activate(selected);
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
        let theme = self.resolved_theme();
        let style = TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            ..theme.body_text_style()
        };
        self.label_measurements = self
            .presets
            .iter()
            .map(|preset| measure_text(ctx, preset, &style))
            .collect();
        self.item_widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                self.item_width.unwrap_or(
                    (measurement.width + PRESET_STRIP_ITEM_HORIZONTAL_PADDING * 2.0)
                        .max(PRESET_STRIP_ITEM_MIN_WIDTH),
                )
            })
            .collect();

        let width = self.item_widths.iter().sum::<f32>()
            + (self.gap * self.presets.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width, self.item_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let selected = self.current_selected();
        let style = TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: palette.text,
            ..theme.body_text_style()
        };

        if ctx.is_focused() {
            ctx.stroke(
                rounded_rect_path(ctx.bounds().inflate(2.0, 2.0), metrics.corner_radius + 2.0),
                palette.focus_ring,
                StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
            );
        }

        for (index, preset) in self.presets.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let is_selected = selected == Some(index);
            let is_pressed = self.pressed == Some(index);
            let is_hovered = self.hovered == Some(index);
            let background = if is_selected {
                palette.accent
            } else if is_pressed {
                palette.control_active
            } else if is_hovered {
                palette.control_hover
            } else {
                palette.surface
            };
            let border = if is_selected {
                palette.accent_border
            } else if is_hovered {
                palette.border_hover
            } else {
                palette.border
            };
            let text_color = if is_selected {
                palette.accent_text
            } else {
                palette.text
            };

            draw_control_shape(
                ctx,
                rect,
                metrics.corner_radius,
                physical_pixels(ctx, metrics.border_width),
                background,
                border,
            );

            let text_rect = centered_text_rect(
                ctx,
                rect,
                Insets::all(4.0),
                self.label_measurements.get(index).copied(),
                style.line_height,
            );
            ctx.push_clip_rect(text_rect);
            ctx.draw_text(
                text_rect,
                preset.clone(),
                TextStyle {
                    color: text_color,
                    ..style.clone()
                },
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = self.selected_text().map(SemanticsValue::Text);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        let selected = self.current_selected();
        for (index, preset) in self.presets.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let mut item = SemanticsNode::new(
                preset_strip_item_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                rect,
            );
            item.parent = Some(ctx.widget_id());
            item.name = Some(preset.clone());
            item.value = Some(SemanticsValue::Text(preset.clone()));
            item.state.hovered = self.hovered == Some(index);
            item.state.selected = selected == Some(index);
            item.actions = vec![SemanticsAction::Activate];
            ctx.push(item);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.presets.is_empty()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn preset_strip_item_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 6_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(487)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

const STATUS_BAR_HEIGHT: f32 = 28.0;
const STATUS_BAR_SEGMENT_PADDING: f32 = 10.0;
const STATUS_BAR_SEGMENT_MIN_WIDTH: f32 = 86.0;

pub struct StatusBarSegment {
    text: String,
    reader: Option<Box<dyn Fn() -> String>>,
    min_width: f32,
    expand: bool,
}

impl StatusBarSegment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reader: None,
            min_width: STATUS_BAR_SEGMENT_MIN_WIDTH,
            expand: false,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self {
            text: fallback.into(),
            reader: Some(Box::new(reader)),
            min_width: STATUS_BAR_SEGMENT_MIN_WIDTH,
            expand: false,
        }
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width.max(0.0);
        self
    }

    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    fn text(&self) -> String {
        self.reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.text.clone())
    }
}

pub struct StatusBar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: Option<String>,
    height: f32,
    segments: Vec<StatusBarSegment>,
    measured_widths: Vec<f32>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            height: STATUS_BAR_HEIGHT,
            segments: Vec::new(),
            measured_widths: Vec::new(),
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(18.0);
        self
    }

    pub fn segment(mut self, segment: StatusBarSegment) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn text_segment(self, text: impl Into<String>) -> Self {
        self.segment(StatusBarSegment::new(text))
    }

    pub fn dynamic_segment<F>(self, fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.segment(StatusBarSegment::dynamic(fallback, reader))
    }

    fn text_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            font_size: 12.0,
            line_height: 18.0,
            color: theme.palette.placeholder,
            ..theme.body_text_style()
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn segment_widths(&self) -> Vec<f32> {
        if self.measured_widths.len() == self.segments.len() {
            self.measured_widths.clone()
        } else {
            self.segments
                .iter()
                .map(|segment| segment.min_width)
                .collect()
        }
    }

    fn segment_rects(&self, bounds: Rect) -> Vec<Rect> {
        let mut widths = self.segment_widths();
        let expandable = self
            .segments
            .iter()
            .filter(|segment| segment.expand)
            .count();
        if expandable > 0 {
            let fixed: f32 = widths.iter().sum();
            let extra = (bounds.width() - fixed).max(0.0) / expandable as f32;
            for (index, segment) in self.segments.iter().enumerate() {
                if segment.expand {
                    widths[index] += extra;
                }
            }
        }

        let mut x = bounds.x();
        widths
            .into_iter()
            .map(|width| {
                let available = (bounds.max_x() - x).max(0.0);
                let rect = Rect::new(x, bounds.y(), width.min(available), bounds.height());
                x = rect.max_x();
                rect
            })
            .collect()
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StatusBarHost {
    content: SingleChild,
    status_bar: SingleChild,
}

impl StatusBarHost {
    pub fn new<C, S>(content: C, status_bar: S) -> Self
    where
        C: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            content: SingleChild::new(content),
            status_bar: SingleChild::new(status_bar),
        }
    }

    pub fn content(&self) -> &sui_runtime::WidgetPod {
        self.content.child()
    }

    pub fn content_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.content.child_mut()
    }

    pub fn status_bar(&self) -> &sui_runtime::WidgetPod {
        self.status_bar.child()
    }

    pub fn status_bar_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.status_bar.child_mut()
    }
}

impl Widget for StatusBarHost {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max = constraints.max;
        let status_size = self.status_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(max.width, max.height)),
        );
        let content_max_height = if max.height.is_finite() {
            (max.height - status_size.height).max(0.0)
        } else {
            f32::INFINITY
        };
        let content_size = self.content.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(max.width, content_max_height)),
        );

        constraints.clamp(Size::new(
            content_size.width.max(status_size.width),
            content_size.height + status_size.height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let status_height = self
            .status_bar
            .child()
            .measured_size()
            .height
            .min(bounds.height())
            .max(0.0);
        let content_height = (bounds.height() - status_height).max(0.0);

        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), bounds.width(), content_height),
        );
        self.status_bar.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + content_height,
                bounds.width(),
                status_height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        self.status_bar.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        self.status_bar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.status_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.status_bar.visit_children_mut(visitor);
    }
}

impl Widget for StatusBar {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.text_style();
        self.measured_widths = self
            .segments
            .iter()
            .map(|segment| {
                let text = segment.text();
                let measured =
                    measure_text(ctx, &text, &text_style).width + STATUS_BAR_SEGMENT_PADDING * 2.0;
                segment.min_width.max(measured.ceil())
            })
            .collect();
        let natural_width: f32 = self.measured_widths.iter().sum();
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural_width
            },
            self.height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        ctx.fill_bounds(palette.surface);
        ctx.stroke_rect(
            Rect::new(bounds.x(), bounds.y(), bounds.width(), 1.0),
            palette.border,
            StrokeStyle::new(theme.metrics.border_width.max(1.0)),
        );

        let text_style = self.text_style();
        for (index, (segment, rect)) in self
            .segments
            .iter()
            .zip(self.segment_rects(bounds))
            .enumerate()
        {
            if rect.is_empty() {
                continue;
            }
            if index > 0 {
                ctx.stroke_rect(
                    Rect::new(
                        rect.x(),
                        rect.y() + 6.0,
                        1.0,
                        (rect.height() - 12.0).max(0.0),
                    ),
                    palette.border.with_alpha(0.7),
                    StrokeStyle::new(1.0),
                );
            }
            let text_rect = Rect::new(
                rect.x() + STATUS_BAR_SEGMENT_PADDING,
                rect.y() + ((rect.height() - text_style.line_height) * 0.5),
                (rect.width() - STATUS_BAR_SEGMENT_PADDING * 2.0).max(0.0),
                text_style.line_height,
            );
            ctx.push_clip_rect(text_rect);
            ctx.draw_text(text_rect, segment.text(), text_style.clone());
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = self.name.clone();
        ctx.push(node);

        for (index, (segment, rect)) in self
            .segments
            .iter()
            .zip(self.segment_rects(ctx.bounds()))
            .enumerate()
        {
            let text = segment.text();
            let mut child = SemanticsNode::new(
                status_bar_segment_id(ctx.widget_id(), index),
                SemanticsRole::Text,
                rect,
            );
            child.parent = Some(ctx.widget_id());
            child.name = Some(text.clone());
            child.value = Some(SemanticsValue::Text(text));
            ctx.push(child);
        }
    }
}

fn status_bar_segment_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 2_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(263)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub struct TabBar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
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
            theme_reader: None,
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
        self.resolved_theme().metrics.min_height
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
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
        let theme = self.resolved_theme();
        let style = theme.body_text_style();
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
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;

        ctx.fill(
            rounded_rect_path(ctx.bounds(), metrics.corner_radius),
            palette.control,
        );

        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let background = if selected {
                palette.surface_raised
            } else if pressed {
                palette.control_active
            } else if hovered {
                palette.control_hover
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
                        theme.text_style(palette.border_focus).line_height
                    } else {
                        theme.body_text_style().line_height
                    },
                ),
                tab.clone(),
                if selected {
                    theme.text_style(palette.border_focus)
                } else {
                    theme.body_text_style()
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
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
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
            theme_reader: None,
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
        self.resolved_theme().metrics.min_height
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
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
        let theme = self.resolved_theme();
        let text_style = theme.body_text_style();
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
            Size::new(0.0, theme.metrics.min_height)
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
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let header = self.header_rect(ctx.bounds());

        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            palette.control,
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
                        palette.surface_raised
                    } else if pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
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
                        theme.text_style(palette.border_focus).line_height
                    } else {
                        theme.body_text_style().line_height
                    },
                ),
                label.clone(),
                if selected {
                    theme.text_style(palette.border_focus)
                } else {
                    theme.body_text_style()
                },
            );
        }

        let content = self.panel_frame.translate(ctx.bounds().origin.to_vector());
        draw_control_frame(
            ctx,
            content,
            metrics.corner_radius + 2.0,
            metrics,
            palette.surface_raised,
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
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    items: Vec<MenuItem>,
    highlighted: Option<usize>,
    pressed: Option<usize>,
    measured_width: f32,
    focus_on_pointer_down: bool,
    on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
    on_activate_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, MenuItem)>>,
}

impl Menu {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            items: Vec::new(),
            highlighted: None,
            pressed: None,
            measured_width: 220.0,
            focus_on_pointer_down: true,
            on_activate: None,
            on_activate_with_ctx: None,
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

    pub fn on_activate_with_ctx<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, MenuItem) + 'static,
    {
        self.on_activate_with_ctx = Some(Box::new(on_activate));
        self
    }

    pub fn focus_on_pointer_down(mut self, focus_on_pointer_down: bool) -> Self {
        self.focus_on_pointer_down = focus_on_pointer_down;
        self
    }

    fn row_height(&self) -> f32 {
        let theme = self.resolved_theme();
        menu_row_height(&theme)
    }

    fn activate(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if !item.enabled {
            return;
        }
        match (&mut self.on_activate, &mut self.on_activate_with_ctx) {
            (Some(on_activate), _) => on_activate(index, item),
            (None, Some(on_activate)) => on_activate(ctx, index, item),
            (None, None) => {}
        }
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let x = bounds.x() + MENU_HORIZONTAL_PADDING;
        let y = bounds.y() + MENU_VERTICAL_PADDING + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (bounds.width() - (MENU_HORIZONTAL_PADDING * 2.0)).max(0.0),
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
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
                if self.focus_on_pointer_down {
                    ctx.request_focus();
                }
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
                    self.activate(ctx, index);
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
                            self.activate(ctx, index);
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
        let theme = self.resolved_theme();
        let label_style = theme.body_text_style();
        let shortcut_style = theme.placeholder_text_style();
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
        let height = menu_height_for_rows(self.row_height(), self.items.len());
        constraints.clamp(Size::new(
            self.measured_width,
            height.max(menu_height_for_rows(self.row_height(), 1)),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;

        // Cast an elevation shadow behind the raised menu surface before any
        // fill so the soft drop shadow is not clipped by the frame.
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(
            ctx,
            ctx.bounds(),
            [surface_radius; 4],
            &theme.shadows.box_shadow.lg,
        );

        draw_control_frame(
            ctx,
            ctx.bounds(),
            surface_radius,
            metrics,
            palette.surface_raised,
            palette.border,
            ctx.is_focused().then_some(palette.focus_ring),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(
                    row.x(),
                    row.y() - (MENU_VERTICAL_PADDING * 0.5),
                    row.width(),
                    1.0,
                );
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let pressed = self.pressed == Some(index);
            let label_style = theme.text_style(item.text_color(&theme));
            let label_measurement = paint_text_measurement(ctx, &item.label, &label_style);
            if highlighted || pressed {
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    if pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
                    },
                );
            }

            ctx.draw_text(
                vertically_centered_text_rect(
                    ctx,
                    Rect::new(row.x() + 12.0, row.y(), row.width() - 24.0, row.height()),
                    Some(label_measurement),
                    label_style.line_height,
                ),
                item.label.clone(),
                label_style,
            );

            if let Some(shortcut) = &item.shortcut {
                let shortcut_style = theme.placeholder_text_style();
                let shortcut_measurement = paint_text_measurement(ctx, shortcut, &shortcut_style);
                ctx.draw_text(
                    vertically_centered_text_rect(
                        ctx,
                        Rect::new(row.max_x() - 120.0, row.y(), 108.0, row.height()),
                        Some(shortcut_measurement),
                        shortcut_style.line_height,
                    ),
                    shortcut.clone(),
                    shortcut_style,
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
        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            ctx.push(menu_item_semantics_node(
                ctx.widget_id(),
                index,
                item,
                row,
                self.highlighted == Some(index),
            ));
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

const PRESENTATION_EPSILON: f32 = 1e-4;
const TOOLTIP_ANIMATION_SECONDS: f64 = 0.18;
const TOOLTIP_REVEAL_OFFSET_PX: f32 = 8.0;
const POPOVER_ANIMATION_SECONDS: f64 = 0.18;
const POPOVER_REVEAL_OFFSET_PX: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimatedScalar {
    value: f32,
    target: f32,
    transition: Option<Transition<f32>>,
}

impl AnimatedScalar {
    const fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            transition: None,
        }
    }

    fn current(&self, time: f64) -> f32 {
        self.transition
            .map(|transition| transition.sample(time))
            .unwrap_or(self.value)
    }

    fn set_target(&mut self, target: f32, time: f64, duration: f64) -> bool {
        let target = target.clamp(0.0, 1.0);
        let current = self.current(time);
        if (current - target).abs() < PRESENTATION_EPSILON {
            self.value = target;
            self.target = target;
            self.transition = None;
            return false;
        }

        self.value = current;
        self.target = target;
        self.transition = Some(Transition::new(
            current,
            target,
            time,
            duration,
            Easing::EaseInOut,
        ));
        true
    }

    fn advance(&mut self, time: f64) -> bool {
        let Some(transition) = self.transition else {
            return false;
        };

        self.value = transition.sample(time);
        if transition.is_complete(time) {
            self.value = self.target;
            self.transition = None;
            return false;
        }

        true
    }

    fn is_presented(&self) -> bool {
        self.value > PRESENTATION_EPSILON
            || self.target > PRESENTATION_EPSILON
            || self.transition.is_some()
    }
}

fn request_child_invalidation(ctx: &mut EventCtx, widget_id: WidgetId, kind: InvalidationKind) {
    ctx.request(InvalidationRequest::new(
        InvalidationTarget::Widget(widget_id),
        kind,
    ));
}

fn tooltip_fallback_measurement(theme: &DefaultTheme) -> TextMeasurement {
    TextMeasurement {
        width: 120.0,
        height: theme.typography.body_line_height,
        bounds: Rect::new(0.0, 0.0, 120.0, theme.typography.body_line_height),
        ascent: theme.typography.body_font_size,
        descent: 0.0,
        cap_height: Some(theme.typography.body_font_size),
    }
}

fn tooltip_bubble_rect(
    trigger_bounds: Rect,
    measurement: Option<TextMeasurement>,
    theme: &DefaultTheme,
    placement: TooltipPlacement,
) -> Rect {
    let measurement = measurement.unwrap_or_else(|| tooltip_fallback_measurement(theme));
    let width = (measurement.width + 24.0).max(96.0);
    let height = measurement.height.max(theme.typography.body_line_height) + 18.0;
    let x = trigger_bounds.x() + ((trigger_bounds.width() - width) * 0.5);
    let y = match placement {
        TooltipPlacement::Above => trigger_bounds.y() - height - 10.0,
        TooltipPlacement::Below => trigger_bounds.max_y() + 10.0,
    };
    Rect::new(x, y, width, height)
}

#[derive(Debug, Clone)]
struct TooltipPresentationState {
    theme: DefaultTheme,
    text: String,
    placement: TooltipPlacement,
    measurement: Option<TextMeasurement>,
    hovered: bool,
    trigger_bounds: Rect,
    bubble_bounds: Rect,
    reveal: AnimatedScalar,
}

impl TooltipPresentationState {
    fn new(text: String) -> Self {
        Self {
            theme: DefaultTheme::default(),
            text,
            placement: TooltipPlacement::Above,
            measurement: None,
            hovered: false,
            trigger_bounds: Rect::ZERO,
            bubble_bounds: Rect::ZERO,
            reveal: AnimatedScalar::new(0.0),
        }
    }

    fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    fn layer_properties(&self) -> LayerProperties {
        let direction = match self.placement {
            TooltipPlacement::Above => -1.0,
            TooltipPlacement::Below => 1.0,
        };
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                TOOLTIP_REVEAL_OFFSET_PX * (1.0 - self.reveal.value) * direction,
            ),
        }
    }
}

struct TooltipOverlay {
    state: Rc<RefCell<TooltipPresentationState>>,
}

impl TooltipOverlay {
    fn new(state: Rc<RefCell<TooltipPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for TooltipOverlay {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if !state.is_presented() {
            return Size::ZERO;
        }
        state.bubble_bounds.size
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        self.state.borrow_mut().bubble_bounds = bounds;
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let bubble = ctx.bounds();
        let metrics = state.theme.metrics;
        // Soft elevation behind the tooltip bubble, drawn before the fill.
        paint_theme_shadow(
            ctx,
            bubble,
            [metrics.corner_radius; 4],
            &state.theme.shadows.box_shadow.sm,
        );
        draw_control_frame(
            ctx,
            bubble,
            metrics.corner_radius,
            metrics,
            Color::rgba(0.10, 0.14, 0.20, 0.96),
            Color::rgba(0.05, 0.08, 0.12, 1.0),
            None,
        );
        let tail = tooltip_tail(state.trigger_bounds, bubble, state.placement);
        ctx.fill(tail, Color::rgba(0.10, 0.14, 0.20, 0.96));
        ctx.draw_text(
            inset_rect(bubble, Insets::all(9.0)),
            state.text.clone(),
            state.theme.text_style(Color::rgba(1.0, 1.0, 1.0, 1.0)),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                ..StackSurfaceOptions::default()
            })
    }
}

pub struct Tooltip {
    child: SingleChild,
    overlay: SingleChild,
    state: Rc<RefCell<TooltipPresentationState>>,
}

impl Tooltip {
    pub fn new<W>(text: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let state = Rc::new(RefCell::new(TooltipPresentationState::new(text.into())));
        Self {
            child: SingleChild::new(child),
            overlay: SingleChild::new(TooltipOverlay::new(Rc::clone(&state))),
            state,
        }
    }

    pub fn theme(self, theme: DefaultTheme) -> Self {
        self.state.borrow_mut().theme = theme;
        self
    }

    pub fn placement(self, placement: TooltipPlacement) -> Self {
        self.state.borrow_mut().placement = placement;
        self
    }

    fn set_hovered(&mut self, ctx: &mut EventCtx, hovered: bool) {
        let overlay_id = self.overlay.child().id();
        let mut state = self.state.borrow_mut();
        if state.hovered == hovered {
            return;
        }
        let was_presented = state.is_presented();
        state.hovered = hovered;
        let should_animate = state.reveal.set_target(
            hovered as u8 as f32,
            ctx.current_time(),
            TOOLTIP_ANIMATION_SECONDS,
        );
        let is_presented = state.is_presented();
        drop(state);

        if was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, overlay_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_semantics();
    }
}

impl Widget for Tooltip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let overlay_id = self.overlay.child().id();
                let mut state = self.state.borrow_mut();
                let was_presented = state.is_presented();
                let previous = state.reveal.value;
                let animating = state.reveal.advance(*time);
                let changed = (state.reveal.value - previous).abs() > PRESENTATION_EPSILON;
                let is_presented = state.is_presented();
                drop(state);

                if changed {
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Effect);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Visibility);
                }
                if animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let mut state = self.state.borrow_mut();
        state.measurement = Some(measure_text(
            ctx,
            &state.text,
            &state.theme.placeholder_text_style(),
        ));
        drop(state);
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_bounds =
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size());
        self.child.arrange(ctx, trigger_bounds);

        let mut state = self.state.borrow_mut();
        state.trigger_bounds = trigger_bounds;
        state.bubble_bounds = tooltip_bubble_rect(
            trigger_bounds,
            state.measurement,
            &state.theme,
            state.placement,
        );
        let overlay_bounds = if state.is_presented() {
            state.bubble_bounds
        } else {
            Rect::from_origin_size(trigger_bounds.origin, Size::ZERO)
        };
        drop(state);
        self.overlay.arrange(ctx, overlay_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        self.overlay.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
        let state = self.state.borrow();
        if state.hovered {
            let mut node =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tooltip, state.bubble_bounds);
            node.name = Some(state.text.clone());
            ctx.push(node);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
        if self.state.borrow().is_presented() {
            self.overlay.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
        if self.state.borrow().is_presented() {
            self.overlay.visit_children_mut(visitor);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PopoverVisuals {
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
    surface_style: Option<ResolvedHdrStyle>,
    arrival_effect: Option<ResolvedEffectStyle>,
}

#[derive(Debug, Clone)]
struct PopoverSurfaceState {
    theme: DefaultTheme,
    padding: Insets,
    frame_rect: Rect,
    arrival_active: bool,
    reveal: AnimatedScalar,
}

impl PopoverSurfaceState {
    fn new() -> Self {
        Self {
            theme: DefaultTheme::default(),
            padding: Insets::all(14.0),
            frame_rect: Rect::ZERO,
            arrival_active: false,
            reveal: AnimatedScalar::new(0.0),
        }
    }

    fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    fn arrival_duration(&self) -> f64 {
        (0.18 / self.theme.hdr.effects.pulse.speed.max(0.25) as f64).clamp(0.10, 0.28)
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(0.0, -POPOVER_REVEAL_OFFSET_PX * (1.0 - self.reveal.value)),
        }
    }

    fn resolved_visuals(&self) -> PopoverVisuals {
        let palette = self.theme.palette;

        if !self.is_presented() || matches!(self.theme.hdr.mode, HdrThemeMode::Disabled) {
            return PopoverVisuals {
                background: palette.surface_raised,
                border: palette.border,
                focus_ring: Some(palette.focus_ring),
                surface_style: None,
                arrival_effect: None,
            };
        }

        let surface_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::SurfaceElevated,
            WidgetLuminanceRole::Standard,
            WidgetMaterialRole::Raised,
            self.arrival_active.then_some(WidgetEffectRole::Pulse),
        ));
        let border_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::SurfaceOutline,
            WidgetLuminanceRole::Standard,
            WidgetMaterialRole::Flat,
            None,
        ));

        PopoverVisuals {
            background: surface_style.color,
            border: border_style.color,
            focus_ring: Some(border_style.color.with_alpha(palette.focus_ring.alpha)),
            surface_style: Some(surface_style),
            arrival_effect: surface_style.effect,
        }
    }
}

struct PopoverSurface {
    content: SingleChild,
    state: Rc<RefCell<PopoverSurfaceState>>,
}

impl PopoverSurface {
    fn new<C>(state: Rc<RefCell<PopoverSurfaceState>>, content: C) -> Self
    where
        C: Widget + 'static,
    {
        Self {
            content: SingleChild::new(content),
            state,
        }
    }
}

impl Widget for PopoverSurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if !state.is_presented() {
            return Size::ZERO;
        }
        let padding = state.padding;
        drop(state);

        let content_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                if constraints.max.width.is_finite() {
                    (constraints.max.width - padding.left - padding.right).max(0.0)
                } else {
                    f32::INFINITY
                },
                if constraints.max.height.is_finite() {
                    (constraints.max.height - padding.top - padding.bottom).max(0.0)
                } else {
                    f32::INFINITY
                },
            ),
        );
        let content_size = self.content.measure(ctx, content_constraints);
        Size::new(
            content_size.width + padding.left + padding.right,
            content_size.height + padding.top + padding.bottom,
        )
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let state = self.state.borrow();
        if !state.is_presented() {
            drop(state);
            self.content
                .arrange(ctx, Rect::from_origin_size(bounds.origin, Size::ZERO));
            return;
        }
        let padding = state.padding;
        drop(state);
        let content_size = self.content.child().measured_size();
        self.content.arrange(
            ctx,
            Rect::new(
                bounds.x() + padding.left,
                bounds.y() + padding.top,
                content_size.width,
                content_size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let rect = ctx.bounds();
        let metrics = state.theme.metrics;
        let visuals = state.resolved_visuals();
        // Elevation shadow behind the popover surface, drawn before the fill.
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(
            ctx,
            rect,
            [surface_radius; 4],
            &state.theme.shadows.box_shadow.md,
        );
        draw_control_frame(
            ctx,
            rect,
            surface_radius,
            metrics,
            visuals.background,
            visuals.border,
            visuals.focus_ring,
        );
        if let Some(arrival_effect) = visuals.arrival_effect {
            draw_popover_arrival_overlay(
                ctx,
                rect,
                metrics,
                visuals.background,
                visuals.border,
                arrival_effect,
            );
        }
        drop(state);
        self.content.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                ..StackSurfaceOptions::default()
            })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

pub struct Popover {
    name: String,
    trigger: SingleChild,
    surface: SingleChild,
    open: bool,
    gap: f32,
    arrival_timer: Option<TimerToken>,
    state: Rc<RefCell<PopoverSurfaceState>>,
}

impl Popover {
    pub fn new<T, C>(name: impl Into<String>, trigger: T, content: C) -> Self
    where
        T: Widget + 'static,
        C: Widget + 'static,
    {
        let state = Rc::new(RefCell::new(PopoverSurfaceState::new()));
        Self {
            name: name.into(),
            trigger: SingleChild::new(trigger),
            surface: SingleChild::new(PopoverSurface::new(Rc::clone(&state), content)),
            open: false,
            gap: 8.0,
            arrival_timer: None,
            state,
        }
    }

    pub fn theme(self, theme: DefaultTheme) -> Self {
        self.state.borrow_mut().theme = theme;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        {
            let mut state = self.state.borrow_mut();
            state.reveal = AnimatedScalar::new(if open { 1.0 } else { 0.0 });
        }
        self
    }

    fn start_arrival(&mut self, ctx: &mut EventCtx) {
        if let Some(token) = self.arrival_timer.take() {
            ctx.cancel_timer(token);
        }

        let mut state = self.state.borrow_mut();
        state.arrival_active = !matches!(state.theme.hdr.mode, HdrThemeMode::Disabled)
            && state.theme.hdr.effects.pulse.intensity > 0.0;
        if state.arrival_active {
            self.arrival_timer = Some(ctx.schedule_timer_after(state.arrival_duration()));
        }
    }

    fn stop_arrival(&mut self, ctx: &mut EventCtx) {
        self.state.borrow_mut().arrival_active = false;
        if let Some(token) = self.arrival_timer.take() {
            ctx.cancel_timer(token);
        }
    }

    fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    fn content_rect(&self) -> Rect {
        self.state.borrow().frame_rect
    }

    fn is_inside_open_regions(&self, position: Point) -> bool {
        self.trigger_rect().contains(position)
            || (self.open && self.content_rect().contains(position))
    }

    fn set_open(&mut self, ctx: &mut EventCtx, open: bool) {
        if self.open == open {
            return;
        }

        if open {
            self.start_arrival(ctx);
        } else {
            self.stop_arrival(ctx);
        }

        self.open = open;
        let surface_id = self.surface.child().id();
        let mut state = self.state.borrow_mut();
        let was_presented = state.is_presented();
        let should_animate = state.reveal.set_target(
            open as u8 as f32,
            ctx.current_time(),
            POPOVER_ANIMATION_SECONDS,
        );
        let is_presented = state.is_presented();
        drop(state);

        if open || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_semantics();
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
                let next = !self.open;
                self.set_open(ctx, next);
                ctx.request_focus();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open
                    && !self.is_inside_open_regions(pointer.position) =>
            {
                self.set_open(ctx, false);
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && key.key == "Escape"
                    && self.open =>
            {
                self.set_open(ctx, false);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let surface_id = self.surface.child().id();
                let mut state = self.state.borrow_mut();
                let was_presented = state.is_presented();
                let previous = state.reveal.value;
                let animating = state.reveal.advance(*time);
                let changed = (state.reveal.value - previous).abs() > PRESENTATION_EPSILON;
                let is_presented = state.is_presented();
                drop(state);

                if changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                }
                if animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::Timer { token, .. }) if self.arrival_timer == Some(*token) => {
                self.arrival_timer = None;
                let surface_id = self.surface.child().id();
                let mut state = self.state.borrow_mut();
                if state.arrival_active {
                    state.arrival_active = false;
                    drop(state);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
                } else {
                    drop(state);
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
        let surface_max = Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                f32::INFINITY
            },
            if constraints.max.height.is_finite() {
                (constraints.max.height - trigger_size.height - self.gap).max(0.0)
            } else {
                f32::INFINITY
            },
        );
        let surface_size = self
            .surface
            .measure(ctx, Constraints::new(Size::ZERO, surface_max));
        let presented = self.state.borrow().is_presented();
        let size = if presented {
            Size::new(
                surface_size.width.max(trigger_size.width),
                trigger_size.height + self.gap + surface_size.height,
            )
        } else {
            trigger_size
        };
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        let trigger_bounds = Rect::from_origin_size(bounds.origin, trigger_size);
        self.trigger.arrange(ctx, trigger_bounds);

        let presented = self.state.borrow().is_presented();
        let surface_bounds = if presented {
            let surface_size = self.surface.child().measured_size();
            Rect::new(
                bounds.x(),
                bounds.y() + trigger_size.height + self.gap,
                surface_size.width.max(trigger_size.width),
                surface_size.height,
            )
        } else {
            Rect::from_origin_size(trigger_bounds.origin, Size::ZERO)
        };
        self.state.borrow_mut().frame_rect = surface_bounds;
        self.surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.state.borrow().is_presented() {
            self.surface.paint(ctx);
        }
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
            self.surface.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.open {
            self.set_open(ctx, false);
        }
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.open {
            self.surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.open {
            self.surface.visit_children_mut(visitor);
        }
    }
}

pub struct ContextMenu {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    trigger: SingleChild,
    items: Vec<MenuItem>,
    open: bool,
    highlighted: Option<usize>,
    pressed: Option<usize>,
    frame_rect: Rect,
    activation_button: PointerButton,
    on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
    on_activate_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, MenuItem)>>,
}

impl ContextMenu {
    pub fn new<W>(name: impl Into<String>, trigger: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            trigger: SingleChild::new(trigger),
            items: Vec::new(),
            open: false,
            highlighted: None,
            pressed: None,
            frame_rect: Rect::ZERO,
            activation_button: PointerButton::Secondary,
            on_activate: None,
            on_activate_with_ctx: None,
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

    pub fn on_activate_with_ctx<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, MenuItem) + 'static,
    {
        self.on_activate_with_ctx = Some(Box::new(on_activate));
        self
    }

    pub fn activation_button(mut self, activation_button: PointerButton) -> Self {
        self.activation_button = activation_button;
        self
    }

    fn row_height(&self) -> f32 {
        menu_row_height(&self.resolved_theme())
    }

    fn measured_menu_width(&self, ctx: &mut MeasureCtx) -> f32 {
        let theme = self.resolved_theme();
        let label_style = theme.body_text_style();
        let shortcut_style = theme.placeholder_text_style();
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() || !self.open {
            return None;
        }
        let menu = self.frame_rect.translate(bounds.origin.to_vector());
        let x = menu.x() + MENU_HORIZONTAL_PADDING;
        let y = menu.y() + MENU_VERTICAL_PADDING + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (menu.width() - (MENU_HORIZONTAL_PADDING * 2.0)).max(0.0),
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

    fn activate(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if !item.enabled {
            return;
        }
        if let Some(on_activate) = &mut self.on_activate {
            on_activate(index, item.clone());
        }
        if let Some(on_activate) = &mut self.on_activate_with_ctx {
            on_activate(ctx, index, item);
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
                    && pointer.button == Some(self.activation_button)
                    && self.trigger_rect().contains(pointer.position) =>
            {
                self.open = !self.open;
                self.highlighted = if self.open {
                    self.items.iter().position(|item| item.enabled)
                } else {
                    None
                };
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
                    self.activate(ctx, index);
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
                            self.activate(ctx, index);
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
            let height = menu_height_for_rows(self.row_height(), self.items.len());
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let palette = theme.palette;
        // Elevation shadow behind the raised context-menu surface.
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(ctx, menu, [surface_radius; 4], &theme.shadows.box_shadow.lg);
        draw_control_frame(
            ctx,
            menu,
            surface_radius,
            metrics,
            palette.surface_raised,
            palette.border,
            Some(palette.focus_ring),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(
                    row.x(),
                    row.y() - (MENU_VERTICAL_PADDING * 0.5),
                    row.width(),
                    1.0,
                );
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let pressed = self.pressed == Some(index);
            let label_style = theme.text_style(item.text_color(&theme));
            let label_measurement = paint_text_measurement(ctx, &item.label, &label_style);
            if highlighted || pressed {
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    if pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
                    },
                );
            }

            ctx.draw_text(
                vertically_centered_text_rect(
                    ctx,
                    Rect::new(row.x() + 12.0, row.y(), row.width() - 24.0, row.height()),
                    Some(label_measurement),
                    label_style.line_height,
                ),
                item.label.clone(),
                label_style,
            );

            if let Some(shortcut) = &item.shortcut {
                let shortcut_style = theme.placeholder_text_style();
                let shortcut_measurement = paint_text_measurement(ctx, shortcut, &shortcut_style);
                ctx.draw_text(
                    vertically_centered_text_rect(
                        ctx,
                        Rect::new(row.max_x() - 120.0, row.y(), 108.0, row.height()),
                        Some(shortcut_measurement),
                        shortcut_style.line_height,
                    ),
                    shortcut.clone(),
                    shortcut_style,
                );
            }
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
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
        if self.open {
            for (index, item) in self.items.iter().enumerate() {
                let Some(row) = self.item_rect(ctx.bounds(), index) else {
                    continue;
                };
                ctx.push(menu_item_semantics_node(
                    ctx.widget_id(),
                    index,
                    item,
                    row,
                    self.highlighted == Some(index),
                ));
            }
        }
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
        // Prominent elevation shadow behind the dialog surface, drawn over the
        // (optional) modal backdrop and before the surface fill.
        let surface_radius = metrics.corner_radius + 3.0;
        paint_theme_shadow(
            ctx,
            dialog,
            [surface_radius; 4],
            &self.theme.shadows.box_shadow.xl,
        );
        draw_control_frame(
            ctx,
            dialog,
            surface_radius,
            metrics,
            palette.surface_raised,
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
            paint_boundary: PaintBoundaryMode::Explicit,
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
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
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
            theme_reader: None,
            name: name.into(),
            min: 0.0,
            max: 1.0,
            value: 0.0,
            show_value: false,
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for ProgressBar {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let min_height = if self.show_value {
            theme.body_text_style().line_height.max(18.0)
        } else {
            18.0
        };
        constraints.clamp(Size::new(240.0, min_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let palette = theme.palette;
        draw_control_shape(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            physical_pixels(ctx, metrics.border_width),
            palette.control,
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
            let label = format!("{:.0}%", self.fraction() * 100.0);
            let text_style = theme.text_style(palette.accent_text);
            let text_measurement = paint_text_measurement(ctx, &label, &text_style);
            ctx.draw_text(
                centered_text_rect(
                    ctx,
                    ctx.bounds(),
                    Insets::all(2.0),
                    Some(text_measurement),
                    text_style.line_height,
                ),
                label,
                text_style,
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
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    size: f32,
    label: Option<String>,
}

impl Spinner {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            size: 20.0,
            label: None,
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

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for Spinner {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let label_width = self
            .label
            .as_ref()
            .map(|label| measure_text(ctx, label, &theme.body_text_style()).width + 12.0)
            .unwrap_or(0.0);
        constraints.clamp(Size::new(self.size + label_width, self.size.max(20.0)))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let indicator = self.indicator_rect(ctx.bounds());
        let center = rect_center(indicator);
        let radius = indicator.width().min(indicator.height()) * 0.4;
        let dot_radius = (indicator.width() * 0.09).max(1.5);
        for index in 0..10 {
            let angle = (index as f32 / 10.0) * std::f32::consts::TAU;
            let alpha = 0.22 + ((index as f32) / 10.0) * 0.72;
            let color = Color::rgba(
                palette.accent.red,
                palette.accent.green,
                palette.accent.blue,
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
                theme.body_text_style(),
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
    ctx.layout()
        .measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}

fn paint_text_measurement(ctx: &PaintCtx, text: &str, style: &TextStyle) -> TextMeasurement {
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

fn mix_color(left: Color, right: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        red: left.red + ((right.red - left.red) * amount),
        green: left.green + ((right.green - left.green) * amount),
        blue: left.blue + ((right.blue - left.blue) * amount),
        alpha: left.alpha + ((right.alpha - left.alpha) * amount),
        ..left
    }
}

fn draw_popover_arrival_overlay(
    ctx: &mut PaintCtx,
    rect: Rect,
    metrics: ControlMetrics,
    background: Color,
    border: Color,
    arrival_effect: ResolvedEffectStyle,
) {
    let overlay_inset = physical_pixels(ctx, 1.0);
    let overlay_rect = rect.inflate(-overlay_inset, -overlay_inset);
    let overlay_radius = (metrics.corner_radius + 2.0 - overlay_inset).max(0.0);
    let overlay_fill = mix_color(background, arrival_effect.color, 0.35)
        .with_alpha((0.10 + (arrival_effect.intensity * 0.12)).clamp(0.0, 0.22));
    let stroke_color = apply_hdr_policy_cap(
        mix_color(border, arrival_effect.color, 0.55),
        arrival_effect
            .color
            .red
            .max(arrival_effect.color.green.max(arrival_effect.color.blue)),
    )
    .with_alpha((0.16 + (arrival_effect.intensity * 0.12)).clamp(0.0, 0.30));

    ctx.fill(
        rounded_rect_path(overlay_rect, overlay_radius),
        overlay_fill,
    );
    ctx.stroke(
        rounded_rect_path(
            overlay_rect.inflate(-overlay_inset * 0.5, -overlay_inset * 0.5),
            (overlay_radius - (overlay_inset * 0.5)).max(0.0),
        ),
        stroke_color,
        StrokeStyle::new(physical_pixels(ctx, 1.0)),
    );
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

fn vertically_centered_text_rect(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: Option<TextMeasurement>,
    line_height: f32,
) -> Rect {
    let Some(measurement) = measurement else {
        return rect;
    };

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
        rect.x(),
        baseline - measurement.ascent - leading_above,
        rect.width(),
        height,
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::Tabs;
    use super::{
        ActionCard, CommandGroup, ContextMenu, Dialog, DockPanel, MENU_VERTICAL_PADDING, Menu,
        MenuItem, PanelSection, Popover, PresetStrip, ProgressBar, PropertyRow, PropertyRowLayout,
        Spinner, StatusBar, StatusBarHost, StatusBarSegment, TabBar, ToolPalette, ToolPaletteItem,
        Toolbar,
    };
    use crate::FloatingStack;
    use crate::{DefaultTheme, HdrThemeMode, SemanticColorToken};
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue,
        Size, WidgetId,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, ArrangeCtx, MeasureCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx, Widget,
        WindowBuilder,
    };
    use sui_scene::{Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor};
    use sui_text::{FontRegistry, TextSystem};

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
    fn action_card_exposes_accessible_description() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 104.0))
                .with_child(
                    ActionCard::new(
                        "Paint",
                        "Pixel canvas painting workspace with editor-style panels.",
                    )
                    .icon(crate::IconGlyph::Brush)
                    .accent(Color::rgba(0.80, 0.22, 0.44, 1.0)),
                ),
        );

        let card = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("action card should expose button semantics");
        assert_eq!(card.name.as_deref(), Some("Paint"));
        assert_eq!(
            card.description.as_deref(),
            Some("Pixel canvas painting workspace with editor-style panels.")
        );
        assert_eq!(
            card.value,
            Some(SemanticsValue::Text(
                "Pixel canvas painting workspace with editor-style panels.".to_string()
            ))
        );
        assert!(card.actions.contains(&SemanticsAction::Focus));
        assert!(card.actions.contains(&SemanticsAction::Activate));
    }

    #[test]
    fn preset_strip_exposes_selected_preset_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(220.0, 32.0))
                .with_child(
                    PresetStrip::new("Brush presets")
                        .presets(["8 px", "18 px", "36 px"])
                        .selected(1),
                ),
        );

        let strip = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Brush presets")
            })
            .expect("preset strip container semantics should exist");
        assert_eq!(strip.value, Some(SemanticsValue::Text("18 px".to_string())));
        assert!(strip.actions.contains(&SemanticsAction::SetValue));

        let selected = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("18 px")
            })
            .expect("selected preset button semantics should exist");
        assert!(selected.state.selected);
        assert_eq!(
            selected.value,
            Some(SemanticsValue::Text("18 px".to_string()))
        );
    }

    #[test]
    fn preset_strip_pointer_activation_updates_selection() -> sui_core::Result<()> {
        let chosen = Rc::new(RefCell::new(None));
        let chosen_writer = Rc::clone(&chosen);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(220.0, 32.0))
                .with_child(
                    PresetStrip::new("Brush presets")
                        .presets(["8 px", "18 px", "36 px"])
                        .selected(0)
                        .on_change(move |index, label| {
                            *chosen_writer.borrow_mut() = Some((index, label));
                        }),
                ),
        );
        let output = runtime.render(window_id)?;
        let preset = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("36 px")
            })
            .expect("target preset button should exist");
        let position = super::rect_center(preset.bounds);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        assert_eq!(*chosen.borrow(), Some((2, "36 px".to_string())));
        let output = runtime.render(window_id)?;
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("36 px")
                && node.state.selected
        }));
        Ok(())
    }

    #[test]
    fn status_bar_exposes_dynamic_segment_semantics() {
        let zoom = Rc::new(RefCell::new("Zoom 35%".to_string()));
        let zoom_reader = Rc::clone(&zoom);
        let output = render(
            StatusBar::new()
                .name("Editor status")
                .segment(StatusBarSegment::new("Ready").min_width(80.0))
                .segment(
                    StatusBarSegment::dynamic("Zoom --", move || zoom_reader.borrow().clone())
                        .min_width(120.0),
                ),
        );

        let status = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Editor status")
            })
            .expect("status bar container semantics should exist");
        assert_eq!(status.bounds.height(), 28.0);
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Zoom 35%")
        }));
    }

    #[test]
    fn status_bar_sizes_segments_from_measured_text() {
        let output = render(
            StatusBar::new()
                .name("Editor status")
                .segment(StatusBarSegment::new(
                    "Layer Paint / Normal / 100% / Unlocked",
                ))
                .segment(StatusBarSegment::new("Cursor --").expand(true)),
        );

        let layer = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paint / Normal / 100% / Unlocked")
            })
            .expect("long status segment should expose text semantics");
        assert!(
            layer.bounds.width() > 220.0,
            "expected status segment width to grow from text measurement, got {:?}",
            layer.bounds
        );
    }

    #[test]
    fn horizontal_toolbar_centers_children_and_exposes_group_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 52.0))
                .with_child(
                    Toolbar::horizontal()
                        .name("Editor toolbar")
                        .with_child(crate::Button::new("Fit").min_width(48.0).min_height(32.0))
                        .with_child(
                            crate::Button::new("Export")
                                .min_width(72.0)
                                .min_height(32.0),
                        ),
                ),
        );

        let toolbar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Editor toolbar")
            })
            .expect("toolbar semantics should exist");
        assert_eq!(toolbar.bounds, Rect::new(0.0, 0.0, 320.0, 52.0));

        let fit = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Fit"))
            .expect("toolbar child button should exist");
        assert!(fit.bounds.y() > 0.0);
        assert!(fit.bounds.max_y() < toolbar.bounds.max_y());
    }

    #[test]
    fn command_group_keeps_natural_size_and_exposes_group_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 48.0))
                .with_child(
                    Toolbar::horizontal()
                        .name("Editor toolbar")
                        .padding(sui_layout::Padding::all(4.0))
                        .with_child(
                            CommandGroup::horizontal("History commands")
                                .with_child(
                                    crate::IconButton::new(crate::IconGlyph::Undo, "Undo")
                                        .size(28.0),
                                )
                                .with_child(
                                    crate::IconButton::new(crate::IconGlyph::Redo, "Redo")
                                        .size(28.0),
                                ),
                        ),
                ),
        );

        let group = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("History commands")
            })
            .expect("command group semantics should exist");
        assert_eq!(group.bounds.width(), 63.0);
        assert_eq!(group.bounds.height(), 32.0);

        for name in ["Undo", "Redo"] {
            let button = output
                .semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(name)
                })
                .expect("command button should exist");
            assert!(button.bounds.x() >= group.bounds.x());
            assert!(button.bounds.max_x() <= group.bounds.max_x());
        }
    }

    #[test]
    fn vertical_toolbar_uses_fixed_extent_and_centers_children() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(80.0, 180.0))
                .with_child(
                    Toolbar::vertical()
                        .name("Paint tools")
                        .extent(60.0)
                        .with_child(
                            crate::IconButton::new(crate::IconGlyph::Brush, "Brush tool")
                                .size(44.0),
                        )
                        .with_child(
                            crate::IconButton::new(crate::IconGlyph::Eraser, "Eraser tool")
                                .size(44.0),
                        ),
                ),
        );

        let toolbar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Paint tools")
            })
            .expect("vertical toolbar semantics should exist");
        assert_eq!(toolbar.bounds, Rect::new(0.0, 0.0, 80.0, 180.0));

        let brush = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Brush tool")
            })
            .expect("toolbar child button should exist");
        assert_eq!(brush.bounds.width(), 44.0);
        assert!((brush.bounds.x() - 18.0).abs() < 0.001);
    }

    #[test]
    fn tool_palette_exposes_selected_tool_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(64.0, 180.0))
                .with_child(
                    ToolPalette::vertical("Paint tools")
                        .items([
                            ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                            ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                            ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                        ])
                        .selected(1),
                ),
        );

        let palette = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Paint tools")
            })
            .expect("tool palette container semantics should exist");
        assert_eq!(
            palette.value,
            Some(SemanticsValue::Text("Eraser tool".to_string()))
        );
        assert!(palette.actions.contains(&SemanticsAction::Focus));
        assert!(palette.actions.contains(&SemanticsAction::SetValue));

        let selected = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
            })
            .expect("selected tool button semantics should exist");
        assert!(selected.state.selected);
        assert!(selected.actions.contains(&SemanticsAction::Activate));
    }

    #[test]
    fn tool_palette_pointer_activation_updates_selection() -> sui_core::Result<()> {
        let chosen = Rc::new(RefCell::new(None));
        let chosen_writer = Rc::clone(&chosen);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(64.0, 180.0))
                .with_child(
                    ToolPalette::vertical("Paint tools")
                        .items([
                            ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                            ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                            ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                        ])
                        .selected(0)
                        .on_change(move |index, label| {
                            *chosen_writer.borrow_mut() = Some((index, label));
                        }),
                ),
        );
        let output = runtime.render(window_id)?;
        let fill = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Fill tool")
            })
            .expect("fill tool button semantics should exist");
        let position = super::rect_center(fill.bounds);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert_eq!(*chosen.borrow(), Some((2, "Fill tool".to_string())));
        let output = runtime.render(window_id)?;
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Fill tool")
                && node.state.selected
        }));
        Ok(())
    }

    #[test]
    fn tool_palette_keyboard_moves_between_tools() -> sui_core::Result<()> {
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let chosen_writer = Rc::clone(&chosen);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(64.0, 180.0))
                .with_child(
                    ToolPalette::vertical("Paint tools")
                        .items([
                            ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                            ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                            ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                        ])
                        .selected(0)
                        .on_change(move |index, label| {
                            chosen_writer.borrow_mut().push((index, label));
                        }),
                ),
        );
        let output = runtime.render(window_id)?;
        let brush = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Brush tool")
            })
            .expect("brush tool button semantics should exist");
        let position = super::rect_center(brush.bounds);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
        )?;

        assert_eq!(
            chosen.borrow().last(),
            Some(&(1, "Eraser tool".to_string()))
        );
        let output = runtime.render(window_id)?;
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Eraser tool")
                && node.state.selected
        }));
        Ok(())
    }

    #[test]
    fn property_row_stacked_exposes_label_and_control_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 72.0))
                .with_child(
                    PropertyRow::new("Brush size", crate::NumberInput::new("Brush size"))
                        .control_width(120.0),
                ),
        );

        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Brush size")
            })
            .expect("property row semantics should exist");
        assert_eq!(row.bounds, Rect::new(0.0, 0.0, 320.0, 72.0));

        let label = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush size")
            })
            .expect("property label semantics should exist");
        let control = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
            })
            .expect("property control semantics should exist");
        assert_eq!(control.bounds.width(), 120.0);
        assert!(control.bounds.y() > label.bounds.y());
    }

    #[test]
    fn property_row_inline_arranges_control_after_label() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 36.0))
                .with_child(
                    PropertyRow::new("Opacity", crate::Slider::new("Opacity"))
                        .layout(PropertyRowLayout::Inline)
                        .label_width(96.0),
                ),
        );

        let label = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Opacity")
            })
            .expect("inline property label semantics should exist");
        let control = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider && node.name.as_deref() == Some("Opacity")
            })
            .expect("inline property control semantics should exist");
        assert!(control.bounds.x() > label.bounds.max_x());
        assert_eq!(control.bounds.width(), 216.0);
    }

    #[test]
    fn property_row_label_id_is_javascript_safe() {
        let id = super::property_row_label_id(WidgetId::new(402)).get();

        assert!(id < (1_u64 << 53));
    }

    #[test]
    fn panel_section_exposes_group_title_and_child_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(240.0, 92.0))
                .with_child(PanelSection::new("Brush", crate::Label::new("Opacity"))),
        );

        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Brush")
            })
            .expect("panel section group semantics should exist");
        let title = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(section.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Brush")
            })
            .expect("panel section title semantics should exist");
        let child = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Opacity")
            })
            .expect("panel section child semantics should exist");

        assert!(child.bounds.y() > title.bounds.max_y());
    }

    #[test]
    fn panel_section_header_action_is_arranged_after_title() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(240.0, 92.0))
                .with_child(
                    PanelSection::new("Layers", crate::Label::new("Paint"))
                        .header_action(crate::IconButton::new(crate::IconGlyph::Add, "Add layer")),
                ),
        );

        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Layers")
            })
            .expect("panel section group semantics should exist");
        let title = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(section.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layers")
            })
            .expect("panel section title semantics should exist");
        let action = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Add layer")
            })
            .expect("panel section header action semantics should exist");
        let child = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Paint"))
            .expect("panel section child semantics should exist");

        assert!(action.bounds.x() > title.bounds.x());
        assert!(child.bounds.y() > action.bounds.max_y());
    }

    #[test]
    fn collapsible_panel_section_hides_collapsed_child_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(240.0, 92.0))
                .with_child(
                    PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                        .collapsible(true)
                        .collapsed(),
                ),
        );

        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Advanced color")
            })
            .expect("collapsible panel section semantics should exist");
        assert_eq!(section.state.expanded, Some(false));
        assert!(section.actions.contains(&SemanticsAction::Expand));
        assert!(
            !output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("RGB sliders")),
            "collapsed section should not expose hidden child semantics"
        );
    }

    #[test]
    fn collapsible_panel_section_pointer_toggle_exposes_child() -> sui_core::Result<()> {
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(240.0, 120.0))
                .with_child(
                    PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                        .collapsible(true)
                        .collapsed(),
                ),
        );
        let output = runtime.render(window_id)?;
        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Advanced color")
            })
            .expect("collapsible panel section semantics should exist");
        let position = Point::new(section.bounds.x() + 20.0, section.bounds.y() + 8.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, position, false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        let output = runtime.render(window_id)?;
        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Advanced color")
            })
            .expect("collapsible panel section semantics should still exist");
        assert_eq!(section.state.expanded, Some(true));
        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("RGB sliders")),
            "expanded section should expose child semantics"
        );

        Ok(())
    }

    #[test]
    fn panel_section_title_id_is_javascript_safe() {
        let id = super::panel_section_title_id(WidgetId::new(402)).get();

        assert!(id < (1_u64 << 53));
    }

    #[test]
    fn dock_panel_exposes_title_and_arranges_child_below_header() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(280.0, 160.0))
                .with_child(
                    DockPanel::new("Tool properties", crate::Label::new("Brush size"))
                        .name("Inspector")
                        .padding(sui_layout::Padding::all(8.0)),
                ),
        );

        let panel = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Inspector")
            })
            .expect("dock panel semantics should exist");
        assert_eq!(panel.bounds, Rect::new(0.0, 0.0, 280.0, 160.0));

        let title = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(panel.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Tool properties")
            })
            .expect("dock panel title semantics should exist");
        let child = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush size")
            })
            .expect("dock panel child semantics should exist");

        assert!(title.bounds.max_y() <= 34.0);
        assert!(child.bounds.y() >= 42.0);
    }

    #[test]
    fn dock_panel_title_id_is_javascript_safe() {
        let id = super::dock_panel_title_id(WidgetId::new(402)).get();

        assert!(id < (1_u64 << 53));
    }

    #[test]
    fn status_bar_host_reserves_footer_height() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 160.0))
                .with_child(StatusBarHost::new(
                    crate::Label::new("Canvas content"),
                    StatusBar::new()
                        .name("Editor status")
                        .segment(StatusBarSegment::new("Ready").min_width(80.0)),
                )),
        );

        let status = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Editor status")
            })
            .expect("status bar container semantics should exist");

        assert_eq!(status.bounds, Rect::new(0.0, 132.0, 320.0, 28.0));
    }

    #[test]
    fn status_bar_segment_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(402);
        let ids = (0..6)
            .map(|index| super::status_bar_segment_id(parent, index).get())
            .collect::<Vec<_>>();

        for id in &ids {
            assert!(*id < (1_u64 << 53));
        }
        for (left_index, left) in ids.iter().enumerate() {
            for right in ids.iter().skip(left_index + 1) {
                assert_ne!(left, right);
            }
        }
    }

    fn first_text_run(output: &RenderOutput) -> sui_text::TextRun {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                sui_scene::SceneCommand::DrawText(text) => Some(text.clone()),
                sui_scene::SceneCommand::DrawShapedText(text) => text
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .map(|layout| sui_text::TextRun {
                        rect: Rect::new(
                            text.origin.x,
                            text.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ),
                        text: layout.text().to_string(),
                        style: layout.style().clone(),
                    }),
                _ => None,
            })
            .expect("text draw command present")
    }

    fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                sui_scene::SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
                sui_scene::SceneCommand::DrawShapedText(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .map(|layout| sui_text::TextRun {
                        rect: Rect::new(
                            run.origin.x,
                            run.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ),
                        text: layout.text().to_string(),
                        style: layout.style().clone(),
                    }),
                _ => None,
            })
            .expect("text draw command present")
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
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

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut event = PointerEvent::new(kind, position);
        event.pointer_id = 1;
        event.button = Some(PointerButton::Primary);
        event.buttons = if pressed {
            PointerButtons::new(1)
        } else {
            PointerButtons::NONE
        };
        Event::Pointer(event)
    }

    fn handle_ready_events(runtime: &mut Runtime) -> Result<usize, String> {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window, event) in ready {
            runtime
                .handle_event(ready_window, event)
                .map_err(|error| error.to_string())?;
        }
        Ok(count)
    }

    fn overlay_layer_descriptor(output: &RenderOutput) -> Option<SceneLayerDescriptor> {
        let mut descriptor = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                descriptor = Some(layer.descriptor.clone());
            }
        });
        descriptor
    }

    fn overlay_layer_owner(output: &RenderOutput) -> Option<WidgetId> {
        let mut owner = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                owner = Some(layer.widget_id());
            }
        });
        owner
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
    fn tab_bar_header_label_visual_center_matches_control_center() {
        let output = render(TabBar::new("Main tabs").tabs(["A", "B"]));
        let text = first_text_run(&output);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("tab header label should shape");
        let line = layout
            .lines()
            .first()
            .expect("tab header label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn menu_row_label_visual_center_matches_row_center() {
        let output = render(
            Menu::new("App menu").items([MenuItem::new("New File"), MenuItem::new("Open...")]),
        );
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
        }));
        let text = text_run_for(&output, "New File");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("menu item text should shape");
        let line = layout
            .lines()
            .first()
            .expect("menu item text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let row_height = (output.frame.viewport.height - (MENU_VERTICAL_PADDING * 2.0)) / 2.0;
        let row_center = MENU_VERTICAL_PADDING + (row_height * 0.5);

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    #[test]
    fn context_menu_row_label_visual_center_matches_row_center() -> Result<(), String> {
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
        );

        let closed = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let trigger = closed
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("context menu trigger present")
            .bounds;
        let trigger_center = Point::new(
            trigger.x() + (trigger.width() * 0.5),
            trigger.y() + (trigger.height() * 0.5),
        );

        let mut down = PointerEvent::new(PointerEventKind::Down, trigger_center);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Secondary);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .map_err(|error| error.to_string())?;

        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let context = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ContextMenu)
            .expect("context menu semantics present");
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename")
        }));
        let text = text_run_for(&output, "Rename");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("context menu item text should shape");
        let line = layout
            .lines()
            .first()
            .expect("context menu item text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let menu_height = context.bounds.height() - trigger.height() - 6.0;
        let row_height = (menu_height - (MENU_VERTICAL_PADDING * 2.0)) / 2.0;
        let row_center = trigger.max_y() + 6.0 + MENU_VERTICAL_PADDING + (row_height * 0.5);

        assert!((actual_visual_center - row_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn progress_bar_value_text_visual_center_matches_control_center() {
        let output = render(
            ProgressBar::new("Export progress")
                .range(0.0, 100.0)
                .value(42.0)
                .show_value(true),
        );
        let text = text_run_for(&output, "42%");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("progress bar label should shape");
        let line = layout
            .lines()
            .first()
            .expect("progress bar label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
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
                "Inline inspector",
                crate::Button::new("Open inspector"),
                crate::Label::new("popover body"),
            )
            .open(true),
        ));

        let descriptor =
            overlay_layer_descriptor(&output).expect("popover layer descriptor present");

        assert!(descriptor.is_stack_surface);
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
    }

    #[test]
    fn tooltip_reveal_animation_updates_layer_properties_until_complete() -> Result<(), String> {
        const TOOLTIP_ANIMATION_SECONDS: f64 = 0.18;

        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            crate::Tooltip::new(
                "Quick access to common commands",
                crate::Button::new("Hover for shortcuts").min_width(180.0),
            ),
        ));

        let initial = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(overlay_layer_descriptor(&initial).is_none());

        let trigger = initial
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hover for shortcuts")
            })
            .expect("tooltip trigger semantics present")
            .bounds;
        let hover_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, hover_point, false),
            )
            .map_err(|error| error.to_string())?;

        let start = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let start_descriptor =
            overlay_layer_descriptor(&start).expect("tooltip overlay layer should appear");
        assert_eq!(
            start_descriptor.properties.translation.y.signum(),
            -1.0,
            "tooltip reveal should start offset upward"
        );
        assert_eq!(start_descriptor.properties.opacity, 0.0);

        runtime.tick(TOOLTIP_ANIMATION_SECONDS * 0.5);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_descriptor =
            overlay_layer_descriptor(&mid).expect("tooltip overlay layer should stay active");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert!(mid_descriptor.properties.translation.y < 0.0);
        assert!(
            mid_descriptor.properties.translation.y.abs()
                < start_descriptor.properties.translation.y.abs()
        );
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some()
        );

        runtime.tick(TOOLTIP_ANIMATION_SECONDS);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_descriptor =
            overlay_layer_descriptor(&settled).expect("tooltip overlay layer should still exist");
        assert_eq!(settled_descriptor.properties.opacity, 1.0);
        assert_eq!(settled_descriptor.properties.translation.y, 0.0);
        assert_eq!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?,
            None
        );

        Ok(())
    }

    #[test]
    fn popover_open_animation_stops_requesting_frames_after_completion() -> Result<(), String> {
        const POPOVER_ANIMATION_SECONDS: f64 = 0.18;

        let content = Rc::new(RefCell::new(PanelCounters::default()));
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            Popover::new(
                "Inline inspector",
                crate::Button::new("Open inspector").min_width(180.0),
                SpyPanel::new("popover-content", Rc::clone(&content)),
            ),
        ));

        let closed = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let trigger = closed
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open inspector")
            })
            .expect("popover trigger semantics present")
            .bounds;
        assert_eq!(content.borrow().paint, 0);

        let press_point = Point::new(trigger.x() + 12.0, trigger.y() + (trigger.height() * 0.5));
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, press_point, true),
            )
            .map_err(|error| error.to_string())?;

        let opened = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let open_descriptor =
            overlay_layer_descriptor(&opened).expect("popover overlay layer should appear");
        assert_eq!(content.borrow().paint, 1);
        assert_eq!(open_descriptor.properties.opacity, 0.0);
        assert!(open_descriptor.properties.translation.y < 0.0);

        runtime.tick(POPOVER_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_descriptor =
            overlay_layer_descriptor(&mid).expect("popover overlay layer should stay active");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert!(mid_descriptor.properties.translation.y < 0.0);
        assert_eq!(
            content.borrow().paint,
            1,
            "popover content should stay retained while only layer properties change"
        );
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some()
        );

        runtime.tick(POPOVER_ANIMATION_SECONDS);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_descriptor =
            overlay_layer_descriptor(&settled).expect("popover overlay layer should remain open");
        assert_eq!(settled_descriptor.properties.opacity, 1.0);
        assert_eq!(settled_descriptor.properties.translation.y, 0.0);
        assert_eq!(
            content.borrow().paint,
            1,
            "popover content should not repaint on retained-only animation frames"
        );
        assert_eq!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?,
            None
        );

        Ok(())
    }

    #[test]
    fn popover_arrival_effect_obeys_hdr_theme_mode() {
        let mut disabled_theme = DefaultTheme::default();
        disabled_theme.hdr.mode = HdrThemeMode::Disabled;
        disabled_theme.hdr.policy.max_large_area_lift = 1.12;
        disabled_theme.hdr.color_roles.surface_elevated =
            SemanticColorToken::from_sdr(disabled_theme.palette.surface_raised)
                .with_hdr(Color::linear_display_p3(1.30, 1.08, 1.05, 1.0));

        let mut disabled = Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(disabled_theme);
        disabled.open = true;
        {
            let mut state = disabled.state.borrow_mut();
            state.reveal = super::AnimatedScalar::new(1.0);
            state.arrival_active = true;
        }
        let disabled_visuals = disabled.state.borrow().resolved_visuals();

        assert_eq!(
            disabled_visuals.background,
            disabled_theme.palette.surface_raised
        );
        assert!(disabled_visuals.surface_style.is_none());
        assert!(disabled_visuals.arrival_effect.is_none());

        let (mut disabled_runtime, disabled_window) = build_runtime(
            Popover::new(
                "Options",
                crate::Button::new("Open"),
                crate::Label::new("Popover body"),
            )
            .theme(disabled_theme),
        );
        let _ = disabled_runtime.render(disabled_window).unwrap();
        disabled_runtime
            .handle_event(
                disabled_window,
                primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
            )
            .unwrap();
        let disabled_output = disabled_runtime.render(disabled_window).unwrap();
        assert!(
            !solid_fill_colors(&disabled_output)
                .iter()
                .any(|color| color.alpha < 1.0)
        );

        let mut hdr_theme = disabled_theme;
        hdr_theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
        hdr_theme.hdr.color_roles.surface_elevated =
            SemanticColorToken::from_sdr(hdr_theme.palette.surface_raised)
                .with_hdr(Color::linear_display_p3(1.30, 1.08, 1.05, 1.0));

        let mut hdr = Popover::new(
            "Options",
            crate::Button::new("Open"),
            crate::Label::new("Popover body"),
        )
        .theme(hdr_theme);
        hdr.open = true;
        {
            let mut state = hdr.state.borrow_mut();
            state.reveal = super::AnimatedScalar::new(1.0);
            state.arrival_active = true;
        }
        let hdr_visuals = hdr.state.borrow().resolved_visuals();
        let surface_style = hdr_visuals
            .surface_style
            .expect("hdr surface style present");
        let arrival_effect = hdr_visuals
            .arrival_effect
            .expect("pulse arrival effect present");

        assert_eq!(hdr_visuals.background, surface_style.color);
        assert!(surface_style.color.red <= hdr_theme.hdr.policy.max_large_area_lift);
        assert_ne!(hdr_visuals.background, hdr_theme.palette.surface_raised);
        assert!(arrival_effect.intensity > 0.0);
        assert!(arrival_effect.speed > 0.0);

        let (mut runtime, window_id) = build_runtime(
            Popover::new(
                "Options",
                crate::Button::new("Open"),
                crate::Label::new("Popover body"),
            )
            .theme(hdr_theme),
        );
        let _ = runtime.render(window_id).unwrap();
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
            )
            .unwrap();
        let arrival_output = runtime.render(window_id).unwrap();
        assert!(
            solid_fill_colors(&arrival_output)
                .iter()
                .any(|color| color.alpha < 1.0)
        );

        runtime.tick(1.0);
        for (ready_window, event) in runtime.drain_ready_events() {
            runtime.handle_event(ready_window, event).unwrap();
        }
        let settled_output = runtime.render(window_id).unwrap();
        assert!(
            !solid_fill_colors(&settled_output)
                .iter()
                .any(|color| color.alpha < 1.0)
        );
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
        let owner = overlay_layer_owner(&output).expect("popover layer owner present");
        let descriptor =
            overlay_layer_descriptor(&output).expect("popover layer descriptor present");
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == owner)
            .expect("popover graph node present");
        let host = graph
            .stack_hosts
            .iter()
            .find(|host| host.host == graph.root)
            .expect("root stack host present");

        assert_eq!(node.stack_host, graph.root);
        assert_eq!(node.stack_surface, owner);
        assert_eq!(node.transient_owner_surface, Some(host.surfaces[0]));
        assert_eq!(host.surfaces.last().copied(), Some(owner));
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
