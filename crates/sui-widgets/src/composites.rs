use std::{cell::RefCell, rc::Rc};

use sui_core::{
    Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Path,
    PathBuilder, Point, PointerButton, PointerEventKind, Rect, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsState, SemanticsValue, Size, TimerToken, Vector, WakeEvent, WidgetId,
};
use sui_layout::{Axis, Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    SemanticsCtx, SingleChild, StackSurfaceOptions, Widget, WidgetChildren, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{LayerCompositionMode, LayerProperties, StrokeStyle};
use sui_text::{
    FontFeature, FontWeight, TextAlign, TextDocument, TextLayoutRequest, TextMeasurement,
    TextStyle, TextWrap,
};

use crate::{
    Button, ControlMetrics, DefaultTheme, HdrThemeMode, IconGlyph, Interpolate, MotionScalar,
    ResolvedEffectStyle, ResolvedHdrStyle, SemanticTone, ThemeTextToken, WidgetColorRole,
    WidgetEffectRole, WidgetLuminanceRole, WidgetMaterialRole,
    controls::{apply_hdr_policy_cap, cap_resolved_hdr_style, draw_icon_glyph},
    paint_theme_shadow, resolve_widget_hdr_style,
    text_align::{paint_aligned_text, paint_single_line_aligned_text},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Window,
    Sidebar,
    Panel,
    Titlebar,
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceBorder {
    None,
    All,
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceElevation {
    None,
    Small,
    Medium,
    Large,
}

pub struct Surface {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: Option<String>,
    role: SurfaceRole,
    border: SurfaceBorder,
    elevation: SurfaceElevation,
    radius: f32,
    padding: Insets,
    fill_width: bool,
    fill_height: bool,
    child: SingleChild,
}

impl Surface {
    pub fn new<W>(role: SurfaceRole, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            role,
            border: SurfaceBorder::None,
            elevation: SurfaceElevation::None,
            radius: 0.0,
            padding: Insets::ZERO,
            fill_width: false,
            fill_height: false,
            child: SingleChild::new(child),
        }
    }

    pub fn window<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Window, child)
    }

    pub fn sidebar<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Sidebar, child).border(SurfaceBorder::Right)
    }

    pub fn panel<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Panel, child)
            .border(SurfaceBorder::All)
            .radius(8.0)
    }

    pub fn titlebar<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Titlebar, child).border(SurfaceBorder::Bottom)
    }

    pub fn field<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Field, child)
            .border(SurfaceBorder::All)
            .radius(6.0)
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

    pub fn border(mut self, border: SurfaceBorder) -> Self {
        self.border = border;
        self
    }

    pub fn elevation(mut self, elevation: SurfaceElevation) -> Self {
        self.elevation = elevation;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn fill(mut self) -> Self {
        self.fill_width = true;
        self.fill_height = true;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.fill_height = true;
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn background_for_role(theme: &DefaultTheme, role: SurfaceRole) -> Color {
        match role {
            SurfaceRole::Window => theme.surfaces.window,
            SurfaceRole::Sidebar => theme.surfaces.sidebar,
            SurfaceRole::Panel => theme.surfaces.panel,
            SurfaceRole::Titlebar => theme.surfaces.titlebar,
            SurfaceRole::Field => theme.surfaces.field,
        }
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }
}

impl Widget for Surface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_child = Size::new(
            if constraints.max.width.is_finite() {
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
            } else {
                f32::INFINITY
            },
            if constraints.max.height.is_finite() {
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0)
            } else {
                f32::INFINITY
            },
        );
        let child_size = self
            .child
            .measure(ctx, Constraints::new(Size::ZERO, max_child));
        let mut size = Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        );
        if self.fill_width && constraints.max.width.is_finite() {
            size.width = constraints.max.width;
        }
        if self.fill_height && constraints.max.height.is_finite() {
            size.height = constraints.max.height;
        }
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let radius = self.radius.min(bounds.width().min(bounds.height()) * 0.5);

        let shadow = match self.elevation {
            SurfaceElevation::None => None,
            SurfaceElevation::Small => Some(&theme.shadows.box_shadow.sm),
            SurfaceElevation::Medium => Some(&theme.shadows.box_shadow.md),
            SurfaceElevation::Large => Some(&theme.shadows.box_shadow.lg),
        };
        if let Some(shadow) = shadow {
            paint_theme_shadow(ctx, bounds, [radius; 4], shadow);
        }

        let background = Self::background_for_role(&theme, self.role);
        let border = theme.surfaces.border;
        if radius > 0.0 {
            ctx.fill(rounded_rect_path(bounds, radius), background);
        } else {
            ctx.fill_rect(bounds, background);
        }

        let stroke_width = physical_pixels(ctx, theme.metrics.border_width.max(1.0));
        match self.border {
            SurfaceBorder::None => {}
            SurfaceBorder::All => {
                ctx.stroke(
                    rounded_rect_path(bounds, radius),
                    border,
                    StrokeStyle::new(stroke_width),
                );
            }
            SurfaceBorder::Top => ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), bounds.width(), stroke_width),
                border,
            ),
            SurfaceBorder::Right => ctx.fill_rect(
                Rect::new(
                    bounds.max_x() - stroke_width,
                    bounds.y(),
                    stroke_width,
                    bounds.height(),
                ),
                border,
            ),
            SurfaceBorder::Bottom => ctx.fill_rect(
                Rect::new(
                    bounds.x(),
                    bounds.max_y() - stroke_width,
                    bounds.width(),
                    stroke_width,
                ),
                border,
            ),
            SurfaceBorder::Left => ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), stroke_width, bounds.height()),
                border,
            ),
        }

        self.child.paint(ctx);
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
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
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
            theme.semantic_tone_color(SemanticTone::Danger)
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

fn menu_row_height(theme: &DefaultTheme) -> f32 {
    theme.metrics.menu_row_height
}

fn themed_menu_height_for_rows(theme: &DefaultTheme, row_height: f32, rows: usize) -> f32 {
    theme.metrics.menu_padding.top + theme.metrics.menu_padding.bottom + (row_height * rows as f32)
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

pub struct Toolbar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: Option<String>,
    extent: Option<f32>,
    padding: Option<Insets>,
    spacing: Option<f32>,
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
            extent: None,
            padding: None,
            spacing: None,
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
        self.extent = Some(extent.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing.max(0.0));
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

    fn resolved_extent(&self, metrics: ControlMetrics) -> f32 {
        self.extent.unwrap_or(metrics.toolbar_extent)
    }

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.toolbar_padding)
    }

    fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.toolbar_spacing)
    }

    fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let padding = self.resolved_padding(metrics);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom).max(0.0),
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let extent = self.resolved_extent(metrics);
        let padding = self.resolved_padding(metrics);
        let spacing = self.resolved_spacing(metrics);
        let content_cross = match self.axis {
            Axis::Horizontal => (extent - padding.top - padding.bottom).max(0.0),
            Axis::Vertical => (extent - padding.left - padding.right).max(0.0),
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
                main += spacing;
            }
            main += toolbar_main(self.axis, child_size);
            cross = cross.max(toolbar_cross(self.axis, child_size));
        }

        let natural = match self.axis {
            Axis::Horizontal => Size::new(
                main + padding.left + padding.right,
                extent.max(cross + padding.top + padding.bottom),
            ),
            Axis::Vertical => Size::new(
                extent.max(cross + padding.left + padding.right),
                main + padding.top + padding.bottom,
            ),
        };
        let filled = match self.axis {
            Axis::Horizontal => Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width
                } else {
                    natural.width
                },
                extent,
            ),
            Axis::Vertical => Size::new(
                extent,
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let spacing = self.resolved_spacing(metrics);
        let content = self.content_bounds(bounds, metrics);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let mut main_offset = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                main_offset += spacing;
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

pub struct CommandGroup {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: Option<String>,
    padding: Option<Insets>,
    spacing: Option<f32>,
    corner_radius: Option<f32>,
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
            padding: None,
            spacing: None,
            corner_radius: None,
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
        self.padding = Some(padding);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing.max(0.0));
        self
    }

    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = Some(corner_radius.max(0.0));
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

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.command_group_padding)
    }

    fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.command_group_spacing)
    }

    fn resolved_corner_radius(&self, metrics: ControlMetrics) -> f32 {
        self.corner_radius.unwrap_or(metrics.command_group_radius)
    }

    fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        inset_rect(bounds, self.resolved_padding(metrics))
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let spacing = self.resolved_spacing(metrics);
        let max_width = if constraints.max.width.is_finite() {
            (constraints.max.width - padding.left - padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        let max_height = if constraints.max.height.is_finite() {
            (constraints.max.height - padding.top - padding.bottom).max(0.0)
        } else {
            f32::INFINITY
        };
        let child_constraints = Constraints::new(Size::ZERO, Size::new(max_width, max_height));

        let mut main: f32 = 0.0;
        let mut cross: f32 = 0.0;
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            let child_size = child.measure(ctx, child_constraints);
            if index > 0 {
                main += spacing;
            }
            main += toolbar_main(self.axis, child_size);
            cross = cross.max(toolbar_cross(self.axis, child_size));
        }

        let natural = match self.axis {
            Axis::Horizontal => Size::new(
                main + padding.left + padding.right,
                cross + padding.top + padding.bottom,
            ),
            Axis::Vertical => Size::new(
                cross + padding.left + padding.right,
                main + padding.top + padding.bottom,
            ),
        };
        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let spacing = self.resolved_spacing(metrics);
        let content = self.content_bounds(bounds, metrics);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let mut main_offset = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                main_offset += spacing;
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
            .resolved_corner_radius(theme.metrics)
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
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    extent: Option<f32>,
    padding: Option<Insets>,
    spacing: Option<f32>,
    item_size: Option<f32>,
    icon_size: Option<f32>,
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
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            extent: None,
            padding: None,
            spacing: None,
            item_size: None,
            icon_size: None,
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
        self.extent = Some(extent.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing.max(0.0));
        self
    }

    pub fn item_size(mut self, item_size: f32) -> Self {
        self.item_size = Some(item_size.max(0.0));
        self
    }

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = Some(icon_size.max(0.0));
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

    fn resolved_extent(&self, metrics: ControlMetrics) -> f32 {
        self.extent.unwrap_or(metrics.toolbar_extent)
    }

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.toolbar_padding)
    }

    fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.toolbar_spacing)
    }

    fn resolved_item_size(&self, metrics: ControlMetrics) -> f32 {
        self.item_size.unwrap_or(metrics.tool_palette_item_size)
    }

    fn resolved_icon_size(&self, metrics: ControlMetrics) -> f32 {
        self.icon_size.unwrap_or(metrics.tool_palette_icon_size)
    }

    fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let padding = self.resolved_padding(metrics);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom).max(0.0),
        )
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }

        let metrics = self.resolved_theme().metrics;
        let item_size = self.resolved_item_size(metrics);
        let spacing = self.resolved_spacing(metrics);
        let content = self.content_bounds(bounds, metrics);
        let content_main = toolbar_main(self.axis, content.size);
        let content_cross = toolbar_cross(self.axis, content.size);
        let item_main = item_size.min(content_main);
        let item_cross = item_size.min(content_cross);
        let main_offset = index as f32 * (item_size + spacing);
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

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        hover_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for ToolPalette {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.hit_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
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
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.select(ctx, index);
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
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let item_size = self.resolved_item_size(metrics);
        let spacing = self.resolved_spacing(metrics);
        let padding = self.resolved_padding(metrics);
        let extent = self.resolved_extent(metrics);
        let item_count = self.items.len();
        let main = if item_count == 0 {
            0.0
        } else {
            (item_size * item_count as f32) + (spacing * (item_count - 1) as f32)
        };
        let natural = match self.axis {
            Axis::Horizontal => Size::new(main + padding.left + padding.right, extent),
            Axis::Vertical => Size::new(extent, main + padding.top + padding.bottom),
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
        let interaction = theme.interaction;
        let icon_size = self.resolved_icon_size(metrics);
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
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let enabled = item.enabled;
            let base_background = if selected_item {
                mix_color(palette.surface, palette.accent, interaction.selected_blend)
            } else {
                palette.surface
            };
            let background = if !enabled {
                mix_color(
                    base_background,
                    palette.surface,
                    interaction.disabled_opacity,
                )
            } else if press_amount > 0.0 {
                mix_color(
                    if hover_amount > 0.0 {
                        mix_color(
                            base_background,
                            palette.control_hover,
                            interaction.hover_blend
                                * if selected_item { 0.35 } else { 1.0 }
                                * hover_amount,
                        )
                    } else {
                        base_background
                    },
                    palette.control_active,
                    interaction.pressed_blend
                        * if selected_item { 0.45 } else { 1.0 }
                        * press_amount,
                )
            } else if hover_amount > 0.0 {
                mix_color(
                    base_background,
                    palette.control_hover,
                    interaction.hover_blend * if selected_item { 0.35 } else { 1.0 } * hover_amount,
                )
            } else {
                base_background
            };
            let border = if !enabled {
                palette.border.with_alpha(0.55)
            } else if ctx.is_focused() && selected_item {
                palette.border_focus
            } else if selected_item {
                palette.accent_border
            } else if hovered || hover_amount > 0.0 || press_amount > 0.0 {
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
                (self.focus_animation.value > AnimatedScalar::EPSILON && selected_item).then_some(
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
                ),
            );
            let center = rect_center(rect);
            let side = icon_size.min(rect.width().min(rect.height())).max(0.0);
            let pressed_offset = press_amount * interaction.pressed_offset;
            let icon_rect = Rect::new(
                center.x - side * 0.5,
                center.y - side * 0.5 + pressed_offset,
                side,
                side,
            );
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
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

pub struct ActionCard {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    title: String,
    description: String,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    accent: Option<Color>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
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
            tone: SemanticTone::Accent,
            accent: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
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

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
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

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.action_card_padding)
    }

    fn resolved_min_width(&self, metrics: ControlMetrics) -> f32 {
        self.min_width.unwrap_or(metrics.action_card_min_width)
    }

    fn resolved_min_height(&self, metrics: ControlMetrics) -> f32 {
        self.min_height.unwrap_or(metrics.action_card_min_height)
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
        let theme = self.resolved_theme();
        self.hovered = hovered;
        set_action_card_hover_animation_target(
            &mut self.hover_animation,
            hovered as u8 as f32,
            &theme,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    fn clear_transient_state_for_hidden_bounds(&mut self, ctx: &mut ArrangeCtx) {
        if !self.hovered && !self.pressed && !self.focus_animation.is_presented() {
            return;
        }

        self.hovered = false;
        self.pressed = false;
        self.hover_animation = AnimatedScalar::new(0.0);
        self.press_animation = AnimatedScalar::new(0.0);
        self.focus_animation = AnimatedScalar::new(0.0);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            weight: FontWeight::SEMIBOLD,
            ..text_token_style(&theme, theme.text.base, theme.palette.text)
        }
    }

    fn resolved_description_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        text_token_style(&theme, theme.text.sm, theme.palette.text_muted)
    }

    fn content_rect(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        inset_rect(bounds, self.resolved_padding(metrics))
    }

    fn text_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let content = self.content_rect(bounds, metrics);
        let icon_extent = self
            .icon
            .map(|_| metrics.action_card_icon_box_size + metrics.action_card_icon_gap)
            .unwrap_or(0.0);
        let trailing = metrics.action_card_trailing_gap;
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
                let theme = self.resolved_theme();
                self.hovered = false;
                self.pressed = false;
                set_action_card_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                set_action_card_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_action_card_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_action_card_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_action_card_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_action_card_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_action_card_hover_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        &theme,
                        ctx,
                    );
                    set_action_card_press_animation_target(
                        &mut self.press_animation,
                        0.0,
                        &theme,
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title = measure_text(ctx, &self.title, &title_style);
        let description = measure_text(ctx, &self.description, &description_style);
        self.title_measurement = Some(title);
        self.description_measurement = Some(description);

        let icon_extent = self
            .icon
            .map(|_| metrics.action_card_icon_box_size + metrics.action_card_icon_gap)
            .unwrap_or(0.0);
        let text_width = title.width.max(description.width).min(320.0);
        let natural = Size::new(
            self.resolved_min_width(metrics).max(
                padding.left
                    + icon_extent
                    + text_width
                    + metrics.action_card_trailing_gap
                    + padding.right,
            ),
            self.resolved_min_height(metrics).max(
                padding.top
                    + title.height.max(title_style.line_height)
                    + metrics.action_card_text_gap
                    + description.height.max(description_style.line_height)
                    + padding.bottom,
            ),
        );
        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            self.clear_transient_state_for_hidden_bounds(ctx);
        }
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
        let accent = self
            .accent
            .unwrap_or_else(|| theme.semantic_tone_color(self.tone));
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
            (self.focus_animation.value > AnimatedScalar::EPSILON && enabled).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
            ),
        );

        let bounds = ctx.bounds();
        let content = self.content_rect(bounds, metrics);
        let accent_inset = metrics.action_card_accent_inset.min(bounds.height() * 0.5);
        let accent_height = (bounds.height() - accent_inset * 2.0).max(0.0);
        let accent_rail = Rect::new(
            bounds.x(),
            bounds.y() + accent_inset,
            metrics.action_card_accent_width,
            accent_height,
        );
        ctx.fill(
            rounded_rect_path(accent_rail, metrics.action_card_accent_width * 0.5),
            accent.with_alpha(0.78),
        );

        if let Some(icon) = self.icon {
            let icon_box_size = metrics
                .action_card_icon_box_size
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
            let icon_size = metrics
                .action_card_icon_size
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

        let text_bounds = self.text_bounds(bounds, metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title_height = title_style.line_height.max(
            self.title_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(title_style.line_height),
        );
        let description_min_height = description_style.line_height.max(
            self.description_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(description_style.line_height),
        );
        let description_height =
            (text_bounds.height() - title_height - metrics.action_card_text_gap)
                .max(description_min_height)
                .min((description_style.line_height * 2.0).max(description_min_height));
        let text_block_height = title_height + metrics.action_card_text_gap + description_height;
        let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
        let description_slot = Rect::new(
            text_bounds.x(),
            title_slot.max_y() + metrics.action_card_text_gap,
            text_bounds.width(),
            description_height,
        );
        let title_paint_style = TextStyle {
            color: if enabled {
                palette.text
            } else {
                palette.text.with_alpha(0.45)
            },
            ..title_style
        };
        let description_paint_style = TextStyle {
            color: if enabled {
                palette.placeholder
            } else {
                palette.placeholder.with_alpha(0.45)
            },
            ..description_style
        };
        let description_layout = {
            let mut layout_style = description_paint_style.clone();
            layout_style.color = Color::WHITE;
            ctx.shape_text(
                self.description.clone(),
                Size::new(
                    description_slot.width().max(1.0),
                    description_slot.height().max(1.0),
                ),
                layout_style,
            )
            .ok()
        };
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_paint_style,
            title_paint_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        ctx.push_clip_rect(description_slot);
        if let Some(layout) = description_layout.filter(|layout| layout.lines().len() > 1) {
            let measurement = layout.measurement();
            let width = measurement.width.min(description_slot.width()).max(0.0);
            let height = description_paint_style
                .line_height
                .max(measurement.height)
                .min(description_slot.height());
            let description_rect = Rect::new(
                description_slot.x(),
                description_slot.y() + ((description_slot.height() - height).max(0.0) * 0.5),
                width,
                height,
            );
            ctx.draw_text_layout_with_color(
                description_rect.origin,
                &layout,
                description_paint_style.color,
            );
        } else {
            paint_aligned_text(
                ctx,
                description_slot,
                &self.description,
                &description_paint_style,
                description_paint_style.line_height,
                0.0,
            );
        }
        ctx.pop_clip();

        let chevron_size = metrics
            .action_card_chevron_size
            .min(content.width())
            .min(content.height())
            .max(0.0);
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

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

fn set_action_card_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) {
    set_animation_target(animation, target, duration, easing, ctx);
}

fn set_action_card_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_action_card_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    );
}

fn set_action_card_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_action_card_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
        ctx,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRowLayout {
    Stacked,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyRowDefaults {
    Property,
    Form,
}

pub struct PropertyRow {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    defaults: PropertyRowDefaults,
    layout: PropertyRowLayout,
    label_width: Option<f32>,
    control_width: Option<f32>,
    auto_control_width: bool,
    gap: Option<f32>,
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
            defaults: PropertyRowDefaults::Property,
            layout: PropertyRowLayout::Stacked,
            label_width: None,
            control_width: None,
            auto_control_width: true,
            gap: None,
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
        self
    }

    pub fn stacked(self) -> Self {
        self.layout(PropertyRowLayout::Stacked)
    }

    pub fn inline(self) -> Self {
        self.layout(PropertyRowLayout::Inline)
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.label_width = Some(width.max(0.0));
        self
    }

    pub fn control_width(mut self, width: f32) -> Self {
        self.control_width = Some(width.max(0.0));
        self.auto_control_width = false;
        self
    }

    pub fn auto_control_width(mut self) -> Self {
        self.control_width = None;
        self.auto_control_width = true;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
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
        self.label_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.sm, theme.palette.text_muted))
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn with_form_defaults(mut self) -> Self {
        self.defaults = PropertyRowDefaults::Form;
        self.auto_control_width = false;
        self
    }

    fn resolved_label_width(&self, metrics: ControlMetrics) -> f32 {
        self.label_width.unwrap_or(match self.defaults {
            PropertyRowDefaults::Property => metrics.property_row_label_width,
            PropertyRowDefaults::Form => metrics.form_row_label_width,
        })
    }

    fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(match self.defaults {
            PropertyRowDefaults::Form => metrics.form_row_gap,
            PropertyRowDefaults::Property => match self.layout {
                PropertyRowLayout::Stacked => metrics.property_row_stacked_gap,
                PropertyRowLayout::Inline => metrics.property_row_inline_gap,
            },
        })
    }

    fn resolved_control_width(&self, metrics: ControlMetrics) -> Option<f32> {
        if self.auto_control_width {
            None
        } else {
            self.control_width.or_else(|| {
                matches!(self.defaults, PropertyRowDefaults::Form)
                    .then_some(metrics.form_row_control_width)
            })
        }
    }

    fn label_height(&self, style: &TextStyle) -> f32 {
        self.label_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn child_constraints(
        &self,
        constraints: Constraints,
        label_extent: f32,
        metrics: ControlMetrics,
    ) -> Constraints {
        let max_width = constraints.max.width;
        let gap = self.resolved_gap(metrics);
        let available = match self.layout {
            PropertyRowLayout::Stacked => max_width,
            PropertyRowLayout::Inline => {
                if max_width.is_finite() {
                    (max_width - label_extent - gap).max(0.0)
                } else {
                    f32::INFINITY
                }
            }
        };
        let width = self
            .resolved_control_width(metrics)
            .map(|width| width.min(available).max(0.0));

        match width {
            Some(width) => Constraints::new(
                Size::new(width, 0.0),
                Size::new(width, constraints.max.height),
            ),
            None => Constraints::new(Size::ZERO, Size::new(available, constraints.max.height)),
        }
    }

    fn child_width_for_bounds(
        &self,
        bounds: Rect,
        label_extent: f32,
        metrics: ControlMetrics,
    ) -> f32 {
        let gap = self.resolved_gap(metrics);
        let available = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => (bounds.width() - label_extent - gap).max(0.0),
        };
        self.resolved_control_width(metrics)
            .unwrap_or(available)
            .min(available)
            .max(0.0)
    }
}

impl Widget for PropertyRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let label_style = self.resolved_label_style();
        let label_measurement = measure_text(ctx, &self.label, &label_style);
        self.label_measurement = Some(label_measurement);
        let label_height = self.label_height(&label_style);
        let label_extent = match self.layout {
            PropertyRowLayout::Stacked => label_measurement.width,
            PropertyRowLayout::Inline => self
                .resolved_label_width(metrics)
                .max(label_measurement.width),
        };
        let child_size = self.child.measure(
            ctx,
            self.child_constraints(constraints, label_extent, metrics),
        );
        let natural = match self.layout {
            PropertyRowLayout::Stacked => Size::new(
                label_measurement.width.max(child_size.width),
                label_height + gap + child_size.height,
            ),
            PropertyRowLayout::Inline => Size::new(
                label_extent + gap + child_size.width,
                label_height.max(child_size.height),
            ),
        };

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let label_width = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => self
                .resolved_label_width(metrics)
                .min(bounds.width())
                .max(0.0),
        };
        let child_measured = self.child.child().measured_size();
        let child_width = self.child_width_for_bounds(bounds, label_width, metrics);
        let child_height = child_measured.height.min(bounds.height()).max(0.0);

        let child_bounds = match self.layout {
            PropertyRowLayout::Stacked => Rect::new(
                bounds.x(),
                bounds.y() + label_height + gap,
                child_width,
                child_height.min((bounds.height() - label_height - gap).max(0.0)),
            ),
            PropertyRowLayout::Inline => Rect::new(
                bounds.x() + label_width + gap,
                bounds.y() + ((bounds.height() - child_height) * 0.5).max(0.0),
                child_width,
                child_height,
            ),
        };
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
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
                self.resolved_label_width(metrics)
                    .min(bounds.width())
                    .max(0.0),
                label_height,
            ),
        };
        ctx.push_clip_rect(label_rect);
        paint_single_line_aligned_text(
            ctx,
            label_rect,
            &self.label,
            &label_style,
            label_height,
            0.0,
        );
        ctx.pop_clip();
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
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
                self.resolved_label_width(metrics)
                    .min(ctx.bounds().width())
                    .max(0.0),
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

pub struct SectionLabel {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    semantic_name: Option<String>,
    color: Option<Color>,
}

impl SectionLabel {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            semantic_name: None,
            color: None,
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

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn display_text(&self) -> String {
        self.label.to_uppercase()
    }

    fn text_style(&self, theme: &DefaultTheme) -> TextStyle {
        section_label_text_style(theme, self.color)
    }
}

impl Widget for SectionLabel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let style = self.text_style(&theme);
        let text = self.display_text();
        let measured = measure_text(ctx, &text, &style);
        constraints.clamp(Size::new(measured.width, style.line_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let style = self.text_style(&theme);
        let text = self.display_text();
        ctx.push_clip_rect(ctx.bounds());
        paint_single_line_aligned_text(ctx, ctx.bounds(), &text, &style, style.line_height, 0.0);
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        let name = self
            .semantic_name
            .clone()
            .unwrap_or_else(|| self.label.clone());
        node.name = Some(name.clone());
        node.value = Some(SemanticsValue::Text(name));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SectionLabelPaint {
    color: Option<Color>,
}

impl SectionLabelPaint {
    pub const fn new() -> Self {
        Self { color: None }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

pub fn paint_section_label(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    paint: SectionLabelPaint,
) {
    let style = section_label_text_style(theme, paint.color);
    let text = label.to_uppercase();
    ctx.push_clip_rect(rect);
    paint_single_line_aligned_text(ctx, rect, &text, &style, style.line_height, 0.0);
    ctx.pop_clip();
}

pub fn paint_section_label_detail(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    detail: &str,
    paint: SectionLabelPaint,
) {
    let style = section_label_text_style(theme, paint.color);
    let text = if detail.trim().is_empty() {
        label.to_uppercase()
    } else {
        format!("{} · {detail}", label.to_uppercase())
    };
    ctx.push_clip_rect(rect);
    paint_single_line_aligned_text(ctx, rect, &text, &style, style.line_height, 0.0);
    ctx.pop_clip();
}

fn section_label_text_style(theme: &DefaultTheme, color: Option<Color>) -> TextStyle {
    let mut style = text_token_style(
        theme,
        theme.text.xs,
        color.unwrap_or(theme.surfaces.text_faint),
    );
    style.weight = FontWeight::SEMIBOLD;
    style.line_height = style.line_height.max(14.0);
    style
}

pub struct DetailRow {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    label_reader: Option<Box<dyn Fn() -> String>>,
    value: String,
    value_reader: Option<Box<dyn Fn() -> String>>,
    max_value_lines: Option<usize>,
}

impl DetailRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            value: value.into(),
            value_reader: None,
            max_value_lines: None,
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

    pub fn label_when<F>(mut self, label: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.label_reader = Some(Box::new(label));
        self
    }

    pub fn value_when<F>(mut self, value: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.value_reader = Some(Box::new(value));
        self
    }

    pub fn max_value_lines(mut self, max_lines: usize) -> Self {
        self.max_value_lines = Some(max_lines.max(1));
        self
    }

    fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    fn value(&self) -> String {
        self.value_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.value.clone())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for DetailRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let label_style = detail_row_label_style(&theme);
        let value_style = detail_row_value_style(&theme);
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.max(0.0)
        } else {
            let label = measure_text(ctx, &self.label().to_uppercase(), &label_style);
            let value = measure_text(ctx, &self.value(), &value_style);
            label.width.max(value.width)
        };
        let value = self.value();
        let lines = wrap_detail_row_value(&value, width, self.max_value_lines, |text| {
            measure_text(ctx, text, &value_style).width
        });
        constraints.clamp(Size::new(width, detail_row_height(&theme, lines.len())))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        paint_detail_row_at(
            ctx,
            &theme,
            Point::new(ctx.bounds().x(), ctx.bounds().y()),
            ctx.bounds().width(),
            &self.label(),
            &self.value(),
            self.max_value_lines,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.label());
        node.value = Some(SemanticsValue::Text(self.value()));
        ctx.push(node);
    }
}

pub fn paint_detail_row_at(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    origin: Point,
    width: f32,
    label: &str,
    value: &str,
    max_value_lines: Option<usize>,
) -> f32 {
    let width = width.max(0.0);
    let label_style = detail_row_label_style(theme);
    let value_style = detail_row_value_style(theme);
    let lines = wrap_detail_row_value(value, width, max_value_lines, |text| {
        ctx.measure_text(text.to_string(), value_style.clone())
            .map(|measurement| measurement.width)
            .unwrap_or(0.0)
    });
    let height = detail_row_height(theme, lines.len());
    let clip = Rect::new(origin.x, origin.y, width, height);

    ctx.push_clip_rect(clip);
    paint_aligned_text(
        ctx,
        Rect::new(origin.x, origin.y, width, label_style.line_height),
        &label.to_uppercase(),
        &label_style,
        label_style.line_height,
        0.0,
    );

    let mut y = origin.y + label_style.line_height + detail_row_label_value_gap(theme);
    for line in lines {
        paint_aligned_text(
            ctx,
            Rect::new(origin.x, y, width, value_style.line_height),
            &line,
            &value_style,
            value_style.line_height,
            0.0,
        );
        y += value_style.line_height;
    }
    ctx.pop_clip();
    height
}

pub fn detail_row_height_for_value(
    ctx: &PaintCtx,
    theme: &DefaultTheme,
    width: f32,
    value: &str,
    max_value_lines: Option<usize>,
) -> f32 {
    let width = width.max(0.0);
    let value_style = detail_row_value_style(theme);
    let lines = wrap_detail_row_value(value, width, max_value_lines, |text| {
        ctx.measure_text(text.to_string(), value_style.clone())
            .map(|measurement| measurement.width)
            .unwrap_or(0.0)
    });
    detail_row_height(theme, lines.len())
}

fn detail_row_label_style(theme: &DefaultTheme) -> TextStyle {
    let mut style = text_token_style(theme, theme.text.xs, theme.palette.text_muted);
    style.weight = FontWeight::SEMIBOLD;
    style.line_height = style.line_height.max(14.0);
    style
}

fn detail_row_value_style(theme: &DefaultTheme) -> TextStyle {
    let mut style = text_token_style(theme, theme.text.sm, theme.palette.text);
    style.line_height = style.line_height.max(17.0);
    style
}

fn detail_row_label_value_gap(theme: &DefaultTheme) -> f32 {
    (theme.metrics.icon_label_gap * 0.35).max(2.0)
}

fn detail_row_bottom_gap(theme: &DefaultTheme) -> f32 {
    theme.metrics.property_row_stacked_gap.max(6.0)
}

fn detail_row_height(theme: &DefaultTheme, value_lines: usize) -> f32 {
    let label_style = detail_row_label_style(theme);
    let value_style = detail_row_value_style(theme);
    label_style.line_height
        + detail_row_label_value_gap(theme)
        + value_style.line_height * value_lines.max(1) as f32
        + detail_row_bottom_gap(theme)
}

fn wrap_detail_row_value<F>(
    value: &str,
    width: f32,
    max_lines: Option<usize>,
    mut measure: F,
) -> Vec<String>
where
    F: FnMut(&str) -> f32,
{
    let max_lines = max_lines.unwrap_or(usize::MAX).max(1);
    let width = width.max(1.0);
    let mut lines = Vec::new();

    for paragraph in value.split('\n') {
        if lines.len() >= max_lines {
            break;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || measure(&candidate) <= width {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                if lines.len() >= max_lines {
                    break;
                }
                current = word.to_string();
            }
        }
        if lines.len() < max_lines {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub struct FormRow {
    row: PropertyRow,
}

impl FormRow {
    pub fn new<W>(label: impl Into<String>, control: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            row: PropertyRow::new(label, control)
                .inline()
                .with_form_defaults(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.row = self.row.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.row = self.row.theme_when(theme);
        self
    }

    pub fn stacked(mut self) -> Self {
        self.row = self.row.stacked();
        self
    }

    pub fn inline(mut self) -> Self {
        self.row = self.row.inline();
        self
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.row = self.row.label_width(width);
        self
    }

    pub fn control_width(mut self, width: f32) -> Self {
        self.row = self.row.control_width(width);
        self
    }

    pub fn auto_control_width(mut self) -> Self {
        self.row = self.row.auto_control_width();
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.row = self.row.gap(gap);
        self
    }

    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.row = self.row.label_style(style);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.row.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.row.child_mut()
    }
}

impl Widget for FormRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.row.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.row.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.row.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.row.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.row.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.row.visit_children_mut(visitor);
    }
}

pub struct FieldGroup {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    children: WidgetChildren,
    spacing: Option<f32>,
    padding: Insets,
    max_width: Option<f32>,
    fill_width: bool,
}

impl FieldGroup {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            children: WidgetChildren::new(),
            spacing: None,
            padding: Insets::ZERO,
            max_width: None,
            fill_width: false,
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

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width.max(0.0));
        self
    }

    pub fn auto_width(mut self) -> Self {
        self.max_width = None;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn children(&self) -> &[sui_runtime::WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [sui_runtime::WidgetPod] {
        self.children.as_mut_slice()
    }

    fn content_max_width(&self, constraints: Constraints) -> f32 {
        let available = if constraints.max.width.is_finite() {
            (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        self.max_width
            .map(|width| width.min(available))
            .unwrap_or(available)
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        let inset = inset_rect(bounds, self.padding);
        let width = self
            .max_width
            .map(|max_width| max_width.min(inset.width()))
            .unwrap_or(inset.width())
            .max(0.0);
        Rect::new(inset.x(), inset.y(), width, inset.height())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_spacing(&self) -> f32 {
        self.spacing
            .unwrap_or_else(|| self.resolved_theme().metrics.field_group_spacing)
    }
}

impl Default for FieldGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for FieldGroup {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let spacing = self.resolved_spacing();
        let content_max_width = self.content_max_width(constraints);
        let mut y: f32 = 0.0;
        let mut width: f32 = 0.0;
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                y += spacing;
            }
            let child_size = child.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(content_max_width, constraints.max.height),
                ),
            );
            y += child_size.height;
            width = width.max(child_size.width);
        }

        if self.fill_width && content_max_width.is_finite() {
            width = content_max_width;
        }

        constraints.clamp(Size::new(
            width + self.padding.left + self.padding.right,
            y + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let spacing = self.resolved_spacing();
        let content = self.content_rect(bounds);
        let mut y = content.y();
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                y += spacing;
            }
            let measured = child.measured_size();
            let width = if self.fill_width {
                content.width()
            } else {
                measured.width.min(content.width())
            };
            child.arrange(ctx, Rect::new(content.x(), y, width, measured.height));
            y += measured.height;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

pub struct FormSection {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    title: String,
    description: Option<String>,
    title_style: Option<TextStyle>,
    description_style: Option<TextStyle>,
    header_action: Option<SingleChild>,
    child: SingleChild,
    padding: Option<Insets>,
    body_gap: Option<f32>,
    header_gap: Option<f32>,
    description_gap: Option<f32>,
    max_width: Option<f32>,
    auto_width: bool,
    radius: Option<f32>,
    elevation: SurfaceElevation,
    fill_width: bool,
    title_measurement: Option<TextMeasurement>,
    description_measurement: Option<TextMeasurement>,
}

impl FormSection {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            description: None,
            title_style: None,
            description_style: None,
            header_action: None,
            child: SingleChild::new(child),
            padding: None,
            body_gap: None,
            header_gap: None,
            description_gap: None,
            max_width: None,
            auto_width: false,
            radius: None,
            elevation: SurfaceElevation::Small,
            fill_width: false,
            title_measurement: None,
            description_measurement: None,
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

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn title_style(mut self, style: TextStyle) -> Self {
        self.title_style = Some(style);
        self
    }

    pub fn description_style(mut self, style: TextStyle) -> Self {
        self.description_style = Some(style);
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn body_gap(mut self, gap: f32) -> Self {
        self.body_gap = Some(gap.max(0.0));
        self
    }

    pub fn header_gap(mut self, gap: f32) -> Self {
        self.header_gap = Some(gap.max(0.0));
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width.max(0.0));
        self.auto_width = false;
        self
    }

    pub fn auto_width(mut self) -> Self {
        self.max_width = None;
        self.auto_width = true;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius.max(0.0));
        self
    }

    pub fn elevation(mut self, elevation: SurfaceElevation) -> Self {
        self.elevation = elevation;
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.form_section_padding)
    }

    fn resolved_body_gap(&self, metrics: ControlMetrics) -> f32 {
        self.body_gap.unwrap_or(metrics.form_section_body_gap)
    }

    fn resolved_header_gap(&self, metrics: ControlMetrics) -> f32 {
        self.header_gap.unwrap_or(metrics.form_section_header_gap)
    }

    fn resolved_description_gap(&self, metrics: ControlMetrics) -> f32 {
        self.description_gap
            .unwrap_or(metrics.form_section_description_gap)
    }

    fn resolved_max_width(&self, metrics: ControlMetrics) -> Option<f32> {
        if self.auto_width {
            None
        } else {
            Some(self.max_width.unwrap_or(metrics.form_section_max_width))
        }
    }

    fn resolved_radius(&self, metrics: ControlMetrics) -> f32 {
        self.radius.unwrap_or(metrics.form_section_radius).max(0.0)
    }

    fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.title_style.clone().unwrap_or_else(|| TextStyle {
            weight: FontWeight::SEMIBOLD,
            ..text_token_style(&theme, theme.text.sm, theme.surfaces.text)
        })
    }

    fn resolved_description_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.description_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.xs, theme.surfaces.text_muted))
    }

    fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn description_height(&self, style: &TextStyle) -> f32 {
        if self.description.is_some() {
            self.description_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(style.line_height)
                .max(style.line_height)
        } else {
            0.0
        }
    }

    fn text_block_height(&self, title_style: &TextStyle, description_style: &TextStyle) -> f32 {
        let title = self.title_height(title_style);
        let description = self.description_height(description_style);
        if description > 0.0 {
            let metrics = self.resolved_theme().metrics;
            title + self.resolved_description_gap(metrics) + description
        } else {
            title
        }
    }

    fn content_max_width(&self, available_width: f32) -> f32 {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let available = if available_width.is_finite() {
            (available_width - padding.left - padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        self.resolved_max_width(metrics)
            .map(|width| width.min(available))
            .unwrap_or(available)
    }

    fn card_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let width = if self.fill_width {
            bounds.width()
        } else {
            self.resolved_max_width(metrics)
                .map(|max_width| (max_width + padding.left + padding.right).min(bounds.width()))
                .unwrap_or(bounds.width())
        }
        .max(0.0);
        let x = if self.fill_width || width >= bounds.width() {
            bounds.x()
        } else {
            bounds.x() + ((bounds.width() - width) * 0.5)
        };
        Rect::new(x, bounds.y(), width, bounds.height())
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        inset_rect(self.card_rect(bounds), self.resolved_padding(theme.metrics))
    }

    fn header_height(&self, title_style: &TextStyle, description_style: &TextStyle) -> f32 {
        let text_height = self.text_block_height(title_style, description_style);
        let action_height = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().height)
            .unwrap_or(0.0);
        text_height.max(action_height)
    }
}

impl Widget for FormSection {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let body_gap = self.resolved_body_gap(metrics);
        let header_gap = self.resolved_header_gap(metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title);
        let description = self
            .description
            .as_ref()
            .map(|description| measure_text(ctx, description, &description_style));
        self.description_measurement = description;

        let content_max_width = self.content_max_width(constraints.max.width);
        let action_size = self
            .header_action
            .as_mut()
            .map(|action| {
                action.measure(
                    ctx,
                    Constraints::new(
                        Size::ZERO,
                        Size::new(content_max_width, constraints.max.height),
                    ),
                )
            })
            .unwrap_or(Size::ZERO);
        let action_extent = if self.header_action.is_some() {
            action_size.width + header_gap
        } else {
            0.0
        };
        let text_width = title.width.max(
            description
                .map(|measurement| measurement.width)
                .unwrap_or(0.0),
        );
        let header_width = (text_width + action_extent).min(content_max_width);
        let child_size = self.child.measure(
            ctx,
            Constraints::new(
                Size::ZERO,
                Size::new(content_max_width, constraints.max.height),
            ),
        );
        let content_width = header_width.max(child_size.width).min(content_max_width);
        let header_height = self.header_height(&title_style, &description_style);

        let mut width = content_width + padding.left + padding.right;
        if self.fill_width && constraints.max.width.is_finite() {
            width = constraints.max.width;
        }
        let height = padding.top + header_height + body_gap + child_size.height + padding.bottom;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let body_gap = self.resolved_body_gap(metrics);
        let content = self.content_rect(bounds);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let header_height = self.header_height(&title_style, &description_style);

        if let Some(action) = &mut self.header_action {
            let action_size = action.child().measured_size();
            action.arrange(
                ctx,
                Rect::new(
                    content.max_x() - action_size.width,
                    content.y() + ((header_height - action_size.height) * 0.5).max(0.0),
                    action_size.width,
                    action_size.height,
                ),
            );
        }

        let child_size = self.child.child().measured_size();
        self.child.arrange(
            ctx,
            Rect::new(
                content.x(),
                content.y() + header_height + body_gap,
                child_size.width.min(content.width()),
                child_size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let card = self.card_rect(ctx.bounds());
        let radius = self
            .resolved_radius(metrics)
            .min(card.width().min(card.height()) * 0.5);
        let shadow = match self.elevation {
            SurfaceElevation::None => None,
            SurfaceElevation::Small => Some(&theme.shadows.box_shadow.sm),
            SurfaceElevation::Medium => Some(&theme.shadows.box_shadow.md),
            SurfaceElevation::Large => Some(&theme.shadows.box_shadow.lg),
        };
        if let Some(shadow) = shadow {
            paint_theme_shadow(ctx, card, [radius; 4], shadow);
        }

        let background = theme.surfaces.panel;
        let border = theme.surfaces.border;
        let shape = rounded_rect_path(card, radius);
        ctx.fill(shape.clone(), background);
        ctx.stroke(
            shape,
            border,
            StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width.max(1.0))),
        );

        let content = inset_rect(card, self.resolved_padding(metrics));
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title_height = self.title_height(&title_style);
        let description_height = self.description_height(&description_style);
        let header_gap = self.resolved_header_gap(metrics);
        let description_gap = self.resolved_description_gap(metrics);
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + header_gap)
            .unwrap_or(0.0)
            .min(content.width());
        let text_width = (content.width() - action_width).max(0.0);
        let text_block_height = self.text_block_height(&title_style, &description_style);
        let header_height = self.header_height(&title_style, &description_style);
        let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        if let Some(description) = &self.description {
            let description_slot = Rect::new(
                content.x(),
                title_slot.max_y() + description_gap,
                text_width,
                description_height,
            );
            ctx.push_clip_rect(description_slot);
            paint_aligned_text(
                ctx,
                description_slot,
                description,
                &description_style,
                description_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }

        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let card = self.card_rect(ctx.bounds());
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::GenericContainer, card);
        node.name = Some(self.title.clone());
        node.description = self.description.clone();
        ctx.push(node);
        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.header_action {
            action.visit_children(visitor);
        }
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.header_action {
            action.visit_children_mut(visitor);
        }
        self.child.visit_children_mut(visitor);
    }
}

pub struct PanelSection {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    title: String,
    gap: Option<f32>,
    action_gap: Option<f32>,
    title_style: Option<TextStyle>,
    header_action: Option<SingleChild>,
    child: SingleChild,
    title_measurement: Option<TextMeasurement>,
    collapsible: bool,
    expanded: bool,
    hovered_header: bool,
    pressed_header: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
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
            gap: None,
            action_gap: None,
            title_style: None,
            header_action: None,
            child: SingleChild::new(child),
            title_measurement: None,
            collapsible: false,
            expanded: true,
            hovered_header: false,
            pressed_header: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
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
        self.gap = Some(gap.max(0.0));
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
        self.action_gap = Some(gap.max(0.0));
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
        self.title_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.xs, theme.palette.text_muted))
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(metrics.panel_section_gap)
    }

    fn resolved_action_gap(&self, metrics: ControlMetrics) -> f32 {
        self.action_gap.unwrap_or(metrics.panel_section_action_gap)
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

    fn disclosure_width(&self, metrics: ControlMetrics) -> f32 {
        if self.collapsible {
            metrics.panel_section_disclosure_size
        } else {
            0.0
        }
    }

    fn title_rect(&self, bounds: Rect, header_height: f32, title_height: f32) -> Rect {
        let metrics = self.resolved_theme().metrics;
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.resolved_action_gap(metrics))
            .unwrap_or(0.0)
            .min(bounds.width());
        let disclosure_width = self.disclosure_width(metrics);
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
        let metrics = self.resolved_theme().metrics;
        let header = self.header_rect(bounds);
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.resolved_action_gap(metrics))
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
        self.set_pressed_header(false, ctx);
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_hovered_header(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered_header == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered_header = hovered;
        set_hover_animation_target(&mut self.hover_animation, hovered as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed_header(&mut self, pressed: bool, ctx: &mut EventCtx) {
        if self.pressed_header == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed_header = pressed;
        set_press_animation_target(&mut self.press_animation, pressed as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
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
                self.set_hovered_header(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .header_hit_rect(ctx.bounds())
                        .contains(pointer.position) =>
            {
                self.set_hovered_header(true, ctx);
                self.set_pressed_header(true, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
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
                self.set_hovered_header(hovered, ctx);
                self.set_pressed_header(false, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered_header(false, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_header || self.hovered_header {
                    self.set_hovered_header(false, ctx);
                    self.set_pressed_header(false, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let action_gap = self.resolved_action_gap(metrics);
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
            self.disclosure_width(metrics)
                + title_measurement.width
                + action_gap
                + action_size.width
        } else {
            self.disclosure_width(metrics) + title_measurement.width
        };
        let natural = Size::new(
            header_width.max(child_size.width),
            header_height
                + if self.is_expanded() && child_size.height > 0.0 {
                    gap + child_size.height
                } else {
                    0.0
                },
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
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
                .min((bounds.height() - header_height - gap).max(0.0))
        } else {
            0.0
        };
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + header_height + gap,
                bounds.width().min(child_size.width).max(0.0),
                child_height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header_height = self.header_height(&title_style);
        let title_slot = self.title_rect(ctx.bounds(), header_height, title_height);
        if self.collapsible {
            let header_hit = self.header_hit_rect(ctx.bounds());
            let hover_amount = self.hover_animation.value;
            let press_amount = self.press_animation.value;
            let focus_amount = self.focus_animation.value;
            if focus_amount > AnimatedScalar::EPSILON {
                let outset = physical_pixels(ctx, metrics.focus_ring_outset);
                ctx.stroke(
                    rounded_rect_path(
                        header_hit.inflate(outset, outset),
                        metrics.indicator_corner_radius + outset,
                    ),
                    theme
                        .palette
                        .focus_ring
                        .with_alpha(theme.palette.focus_ring.alpha * focus_amount),
                    StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
                );
            }
            let hover_alpha = (theme.interaction.hover_blend * 0.07 * hover_amount).min(0.08);
            let press_alpha = (theme.interaction.selected_blend * 0.48 * press_amount).min(0.14);
            let header_fill = if press_alpha > 0.0 {
                theme.palette.accent.with_alpha(press_alpha)
            } else if hover_alpha > 0.0 {
                theme.palette.accent.with_alpha(hover_alpha)
            } else {
                theme.palette.surface.with_alpha(0.001)
            };
            ctx.fill(
                rounded_rect_path(header_hit, metrics.indicator_corner_radius),
                header_fill,
            );
            paint_panel_section_disclosure(
                ctx,
                self.header_rect(ctx.bounds()),
                self.expanded,
                hover_amount,
                press_amount,
                &theme,
                metrics.panel_section_disclosure_size,
            );
        }
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if self.collapsible {
            let theme = self.resolved_theme();
            set_focus_animation_target(
                &mut self.focus_animation,
                focused as u8 as f32,
                &theme,
                ctx,
            );
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
    hover_amount: f32,
    press_amount: f32,
    theme: &DefaultTheme,
    disclosure_size: f32,
) {
    let palette = theme.palette;
    let center = Point::new(
        header.x() + disclosure_size * 0.5,
        header.y() + header.height() * 0.5,
    );
    let half = disclosure_size * 0.25;
    let tip = disclosure_size * 0.22;
    let base_color = palette.text.with_alpha(0.68);
    let hover_color = mix_color(base_color, palette.text, hover_amount);
    let color = mix_color(hover_color, palette.accent, press_amount);
    let mut builder = PathBuilder::new();
    if expanded {
        builder
            .move_to(Point::new(center.x - half, center.y - tip * 0.55))
            .line_to(Point::new(center.x + half, center.y - tip * 0.55))
            .line_to(Point::new(center.x, center.y + tip));
    } else {
        builder
            .move_to(Point::new(center.x - tip * 0.55, center.y - half))
            .line_to(Point::new(center.x + tip, center.y))
            .line_to(Point::new(center.x - tip * 0.55, center.y + half));
    }
    ctx.fill(builder.build(), color);
}

pub struct DockPanel {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: Option<String>,
    title: String,
    header_height: Option<f32>,
    padding: Option<Insets>,
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
            header_height: None,
            padding: None,
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
        self.header_height = Some(height.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
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
        text_token_style(&theme, theme.text.sm, theme.palette.text)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_header_height(&self, metrics: ControlMetrics) -> f32 {
        self.header_height
            .unwrap_or(metrics.dock_panel_header_height)
    }

    fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.dock_panel_padding)
    }

    fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            self.resolved_header_height(theme.metrics),
        )
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let header_height = self.resolved_header_height(metrics);
        inset_rect(
            Rect::new(
                bounds.x(),
                bounds.y() + header_height,
                bounds.width(),
                (bounds.height() - header_height).max(0.0),
            ),
            self.resolved_padding(metrics),
        )
    }

    fn child_constraints(&self, constraints: Constraints) -> Constraints {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let header_height = self.resolved_header_height(metrics);
        let width = if constraints.max.width.is_finite() {
            (constraints.max.width - padding.left - padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        let height = if constraints.max.height.is_finite() {
            (constraints.max.height - header_height - padding.top - padding.bottom).max(0.0)
        } else {
            f32::INFINITY
        };
        Constraints::new(Size::ZERO, Size::new(width, height))
    }
}

impl Widget for DockPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let header_height = self.resolved_header_height(metrics);
        let title_style = self.resolved_title_style();
        let title_measurement = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title_measurement);
        let child_size = self.child.measure(ctx, self.child_constraints(constraints));
        let natural = Size::new(
            (title_measurement.width + padding.left + padding.right)
                .max(child_size.width + padding.left + padding.right),
            header_height + padding.top + child_size.height + padding.bottom,
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let bounds = ctx.bounds();
        let header = self.header_rect(bounds);
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let title_slot = Rect::new(
            header.x() + padding.left,
            header.y() + ((header.height() - title_height) * 0.5).max(0.0),
            (header.width() - padding.left - padding.right).max(0.0),
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
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();

        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let padding = self.resolved_padding(theme.metrics);
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
                header.x() + padding.left,
                header.y() + ((header.height() - title_height) * 0.5).max(0.0),
                (header.width() - padding.left - padding.right).max(0.0),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HairlineEdge {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn paint_rounded_rect(ctx: &mut PaintCtx, rect: Rect, color: Color, radius: f32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    if radius <= 0.5 {
        ctx.fill_rect(rect, color);
    } else {
        ctx.fill(Path::rounded_rect(rect, radius), color);
    }
}

pub fn paint_rounded_panel(
    ctx: &mut PaintCtx,
    rect: Rect,
    fill: Color,
    border: Color,
    radius: f32,
) {
    paint_rounded_rect(ctx, rect, border, radius);
    paint_rounded_rect(ctx, rect.inflate(-1.0, -1.0), fill, (radius - 1.0).max(0.0));
}

pub fn paint_hairline(ctx: &mut PaintCtx, rect: Rect, edge: HairlineEdge, color: Color) {
    let line = match edge {
        HairlineEdge::Top => Rect::new(rect.x(), rect.y(), rect.width(), 1.0),
        HairlineEdge::Right => Rect::new(rect.max_x() - 1.0, rect.y(), 1.0, rect.height()),
        HairlineEdge::Bottom => Rect::new(rect.x(), rect.max_y() - 1.0, rect.width(), 1.0),
        HairlineEdge::Left => Rect::new(rect.x(), rect.y(), 1.0, rect.height()),
    };
    ctx.fill_rect(line, color);
}

pub fn paint_border(ctx: &mut PaintCtx, rect: Rect, color: Color) {
    paint_hairline(ctx, rect, HairlineEdge::Top, color);
    paint_hairline(ctx, rect, HairlineEdge::Right, color);
    paint_hairline(ctx, rect, HairlineEdge::Bottom, color);
    paint_hairline(ctx, rect, HairlineEdge::Left, color);
}

fn paint_centered_text(ctx: &mut PaintCtx, text: &str, style: TextStyle, cx: f32, cy: f32) {
    let measurement = ctx.measure_text(text.to_string(), style.clone()).ok();
    let (width, height) = measurement
        .map(|measurement| (measurement.width, measurement.height))
        .unwrap_or((
            text.chars().count() as f32 * style.font_size * 0.55,
            style.line_height,
        ));
    ctx.draw_text(
        Rect::new(
            cx - width * 0.5,
            cy - height * 0.5,
            width + 2.0,
            height + 2.0,
        ),
        text.to_string(),
        style,
    );
}

#[derive(Clone, Copy)]
pub struct EmptyStatePaint<'a> {
    icon: Option<IconGlyph>,
    title: &'a str,
    description: &'a str,
    detail: Option<&'a str>,
    background: Option<Color>,
    center_offset_y: f32,
    reserve_action_space: bool,
}

impl<'a> EmptyStatePaint<'a> {
    pub const fn new(title: &'a str, description: &'a str) -> Self {
        Self {
            icon: None,
            title,
            description,
            detail: None,
            background: None,
            center_offset_y: 0.0,
            reserve_action_space: false,
        }
    }

    pub const fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    pub const fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub const fn center_offset_y(mut self, offset: f32) -> Self {
        self.center_offset_y = offset;
        self
    }

    pub const fn reserve_action_space(mut self, reserve: bool) -> Self {
        self.reserve_action_space = reserve;
        self
    }
}

pub fn paint_empty_state(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    bounds: Rect,
    paint: EmptyStatePaint<'_>,
) {
    if let Some(background) = paint.background {
        ctx.fill_rect(bounds, background);
    }

    let cx = bounds.x() + bounds.width() * 0.5;
    let cy = bounds.y() + bounds.height() * 0.5
        - if paint.reserve_action_space {
            18.0
        } else {
            0.0
        }
        + paint.center_offset_y;
    let icon_color = theme.surfaces.text_faint;
    if let Some(icon) = paint.icon {
        let side = 40.0;
        draw_icon_glyph(
            ctx,
            icon,
            Rect::new(cx - side * 0.5, cy - 46.0 - side * 0.5, side, side),
            icon_color,
        );
    }

    let mut title_style = text_token_style(theme, theme.text.lg, theme.surfaces.text_muted);
    title_style.weight = FontWeight::SEMIBOLD;
    paint_centered_text(ctx, paint.title, title_style, cx, cy + 4.0);

    let description_style = text_token_style(theme, theme.text.sm, theme.surfaces.text_faint);
    paint_centered_text(ctx, paint.description, description_style, cx, cy + 30.0);

    if let Some(detail) = paint.detail {
        let mut detail_style = text_token_style(theme, theme.text.xs, theme.surfaces.text_muted);
        detail_style.weight = FontWeight::MEDIUM;
        paint_centered_text(ctx, detail, detail_style, cx, cy + 48.0);
    }
}

pub struct EmptyState {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: Option<String>,
    icon: Option<IconGlyph>,
    title: String,
    description: String,
    detail: Option<String>,
    action: Option<SingleChild>,
    action_height: f32,
    action_max_width: f32,
    background: Option<Color>,
}

impl EmptyState {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            icon: None,
            title: title.into(),
            description: description.into(),
            detail: None,
            action: None,
            action_height: 32.0,
            action_max_width: 360.0,
            background: None,
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

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.action = Some(SingleChild::new(action));
        self
    }

    pub fn action_height(mut self, height: f32) -> Self {
        self.action_height = height.max(32.0);
        self
    }

    pub fn action_max_width(mut self, width: f32) -> Self {
        self.action_max_width = width.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn transparent(mut self) -> Self {
        self.background = Some(Color::TRANSPARENT);
        self
    }

    pub fn action_child(&self) -> Option<&WidgetPod> {
        self.action.as_ref().map(SingleChild::child)
    }

    pub fn action_child_mut(&mut self) -> Option<&mut WidgetPod> {
        self.action.as_mut().map(SingleChild::child_mut)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| (*self.theme).clone())
    }

    fn action_rect(&self, bounds: Rect) -> Rect {
        let width = self.action_max_width.min((bounds.width() - 32.0).max(0.0));
        let cx = bounds.x() + bounds.width() * 0.5;
        let cy = bounds.y() + bounds.height() * 0.5 - 18.0;
        Rect::new(cx - width * 0.5, cy + 54.0, width, self.action_height)
    }
}

impl Widget for EmptyState {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(self.action_max_width + 32.0)
        };
        let action_height = if let Some(action) = &mut self.action {
            let action_width = self.action_max_width.min((width - 32.0).max(0.0));
            action.measure(
                ctx,
                Constraints::tight(Size::new(action_width, self.action_height)),
            );
            self.action_height
        } else {
            0.0
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints
                .min
                .height
                .max(142.0 + action_height + if action_height > 0.0 { 12.0 } else { 0.0 })
        };
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let action_rect = self.action_rect(bounds);
        if let Some(action) = &mut self.action {
            action.arrange(ctx, action_rect);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let mut paint = EmptyStatePaint::new(&self.title, &self.description)
            .background(self.background.unwrap_or(theme.surfaces.window))
            .reserve_action_space(self.action.is_some());
        if let Some(icon) = self.icon {
            paint = paint.icon(icon);
        }
        if let Some(detail) = self.detail.as_deref() {
            paint = paint.detail(detail);
        }
        paint_empty_state(ctx, &theme, bounds, paint);

        if let Some(action) = &self.action {
            action.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone().unwrap_or_else(|| self.title.clone()));
        node.description = Some(match &self.detail {
            Some(detail)
                if matches!(
                    self.description.chars().last(),
                    Some('.') | Some('!') | Some('?')
                ) =>
            {
                format!("{} {}", self.description, detail)
            }
            Some(detail) => format!("{}. {}", self.description, detail),
            None => self.description.clone(),
        });
        ctx.push(node);
        if let Some(action) = &self.action {
            action.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.action {
            action.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.action {
            action.visit_children_mut(visitor);
        }
    }
}

pub struct PresetStrip {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    presets: Vec<String>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    item_width: Option<f32>,
    item_height: Option<f32>,
    gap: Option<f32>,
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
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            item_width: None,
            item_height: None,
            gap: None,
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
        self.item_height = Some(height.max(20.0));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
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

    fn resolved_item_height(&self, metrics: ControlMetrics) -> f32 {
        self.item_height.unwrap_or(metrics.preset_strip_item_height)
    }

    fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(metrics.preset_strip_gap)
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.presets.len() || self.item_widths.len() != self.presets.len() {
            return None;
        }

        let metrics = self.resolved_theme().metrics;
        let item_height = self.resolved_item_height(metrics);
        let gap = self.resolved_gap(metrics);
        let mut x = bounds.x();
        for (current, width) in self.item_widths.iter().enumerate() {
            let available = (bounds.max_x() - x).max(0.0);
            let rect = Rect::new(x, bounds.y(), width.min(available), item_height);
            if current == index {
                return (!rect.is_empty()).then_some(rect);
            }
            x += *width + gap;
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

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        hover_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for PresetStrip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.item_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                if self.hovered.is_some() {
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
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
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let item_height = self.resolved_item_height(metrics);
        let gap = self.resolved_gap(metrics);
        let style = text_token_style(&theme, theme.text.xs, theme.palette.text);
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
                    (measurement.width
                        + metrics.preset_strip_item_padding.left
                        + metrics.preset_strip_item_padding.right)
                        .max(metrics.preset_strip_item_min_width),
                )
            })
            .collect();

        let width = self.item_widths.iter().sum::<f32>()
            + (gap * self.presets.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width, item_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let selected = self.current_selected();
        let style = text_token_style(&theme, theme.text.xs, palette.text);

        if self.focus_animation.value > AnimatedScalar::EPSILON {
            ctx.stroke(
                rounded_rect_path(ctx.bounds().inflate(2.0, 2.0), metrics.corner_radius + 2.0),
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
                StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
            );
        }

        for (index, preset) in self.presets.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let is_selected = selected == Some(index);
            let is_hovered = self.hovered == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let base_background = if is_selected {
                palette.accent
            } else {
                palette.surface
            };
            let hover_background = if hover_amount > 0.0 {
                mix_color(
                    base_background,
                    palette.control_hover,
                    interaction.hover_blend * if is_selected { 0.35 } else { 1.0 } * hover_amount,
                )
            } else {
                base_background
            };
            let background = if press_amount > 0.0 {
                mix_color(
                    hover_background,
                    palette.control_active,
                    interaction.pressed_blend * if is_selected { 0.45 } else { 1.0 } * press_amount,
                )
            } else {
                hover_background
            };
            let border = if is_selected {
                palette.accent_border
            } else if is_hovered || hover_amount > 0.0 || press_amount > 0.0 {
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

            let text_slot = inset_rect(rect, metrics.preset_strip_label_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            let text_style = TextStyle {
                color: text_color,
                ..style.clone()
            };
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                preset,
                &text_style,
                text_style.line_height,
                0.5,
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
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

pub struct StatusBarSegment {
    text: String,
    reader: Option<Box<dyn Fn() -> String>>,
    min_width: Option<f32>,
    tone: SemanticTone,
    expand: bool,
}

impl StatusBarSegment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reader: None,
            min_width: None,
            tone: SemanticTone::Neutral,
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
            min_width: None,
            tone: SemanticTone::Neutral,
            expand: false,
        }
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
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
    description: Option<String>,
    description_reader: Option<Box<dyn Fn() -> String>>,
    height: Option<f32>,
    segments: Vec<StatusBarSegment>,
    measured_widths: Vec<f32>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            description: None,
            description_reader: None,
            height: None,
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

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self.description_reader = None;
        self
    }

    pub fn description_when<F>(mut self, description: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.description_reader = Some(Box::new(description));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(18.0));
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
        text_token_style(&theme, theme.text.xs, theme.palette.placeholder)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_height(&self, metrics: ControlMetrics) -> f32 {
        self.height.unwrap_or(metrics.status_bar_height)
    }

    fn description_text(&self) -> Option<String> {
        self.description_reader
            .as_ref()
            .map(|reader| reader())
            .or_else(|| self.description.clone())
    }

    fn resolved_segment_min_width(segment: &StatusBarSegment, metrics: ControlMetrics) -> f32 {
        segment
            .min_width
            .unwrap_or(metrics.status_bar_segment_min_width)
    }

    fn segment_widths(&self, metrics: ControlMetrics) -> Vec<f32> {
        if self.measured_widths.len() == self.segments.len() {
            self.measured_widths.clone()
        } else {
            self.segments
                .iter()
                .map(|segment| Self::resolved_segment_min_width(segment, metrics))
                .collect()
        }
    }

    fn segment_rects(&self, bounds: Rect, metrics: ControlMetrics) -> Vec<Rect> {
        let mut widths = self.segment_widths(metrics);
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
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let text_style = self.text_style();
        self.measured_widths = self
            .segments
            .iter()
            .map(|segment| {
                let text = segment.text();
                let segment_style = numeric_text_style_if_numeric(&text, text_style.clone());
                let measured = measure_text(ctx, &text, &segment_style).width
                    + metrics.status_bar_segment_padding * 2.0;
                Self::resolved_segment_min_width(segment, metrics).max(measured.ceil())
            })
            .collect();
        let natural_width: f32 = self.measured_widths.iter().sum();
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural_width
            },
            self.resolved_height(metrics),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
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
            .zip(self.segment_rects(bounds, metrics))
            .enumerate()
        {
            if rect.is_empty() {
                continue;
            }
            if index > 0 {
                let inset = metrics.status_bar_separator_inset.min(rect.height() * 0.5);
                ctx.stroke_rect(
                    Rect::new(
                        rect.x(),
                        rect.y() + inset,
                        1.0,
                        (rect.height() - inset * 2.0).max(0.0),
                    ),
                    palette.border.with_alpha(0.7),
                    StrokeStyle::new(1.0),
                );
            }
            let segment_text = segment.text();
            let segment_style = if segment.tone == SemanticTone::Neutral {
                text_style.clone()
            } else {
                let tone = theme.semantic_tone_color(segment.tone);
                let pill = Rect::new(
                    rect.x() + metrics.status_bar_separator_inset.min(rect.width() * 0.5),
                    rect.y() + metrics.status_bar_separator_inset.min(rect.height() * 0.5),
                    (rect.width() - metrics.status_bar_separator_inset * 2.0).max(0.0),
                    (rect.height() - metrics.status_bar_separator_inset * 2.0).max(0.0),
                );
                if !pill.is_empty() {
                    ctx.fill(
                        rounded_rect_path(pill, metrics.indicator_corner_radius),
                        tone.with_alpha(0.12),
                    );
                }
                TextStyle {
                    color: tone,
                    ..text_style.clone()
                }
            };
            let segment_style = numeric_text_style_if_numeric(&segment_text, segment_style);
            let content_rect = Rect::new(
                rect.x() + metrics.status_bar_segment_padding,
                rect.y(),
                (rect.width() - metrics.status_bar_segment_padding * 2.0).max(0.0),
                rect.height(),
            );
            ctx.push_clip_rect(content_rect);
            paint_aligned_text(
                ctx,
                content_rect,
                &segment_text,
                &segment_style,
                segment_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = self.name.clone();
        if let Some(description) = self.description_text() {
            node.value = Some(SemanticsValue::Text(description.clone()));
            node.description = Some(description);
        }
        ctx.push(node);

        for (index, (segment, rect)) in self
            .segments
            .iter()
            .zip(self.segment_rects(ctx.bounds(), metrics))
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

type SegmentedControlChange = Box<dyn FnMut(usize, String)>;
type SegmentedControlContextChange = Box<dyn FnMut(usize, String, &mut EventCtx)>;

fn segmented_control_item_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(269)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub struct StatusBadge {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    label_reader: Option<Box<dyn Fn() -> String>>,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    tone_reader: Option<Box<dyn Fn() -> SemanticTone>>,
    min_width: Option<f32>,
}

impl StatusBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            icon: None,
            tone: SemanticTone::Neutral,
            tone_reader: None,
            min_width: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self {
            label: fallback.into(),
            label_reader: Some(Box::new(reader)),
            ..Self::new("")
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

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self.tone_reader = None;
        self
    }

    pub fn tone_when<F>(mut self, tone: F) -> Self
    where
        F: Fn() -> SemanticTone + 'static,
    {
        self.tone_reader = Some(Box::new(tone));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    fn resolved_tone(&self) -> SemanticTone {
        self.tone_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.tone)
    }

    fn text_style(&self, theme: &DefaultTheme, label: &str, tone: SemanticTone) -> TextStyle {
        // Mesh badge label: xs/600 in the status ink that reads on the soft wash.
        let (_, tone_ink) = theme.semantic_tone_soft_colors(tone);
        let mut style = text_token_style(theme, theme.text.xs, tone_ink);
        style.weight = FontWeight::SEMIBOLD;
        numeric_text_style_if_numeric(label, style)
    }

    fn metrics(&self, theme: &DefaultTheme) -> (f32, f32, f32, f32) {
        let height = (theme.metrics.min_height - 2.0).max(22.0);
        let icon_size = (height - 13.0).clamp(11.0, 15.0);
        let gap = theme.metrics.icon_label_gap.max(4.0);
        let padding = theme.metrics.button_padding.left.max(8.0);
        (height, icon_size, gap, padding)
    }
}

pub fn paint_status_badge(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    // Mesh badge: soft status wash, no border, status-hued ink (`--sm-*-soft`
    // fill + `--sm-*-text` content) at the xs/600 label size, r-1 corners.
    let (tone_soft, tone_ink) = theme.semantic_tone_soft_colors(tone);
    let icon_size = (rect.height() - 13.0).clamp(11.0, 15.0);
    let gap = theme.metrics.icon_label_gap.max(4.0);
    let padding = theme.metrics.button_padding.left.max(6.0);
    let radius = theme.radius.sm.min(rect.height() * 0.5);

    ctx.fill(rounded_rect_path(rect, radius), tone_soft);

    let mut x = rect.x() + padding.min(rect.width() * 0.5);
    if let Some(icon) = icon {
        let icon_rect = Rect::new(
            x,
            rect.y() + (rect.height() - icon_size) * 0.5,
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, tone_ink);
        x = icon_rect.max_x() + gap;
    }
    let content_rect = Rect::new(
        x,
        rect.y(),
        (rect.max_x() - x - padding * 0.5).max(0.0),
        rect.height(),
    );
    if content_rect.width() <= 0.0 {
        return;
    }

    let mut style = text_token_style(theme, theme.text.xs, tone_ink);
    style.weight = FontWeight::SEMIBOLD;
    let style = numeric_text_style_if_numeric(label, style);
    ctx.push_clip_rect(content_rect);
    paint_aligned_text(ctx, content_rect, label, &style, style.line_height, 0.0);
    ctx.pop_clip();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandButtonFill {
    Surface,
    Filled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandButtonPaint {
    pub tone: SemanticTone,
    pub icon_tone: Option<SemanticTone>,
    pub fill: CommandButtonFill,
    pub hovered: bool,
    pub pressed: bool,
}

impl CommandButtonPaint {
    pub const fn neutral() -> Self {
        Self {
            tone: SemanticTone::Neutral,
            icon_tone: None,
            fill: CommandButtonFill::Surface,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn tonal(tone: SemanticTone) -> Self {
        Self {
            tone,
            icon_tone: None,
            fill: CommandButtonFill::Surface,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn filled(tone: SemanticTone) -> Self {
        Self {
            tone,
            icon_tone: None,
            fill: CommandButtonFill::Filled,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn icon_tone(mut self, tone: SemanticTone) -> Self {
        self.icon_tone = Some(tone);
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }
}

impl Default for CommandButtonPaint {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisclosureButtonPaint {
    pub command: CommandButtonPaint,
}

impl DisclosureButtonPaint {
    pub const fn new() -> Self {
        Self {
            command: CommandButtonPaint::tonal(SemanticTone::Accent)
                .icon_tone(SemanticTone::Accent),
        }
    }

    pub const fn command(mut self, command: CommandButtonPaint) -> Self {
        self.command = command;
        self
    }

    pub const fn tone(mut self, tone: SemanticTone) -> Self {
        self.command = CommandButtonPaint::tonal(tone).icon_tone(tone);
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.command = self.command.hovered(hovered);
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.command = self.command.pressed(pressed);
        self
    }
}

impl Default for DisclosureButtonPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_command_button(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    style: CommandButtonPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (tone_color, tone_text) = theme.semantic_tone_colors(style.tone);
    let (base_fill, border, label_color) = match style.fill {
        CommandButtonFill::Surface => {
            let label_color = if style.tone == SemanticTone::Neutral {
                theme.palette.text
            } else {
                tone_color
            };
            let border = if style.tone == SemanticTone::Neutral {
                theme.palette.border
            } else {
                tone_color.with_alpha(0.72)
            };
            (theme.surfaces.field, border, label_color)
        }
        CommandButtonFill::Filled => (tone_color, tone_color, tone_text),
    };
    let fill = match style.fill {
        CommandButtonFill::Surface if style.pressed => theme.palette.control_active,
        CommandButtonFill::Surface if style.hovered => theme.palette.control_hover,
        CommandButtonFill::Filled
            if style.pressed && matches!(style.tone, SemanticTone::Accent) =>
        {
            theme.palette.accent_pressed
        }
        CommandButtonFill::Filled
            if style.hovered && matches!(style.tone, SemanticTone::Accent) =>
        {
            theme.palette.accent_hover
        }
        _ => base_fill,
    };
    let icon_color = style
        .icon_tone
        .map(|tone| {
            if style.fill == CommandButtonFill::Filled && tone == style.tone {
                theme.semantic_tone_text_color(tone)
            } else if tone == SemanticTone::Neutral {
                theme.palette.text_muted
            } else {
                theme.semantic_tone_color(tone)
            }
        })
        .unwrap_or_else(|| match style.fill {
            CommandButtonFill::Surface => {
                if style.tone == SemanticTone::Neutral {
                    theme.palette.text_muted
                } else {
                    tone_color
                }
            }
            CommandButtonFill::Filled => tone_text,
        });

    let radius = theme
        .metrics
        .corner_radius
        .min(rect.height() * 0.35)
        .max(0.0);
    ctx.fill(rounded_rect_path(rect, radius), fill);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let icon_size = (rect.height() - 14.0).clamp(12.0, 16.0);
    let padding = theme
        .metrics
        .button_padding
        .left
        .max(8.0)
        .min(rect.width() * 0.4);
    let gap = theme.metrics.icon_label_gap.max(5.0);
    let mut text_x = rect.x() + padding;
    if let Some(icon) = icon {
        let icon_rect = Rect::new(
            rect.x() + padding,
            rect.y() + (rect.height() - icon_size) * 0.5,
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, icon_color);
        text_x = icon_rect.max_x() + gap;
    }

    let label_rect = Rect::new(
        text_x,
        rect.y(),
        (rect.max_x() - text_x - padding * 0.75).max(0.0),
        rect.height(),
    );
    if label_rect.width() <= 0.0 {
        return;
    }

    let mut text_style = text_token_style(theme, theme.text.sm, label_color);
    text_style.weight = FontWeight::SEMIBOLD;
    let text_style = fit_command_button_label_style(ctx, label, text_style, label_rect.width());
    let text_style = numeric_text_style_if_numeric(label, text_style);
    ctx.push_clip_rect(label_rect);
    paint_aligned_text(
        ctx,
        label_rect,
        label,
        &text_style,
        text_style.line_height,
        0.0,
    );
    ctx.pop_clip();
}

pub fn paint_disclosure_button(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    expanded: bool,
    paint: DisclosureButtonPaint,
) {
    paint_command_button(
        ctx,
        theme,
        rect,
        label,
        Some(if expanded {
            IconGlyph::ChevronUp
        } else {
            IconGlyph::ChevronDown
        }),
        paint.command,
    );
}

fn fit_command_button_label_style(
    ctx: &PaintCtx,
    label: &str,
    mut style: TextStyle,
    max_width: f32,
) -> TextStyle {
    if label.is_empty() || max_width <= 0.0 || style.font_size <= 0.0 {
        return style;
    }

    let measured_width = ctx
        .measure_text(label.to_string(), style.clone())
        .ok()
        .map(|measurement| measurement.width)
        .unwrap_or_else(|| label.chars().count() as f32 * style.font_size * 0.56);
    if measured_width <= max_width || measured_width <= 0.0 {
        return style;
    }

    let min_font_size = 10.0_f32.min(style.font_size);
    let scale = ((max_width * 0.98) / measured_width).clamp(min_font_size / style.font_size, 1.0);
    style.font_size = (style.font_size * scale).max(min_font_size);
    style.line_height = (style.line_height * scale).max(min_font_size + 2.0);
    style
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionTilePaint {
    pub tone: SemanticTone,
    pub highlighted: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub enabled: bool,
    pub background: Option<Color>,
    pub border: Option<Color>,
    pub title_color: Option<Color>,
    pub subtitle_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub leading_tone_dot: Option<SemanticTone>,
    pub radius: Option<f32>,
    pub padding_x: Option<f32>,
    pub leading_width: f32,
    pub trailing_width: f32,
}

impl ActionTilePaint {
    pub const fn neutral() -> Self {
        Self {
            tone: SemanticTone::Neutral,
            highlighted: false,
            hovered: false,
            pressed: false,
            enabled: true,
            background: None,
            border: None,
            title_color: None,
            subtitle_color: None,
            icon_color: None,
            leading_tone_dot: None,
            radius: None,
            padding_x: None,
            leading_width: 0.0,
            trailing_width: 0.0,
        }
    }

    pub const fn tonal(tone: SemanticTone) -> Self {
        Self {
            tone,
            highlighted: true,
            hovered: false,
            pressed: false,
            enabled: true,
            background: None,
            border: None,
            title_color: None,
            subtitle_color: None,
            icon_color: None,
            leading_tone_dot: None,
            radius: None,
            padding_x: None,
            leading_width: 0.0,
            trailing_width: 0.0,
        }
    }

    pub const fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub const fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn subtitle_color(mut self, subtitle_color: Color) -> Self {
        self.subtitle_color = Some(subtitle_color);
        self
    }

    pub const fn icon_color(mut self, icon_color: Color) -> Self {
        self.icon_color = Some(icon_color);
        self
    }

    pub const fn leading_tone_dot(mut self, tone: SemanticTone) -> Self {
        self.leading_tone_dot = Some(tone);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn padding_x(mut self, padding_x: f32) -> Self {
        self.padding_x = Some(padding_x);
        self
    }

    pub const fn leading_width(mut self, leading_width: f32) -> Self {
        self.leading_width = leading_width;
        self
    }

    pub const fn trailing_width(mut self, trailing_width: f32) -> Self {
        self.trailing_width = trailing_width;
        self
    }
}

impl Default for ActionTilePaint {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalloutPaint {
    pub tone: SemanticTone,
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub rail_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub title_color: Option<Color>,
    pub body_color: Option<Color>,
    pub radius: Option<f32>,
    pub padding: Insets,
    pub icon_size: f32,
    pub icon_gap: f32,
    pub rail_width: f32,
    pub reserved_bottom: f32,
}

impl CalloutPaint {
    pub const fn new(tone: SemanticTone) -> Self {
        Self {
            tone,
            fill: None,
            border: None,
            rail_color: None,
            icon_color: None,
            title_color: None,
            body_color: None,
            radius: None,
            padding: Insets {
                left: 12.0,
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
            },
            icon_size: 15.0,
            icon_gap: 9.0,
            rail_width: 2.0,
            reserved_bottom: 0.0,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn rail_color(mut self, rail_color: Color) -> Self {
        self.rail_color = Some(rail_color);
        self
    }

    pub const fn icon_color(mut self, icon_color: Color) -> Self {
        self.icon_color = Some(icon_color);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn body_color(mut self, body_color: Color) -> Self {
        self.body_color = Some(body_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub const fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }

    pub const fn icon_gap(mut self, icon_gap: f32) -> Self {
        self.icon_gap = icon_gap;
        self
    }

    pub const fn rail_width(mut self, rail_width: f32) -> Self {
        self.rail_width = rail_width;
        self
    }

    pub const fn reserved_bottom(mut self, reserved_bottom: f32) -> Self {
        self.reserved_bottom = reserved_bottom;
        self
    }
}

impl Default for CalloutPaint {
    fn default() -> Self {
        Self::new(SemanticTone::Info)
    }
}

pub fn paint_callout(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    icon: Option<IconGlyph>,
    title: Option<&str>,
    body: &str,
    style: CalloutPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let (tone_color, _) = theme.semantic_tone_colors(style.tone);
    // Mesh callout: quiet soft wash, hairline border, a 2px status rail, and
    // the status-hued ink (not the on-solid content color) for the icon.
    let (tone_soft, tone_ink) = theme.semantic_tone_soft_colors(style.tone);
    let fill = style.fill.unwrap_or(tone_soft);
    let border = style.border.unwrap_or_else(|| palette.border);
    let rail = style.rail_color.unwrap_or(tone_color);
    let radius = style.radius.unwrap_or(theme.radius.md).max(0.0);

    ctx.fill(rounded_rect_path(rect, radius), fill);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let rail_width = style.rail_width.max(0.0).min(rect.width());
    if rail_width > 0.0 {
        let rail_rect = Rect::new(rect.x(), rect.y(), rail_width, rect.height());
        ctx.fill(rounded_rect_path(rail_rect, rail_width * 0.5), rail);
    }

    let padding = style.padding;
    let content_bottom = (rect.max_y() - padding.bottom.max(0.0) - style.reserved_bottom.max(0.0))
        .max(rect.y() + padding.top.max(0.0));
    let content = Rect::new(
        rect.x() + padding.left.max(0.0),
        rect.y() + padding.top.max(0.0),
        (rect.width() - padding.left.max(0.0) - padding.right.max(0.0)).max(0.0),
        (content_bottom - rect.y() - padding.top.max(0.0)).max(0.0),
    );
    if content.width() <= 0.0 || content.height() <= 0.0 {
        return;
    }

    let icon_size = style
        .icon_size
        .max(0.0)
        .min(content.height())
        .min(content.width());
    let mut text_x = content.x();
    if let Some(icon) = icon.filter(|_| icon_size > 0.0) {
        let icon_rect = Rect::new(
            content.x(),
            content.y() + 2.0_f32.min((content.height() - icon_size).max(0.0)),
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, style.icon_color.unwrap_or(tone_ink));
        text_x = icon_rect.max_x() + style.icon_gap.max(0.0);
    }

    let text_rect = Rect::new(
        text_x,
        content.y(),
        (content.max_x() - text_x).max(0.0),
        content.height(),
    );
    if text_rect.width() <= 0.0 || text_rect.height() <= 0.0 {
        return;
    }

    let title_line = if title.is_some() {
        theme.text.sm.line_height
    } else {
        0.0
    };
    if let Some(title) = title {
        let mut title_style = text_token_style(
            theme,
            theme.text.sm,
            style.title_color.unwrap_or(palette.text),
        );
        title_style.weight = FontWeight::SEMIBOLD;
        let title_rect = Rect::new(text_rect.x(), text_rect.y(), text_rect.width(), title_line);
        ctx.push_clip_rect(title_rect);
        paint_single_line_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    if body.trim().is_empty() {
        return;
    }
    let body_y = text_rect.y() + title_line + if title.is_some() { 4.0 } else { 0.0 };
    let body_rect = Rect::new(
        text_rect.x(),
        body_y,
        text_rect.width(),
        (text_rect.max_y() - body_y).max(0.0),
    );
    if body_rect.width() <= 0.0 || body_rect.height() <= 0.0 {
        return;
    }

    let color = style.body_color.unwrap_or(palette.text_muted);
    let mut layout_style = text_token_style(theme, theme.text.sm, color);
    layout_style.color = Color::WHITE;
    let mut document = TextDocument::from_plain_text(body.to_string(), layout_style);
    for paragraph in &mut document.paragraphs {
        paragraph.style.align = TextAlign::Start;
        paragraph.style.wrap = TextWrap::Word;
    }

    ctx.push_clip_rect(body_rect);
    if let Ok(layout) = ctx.layout_text_document(TextLayoutRequest::new(document).with_box_size(
        Size::new(body_rect.width().max(1.0), body_rect.height().max(1.0)),
    )) {
        ctx.draw_text_layout_with_color(body_rect.origin, &layout, color);
    } else {
        let fallback_style = text_token_style(theme, theme.text.sm, color);
        ctx.draw_text(body_rect, body.to_string(), fallback_style);
    }
    ctx.pop_clip();
}

pub fn paint_action_tile(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    title: &str,
    subtitle: Option<&str>,
    icon: Option<IconGlyph>,
    style: ActionTilePaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let tone_color = theme.semantic_tone_color(style.tone);
    let effective_tone = if style.tone == SemanticTone::Neutral {
        palette.text_muted
    } else {
        tone_color
    };
    let base_background = if !style.enabled {
        mix_color(palette.control, palette.surface, 0.68).with_alpha(0.82)
    } else if style.pressed {
        palette.control_active
    } else if style.hovered {
        palette.control_hover
    } else {
        palette.control
    };
    let background = style.background.unwrap_or(base_background);
    let base_border = if !style.enabled {
        palette.border.with_alpha(0.55)
    } else if style.highlighted {
        effective_tone.with_alpha(0.84)
    } else if style.hovered {
        palette.border_hover
    } else {
        palette.border
    };
    let border = style.border.unwrap_or(base_border);
    let radius = theme
        .metrics
        .corner_radius
        .min(rect.height() * 0.28)
        .max(0.0);
    let radius = style.radius.unwrap_or(radius).max(0.0);
    ctx.fill(rounded_rect_path(rect, radius), background);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let padding_x = style
        .padding_x
        .unwrap_or_else(|| theme.metrics.button_padding.left.max(10.0))
        .max(0.0)
        .min(rect.width() * 0.45);
    let compact = rect.height() <= 46.0 || subtitle.is_none();
    let base_icon_side: f32 = if compact { 14.0 } else { 17.0 };
    let icon_side = base_icon_side
        .min((rect.height() - 14.0).max(10.0))
        .max(0.0);
    let icon_y = if compact {
        rect.y() + (rect.height() - icon_side) * 0.5
    } else {
        rect.y() + 12.0
    };
    let mut text_x = rect.x() + padding_x;
    if let Some(icon) = icon {
        let icon_rect = Rect::new(rect.x() + padding_x, icon_y, icon_side, icon_side);
        let icon_color = style.icon_color.unwrap_or_else(|| {
            if style.enabled {
                if style.highlighted || style.hovered {
                    effective_tone
                } else {
                    palette.text_muted
                }
            } else {
                palette.text.with_alpha(0.34)
            }
        });
        draw_icon_glyph(ctx, icon, icon_rect, icon_color);
        text_x = icon_rect.max_x() + theme.metrics.icon_label_gap.max(7.0);
    } else if let Some(dot_tone) = style.leading_tone_dot {
        let dot_side = 8.0_f32.min((rect.height() - 12.0).max(4.0)).max(4.0);
        let leading_width = style
            .leading_width
            .max(dot_side + theme.metrics.icon_label_gap.max(7.0));
        let dot_rect = Rect::new(
            rect.x() + padding_x,
            if compact {
                rect.y() + (rect.height() - dot_side) * 0.5
            } else {
                rect.y() + 15.0_f32.min((rect.height() - dot_side).max(0.0))
            },
            dot_side,
            dot_side,
        );
        ctx.fill(
            rounded_rect_path(dot_rect, dot_side * 0.5),
            theme.semantic_tone_color(dot_tone),
        );
        text_x += leading_width;
    } else if style.leading_width > 0.0 {
        text_x += style.leading_width;
    }

    let text_width = (rect.max_x() - text_x - padding_x - style.trailing_width.max(0.0)).max(0.0);
    if text_width <= 0.0 {
        return;
    }

    let title_color = style.title_color.unwrap_or_else(|| {
        if !style.enabled {
            palette.text.with_alpha(0.42)
        } else if style.highlighted {
            palette.text
        } else {
            palette.text_muted
        }
    });
    let subtitle_color = style.subtitle_color.unwrap_or_else(|| {
        if !style.enabled {
            palette.text.with_alpha(0.32)
        } else {
            palette.placeholder
        }
    });
    let mut title_style = text_token_style(theme, theme.text.sm, title_color);
    title_style.weight = FontWeight::SEMIBOLD;
    let title_style = numeric_text_style_if_numeric(title, title_style);
    let subtitle_style = text_token_style(theme, theme.text.xs, subtitle_color);

    if compact {
        let title_rect = Rect::new(text_x, rect.y(), text_width, rect.height());
        ctx.push_clip_rect(title_rect);
        paint_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        return;
    }

    let title_rect = Rect::new(text_x, rect.y() + 8.0, text_width, title_style.line_height);
    ctx.push_clip_rect(title_rect);
    paint_single_line_aligned_text(
        ctx,
        title_rect,
        title,
        &title_style,
        title_style.line_height,
        0.0,
    );
    ctx.pop_clip();

    if let Some(subtitle) = subtitle {
        let subtitle_rect = Rect::new(
            text_x,
            title_rect.max_y() + theme.metrics.action_card_text_gap.min(3.0),
            text_width,
            subtitle_style.line_height,
        );
        ctx.push_clip_rect(subtitle_rect);
        paint_single_line_aligned_text(
            ctx,
            subtitle_rect,
            subtitle,
            &subtitle_style,
            subtitle_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodePanelPaint {
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub header_fill: Option<Color>,
    pub label_color: Option<Color>,
    pub radius: Option<f32>,
    pub header_height: f32,
    pub content_padding: Insets,
    pub label_inset_x: f32,
}

impl CodePanelPaint {
    pub const fn new() -> Self {
        Self {
            fill: None,
            border: None,
            header_fill: None,
            label_color: None,
            radius: None,
            header_height: 24.0,
            content_padding: Insets {
                left: 8.0,
                top: 6.0,
                right: 8.0,
                bottom: 4.0,
            },
            label_inset_x: 10.0,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn header_fill(mut self, header_fill: Color) -> Self {
        self.header_fill = Some(header_fill);
        self
    }

    pub const fn label_color(mut self, label_color: Color) -> Self {
        self.label_color = Some(label_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn header_height(mut self, header_height: f32) -> Self {
        self.header_height = header_height;
        self
    }

    pub const fn content_padding(mut self, content_padding: Insets) -> Self {
        self.content_padding = content_padding;
        self
    }

    pub const fn label_inset_x(mut self, label_inset_x: f32) -> Self {
        self.label_inset_x = label_inset_x;
        self
    }
}

impl Default for CodePanelPaint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextSpan<'a> {
    pub text: &'a str,
    pub color: Option<Color>,
}

impl<'a> CodeTextSpan<'a> {
    pub const fn new(text: &'a str) -> Self {
        Self { text, color: None }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextLine<'a> {
    pub spans: &'a [CodeTextSpan<'a>],
    pub background: Option<Color>,
    pub fallback_color: Option<Color>,
}

impl<'a> CodeTextLine<'a> {
    pub const fn new(spans: &'a [CodeTextSpan<'a>]) -> Self {
        Self {
            spans,
            background: None,
            fallback_color: None,
        }
    }

    pub const fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    pub const fn fallback_color(mut self, color: Color) -> Self {
        self.fallback_color = Some(color);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextPaint {
    pub color: Option<Color>,
    pub font_size: f32,
    pub line_height: f32,
    pub x_padding: f32,
    pub weight: FontWeight,
}

impl CodeTextPaint {
    pub const fn new() -> Self {
        Self {
            color: None,
            font_size: 12.0,
            line_height: 17.0,
            x_padding: 2.0,
            weight: FontWeight::NORMAL,
        }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub const fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub const fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    pub const fn x_padding(mut self, x_padding: f32) -> Self {
        self.x_padding = x_padding;
        self
    }

    pub const fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }
}

impl Default for CodeTextPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_code_lines(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    lines: &[CodeTextLine<'_>],
    style: CodeTextPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || lines.is_empty() {
        return;
    }

    let mut base_style = TextStyle {
        font_size: style.font_size.max(1.0),
        line_height: style.line_height.max(1.0),
        color: style.color.unwrap_or(theme.palette.text),
        ..theme.body_text_style()
    };
    base_style.weight = style.weight;

    let line_height = base_style.line_height;
    let mut y = rect.y();
    ctx.push_clip_rect(rect);
    for line in lines {
        if y + line_height > rect.max_y() {
            break;
        }
        if let Some(background) = line.background {
            ctx.fill_rect(
                Rect::new(rect.x(), y, rect.width(), line_height),
                background,
            );
        }

        let mut x = rect.x() + style.x_padding.max(0.0);
        for span in line.spans {
            if span.text.is_empty() || x > rect.max_x() {
                continue;
            }
            let mut span_style = base_style.clone();
            span_style.color = span
                .color
                .or(line.fallback_color)
                .unwrap_or(base_style.color);
            let width = ctx
                .measure_text(span.text.to_string(), span_style.clone())
                .ok()
                .map(|measurement| measurement.width)
                .unwrap_or(0.0);
            ctx.draw_text(
                Rect::new(x, y, (rect.max_x() - x).max(0.0), line_height),
                span.text,
                span_style,
            );
            x += width;
        }
        y += line_height;
    }
    ctx.pop_clip();
}

pub fn paint_code_panel(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    style: CodePanelPaint,
) -> Rect {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Rect::ZERO;
    }

    let fill = style.fill.unwrap_or(theme.surfaces.field);
    let border = style.border.unwrap_or(theme.surfaces.border);
    let header_fill = style.header_fill.unwrap_or(theme.surfaces.titlebar);
    let label_color = style.label_color.unwrap_or(theme.surfaces.text_faint);
    let radius = style
        .radius
        .unwrap_or(theme.radius.xl)
        .min(rect.width().min(rect.height()) * 0.5)
        .max(0.0);
    let border_width = physical_pixels(ctx, theme.metrics.border_width.max(1.0));

    let panel_shape = rounded_rect_path(rect, radius);
    ctx.fill(panel_shape.clone(), fill);

    let header_height = style.header_height.clamp(0.0, rect.height());
    if header_height > 0.0 {
        let header_rect = Rect::new(rect.x(), rect.y(), rect.width(), header_height);
        let header_radius = radius.min(header_height * 0.5);
        ctx.fill(rounded_rect_path(header_rect, header_radius), header_fill);
        if header_height > header_radius {
            ctx.fill_rect(
                Rect::new(
                    header_rect.x(),
                    (header_rect.max_y() - header_radius).max(header_rect.y()),
                    header_rect.width(),
                    header_radius,
                ),
                header_fill,
            );
        }

        let mut label_style = text_token_style(theme, theme.text.xs, label_color);
        label_style.weight = FontWeight::SEMIBOLD;
        let label_x = rect.x() + style.label_inset_x.max(0.0);
        let label_rect = Rect::new(
            label_x,
            rect.y() + ((header_height - label_style.line_height) * 0.5).max(0.0),
            (rect.max_x() - label_x - style.label_inset_x.max(0.0)).max(0.0),
            label_style.line_height,
        );
        if label_rect.width() > 0.0 {
            ctx.push_clip_rect(label_rect);
            paint_single_line_aligned_text(
                ctx,
                label_rect,
                label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
    }

    ctx.stroke(panel_shape, border, StrokeStyle::new(border_width));

    Rect::new(
        rect.x() + style.content_padding.left,
        rect.y() + header_height + style.content_padding.top,
        (rect.width() - style.content_padding.left - style.content_padding.right).max(0.0),
        (rect.height() - header_height - style.content_padding.top - style.content_padding.bottom)
            .max(0.0),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPanelPaint {
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub title_color: Option<Color>,
    pub radius: Option<f32>,
    pub header_height: f32,
    pub content_padding: Insets,
    pub title_inset_x: f32,
    pub trailing_width: f32,
    pub title_token: Option<ThemeTextToken>,
    pub title_weight: FontWeight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPanelGeometry {
    pub panel_rect: Rect,
    pub header_rect: Rect,
    pub title_rect: Rect,
    pub content_rect: Rect,
}

impl SectionPanelPaint {
    pub const fn new() -> Self {
        Self {
            fill: None,
            border: None,
            title_color: None,
            radius: None,
            header_height: 34.0,
            content_padding: Insets {
                left: 12.0,
                top: 0.0,
                right: 12.0,
                bottom: 8.0,
            },
            title_inset_x: 12.0,
            trailing_width: 0.0,
            title_token: None,
            title_weight: FontWeight::SEMIBOLD,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn header_height(mut self, header_height: f32) -> Self {
        self.header_height = header_height;
        self
    }

    pub const fn content_padding(mut self, content_padding: Insets) -> Self {
        self.content_padding = content_padding;
        self
    }

    pub const fn title_inset_x(mut self, title_inset_x: f32) -> Self {
        self.title_inset_x = title_inset_x;
        self
    }

    pub const fn trailing_width(mut self, trailing_width: f32) -> Self {
        self.trailing_width = trailing_width;
        self
    }

    pub const fn title_token(mut self, title_token: ThemeTextToken) -> Self {
        self.title_token = Some(title_token);
        self
    }

    pub const fn title_weight(mut self, title_weight: FontWeight) -> Self {
        self.title_weight = title_weight;
        self
    }
}

impl Default for SectionPanelPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_section_panel(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    title: &str,
    style: SectionPanelPaint,
) -> SectionPanelGeometry {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return SectionPanelGeometry {
            panel_rect: Rect::ZERO,
            header_rect: Rect::ZERO,
            title_rect: Rect::ZERO,
            content_rect: Rect::ZERO,
        };
    }

    let fill = style.fill.unwrap_or(theme.surfaces.panel);
    let border = style.border.unwrap_or(theme.surfaces.border);
    let title_color = style.title_color.unwrap_or(theme.surfaces.text);
    let radius = style
        .radius
        .unwrap_or(theme.radius.lg)
        .min(rect.width().min(rect.height()) * 0.5)
        .max(0.0);
    let header_height = style.header_height.clamp(0.0, rect.height());
    let shape = rounded_rect_path(rect, radius);
    ctx.fill(shape.clone(), fill);
    ctx.stroke(
        shape,
        border,
        StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width.max(1.0))),
    );

    let header_rect = Rect::new(rect.x(), rect.y(), rect.width(), header_height);
    let mut title_style = text_token_style(
        theme,
        style.title_token.unwrap_or(theme.text.sm),
        title_color,
    );
    title_style.weight = style.title_weight;
    let title_x = rect.x() + style.title_inset_x.max(0.0);
    let title_rect = Rect::new(
        title_x,
        rect.y() + ((header_height - title_style.line_height) * 0.5).max(0.0),
        (rect.max_x() - title_x - style.title_inset_x.max(0.0) - style.trailing_width.max(0.0))
            .max(0.0),
        title_style.line_height,
    );
    if title_rect.width() > 0.0 && !title.is_empty() {
        ctx.push_clip_rect(title_rect);
        paint_single_line_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    let content_rect = Rect::new(
        rect.x() + style.content_padding.left,
        rect.y() + header_height + style.content_padding.top,
        (rect.width() - style.content_padding.left - style.content_padding.right).max(0.0),
        (rect.height() - header_height - style.content_padding.top - style.content_padding.bottom)
            .max(0.0),
    );

    SectionPanelGeometry {
        panel_rect: rect,
        header_rect,
        title_rect,
        content_rect,
    }
}

impl Widget for StatusBadge {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let (height, icon_size, gap, padding) = self.metrics(&theme);
        let label = self.label();
        let tone = self.resolved_tone();
        let text = measure_text(ctx, &label, &self.text_style(&theme, &label, tone));
        let icon_w = self.icon.map(|_| icon_size + gap).unwrap_or(0.0);
        let natural_w = text.width.ceil() + icon_w + padding * 2.0;
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(0.0).max(natural_w),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let label = self.label();
        let tone = self.resolved_tone();
        paint_status_badge(ctx, ctx.bounds(), &theme, &label, self.icon, tone);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.label();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(label.clone());
        node.value = Some(SemanticsValue::Text(label));
        ctx.push(node);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageDotsConfig {
    pub current: usize,
    pub target: usize,
    pub tone: SemanticTone,
    pub max_dots: usize,
    pub show_label: bool,
}

impl CoverageDotsConfig {
    pub fn new(current: usize, target: usize) -> Self {
        Self {
            current,
            target,
            tone: SemanticTone::Neutral,
            max_dots: 4,
            show_label: true,
        }
    }

    fn normalized_target(self) -> usize {
        self.target.max(self.current)
    }

    fn normalized_max_dots(self) -> usize {
        self.max_dots.max(1)
    }

    fn label(self) -> String {
        format!("{}/{}", self.current, self.normalized_target())
    }
}

pub fn paint_coverage_dots(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    current: usize,
    target: usize,
    tone: SemanticTone,
) {
    let mut config = CoverageDotsConfig::new(current, target);
    config.tone = tone;
    paint_coverage_dots_with_config(ctx, theme, rect, config);
}

pub fn paint_coverage_dots_with_config(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    config: CoverageDotsConfig,
) {
    let target = config.normalized_target();
    if target == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (dot, gap, label_gap) = coverage_dot_metrics(theme);
    let shown = target.min(config.normalized_max_dots());
    let dots_width = shown as f32 * dot + shown.saturating_sub(1) as f32 * gap;
    let label = config.label();
    let label_width = if config.show_label {
        (label.len() as f32 * theme.text.xs.size * 0.56).min(34.0)
    } else {
        0.0
    };
    let total_width = dots_width
        + if config.show_label {
            label_gap + label_width
        } else {
            0.0
        };
    let mut x = rect.x() + ((rect.width() - total_width) * 0.5).max(0.0);
    let y = rect.y() + (rect.height() - dot) * 0.5;
    let (tone_color, _) = theme.semantic_tone_colors(config.tone);
    for index in 0..shown {
        let dot_rect = Rect::new(x, y, dot, dot);
        if index < config.current.min(shown) {
            ctx.fill(rounded_rect_path(dot_rect, dot * 0.5), tone_color);
        } else {
            ctx.stroke(
                rounded_rect_path(dot_rect, dot * 0.5),
                theme.palette.border,
                StrokeStyle::new(theme.metrics.border_width.max(1.0)),
            );
        }
        x += dot + gap;
    }

    if config.show_label {
        let label_rect = Rect::new(
            x + (label_gap - gap).max(0.0),
            rect.y(),
            (rect.max_x() - x).max(0.0),
            rect.height(),
        );
        let text_style = numeric_text_style(text_token_style(
            theme,
            theme.text.xs,
            theme.palette.text_muted,
        ));
        ctx.push_clip_rect(label_rect);
        paint_aligned_text(
            ctx,
            label_rect,
            &label,
            &text_style,
            text_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }
}

fn coverage_dot_metrics(theme: &DefaultTheme) -> (f32, f32, f32) {
    let dot = (theme.text.xs.size * 0.42).clamp(4.0, 6.0);
    let gap = (dot * 0.65).clamp(2.0, 4.0);
    let label_gap = (theme.metrics.icon_label_gap * 0.7).max(4.0);
    (dot, gap, label_gap)
}

pub struct CoverageDots {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    config: CoverageDotsConfig,
    min_width: Option<f32>,
}

impl CoverageDots {
    pub fn new(name: impl Into<String>, current: usize, target: usize) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            config: CoverageDotsConfig::new(current, target),
            min_width: None,
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

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.config.tone = tone;
        self
    }

    pub fn max_dots(mut self, max_dots: usize) -> Self {
        self.config.max_dots = max_dots;
        self
    }

    pub fn show_label(mut self, show_label: bool) -> Self {
        self.config.show_label = show_label;
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for CoverageDots {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let target = self.config.normalized_target();
        if target == 0 {
            return constraints.clamp(Size::ZERO);
        }
        let (dot, gap, label_gap) = coverage_dot_metrics(&theme);
        let shown = target.min(self.config.normalized_max_dots());
        let dots_width = shown as f32 * dot + shown.saturating_sub(1) as f32 * gap;
        let label_width = if self.config.show_label {
            measure_text(
                ctx,
                &self.config.label(),
                &numeric_text_style(text_token_style(
                    &theme,
                    theme.text.xs,
                    theme.palette.text_muted,
                )),
            )
            .width
                + label_gap
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(0.0).max(dots_width + label_width),
            theme
                .text
                .xs
                .line_height
                .max(dot + 2.0)
                .max(theme.metrics.min_height * 0.55),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_coverage_dots_with_config(ctx, &self.resolved_theme(), ctx.bounds(), self.config);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let target = self.config.normalized_target();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.config.label()));
        node.description = Some(format!(
            "{} of {} covered",
            self.config.current.min(target),
            target
        ));
        ctx.push(node);
    }
}

pub struct PlacementBadge {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    label_reader: Option<Box<dyn Fn() -> String>>,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    tone_reader: Option<Box<dyn Fn() -> SemanticTone>>,
    coverage: Option<(usize, usize)>,
    coverage_reader: Option<Box<dyn Fn() -> Option<(usize, usize)>>>,
    min_width: Option<f32>,
}

impl PlacementBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            icon: None,
            tone: SemanticTone::Neutral,
            tone_reader: None,
            coverage: None,
            coverage_reader: None,
            min_width: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        let mut badge = Self::new(fallback);
        badge.label_reader = Some(Box::new(reader));
        badge
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

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self.tone_reader = None;
        self
    }

    pub fn tone_when<F>(mut self, tone: F) -> Self
    where
        F: Fn() -> SemanticTone + 'static,
    {
        self.tone_reader = Some(Box::new(tone));
        self
    }

    pub fn coverage(mut self, current: usize, target: usize) -> Self {
        self.coverage = Some((current, target));
        self.coverage_reader = None;
        self
    }

    pub fn coverage_when<F>(mut self, coverage: F) -> Self
    where
        F: Fn() -> Option<(usize, usize)> + 'static,
    {
        self.coverage_reader = Some(Box::new(coverage));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    fn resolved_tone(&self) -> SemanticTone {
        self.tone_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.tone)
    }

    fn resolved_coverage(&self) -> Option<(usize, usize)> {
        self.coverage_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.coverage)
            .filter(|(_, target)| *target > 0)
    }

    fn metrics(theme: &DefaultTheme) -> (f32, f32, f32) {
        let height = (theme.metrics.min_height - 2.0).max(22.0);
        let coverage_width = 50.0;
        let gap = theme.metrics.icon_label_gap.max(6.0);
        (height, coverage_width, gap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementBadgePaint {
    pub padding: Insets,
}

impl PlacementBadgePaint {
    pub const fn new() -> Self {
        Self {
            padding: Insets::ZERO,
        }
    }

    pub const fn padding(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.padding = Insets {
            left,
            top,
            right,
            bottom,
        };
        self
    }

    fn content_rect(self, rect: Rect) -> Rect {
        Rect::new(
            rect.x() + self.padding.left,
            rect.y() + self.padding.top,
            (rect.width() - self.padding.left - self.padding.right).max(0.0),
            (rect.height() - self.padding.top - self.padding.bottom).max(0.0),
        )
    }
}

impl Default for PlacementBadgePaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_placement_badge(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    coverage: Option<(usize, usize)>,
) {
    paint_placement_badge_with(
        ctx,
        theme,
        rect,
        label,
        icon,
        tone,
        coverage,
        PlacementBadgePaint::new(),
    );
}

pub fn paint_placement_badge_with(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    coverage: Option<(usize, usize)>,
    paint: PlacementBadgePaint,
) {
    let rect = paint.content_rect(rect);
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (_, coverage_width, gap) = PlacementBadge::metrics(theme);
    let show_coverage = coverage.is_some() && rect.width() >= 118.0;
    let coverage_slot = if show_coverage { coverage_width } else { 0.0 };
    let slot_gap = if show_coverage { gap } else { 0.0 };
    let badge_rect = Rect::new(
        rect.x(),
        rect.y(),
        (rect.width() - coverage_slot - slot_gap).clamp(48.0, 86.0),
        rect.height(),
    );
    paint_status_badge(ctx, badge_rect, theme, label, icon, tone);

    if show_coverage && let Some((current, target)) = coverage {
        let dots_rect = Rect::new(
            badge_rect.max_x() + slot_gap,
            rect.y(),
            (rect.max_x() - badge_rect.max_x() - slot_gap).max(0.0),
            rect.height(),
        );
        paint_coverage_dots(ctx, theme, dots_rect, current, target, tone);
    }
}

impl Widget for PlacementBadge {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let (height, coverage_width, gap) = Self::metrics(&theme);
        let label = self.label();
        let tone = self.resolved_tone();
        let icon_w = self
            .icon
            .map(|_| (height - 13.0).clamp(11.0, 15.0) + theme.metrics.icon_label_gap.max(4.0))
            .unwrap_or(0.0);
        let text = measure_text(
            ctx,
            &label,
            &StatusBadge::new(&label).text_style(&theme, &label, tone),
        );
        let badge_width =
            (text.width.ceil() + icon_w + theme.metrics.button_padding.left.max(8.0) * 2.0)
                .clamp(48.0, 86.0);
        let coverage_width = if self.resolved_coverage().is_some() {
            coverage_width + gap
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            self.min_width
                .unwrap_or(0.0)
                .max(badge_width + coverage_width),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        paint_placement_badge(
            ctx,
            &theme,
            ctx.bounds(),
            &self.label(),
            self.icon,
            self.resolved_tone(),
            self.resolved_coverage(),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.label();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(label.clone());
        node.value = Some(SemanticsValue::Text(label.clone()));
        if let Some((current, target)) = self.resolved_coverage() {
            let target = target.max(current);
            node.description = Some(format!("{current} of {target} replicas available"));
        }
        ctx.push(node);
    }
}

pub struct TabBar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    tabs: Vec<String>,
    selected: usize,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    selection_from: usize,
    selection_animation: AnimatedScalar,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    gap: Option<f32>,
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
            selected_reader: None,
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            gap: None,
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
        self.selected_reader = None;
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        if let Some(index) = selected() {
            self.selected = index;
            self.selection_from = index;
        }
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
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
        let selected = self
            .selected_reader
            .as_ref()
            .and_then(|reader| reader())
            .unwrap_or(self.selected);
        if self.tabs.is_empty() {
            0
        } else {
            selected.min(self.tabs.len() - 1)
        }
    }

    fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() {
            return;
        }

        let index = index.min(self.tabs.len() - 1);
        let selected = self.normalized_selected();
        if selected != index {
            self.selected = index;
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.tabs[index].clone());
            }
            let target = self.normalized_selected();
            self.selected = target;
            self.start_selection_animation(selected, target, ctx);
        }
    }

    fn start_selection_animation(&mut self, from: usize, to: usize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() || from == to {
            self.selection_from = to;
            self.selection_animation = AnimatedScalar::new(1.0);
            return;
        }

        let theme = self.resolved_theme();
        self.selection_from = from.min(self.tabs.len() - 1);
        self.selection_animation = AnimatedScalar::new(0.0);
        self.selection_animation.set_target_event(
            1.0,
            theme.motion.tab_switch_duration(),
            theme.motion.tab_switch_easing(),
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn sync_external_selected(&mut self, ctx: &mut EventCtx) {
        if self.selected_reader.is_none() || self.tabs.is_empty() {
            return;
        }

        let previous = self.selected.min(self.tabs.len() - 1);
        let selected = self.normalized_selected();
        if previous != selected {
            self.selected = selected;
            self.start_selection_animation(previous, selected, ctx);
        }
    }

    fn tab_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.tab_gap)
            .max(0.0)
    }

    fn measured_widths(&self) -> &[f32] {
        &self.widths
    }

    fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() || self.measured_widths().len() != self.tabs.len() {
            return None;
        }

        let gap = self.resolved_gap();
        let base_total =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        let extra_per_tab = if bounds.width() > base_total && !self.tabs.is_empty() {
            (bounds.width() - base_total) / self.tabs.len() as f32
        } else {
            0.0
        };

        let tab_height = self.tab_height().min(bounds.height()).max(0.0);
        let tab_y = bounds.y() + ((bounds.height() - tab_height) * 0.5).max(0.0);
        let mut x = bounds.x();
        for (current, width) in self.widths.iter().enumerate() {
            let width = *width + extra_per_tab;
            let rect = Rect::new(x, tab_y, width, tab_height);
            if current == index {
                return Some(rect);
            }
            x += width + gap;
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

    fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() {
            return;
        }

        let selected = self.normalized_selected() as isize;
        let last = self.tabs.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(next), ctx);
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }
        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

impl Widget for TabBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_external_selected(ctx);
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.tab_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
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
                    self.activate(index, ctx);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.tabs.is_empty() => self.activate(self.tabs.len() - 1, ctx),
                    _ => return,
                }
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
        let theme = self.resolved_theme();
        let style = text_token_style(&theme, theme.text.sm, theme.palette.text);
        let padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .tabs
            .iter()
            .map(|tab| measure_text(ctx, tab, &style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                (measurement.width + padding.left + padding.right).max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = self.resolved_gap();
        let width =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width.max(160.0), self.tab_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let tab_padding = metrics.tab_padding;
        let label_style = text_token_style(&theme, theme.text.sm, palette.text_muted);
        let selected_label_style = text_token_style(&theme, theme.text.sm, palette.text);

        ctx.fill(
            rounded_rect_path(ctx.bounds(), metrics.corner_radius),
            palette.control,
        );

        let focus_progress = self.focus_animation.value;
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if let Some((background, border)) = tab_state_visuals(
                &theme,
                selected,
                hovered,
                pressed,
                hover_amount,
                press_amount,
            ) {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, tab_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                tab,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }

        if let Some(accent) = tab_indicator_rect(
            |index| self.tab_rect(ctx.bounds(), index),
            self.selection_from,
            self.normalized_selected(),
            self.selection_animation.value,
            tab_padding,
            interaction.active_indicator_thickness,
        ) {
            ctx.fill(
                rounded_rect_path(accent, accent.height() * 0.5),
                palette.accent,
            );
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrowserTabHit {
    Tab(usize),
    Close(usize),
}

impl BrowserTabHit {
    fn index(self) -> usize {
        match self {
            Self::Tab(index) | Self::Close(index) => index,
        }
    }
}

type BrowserTabBarChange = Box<dyn FnMut(usize, String)>;
type BrowserTabBarContextChange = Box<dyn FnMut(usize, String, &mut EventCtx)>;

pub struct BrowserTabBar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    tabs: Vec<String>,
    tabs_reader: Option<Box<dyn Fn() -> Vec<String>>>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    selection_from: Option<usize>,
    selection_to: Option<usize>,
    selection_animation: AnimatedScalar,
    hovered: Option<BrowserTabHit>,
    hover_visual: Option<BrowserTabHit>,
    pressed: Option<BrowserTabHit>,
    press_visual: Option<BrowserTabHit>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurements: Vec<TextMeasurement>,
    widths: Vec<f32>,
    on_change: Option<BrowserTabBarChange>,
    on_change_with_ctx: Option<BrowserTabBarContextChange>,
    on_close: Option<BrowserTabBarChange>,
    on_close_with_ctx: Option<BrowserTabBarContextChange>,
}

impl BrowserTabBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            tabs: Vec::new(),
            tabs_reader: None,
            selected: None,
            selected_reader: None,
            selection_from: None,
            selection_to: None,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            widths: Vec::new(),
            on_change: None,
            on_change_with_ctx: None,
            on_close: None,
            on_close_with_ctx: None,
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

    pub fn tabs<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tabs.extend(labels.into_iter().map(Into::into));
        self
    }

    pub fn tabs_when<F>(mut self, tabs: F) -> Self
    where
        F: Fn() -> Vec<String> + 'static,
    {
        self.tabs_reader = Some(Box::new(tabs));
        self
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self.selection_from = index;
        self.selection_to = index;
        self.selection_animation = AnimatedScalar::new(1.0);
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

    pub fn on_close<F>(mut self, on_close: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_close = Some(Box::new(on_close));
        self
    }

    pub fn on_close_with_ctx<F>(mut self, on_close: F) -> Self
    where
        F: FnMut(usize, String, &mut EventCtx) + 'static,
    {
        self.on_close_with_ctx = Some(Box::new(on_close));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.normalized_selected()
    }

    fn refresh_tabs(&mut self) {
        if let Some(reader) = &self.tabs_reader {
            self.tabs = reader();
        }
        self.selected = self.resolved_selected_raw();
        let selected = self.normalized_selected();
        if self.selection_animation.value >= 1.0 - AnimatedScalar::EPSILON
            && self.selection_to != selected
        {
            self.selection_from = selected;
            self.selection_to = selected;
            self.selection_animation = AnimatedScalar::new(1.0);
        }
    }

    fn resolved_selected_raw(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.selected)
    }

    fn normalized_selected(&self) -> Option<usize> {
        let selected = self.resolved_selected_raw()?;
        (selected < self.tabs.len()).then_some(selected)
    }

    fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if index >= self.tabs.len() {
            return;
        }
        let from = self.normalized_selected();
        if from == Some(index) {
            return;
        }
        let label = self.tabs[index].clone();
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, label.clone());
        }
        if let Some(on_change) = &mut self.on_change_with_ctx {
            on_change(index, label, ctx);
        }
        self.refresh_tabs();
        self.start_selection_animation(from, self.normalized_selected(), ctx);
    }

    fn close(&mut self, index: usize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if index >= self.tabs.len() {
            return;
        }
        let from = self.normalized_selected();
        let label = self.tabs[index].clone();
        if let Some(on_close) = &mut self.on_close {
            on_close(index, label.clone());
        }
        if let Some(on_close) = &mut self.on_close_with_ctx {
            on_close(index, label, ctx);
        }
        self.refresh_tabs();
        self.start_selection_animation(from, self.normalized_selected(), ctx);
    }

    fn start_selection_animation(
        &mut self,
        from: Option<usize>,
        to: Option<usize>,
        ctx: &mut EventCtx,
    ) {
        self.selection_from = from.or(to);
        self.selection_to = to;
        if from.zip(to).is_some_and(|(from, to)| from != to) {
            let theme = self.resolved_theme();
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
        } else {
            self.selection_animation = AnimatedScalar::new(1.0);
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn close_size(theme: &DefaultTheme) -> f32 {
        (theme.metrics.tab_height * 0.56).clamp(16.0, 20.0)
    }

    fn close_gap(theme: &DefaultTheme) -> f32 {
        theme.metrics.icon_label_gap.max(7.0)
    }

    fn tab_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    fn measured_widths(&self) -> &[f32] {
        &self.widths
    }

    fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() || self.measured_widths().len() != self.tabs.len() {
            return None;
        }

        let theme = self.resolved_theme();
        let gap = theme.metrics.tab_gap;
        let tab_height = self.tab_height().min(bounds.height()).max(0.0);
        let tab_y = bounds.y() + ((bounds.height() - tab_height) * 0.5).max(0.0);
        let mut x = bounds.x();
        for (current, measured_width) in self.widths.iter().enumerate() {
            let visible_width = (*measured_width).min((bounds.max_x() - x).max(0.0));
            let rect = Rect::new(x, tab_y, visible_width, tab_height);
            if current == index {
                return (visible_width > 0.0).then_some(rect);
            }
            x += *measured_width + gap;
            if x >= bounds.max_x() {
                break;
            }
        }

        None
    }

    fn close_rect_for(&self, tab_rect: Rect) -> Rect {
        let theme = self.resolved_theme();
        let close = Self::close_size(&theme)
            .min(tab_rect.width())
            .min(tab_rect.height());
        Rect::new(
            tab_rect.max_x() - close - Self::close_gap(&theme),
            tab_rect.y() + ((tab_rect.height() - close) * 0.5),
            close,
            close,
        )
    }

    fn label_rect_for(&self, tab_rect: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.tab_padding;
        let close = self.close_rect_for(tab_rect);
        Rect::new(
            tab_rect.x() + padding.left,
            tab_rect.y() + padding.top,
            (close.x() - tab_rect.x() - padding.left - Self::close_gap(&theme)).max(0.0),
            (tab_rect.height() - padding.top - padding.bottom).max(0.0),
        )
    }

    fn hit_at(&self, bounds: Rect, position: Point) -> Option<BrowserTabHit> {
        for index in 0..self.tabs.len() {
            let Some(rect) = self.tab_rect(bounds, index) else {
                continue;
            };
            if self.close_rect_for(rect).contains(position) {
                return Some(BrowserTabHit::Close(index));
            }
            if rect.contains(position) {
                return Some(BrowserTabHit::Tab(index));
            }
        }
        None
    }

    fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if self.tabs.is_empty() {
            return;
        }
        let selected = self.normalized_selected().unwrap_or(0) as isize;
        let last = self.tabs.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(BrowserTabHit::Tab(next)), ctx);
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }
        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }

    fn set_hovered(&mut self, hovered: Option<BrowserTabHit>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(hit) = hovered {
            self.hover_visual = Some(hit);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<BrowserTabHit>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(hit) = pressed {
            self.press_visual = Some(hit);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual.is_some_and(|hit| hit.index() == index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual.is_some_and(|hit| hit.index() == index) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

impl Widget for BrowserTabBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.refresh_tabs();
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.hit_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
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
                        BrowserTabHit::Tab(index) => self.activate(index, ctx),
                        BrowserTabHit::Close(index) => self.close(index, ctx),
                    }
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.tabs.is_empty() => self.activate(self.tabs.len() - 1, ctx),
                    _ => return,
                }
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
        self.refresh_tabs();
        let theme = self.resolved_theme();
        let style = text_token_style(&theme, theme.text.sm, theme.palette.text);
        let padding = theme.metrics.tab_padding;
        let close_extent = Self::close_size(&theme) + (Self::close_gap(&theme) * 2.0);
        self.label_measurements = self
            .tabs
            .iter()
            .map(|tab| measure_text(ctx, tab, &style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                (measurement.width + padding.left + padding.right + close_extent)
                    .max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = theme.metrics.tab_gap;
        let width =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width, self.tab_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let interaction = theme.interaction;
        let selected_index = self.normalized_selected();
        let focus_progress = self.focus_animation.value;

        ctx.push_clip_rect(ctx.bounds());
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = selected_index == Some(index);
            let hovered = self.hovered.is_some_and(|hit| hit.index() == index);
            let pressed = self.pressed.is_some_and(|hit| hit.index() == index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if let Some((background, border)) = tab_state_visuals(
                &theme,
                selected,
                hovered,
                pressed,
                hover_amount,
                press_amount,
            ) {
                draw_control_shape(
                    ctx,
                    rect,
                    theme.metrics.corner_radius,
                    physical_pixels(ctx, theme.metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    theme.metrics.corner_radius,
                    theme.metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = text_token_style(
                &theme,
                theme.text.sm,
                if selected {
                    palette.text
                } else {
                    palette.text_muted
                },
            );
            let text_slot = self.label_rect_for(rect);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                tab,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();

            let close = self.close_rect_for(rect);
            let close_hovered = self.hovered == Some(BrowserTabHit::Close(index));
            let close_pressed = self.pressed == Some(BrowserTabHit::Close(index));
            if close_hovered || close_pressed {
                ctx.fill(
                    rounded_rect_path(close, theme.metrics.corner_radius.min(5.0)),
                    if close_pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
                    },
                );
            }
            draw_icon_glyph(
                ctx,
                IconGlyph::Close,
                close.inflate(-3.0, -3.0),
                if close_hovered || selected {
                    palette.text
                } else {
                    palette.placeholder
                }
                .with_alpha(if close_pressed { 0.95 } else { 0.78 }),
            );
        }

        if let Some(selected) = selected_index {
            let progress = if self.selection_to == Some(selected) {
                self.selection_animation.value
            } else {
                1.0
            };
            if let Some(accent) = tab_indicator_rect(
                |index| self.tab_rect(ctx.bounds(), index),
                self.selection_from.unwrap_or(selected),
                selected,
                progress,
                theme.metrics.tab_padding,
                interaction.active_indicator_thickness,
            ) {
                ctx.fill(
                    rounded_rect_path(accent, accent.height() * 0.5),
                    palette.accent,
                );
            }
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TabBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .normalized_selected()
            .and_then(|index| self.tabs.get(index))
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let tab_id = browser_tab_semantics_id(ctx.widget_id(), index);
            let mut tab_node = SemanticsNode::new(tab_id, SemanticsRole::Button, rect);
            tab_node.parent = Some(ctx.widget_id());
            tab_node.name = Some(tab.clone());
            tab_node.state.selected = self.normalized_selected() == Some(index);
            tab_node.state.hovered = self.hovered.is_some_and(|hit| hit.index() == index);
            tab_node.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(tab_node);

            let mut close_node = SemanticsNode::new(
                browser_tab_close_semantics_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                self.close_rect_for(rect),
            );
            close_node.parent = Some(tab_id);
            close_node.name = Some(format!("Close {tab} tab"));
            close_node.state.hovered = self.hovered == Some(BrowserTabHit::Close(index));
            close_node.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(close_node);
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

fn browser_tab_semantics_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn browser_tab_close_semantics_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(10_000 + index as u64)
            & LOW_MASK),
    )
}

pub struct SegmentedControl {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    segments: Vec<SegmentedControlItem>,
    selected: usize,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    selection_from: usize,
    selection_animation: AnimatedScalar,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurements: Vec<TextMeasurement>,
    on_change: Option<SegmentedControlChange>,
    on_change_with_ctx: Option<SegmentedControlContextChange>,
}

pub struct SegmentedControlItem {
    label: String,
    semantic_name: Option<String>,
    description: Option<String>,
    disabled: bool,
}

impl SegmentedControlItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            semantic_name: None,
            description: None,
            disabled: false,
        }
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
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

impl SegmentedControl {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            segments: Vec::new(),
            selected: 0,
            selected_reader: None,
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
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

    pub fn item(mut self, item: SegmentedControlItem) -> Self {
        self.segments.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = SegmentedControlItem>,
    {
        self.segments.extend(items);
        self
    }

    pub fn segments<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.segments
            .extend(labels.into_iter().map(SegmentedControlItem::new));
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self.selected_reader = None;
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
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

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    fn normalized_selected(&self) -> usize {
        let selected = self
            .selected_reader
            .as_ref()
            .and_then(|reader| reader())
            .unwrap_or(self.selected);
        if self.segments.is_empty() {
            0
        } else {
            selected.min(self.segments.len() - 1)
        }
    }

    fn segment_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    fn segment_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.segments.len() {
            return None;
        }
        let count = self.segments.len().max(1);
        let width = bounds.width() / count as f32;
        let x = bounds.x() + width * index as f32;
        let width = if index + 1 == count {
            bounds.max_x() - x
        } else {
            width
        };
        Some(Rect::new(
            x,
            bounds.y(),
            width.max(0.0),
            bounds.height().max(0.0),
        ))
    }

    fn segment_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        if !bounds.contains(position) || self.segments.is_empty() {
            return None;
        }
        let slot_width = (bounds.width() / self.segments.len() as f32).max(1.0);
        let index = ((position.x - bounds.x()) / slot_width).floor() as usize;
        Some(index.min(self.segments.len() - 1))
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        let Some(segment) = self.segments.get(index) else {
            return;
        };
        if segment.disabled {
            return;
        }

        let index = index.min(self.segments.len() - 1);
        let selected = self.normalized_selected();
        if selected != index {
            let theme = self.resolved_theme();
            self.selection_from = selected;
            self.selected = index;
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
            let label = self.segments[index].label.clone();
            if let Some(on_change) = &mut self.on_change {
                on_change(index, label.clone());
            }
            if let Some(on_change) = &mut self.on_change_with_ctx {
                on_change(index, label, ctx);
            }
        }
    }

    fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.segments.is_empty() {
            return;
        }
        let selected = self.normalized_selected() as isize;
        let last = self.segments.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(next), ctx);
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }
        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }
}

impl Widget for SegmentedControl {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.segment_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Leave
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                if pointer.kind == PointerEventKind::Cancel && self.pressed.is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
                self.set_pressed(None, ctx);
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.segment_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.segment_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index, ctx);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.segments.is_empty() => {
                        self.activate(self.segments.len() - 1, ctx)
                    }
                    _ => return,
                }
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
        let theme = self.resolved_theme();
        let style = theme.body_text_style();
        let padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .segments
            .iter()
            .map(|segment| measure_text(ctx, &segment.label, &style))
            .collect();
        let widest = self
            .label_measurements
            .iter()
            .map(|measurement| measurement.width + padding.left + padding.right)
            .fold(theme.metrics.tab_min_width, f32::max);
        let width = widest * self.segments.len().max(1) as f32;
        constraints.clamp(Size::new(width.max(160.0), self.segment_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let padding = metrics.tab_padding;
        let label_style = TextStyle {
            font_size: theme.text.xs.size,
            line_height: theme.text.xs.line_height,
            weight: sui_text::FontWeight(theme.font_weights.semibold),
            color: palette.text_muted,
            ..theme.body_text_style()
        };
        let selected_label_style = TextStyle {
            color: palette.accent,
            ..label_style.clone()
        };
        let radius = metrics.corner_radius;

        ctx.fill(rounded_rect_path(ctx.bounds(), radius), palette.control);

        if !self.segments.is_empty() {
            let from = self.selection_from.min(self.segments.len() - 1);
            let selected = self.normalized_selected();
            if let Some(thumb) = tab_indicator_rect(
                |index| self.segment_rect(ctx.bounds(), index),
                from,
                selected,
                self.selection_animation.value,
                Insets::all(2.0),
                (ctx.bounds().height() - 4.0).max(0.0),
            ) {
                draw_control_shape(
                    ctx,
                    thumb,
                    (thumb.height() * 0.5).min(radius),
                    physical_pixels(ctx, metrics.border_width),
                    palette.selection,
                    palette.accent.with_alpha(0.36),
                );
            }
        }

        let focus_progress = self.focus_animation.value;
        for (index, segment) in self.segments.iter().enumerate() {
            let Some(rect) = self.segment_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if !selected
                && let Some((background, border)) =
                    tab_state_visuals(&theme, false, hovered, pressed, hover_amount, press_amount)
            {
                draw_control_shape(
                    ctx,
                    rect.inflate(-1.0, -1.0),
                    radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect.inflate(-1.0, -1.0),
                    radius,
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                &segment.label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let selected = self.normalized_selected();
        let value = self
            .segments
            .get(selected)
            .map(|segment| segment.label.clone());
        let mut group =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::RadioGroup, ctx.bounds());
        group.name = Some(self.name.clone());
        group.value = value.map(SemanticsValue::Text);
        group.state.focused = ctx.is_focused();
        group.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(group);

        for (index, segment) in self.segments.iter().enumerate() {
            let Some(bounds) = self.segment_rect(ctx.bounds(), index) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                segmented_control_item_id(ctx.widget_id(), index),
                SemanticsRole::RadioButton,
                bounds,
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(
                segment
                    .semantic_name
                    .clone()
                    .unwrap_or_else(|| segment.label.clone()),
            );
            node.description = segment.description.clone();
            node.value = Some(SemanticsValue::Text(segment.label.clone()));
            node.actions = if segment.disabled {
                Vec::new()
            } else {
                vec![SemanticsAction::Activate]
            };
            node.state.disabled = segment.disabled;
            node.state.hovered = self.hovered == Some(index);
            node.state.selected = selected == index;
            node.state.checked = Some(if selected == index {
                sui_core::ToggleState::Checked
            } else {
                sui_core::ToggleState::Unchecked
            });
            ctx.push(node);
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

pub struct Tabs {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    labels: Vec<String>,
    panels: WidgetChildren,
    selected: usize,
    selection_from: usize,
    selection_animation: AnimatedScalar,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurements: Vec<TextMeasurement>,
    widths: Vec<f32>,
    gap: Option<f32>,
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
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            widths: Vec::new(),
            gap: None,
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
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
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
        self.resolved_theme().metrics.tab_height
    }

    fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.tab_gap)
            .max(0.0)
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), self.header_height())
    }

    fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.labels.len() || self.widths.len() != self.labels.len() {
            return None;
        }

        let header = self.header_rect(bounds);
        let gap = self.resolved_gap();
        let base_total =
            self.widths.iter().sum::<f32>() + (gap * self.labels.len().saturating_sub(1) as f32);
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
            x += rect.width() + gap;
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

    fn select(&mut self, index: usize, ctx: &mut EventCtx) {
        if self.labels.is_empty() {
            return;
        }

        let index = index.min(self.labels.len() - 1);
        if self.selected != index {
            let theme = self.resolved_theme();
            self.selection_from = self.normalized_selected();
            self.selected = index;
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.labels[index].clone());
            }
        }
    }

    fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.labels.is_empty() {
            return;
        }

        let next = (self.normalized_selected() as isize + delta)
            .clamp(0, self.labels.len() as isize - 1) as usize;
        self.set_hovered(Some(next), ctx);
        self.select(next, ctx);
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }
        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
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

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

impl Widget for Tabs {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.tab_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.header_rect(ctx.bounds()).contains(pointer.position) =>
            {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
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
                        self.select(index, ctx);
                        ctx.request_measure();
                    }
                    self.set_hovered(hovered, ctx);
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.select(0, ctx),
                    "End" if !self.labels.is_empty() => self.select(self.labels.len() - 1, ctx),
                    _ => return,
                }
                ctx.request_measure();
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
        let theme = self.resolved_theme();
        let text_style = text_token_style(&theme, theme.text.sm, theme.palette.text);
        let tab_padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .labels
            .iter()
            .map(|label| measure_text(ctx, label, &text_style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                (measurement.width + tab_padding.left + tab_padding.right)
                    .max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = self.resolved_gap();
        let header_width =
            self.widths.iter().sum::<f32>() + (gap * self.labels.len().saturating_sub(1) as f32);
        let available_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(header_width)
        } else {
            header_width.max(320.0)
        };
        let header_height = self.header_height();
        let padding = theme.metrics.tab_panel_padding;
        let panel_gap = theme.metrics.tab_panel_gap;

        let panel_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (available_width - padding.left - padding.right).max(0.0),
                if constraints.max.height.is_finite() {
                    (constraints.max.height
                        - header_height
                        - panel_gap
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
            header_height + panel_gap,
            content_width,
            content_height,
        );

        constraints.clamp(Size::new(
            content_width,
            header_height + panel_gap + content_height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let header_height = self.header_height();
        let padding = theme.metrics.tab_panel_padding;
        let panel_gap = theme.metrics.tab_panel_gap;
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
        let interaction = theme.interaction;
        let tab_padding = metrics.tab_padding;
        let header = self.header_rect(ctx.bounds());
        let label_style = text_token_style(&theme, theme.text.sm, palette.text_muted);
        let selected_label_style = text_token_style(&theme, theme.text.sm, palette.text);

        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            palette.control,
        );

        let focus_progress = self.focus_animation.value;
        for (index, label) in self.labels.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if let Some((background, border)) = tab_state_visuals(
                &theme,
                selected,
                hovered,
                pressed,
                hover_amount,
                press_amount,
            ) {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, tab_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }

        if let Some(accent) = tab_indicator_rect(
            |index| self.tab_rect(ctx.bounds(), index),
            self.selection_from,
            self.normalized_selected(),
            self.selection_animation.value,
            tab_padding,
            interaction.active_indicator_thickness,
        ) {
            ctx.fill(
                rounded_rect_path(accent, accent.height() * 0.5),
                palette.accent,
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
            let panel_translation = tab_panel_transition_translation(
                self.selection_from,
                self.normalized_selected(),
                self.selection_animation.value,
                metrics,
            );
            if panel_translation == Vector::ZERO {
                panel.paint(ctx);
            } else {
                ctx.push_clip_rect(content);
                ctx.translate(panel_translation);
                panel.paint(ctx);
                ctx.pop_transform();
                ctx.pop_clip();
            }
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
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
    highlight_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    highlight_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
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
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
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

    pub fn highlighted(mut self, index: usize) -> Self {
        self.highlighted = Some(index);
        self.highlight_visual = Some(index);
        self.highlight_animation = AnimatedScalar::new(1.0);
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
        let theme = self.resolved_theme();
        let padding = theme.metrics.menu_padding;
        let x = bounds.x() + padding.left;
        let y = bounds.y() + padding.top + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (bounds.width() - padding.left - padding.right).max(0.0),
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

    fn move_highlight(&mut self, delta: isize, ctx: &mut EventCtx) {
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
        self.set_highlighted(Some(index as usize), ctx);
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn set_highlighted(&mut self, highlighted: Option<usize>, ctx: &mut EventCtx) {
        if self.highlighted == highlighted {
            return;
        }
        let theme = self.resolved_theme();
        self.highlighted = highlighted;
        if let Some(index) = highlighted {
            self.highlight_visual = Some(index);
            self.highlight_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.highlight_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.highlight_animation, 0.0, &theme, ctx) {
            self.highlight_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn highlight_amount_for(&self, index: usize) -> f32 {
        if self.highlight_visual == Some(index) {
            self.highlight_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let highlight_animating = self.highlight_animation.advance(time);
        if !highlight_animating
            && self.highlighted.is_none()
            && self.highlight_animation.value <= AnimatedScalar::EPSILON
        {
            self.highlight_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        highlight_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for Menu {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_highlighted(self.item_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                self.set_highlighted(highlighted, ctx);
                self.set_pressed(
                    highlighted
                        .filter(|index| self.items.get(*index).is_some_and(|item| item.enabled)),
                    ctx,
                );
                if self.focus_on_pointer_down {
                    ctx.request_focus();
                }
                ctx.request_pointer_capture(pointer.pointer_id);
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
                self.set_highlighted(highlighted, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowDown" => self.move_highlight(1, ctx),
                    "ArrowUp" => self.move_highlight(-1, ctx),
                    "Home" => {
                        self.set_highlighted(self.items.iter().position(|item| item.enabled), ctx);
                    }
                    "End" => {
                        self.set_highlighted(self.items.iter().rposition(|item| item.enabled), ctx);
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
            width = width.max(
                label
                    + shortcut
                    + theme.metrics.menu_item_padding.left
                    + theme.metrics.menu_item_padding.right
                    + theme.metrics.menu_shortcut_width,
            );
        }
        self.measured_width = width.max(220.0);
        let height = themed_menu_height_for_rows(&theme, self.row_height(), self.items.len());
        constraints.clamp(Size::new(
            self.measured_width,
            height.max(themed_menu_height_for_rows(&theme, self.row_height(), 1)),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let item_padding = metrics.menu_item_padding;

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
            (self.focus_animation.value > AnimatedScalar::EPSILON).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
            ),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(
                    row.x(),
                    row.y() - (metrics.menu_padding.top * 0.5),
                    row.width(),
                    1.0,
                );
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let highlight_amount = self.highlight_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let label_style = theme.text_style(item.text_color(&theme));
            let label_slot = Rect::new(
                row.x() + item_padding.left,
                row.y(),
                (row.width()
                    - item_padding.left
                    - item_padding.right
                    - item
                        .shortcut
                        .as_ref()
                        .map(|_| metrics.menu_shortcut_width)
                        .unwrap_or(0.0))
                .max(0.0),
                row.height(),
            );
            if highlighted || highlight_amount > 0.0 || press_amount > 0.0 {
                let highlight_background = mix_color(
                    palette.control,
                    palette.accent,
                    interaction.selected_blend * highlight_amount,
                );
                let background = if press_amount > 0.0 {
                    mix_color(
                        highlight_background,
                        palette.control_active,
                        interaction.pressed_blend * press_amount,
                    )
                } else {
                    highlight_background
                };
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    background,
                );
            }

            ctx.push_clip_rect(label_slot);
            paint_aligned_text(
                ctx,
                label_slot,
                &item.label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            ctx.pop_clip();

            if let Some(shortcut) = &item.shortcut {
                let shortcut_style = theme.placeholder_text_style();
                let shortcut_slot = Rect::new(
                    row.max_x() - item_padding.right - metrics.menu_shortcut_width,
                    row.y(),
                    metrics.menu_shortcut_width,
                    row.height(),
                );
                ctx.push_clip_rect(shortcut_slot);
                paint_aligned_text(
                    ctx,
                    shortcut_slot,
                    shortcut,
                    &shortcut_style,
                    shortcut_style.line_height,
                    1.0,
                );
                ctx.pop_clip();
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

type AnimatedScalar = MotionScalar;

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
    let padding = theme.metrics.tooltip_padding;
    let width =
        (measurement.width + padding.left + padding.right).max(theme.metrics.tooltip_min_width);
    let height =
        measurement.height.max(theme.typography.body_line_height) + padding.top + padding.bottom;
    let x = trigger_bounds.x() + ((trigger_bounds.width() - width) * 0.5);
    let y = match placement {
        TooltipPlacement::Above => trigger_bounds.y() - height - theme.metrics.tooltip_gap,
        TooltipPlacement::Below => trigger_bounds.max_y() + theme.metrics.tooltip_gap,
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
                self.theme.metrics.tooltip_reveal_offset * (1.0 - self.reveal.value) * direction,
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
            state.theme.surfaces.tooltip,
            state.theme.surfaces.tooltip_border,
            None,
        );
        let tail = tooltip_tail(state.trigger_bounds, bubble, state.placement);
        ctx.fill(tail, state.theme.surfaces.tooltip);
        let text_style = text_token_style(
            &state.theme,
            state.theme.text.sm,
            state.theme.surfaces.tooltip_text,
        );
        let text_slot = inset_rect(bubble, metrics.tooltip_padding);
        ctx.push_clip_rect(text_slot);
        paint_aligned_text(
            ctx,
            text_slot,
            &state.text,
            &text_style,
            text_style.line_height,
            0.0,
        );
        ctx.pop_clip();
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
                hit_test: false,
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
        let motion = state.theme.motion;
        state.hovered = hovered;
        let should_animate = state.reveal.set_target(
            hovered as u8 as f32,
            ctx.current_time(),
            motion.entrance_duration(),
            motion.entrance_easing(),
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
                let changed = state.reveal.changed_since(previous);
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
        let text_style = text_token_style(
            &state.theme,
            state.theme.text.sm,
            state.theme.surfaces.tooltip_text,
        );
        state.measurement = Some(measure_text(ctx, &state.text, &text_style));
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
    frame_rect: Rect,
    arrival_active: bool,
    reveal: AnimatedScalar,
    focus_animation: AnimatedScalar,
}

impl PopoverSurfaceState {
    fn new() -> Self {
        Self {
            theme: DefaultTheme::default(),
            frame_rect: Rect::ZERO,
            arrival_active: false,
            reveal: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
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
            translation: Vector::new(
                0.0,
                -self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value),
            ),
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
        let padding = state.theme.metrics.popover_padding;
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
        let padding = state.theme.metrics.popover_padding;
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
            None,
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

struct PopoverFocusSurface {
    state: Rc<RefCell<PopoverSurfaceState>>,
}

impl PopoverFocusSurface {
    fn new(state: Rc<RefCell<PopoverSurfaceState>>) -> Self {
        Self { state }
    }
}

impl Widget for PopoverFocusSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.frame_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() || !state.focus_animation.is_presented() {
            return;
        }

        let Some(focus_ring) = state.resolved_visuals().focus_ring else {
            return;
        };
        let progress = state.focus_animation.value;
        if progress <= AnimatedScalar::EPSILON {
            return;
        }

        let metrics = state.theme.metrics;
        draw_focus_ring_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius + 2.0,
            metrics,
            focus_ring.with_alpha(focus_ring.alpha * progress),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        let state = self.state.borrow();
        (state.is_presented() && state.focus_animation.is_presented()).then_some(
            StackSurfaceOptions {
                transient: true,
                hit_test: false,
                ..StackSurfaceOptions::default()
            },
        )
    }
}

pub struct Popover {
    name: String,
    trigger: SingleChild,
    surface: SingleChild,
    focus_surface: SingleChild,
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
            focus_surface: SingleChild::new(PopoverFocusSurface::new(Rc::clone(&state))),
            open: false,
            gap: DefaultTheme::default().metrics.popover_gap,
            arrival_timer: None,
            state,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.gap = theme.metrics.popover_gap;
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
        let focus_surface_id = self.focus_surface.child().id();
        let mut state = self.state.borrow_mut();
        let was_presented = state.is_presented();
        let motion = state.theme.motion;
        let should_animate = state.reveal.set_target(
            open as u8 as f32,
            ctx.current_time(),
            motion.entrance_duration(),
            motion.entrance_easing(),
        );
        let is_presented = state.is_presented();
        drop(state);

        if open || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
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
                let focus_surface_id = self.focus_surface.child().id();
                let mut state = self.state.borrow_mut();
                let was_presented = state.is_presented();
                let was_focus_presented = state.focus_animation.is_presented();
                let previous_reveal = state.reveal.value;
                let previous_focus = state.focus_animation.value;
                let reveal_animating = state.reveal.advance(*time);
                let focus_animating = state.focus_animation.advance(*time);
                let reveal_changed = state.reveal.changed_since(previous_reveal);
                let focus_changed = state.focus_animation.changed_since(previous_focus);
                let is_presented = state.is_presented();
                let is_focus_presented = state.focus_animation.is_presented();
                drop(state);

                if reveal_changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Effect);
                }
                if focus_changed {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if was_focus_presented != is_focus_presented {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if reveal_animating || focus_animating {
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
        let focus_size = if presented { surface_size } else { Size::ZERO };
        self.focus_surface
            .measure(ctx, Constraints::tight(focus_size));
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
        self.focus_surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.state.borrow().is_presented() {
            self.surface.paint(ctx);
            self.focus_surface.paint(ctx);
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
        let focus_surface_id = self.focus_surface.child().id();
        let mut state = self.state.borrow_mut();
        let was_focus_presented = state.focus_animation.is_presented();
        let theme = state.theme;
        set_focus_animation_target(
            &mut state.focus_animation,
            focused as u8 as f32,
            &theme,
            ctx,
        );
        let is_focus_presented = state.focus_animation.is_presented();
        drop(state);

        if was_focus_presented != is_focus_presented {
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
        }
        request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
        if !focused && self.open {
            self.set_open(ctx, false);
        }
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.open || self.state.borrow().is_presented() {
            self.surface.visit_children(visitor);
            self.focus_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.open || self.state.borrow().is_presented() {
            self.surface.visit_children_mut(visitor);
            self.focus_surface.visit_children_mut(visitor);
        }
    }
}

#[derive(Debug, Clone)]
struct ContextMenuPresentationState {
    theme: DefaultTheme,
    items: Vec<MenuItem>,
    highlighted: Option<usize>,
    highlight_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    frame_rect: Rect,
    row_height: f32,
    reveal: AnimatedScalar,
    focus_animation: AnimatedScalar,
    highlight_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
}

impl ContextMenuPresentationState {
    fn new() -> Self {
        let theme = DefaultTheme::default();
        Self {
            theme,
            items: Vec::new(),
            highlighted: None,
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            frame_rect: Rect::ZERO,
            row_height: menu_row_height(&theme),
            reveal: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
        }
    }

    fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let padding = self.theme.metrics.menu_padding;
        let x = bounds.x() + padding.left;
        let y = bounds.y() + padding.top + (index as f32 * self.row_height);
        Some(Rect::new(
            x,
            y,
            (bounds.width() - padding.left - padding.right).max(0.0),
            self.row_height,
        ))
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                -self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value),
            ),
        }
    }

    fn highlight_amount_for(&self, index: usize) -> f32 {
        if self.highlight_visual == Some(index) {
            self.highlight_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

struct ContextMenuSurface {
    state: Rc<RefCell<ContextMenuPresentationState>>,
}

impl ContextMenuSurface {
    fn new(state: Rc<RefCell<ContextMenuPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for ContextMenuSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.frame_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let menu = ctx.bounds();
        let theme = state.theme;
        let metrics = theme.metrics;
        let palette = theme.palette;
        let interaction = theme.interaction;
        let item_padding = metrics.menu_item_padding;
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(ctx, menu, [surface_radius; 4], &theme.shadows.box_shadow.lg);
        draw_control_frame(
            ctx,
            menu,
            surface_radius,
            metrics,
            palette.surface_raised,
            palette.border,
            None,
        );

        for (index, item) in state.items.iter().enumerate() {
            let Some(row) = state.item_rect(menu, index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(
                    row.x(),
                    row.y() - (metrics.menu_padding.top * 0.5),
                    row.width(),
                    1.0,
                );
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = state.highlighted == Some(index);
            let highlight_amount = state.highlight_amount_for(index);
            let press_amount = state.press_amount_for(index);
            let label_style = theme.text_style(item.text_color(&theme));
            let label_slot = Rect::new(
                row.x() + item_padding.left,
                row.y(),
                (row.width()
                    - item_padding.left
                    - item_padding.right
                    - item
                        .shortcut
                        .as_ref()
                        .map(|_| metrics.menu_shortcut_width)
                        .unwrap_or(0.0))
                .max(0.0),
                row.height(),
            );
            if highlighted || highlight_amount > 0.0 || press_amount > 0.0 {
                let highlight_background = mix_color(
                    palette.control,
                    palette.accent,
                    interaction.selected_blend * highlight_amount,
                );
                let background = if press_amount > 0.0 {
                    mix_color(
                        highlight_background,
                        palette.control_active,
                        interaction.pressed_blend * press_amount,
                    )
                } else {
                    highlight_background
                };
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    background,
                );
            }

            ctx.push_clip_rect(label_slot);
            paint_aligned_text(
                ctx,
                label_slot,
                &item.label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            ctx.pop_clip();

            if let Some(shortcut) = &item.shortcut {
                let shortcut_style = theme.placeholder_text_style();
                let shortcut_slot = Rect::new(
                    row.max_x() - item_padding.right - metrics.menu_shortcut_width,
                    row.y(),
                    metrics.menu_shortcut_width,
                    row.height(),
                );
                ctx.push_clip_rect(shortcut_slot);
                paint_aligned_text(
                    ctx,
                    shortcut_slot,
                    shortcut,
                    &shortcut_style,
                    shortcut_style.line_height,
                    1.0,
                );
                ctx.pop_clip();
            }
        }
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

struct ContextMenuFocusSurface {
    state: Rc<RefCell<ContextMenuPresentationState>>,
}

impl ContextMenuFocusSurface {
    fn new(state: Rc<RefCell<ContextMenuPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for ContextMenuFocusSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.frame_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() || !state.focus_animation.is_presented() {
            return;
        }

        let progress = state.focus_animation.value;
        if progress <= AnimatedScalar::EPSILON {
            return;
        }

        let metrics = state.theme.metrics;
        let palette = state.theme.palette;
        draw_focus_ring_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius + 2.0,
            metrics,
            palette
                .focus_ring
                .with_alpha(palette.focus_ring.alpha * progress),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        let state = self.state.borrow();
        (state.is_presented() && state.focus_animation.is_presented()).then_some(
            StackSurfaceOptions {
                transient: true,
                hit_test: false,
                ..StackSurfaceOptions::default()
            },
        )
    }
}

pub struct ContextMenu {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    trigger: SingleChild,
    items: Vec<MenuItem>,
    items_provider: Option<Box<dyn Fn() -> Vec<MenuItem>>>,
    open: bool,
    highlighted: Option<usize>,
    highlight_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    highlight_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    frame_rect: Rect,
    surface: SingleChild,
    focus_surface: SingleChild,
    surface_state: Rc<RefCell<ContextMenuPresentationState>>,
    activation_button: PointerButton,
    anchor_to_pointer: Option<bool>,
    open_position: Option<Point>,
    on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
    on_activate_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, MenuItem)>>,
}

impl ContextMenu {
    pub fn new<W>(name: impl Into<String>, trigger: W) -> Self
    where
        W: Widget + 'static,
    {
        let surface_state = Rc::new(RefCell::new(ContextMenuPresentationState::new()));
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            trigger: SingleChild::new(trigger),
            items: Vec::new(),
            items_provider: None,
            open: false,
            highlighted: None,
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            frame_rect: Rect::ZERO,
            surface: SingleChild::new(ContextMenuSurface::new(Rc::clone(&surface_state))),
            focus_surface: SingleChild::new(ContextMenuFocusSurface::new(Rc::clone(
                &surface_state,
            ))),
            surface_state,
            activation_button: PointerButton::Secondary,
            anchor_to_pointer: None,
            open_position: None,
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

    /// Rebuild the item list every time the menu opens, so per-item enabled
    /// state can reflect current application state (selection, clipboard, …).
    pub fn items_when<F>(mut self, provider: F) -> Self
    where
        F: Fn() -> Vec<MenuItem> + 'static,
    {
        self.items_provider = Some(Box::new(provider));
        self
    }

    /// Widget id of the wrapped trigger. Menu activations can route commands
    /// back to it via `EventCtx::post_event` — for example the standard text
    /// editing commands (`TextCommand`) understood by the text widgets.
    pub fn trigger_id(&self) -> WidgetId {
        self.trigger.child().id()
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

    /// Whether the menu opens at the press position instead of dropping below
    /// the trigger. Defaults by activation button: right-click menus anchor to
    /// the pointer (standard context-menu behavior, and the only sensible
    /// placement for large triggers), other buttons keep the dropdown layout.
    pub fn anchor_to_pointer(mut self, anchor_to_pointer: bool) -> Self {
        self.anchor_to_pointer = Some(anchor_to_pointer);
        self
    }

    fn anchors_to_pointer(&self) -> bool {
        self.anchor_to_pointer
            .unwrap_or(self.activation_button == PointerButton::Secondary)
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
            width = width.max(
                label
                    + shortcut
                    + theme.metrics.menu_item_padding.left
                    + theme.metrics.menu_item_padding.right
                    + theme.metrics.menu_shortcut_width,
            );
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
        let theme = self.resolved_theme();
        let padding = theme.metrics.menu_padding;
        let menu = self.frame_rect.translate(bounds.origin.to_vector());
        let x = menu.x() + padding.left;
        let y = menu.y() + padding.top + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (menu.width() - padding.left - padding.right).max(0.0),
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

    fn sync_surface_state(&self, bounds: Rect) {
        let theme = self.resolved_theme();
        let mut state = self.surface_state.borrow_mut();
        state.theme = theme;
        state.items = self.items.clone();
        state.highlighted = self.highlighted;
        state.highlight_visual = self.highlight_visual;
        state.pressed = self.pressed;
        state.press_visual = self.press_visual;
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        state.frame_rect = self.frame_rect.translate(bounds.origin.to_vector());
        state.row_height = self.row_height();
    }

    fn refresh_surface_interaction_state(&self, ctx: &mut EventCtx) {
        let surface_id = self.surface.child().id();
        let mut state = self.surface_state.borrow_mut();
        let changed = state.highlighted != self.highlighted
            || state.highlight_visual != self.highlight_visual
            || state.pressed != self.pressed
            || state.press_visual != self.press_visual
            || state.highlight_animation != self.highlight_animation
            || state.press_animation != self.press_animation;
        state.highlighted = self.highlighted;
        state.highlight_visual = self.highlight_visual;
        state.pressed = self.pressed;
        state.press_visual = self.press_visual;
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        let presented = state.is_presented();
        drop(state);

        if changed && presented {
            request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
        }
    }

    fn set_highlighted(&mut self, highlighted: Option<usize>, ctx: &mut EventCtx) {
        if self.highlighted == highlighted {
            return;
        }
        let theme = self.resolved_theme();
        self.highlighted = highlighted;
        if let Some(index) = highlighted {
            self.highlight_visual = Some(index);
            self.highlight_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.highlight_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.highlight_animation, 0.0, &theme, ctx) {
            self.highlight_visual = None;
        }
        self.refresh_surface_interaction_state(ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        self.refresh_surface_interaction_state(ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_row_animations(&mut self, time: f64) -> bool {
        let highlight_animating = self.highlight_animation.advance(time);
        if !highlight_animating
            && self.highlighted.is_none()
            && self.highlight_animation.value <= AnimatedScalar::EPSILON
        {
            self.highlight_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        highlight_animating | press_animating
    }

    fn set_open(&mut self, ctx: &mut EventCtx, open: bool) {
        if self.open == open {
            return;
        }

        if open && let Some(provider) = &self.items_provider {
            self.items = provider();
        }
        if !open {
            self.open_position = None;
        }

        self.open = open;
        self.highlighted = if open {
            self.items.iter().position(|item| item.enabled)
        } else {
            None
        };
        self.highlight_visual = self.highlighted;
        self.highlight_animation = AnimatedScalar::new(self.highlighted.is_some() as u8 as f32);
        self.pressed = None;
        self.press_visual = None;
        self.press_animation = AnimatedScalar::new(0.0);

        let surface_id = self.surface.child().id();
        let focus_surface_id = self.focus_surface.child().id();
        let theme = self.resolved_theme();
        let mut state = self.surface_state.borrow_mut();
        state.theme = theme;
        state.items = self.items.clone();
        state.highlighted = self.highlighted;
        state.highlight_visual = self.highlight_visual;
        state.pressed = self.pressed;
        state.press_visual = self.press_visual;
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        let was_presented = state.is_presented();
        let should_animate = if open {
            let motion = theme.motion;
            state.reveal.set_target(
                1.0,
                ctx.current_time(),
                motion.entrance_duration(),
                motion.entrance_easing(),
            )
        } else {
            state.reveal = AnimatedScalar::new(0.0);
            false
        };
        let is_presented = state.is_presented();
        drop(state);

        if open || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_paint();
        ctx.request_semantics();
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
                self.set_highlighted(self.item_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(self.activation_button)
                    && ctx.phase() != EventPhase::Capture
                    && self.trigger_rect().contains(pointer.position) =>
            {
                // Target/Bubble only: the trigger widget sees the press first
                // (to focus itself or record the press position) before the
                // menu opens and takes keyboard focus.
                let open = !self.open;
                self.open_position = (open && self.anchors_to_pointer()).then(|| {
                    let origin = ctx.bounds().origin;
                    Point::new(pointer.position.x - origin.x, pointer.position.y - origin.y)
                });
                self.set_open(ctx, open);
                ctx.request_focus();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open =>
            {
                if let Some(index) = self.item_at(ctx.bounds(), pointer.position) {
                    self.set_highlighted(Some(index), ctx);
                    self.set_pressed(
                        self.items
                            .get(index)
                            .filter(|item| item.enabled)
                            .map(|_| index),
                        ctx,
                    );
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                } else if !self.trigger_rect().contains(pointer.position) {
                    self.set_open(ctx, false);
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
                    self.set_open(ctx, false);
                }
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
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
                        menu.move_highlight(1, ctx);
                        self.set_highlighted(menu.highlighted, ctx);
                    }
                    "ArrowUp" => {
                        let mut menu = Menu::new("temp").items(self.items.clone());
                        menu.highlighted = self.highlighted;
                        menu.move_highlight(-1, ctx);
                        self.set_highlighted(menu.highlighted, ctx);
                    }
                    "Enter" | " " => {
                        if let Some(index) = self.highlighted {
                            self.activate(ctx, index);
                            self.set_open(ctx, false);
                        }
                    }
                    "Escape" => {
                        self.set_open(ctx, false);
                    }
                    _ => return,
                }
                self.refresh_surface_interaction_state(ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let surface_id = self.surface.child().id();
                let focus_surface_id = self.focus_surface.child().id();
                let mut state = self.surface_state.borrow_mut();
                let was_presented = state.is_presented();
                let was_focus_presented = state.focus_animation.is_presented();
                let previous = state.reveal.value;
                let previous_focus = state.focus_animation.value;
                let reveal_animating = state.reveal.advance(*time);
                let focus_animating = state.focus_animation.advance(*time);
                let reveal_changed = state.reveal.changed_since(previous);
                let focus_changed = state.focus_animation.changed_since(previous_focus);
                let is_presented = state.is_presented();
                let is_focus_presented = state.focus_animation.is_presented();
                drop(state);

                let previous_highlight = self.highlight_animation.value;
                let previous_press = self.press_animation.value;
                let row_animating = self.advance_row_animations(*time);
                let row_changed = self.highlight_animation.changed_since(previous_highlight)
                    || self.press_animation.changed_since(previous_press);
                if row_changed {
                    self.refresh_surface_interaction_state(ctx);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
                }

                if reveal_changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Effect);
                }
                if focus_changed {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if was_focus_presented != is_focus_presented {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if reveal_animating || row_animating || focus_animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
        let mut size = trigger_size;
        if self.open {
            let theme = self.resolved_theme();
            let pointer_anchored = self.open_position.is_some();
            let width = if pointer_anchored {
                self.measured_menu_width(ctx)
            } else {
                self.measured_menu_width(ctx).max(trigger_size.width)
            };
            let height = themed_menu_height_for_rows(&theme, self.row_height(), self.items.len());
            let gap = theme.metrics.popover_gap;
            self.frame_rect = match self.open_position {
                Some(position) => Rect::new(position.x, position.y, width, height),
                None => Rect::new(0.0, trigger_size.height + gap, width, height),
            };
            {
                let mut state = self.surface_state.borrow_mut();
                state.theme = theme;
                state.items = self.items.clone();
                state.highlighted = self.highlighted;
                state.highlight_visual = self.highlight_visual;
                state.pressed = self.pressed;
                state.press_visual = self.press_visual;
                state.highlight_animation = self.highlight_animation;
                state.press_animation = self.press_animation;
                state.frame_rect = Rect::from_origin_size(Point::ZERO, self.frame_rect.size);
                state.row_height = self.row_height();
            }
            self.surface
                .measure(ctx, Constraints::tight(self.frame_rect.size));
            self.focus_surface
                .measure(ctx, Constraints::tight(self.frame_rect.size));
            if !pointer_anchored {
                size = Size::new(
                    width.max(trigger_size.width),
                    trigger_size.height + gap + height,
                );
            }
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
        if self.open && self.open_position.is_some() {
            // Pointer-anchored menus stay inside the trigger bounds instead of
            // spilling past the window edge.
            let size = self.frame_rect.size;
            let max_x = (bounds.width() - size.width).max(0.0);
            let max_y = (bounds.height() - size.height).max(0.0);
            self.frame_rect = Rect::from_origin_size(
                Point::new(
                    self.frame_rect.x().clamp(0.0, max_x),
                    self.frame_rect.y().clamp(0.0, max_y),
                ),
                size,
            );
        }
        self.sync_surface_state(bounds);
        let state = self.surface_state.borrow();
        let surface_bounds = if state.is_presented() {
            state.frame_rect
        } else {
            Rect::from_origin_size(bounds.origin, Size::ZERO)
        };
        drop(state);
        self.surface.arrange(ctx, surface_bounds);
        self.focus_surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.surface_state.borrow().is_presented() {
            self.surface.paint(ctx);
            self.focus_surface.paint(ctx);
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
            self.set_open(ctx, false);
        }
        let focus_surface_id = self.focus_surface.child().id();
        {
            let mut state = self.surface_state.borrow_mut();
            let was_focus_presented = state.focus_animation.is_presented();
            let theme = state.theme;
            set_focus_animation_target(
                &mut state.focus_animation,
                focused as u8 as f32,
                &theme,
                ctx,
            );
            if was_focus_presented != state.focus_animation.is_presented() {
                request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
            }
        }
        request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.surface_state.borrow().is_presented() {
            self.surface.visit_children(visitor);
            self.focus_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.surface_state.borrow().is_presented() {
            self.surface.visit_children_mut(visitor);
            self.focus_surface.visit_children_mut(visitor);
        }
    }
}

pub struct Dialog {
    theme: Box<DefaultTheme>,
    title: String,
    description: Option<String>,
    shown: bool,
    modal: bool,
    dismiss_on_scrim: bool,
    max_width: Option<f32>,
    body: SingleChild,
    actions: WidgetChildren,
    body_frame: Rect,
    dialog_frame: Rect,
    title_measurement: Option<TextMeasurement>,
    description_measurement: Option<TextMeasurement>,
    reveal: AnimatedScalar,
    focus_animation: AnimatedScalar,
    entrance_started: bool,
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
            max_width: None,
            body: SingleChild::new(body),
            actions: WidgetChildren::new(),
            body_frame: Rect::ZERO,
            dialog_frame: Rect::ZERO,
            title_measurement: None,
            description_measurement: None,
            reveal: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            entrance_started: false,
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
        if !shown {
            self.reveal = AnimatedScalar::new(0.0);
            self.focus_animation = AnimatedScalar::new(0.0);
            self.entrance_started = false;
        }
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
        self.max_width = Some(max_width.max(self.theme.metrics.dialog_min_width));
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
                .min_width(self.theme.metrics.dialog_action_min_width)
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
                .min_width(self.theme.metrics.dialog_action_min_width)
                .on_press(on_press),
        );
        self
    }

    fn resolved_max_width(&self) -> f32 {
        self.max_width
            .unwrap_or(self.theme.metrics.dialog_max_width)
    }

    fn title_style(&self) -> TextStyle {
        TextStyle {
            font_size: self.theme.metrics.dialog_title_font_size,
            line_height: self.theme.metrics.dialog_title_line_height,
            color: self.theme.palette.text,
            ..self.theme.body_text_style()
        }
    }

    fn dismiss(&mut self) {
        if let Some(on_dismiss) = &mut self.on_dismiss {
            on_dismiss();
        }
    }

    fn ensure_entrance_started(&mut self, ctx: &mut MeasureCtx) {
        if self.entrance_started {
            return;
        }
        self.entrance_started = true;
        let motion = self.theme.motion;
        if self.reveal.set_target(
            1.0,
            ctx.current_time(),
            motion.entrance_duration(),
            motion.entrance_easing(),
        ) {
            ctx.request_animation_frame();
        }
    }
}

impl Widget for Dialog {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.shown {
            return;
        }

        match event {
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let previous = self.reveal.value;
                let animating = self.reveal.advance(*time);
                if self.reveal.changed_since(previous) {
                    ctx.request_effect();
                    if !self.modal {
                        ctx.request_transform();
                    }
                }
                let previous_focus = self.focus_animation.value;
                let focus_animating = self.focus_animation.advance(*time);
                if self.focus_animation.changed_since(previous_focus) {
                    ctx.request_paint();
                }
                if animating || focus_animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .dialog_frame
                        .translate(ctx.bounds().origin.to_vector())
                        .contains(pointer.position) =>
            {
                ctx.request_focus();
                ctx.request_semantics();
            }
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
            self.reveal = AnimatedScalar::new(0.0);
            self.focus_animation = AnimatedScalar::new(0.0);
            self.entrance_started = false;
            return Size::ZERO;
        }
        self.ensure_entrance_started(ctx);

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
        let metrics = self.theme.metrics;
        let outer_margin = metrics.dialog_outer_margin;
        let padding = metrics.dialog_padding;
        let title_style = self.title_style();
        let description_style = self.theme.placeholder_text_style();
        self.title_measurement = Some(measure_text(ctx, &self.title, &title_style));
        self.description_measurement = self
            .description
            .as_ref()
            .map(|text| measure_text(ctx, text, &description_style));

        let dialog_width = (viewport.width - (outer_margin * 2.0))
            .min(self.resolved_max_width())
            .max(metrics.dialog_min_width);
        let mut footer_height: f32 = 0.0;
        for button in self.actions.as_mut_slice().iter_mut() {
            let button_size = button.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(dialog_width, metrics.min_height + metrics.dialog_action_gap),
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
        let header_gap = if self.description.is_some() {
            metrics.dialog_description_gap
        } else {
            0.0
        };
        let body_top =
            padding.top + title_height + header_gap + description_height + metrics.dialog_body_gap;
        let footer_gap = if self.actions.is_empty() {
            0.0
        } else {
            metrics.dialog_footer_gap
        };
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
            let metrics = self.theme.metrics;
            let padding = metrics.dialog_padding;
            let action_gap = metrics.dialog_action_gap;
            let footer_width = self
                .actions
                .as_slice()
                .iter()
                .map(|button| button.measured_size().width)
                .sum::<f32>()
                + (action_gap * self.actions.len().saturating_sub(1) as f32);
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
                x += size.width + action_gap;
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(ctx.bounds().origin.to_vector());

        if self.modal {
            ctx.fill_bounds(self.theme.surfaces.overlay_scrim);
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
            (self.focus_animation.value > AnimatedScalar::EPSILON).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
            ),
        );

        let title_style = self.title_style();
        let description_style = self.theme.placeholder_text_style();
        let padding = metrics.dialog_padding;
        let text_x = dialog.x() + padding.left;
        let text_y = dialog.y() + padding.top;
        let text_width = (dialog.width() - padding.left - padding.right).max(0.0);
        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height);
        let title_slot = Rect::new(text_x, text_y, text_width, title_height);
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        if let Some(description) = &self.description {
            let description_height = self
                .description_measurement
                .map(|measurement| measurement.height.max(description_style.line_height))
                .unwrap_or(description_style.line_height);
            let description_slot = Rect::new(
                text_x,
                title_slot.max_y() + metrics.dialog_description_gap,
                text_width,
                description_height,
            );
            ctx.push_clip_rect(description_slot);
            paint_aligned_text(
                ctx,
                description_slot,
                description,
                &description_style,
                description_style.line_height,
                0.0,
            );
            ctx.pop_clip();
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

    fn layer_properties(&self) -> LayerProperties {
        let translation = if self.modal {
            Vector::ZERO
        } else {
            Vector::new(
                0.0,
                self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value),
            )
        };
        LayerProperties::new(self.reveal.value, translation)
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        set_focus_animation_target(
            &mut self.focus_animation,
            focused as u8 as f32,
            &self.theme,
            ctx,
        );
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
    tone: SemanticTone,
    min_width: Option<f32>,
    height: Option<f32>,
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
            tone: SemanticTone::Accent,
            min_width: None,
            height: None,
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

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(1.0));
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

pub fn paint_progress_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    fraction: f32,
    tone: SemanticTone,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let metrics = theme.metrics;
    let palette = theme.palette;
    let (tone, _) = theme.semantic_tone_colors(tone);
    draw_control_shape(
        ctx,
        rect,
        metrics.corner_radius,
        physical_pixels(ctx, metrics.border_width).min(rect.height() * 0.5),
        palette.control,
        palette.border,
    );

    let fill = Rect::new(
        rect.x(),
        rect.y(),
        rect.width() * fraction.clamp(0.0, 1.0),
        rect.height(),
    );
    if fill.width() > 0.0 {
        ctx.fill(rounded_rect_path(fill, metrics.corner_radius), tone);
    }
}

impl Widget for ProgressBar {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let min_height = if let Some(height) = self.height {
            height
        } else if self.show_value {
            metrics
                .progress_bar_value_height
                .max(text_token_style(&theme, theme.text.sm, theme.palette.text).line_height)
        } else {
            metrics.progress_bar_height
        };
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(metrics.progress_bar_min_width),
            min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let (_, tone_text) = theme.semantic_tone_colors(self.tone);
        paint_progress_bar(ctx, ctx.bounds(), &theme, self.fraction(), self.tone);
        if self.show_value {
            let label = format!("{:.0}%", self.fraction() * 100.0);
            let text_style = numeric_text_style(text_token_style(&theme, theme.text.sm, tone_text));
            let label_padding = Insets {
                top: 0.0,
                bottom: 0.0,
                ..metrics.progress_bar_label_padding
            };
            let label_slot = inset_rect(ctx.bounds(), label_padding);
            ctx.push_clip_rect(label_slot);
            paint_aligned_text(
                ctx,
                label_slot,
                &label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
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
        let text_style = text_token_style(&theme, theme.text.sm, theme.palette.text);
        let label_measurement = self
            .label
            .as_ref()
            .map(|label| measure_text(ctx, label, &text_style));
        let label_width = label_measurement
            .map(|measurement| measurement.width + 12.0)
            .unwrap_or(0.0);
        let label_height = label_measurement
            .map(|measurement| measurement.height.max(text_style.line_height))
            .unwrap_or(0.0);
        constraints.clamp(Size::new(
            self.size + label_width,
            self.size.max(20.0).max(label_height),
        ))
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
            let text_style = text_token_style(&theme, theme.text.sm, palette.text);
            let text_slot = Rect::new(
                indicator.max_x() + 12.0,
                ctx.bounds().y(),
                ctx.bounds().width() - indicator.width() - 12.0,
                ctx.bounds().height(),
            );
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot,
                label,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();
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

fn text_token_style(theme: &DefaultTheme, token: ThemeTextToken, color: Color) -> TextStyle {
    TextStyle {
        font_size: token.size.max(1.0),
        line_height: token.line_height.max(1.0),
        color,
        ..theme.body_text_style()
    }
}

fn numeric_text_style(mut style: TextStyle) -> TextStyle {
    style.features.enable(FontFeature::TABULAR_FIGURES);
    style
}

fn numeric_text_style_if_numeric(text: &str, style: TextStyle) -> TextStyle {
    if text_contains_ascii_digit(text) {
        numeric_text_style(style)
    } else {
        style
    }
}

fn text_contains_ascii_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
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

fn tab_indicator_rect<F>(
    mut tab_rect: F,
    from_index: usize,
    selected_index: usize,
    progress: f32,
    padding: Insets,
    thickness: f32,
) -> Option<Rect>
where
    F: FnMut(usize) -> Option<Rect>,
{
    let to = tab_indicator_from_tab_rect(tab_rect(selected_index)?, padding, thickness);
    let from = tab_rect(from_index)
        .map(|rect| tab_indicator_from_tab_rect(rect, padding, thickness))
        .unwrap_or(to);
    Some(lerp_rect(from, to, progress))
}

fn tab_indicator_from_tab_rect(rect: Rect, padding: Insets, thickness: f32) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.max_y() - thickness,
        (rect.width() - padding.left - padding.right).max(0.0),
        thickness,
    )
}

fn tab_panel_transition_translation(
    from_index: usize,
    selected_index: usize,
    progress: f32,
    metrics: ControlMetrics,
) -> Vector {
    if from_index == selected_index {
        return Vector::ZERO;
    }

    let remaining = 1.0 - progress.clamp(0.0, 1.0);
    if remaining <= AnimatedScalar::EPSILON {
        return Vector::ZERO;
    }

    let direction = if selected_index > from_index {
        1.0
    } else {
        -1.0
    };
    Vector::new(direction * metrics.tab_panel_gap * remaining, 0.0)
}

fn lerp_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    let progress = progress.clamp(0.0, 1.0);
    Rect::new(
        f32::interpolate(from.x(), to.x(), progress),
        f32::interpolate(from.y(), to.y(), progress),
        f32::interpolate(from.width(), to.width(), progress),
        f32::interpolate(from.height(), to.height(), progress),
    )
}

fn tab_state_visuals(
    theme: &DefaultTheme,
    selected: bool,
    hovered: bool,
    pressed: bool,
    hover_amount: f32,
    press_amount: f32,
) -> Option<(Color, Color)> {
    let palette = theme.palette;
    let interaction = theme.interaction;
    if selected {
        Some((
            mix_color(
                palette.surface_raised,
                palette.accent,
                interaction.tab_selected_blend,
            ),
            palette.border.with_alpha(0.72),
        ))
    } else if pressed || press_amount > 0.0 {
        Some((
            mix_color(
                if hover_amount > 0.0 {
                    mix_color(
                        palette.control,
                        palette.control_hover,
                        interaction.hover_blend * hover_amount,
                    )
                } else {
                    palette.control
                },
                palette.control_active,
                interaction.pressed_blend * press_amount,
            ),
            palette.border_hover,
        ))
    } else if hovered || hover_amount > 0.0 {
        Some((
            mix_color(
                palette.control,
                palette.control_hover,
                interaction.hover_blend * hover_amount,
            ),
            palette.border_hover,
        ))
    } else {
        None
    }
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
    draw_control_shape(
        ctx,
        bounds,
        radius,
        physical_pixels(ctx, metrics.border_width),
        background,
        border,
    );

    if let Some(focus_ring) = focus_ring {
        draw_focus_ring_frame(ctx, bounds, radius, metrics, focus_ring);
    }
}

fn draw_focus_ring_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    focus_ring: Color,
) {
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
    crate::animation::Interpolate::interpolate(left, right, amount)
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

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeSet,
        rc::Rc,
    };

    use super::Tabs;
    use super::{
        ActionCard, ActionTilePaint, BrowserTabBar, CalloutPaint, CodePanelPaint, CodeTextLine,
        CodeTextPaint, CodeTextSpan, CommandButtonPaint, CommandGroup, ContextMenu, CoverageDots,
        DetailRow, Dialog, DisclosureButtonPaint, DockPanel, EmptyState, FieldGroup, FormRow,
        FormSection, HairlineEdge, Menu, MenuItem, PanelSection, PlacementBadge,
        PlacementBadgePaint, Popover, PresetStrip, ProgressBar, PropertyRow, PropertyRowLayout,
        SectionLabel, SectionLabelPaint, SectionPanelPaint, SegmentedControl, SegmentedControlItem,
        Spinner, StatusBadge, StatusBar, StatusBarHost, StatusBarSegment, TabBar, ToolPalette,
        ToolPaletteItem, Toolbar, paint_action_tile, paint_border, paint_callout, paint_code_lines,
        paint_code_panel, paint_command_button, paint_disclosure_button, paint_hairline,
        paint_placement_badge_with, paint_rounded_panel, paint_section_label,
        paint_section_label_detail, paint_section_panel, text_token_style,
    };
    use crate::FloatingStack;
    use crate::{DefaultTheme, HdrThemeMode, SemanticColorToken, SemanticTone, ThemeTextToken};
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue,
        Size, Vector, WidgetId, WindowEvent,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, ArrangeCtx, MeasureCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx, Widget,
        WindowBuilder,
    };
    use sui_scene::{
        Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor, SceneLayerUpdateKind,
    };
    use sui_text::{FontFeature, FontRegistry, FontWeight, TextSystem};

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

    fn render_isolated<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Unused")
                    .root(crate::Label::new("Unused")),
            )
            .window(WindowBuilder::new().title("Composites").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[1];
        runtime.render(window_id).unwrap()
    }

    #[test]
    fn density_modes_resize_menu_and_tabs() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(
                Menu::new("Actions")
                    .theme(compact)
                    .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")])
            )
            .frame
            .viewport
            .height
                < render(
                    Menu::new("Actions")
                        .theme(touch)
                        .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                TabBar::new("Tabs")
                    .theme(compact)
                    .tabs(["Canvas", "Inspector"])
            )
            .frame
            .viewport
            .height
                < render(
                    TabBar::new("Tabs")
                        .theme(touch)
                        .tabs(["Canvas", "Inspector"])
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                Tabs::new("Tabs")
                    .theme(compact)
                    .tab("Canvas", crate::Label::new("Canvas"))
                    .tab("Inspector", crate::Label::new("Inspector"))
            )
            .frame
            .viewport
            .height
                < render(
                    Tabs::new("Tabs")
                        .theme(touch)
                        .tab("Canvas", crate::Label::new("Canvas"))
                        .tab("Inspector", crate::Label::new("Inspector"))
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                SegmentedControl::new("View")
                    .theme(compact)
                    .segments(["All", "Chats", "Channels"])
            )
            .frame
            .viewport
            .height
                < render(
                    SegmentedControl::new("View")
                        .theme(touch)
                        .segments(["All", "Chats", "Channels"])
                )
                .frame
                .viewport
                .height
        );
    }

    #[test]
    fn segmented_control_click_updates_radio_semantics() -> Result<(), String> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new().width(260.0).height(28.0).with_child(
                SegmentedControl::new("Conversation view")
                    .items([
                        SegmentedControlItem::new("All 2").semantic_name("Show all conversations"),
                        SegmentedControlItem::new("Chats 1").semantic_name("Show chats only"),
                        SegmentedControlItem::new("Channels 1")
                            .semantic_name("Show channels only")
                            .description("1 visible conversation(s)"),
                    ])
                    .on_change(move |index, label| on_change.borrow_mut().push((index, label))),
            ),
        );

        let _ = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(220.0, 14.0), true),
            )
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, Point::new(220.0, 14.0), false),
            )
            .map_err(|error| error.to_string())?;

        assert_eq!(
            changes.borrow().as_slice(),
            &[(2, "Channels 1".to_string())]
        );
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let group = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioGroup)
            .expect("radio group semantics present");
        assert_eq!(group.name.as_deref(), Some("Conversation view"));
        assert_eq!(
            group.value,
            Some(SemanticsValue::Text("Channels 1".to_string()))
        );
        let channel = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::RadioButton
                    && node.name.as_deref() == Some("Show channels only")
            })
            .expect("selected segment semantics present");
        assert_eq!(channel.parent, Some(group.id));
        assert_eq!(
            channel.description.as_deref(),
            Some("1 visible conversation(s)")
        );
        assert!(channel.state.selected);
        assert_eq!(channel.state.checked, Some(sui_core::ToggleState::Checked));
        assert!(channel.actions.contains(&SemanticsAction::Activate));
        Ok(())
    }

    #[test]
    fn segmented_control_keyboard_changes_selection() -> Result<(), String> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SegmentedControl::new("Conversation view")
                .segments(["All", "Chats", "Channels"])
                .on_change_with_ctx(move |index, label, ctx| {
                    on_change.borrow_mut().push((index, label));
                    ctx.request_measure();
                    ctx.request_arrange();
                    ctx.request_paint();
                    ctx.request_semantics();
                }),
        );

        let _ = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;

        assert_eq!(changes.borrow().as_slice(), &[(1, "Chats".to_string())]);
        Ok(())
    }

    #[test]
    fn density_modes_resize_tool_command_widgets() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(
                Toolbar::horizontal()
                    .theme(compact)
                    .with_child(crate::Button::new("Undo"))
                    .with_child(crate::Button::new("Redo"))
            )
            .frame
            .viewport
            .height
                < render(
                    Toolbar::horizontal()
                        .theme(touch)
                        .with_child(crate::Button::new("Undo"))
                        .with_child(crate::Button::new("Redo"))
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                CommandGroup::horizontal("History")
                    .theme(compact)
                    .with_child(crate::Button::new("Undo"))
                    .with_child(crate::Button::new("Redo"))
            )
            .frame
            .viewport
            .height
                < render(
                    CommandGroup::horizontal("History")
                        .theme(touch)
                        .with_child(crate::Button::new("Undo"))
                        .with_child(crate::Button::new("Redo"))
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                ToolPalette::vertical("Tools")
                    .theme(compact)
                    .item(ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"))
                    .item(ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"))
            )
            .frame
            .viewport
            .width
                < render(
                    ToolPalette::vertical("Tools")
                        .theme(touch)
                        .item(ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"))
                        .item(ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"))
                )
                .frame
                .viewport
                .width
        );
        assert!(
            render(
                PresetStrip::new("Brush presets")
                    .theme(compact)
                    .presets(["8 px", "18 px", "36 px"])
            )
            .frame
            .viewport
            .height
                < render(
                    PresetStrip::new("Brush presets")
                        .theme(touch)
                        .presets(["8 px", "18 px", "36 px"])
                )
                .frame
                .viewport
                .height
        );
    }

    #[test]
    fn density_modes_resize_overlay_widgets() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        let compact_dialog = render(
            Dialog::new("Export", crate::Label::new("Export settings"))
                .theme(compact)
                .description("Choose file settings"),
        );
        let touch_dialog = render(
            Dialog::new("Export", crate::Label::new("Export settings"))
                .theme(touch)
                .description("Choose file settings"),
        );
        let compact_bounds = compact_dialog
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("compact dialog semantics present")
            .bounds;
        let touch_bounds = touch_dialog
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("touch dialog semantics present")
            .bounds;
        assert!(compact_bounds.width() < touch_bounds.width());
        assert!(compact_bounds.height() < touch_bounds.height());

        let (mut compact_popover, compact_window) = build_runtime(
            Popover::new(
                "Options",
                crate::Button::new("Open"),
                crate::Label::new("Popover body"),
            )
            .theme(compact),
        );
        let _ = compact_popover.render(compact_window).unwrap();
        compact_popover
            .handle_event(
                compact_window,
                primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
            )
            .unwrap();
        let compact_output = compact_popover.render(compact_window).unwrap();
        let compact_offset = overlay_layer_descriptor(&compact_output)
            .expect("compact popover overlay present")
            .properties
            .translation
            .y
            .abs();

        let (mut touch_popover, touch_window) = build_runtime(
            Popover::new(
                "Options",
                crate::Button::new("Open"),
                crate::Label::new("Popover body"),
            )
            .theme(touch),
        );
        let _ = touch_popover.render(touch_window).unwrap();
        touch_popover
            .handle_event(
                touch_window,
                primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
            )
            .unwrap();
        let touch_output = touch_popover.render(touch_window).unwrap();
        let touch_offset = overlay_layer_descriptor(&touch_output)
            .expect("touch popover overlay present")
            .properties
            .translation
            .y
            .abs();
        assert!(compact_offset < touch_offset);
    }

    #[test]
    fn dialog_title_and_description_visual_centers_match_header_slots() {
        let theme = DefaultTheme::default();
        let output = render(
            Dialog::new("Export", crate::Label::new("Export settings"))
                .theme(theme)
                .description("Choose file settings"),
        );
        let dialog = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present")
            .bounds;

        let title = text_run_for(&output, "Export");
        let title_layout = text_run_layout(&title);
        let title_line = title_layout
            .lines()
            .first()
            .expect("dialog title should contain one line");
        let title_visual_center = title.rect.y()
            + title_line.baseline
            + optical_visual_center(title_layout.measurement());

        let description = text_run_for(&output, "Choose file settings");
        let description_layout = text_run_layout(&description);
        let description_line = description_layout
            .lines()
            .first()
            .expect("dialog description should contain one line");
        let description_visual_center = description.rect.y()
            + description_line.baseline
            + optical_visual_center(description_layout.measurement());

        let metrics = theme.metrics;
        let padding = metrics.dialog_padding;
        let text_width = (dialog.width() - padding.left - padding.right).max(0.0);
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_height = description
            .style
            .line_height
            .max(description_layout.measurement().height);
        let title_slot = Rect::new(
            dialog.x() + padding.left,
            dialog.y() + padding.top,
            text_width,
            title_height,
        );
        let description_slot = Rect::new(
            dialog.x() + padding.left,
            title_slot.max_y() + metrics.dialog_description_gap,
            text_width,
            description_height,
        );

        assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
        assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
    }

    #[test]
    fn dialog_header_text_preserves_tall_measurements_in_compact_line_boxes() {
        let mut theme = DefaultTheme::default();
        theme.metrics.dialog_title_font_size = 32.0;
        theme.metrics.dialog_title_line_height = 12.0;
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 10.0;

        let output = render(
            Dialog::new("Export", crate::Label::new("Export settings"))
                .theme(theme)
                .description("Choose file settings"),
        );
        let dialog = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present")
            .bounds;
        let title = text_run_for(&output, "Export");
        let title_layout = TextSystem::new()
            .shape_text_run(&title, &FontRegistry::new())
            .expect("dialog title should shape");
        let description = text_run_for(&output, "Choose file settings");
        let description_layout = TextSystem::new()
            .shape_text_run(&description, &FontRegistry::new())
            .expect("dialog description should shape");
        let metrics = theme.metrics;
        let padding = metrics.dialog_padding;
        let text_width = (dialog.width() - padding.left - padding.right).max(0.0);
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_height = description
            .style
            .line_height
            .max(description_layout.measurement().height);
        let title_slot = Rect::new(
            dialog.x() + padding.left,
            dialog.y() + padding.top,
            text_width,
            title_height,
        );
        let description_slot = Rect::new(
            dialog.x() + padding.left,
            title_slot.max_y() + metrics.dialog_description_gap,
            text_width,
            description_height,
        );

        assert!(title.rect.height() >= title_layout.measurement().height - 0.01);
        assert!(title.rect.height() > title.style.line_height);
        assert!(description.rect.height() >= description_layout.measurement().height - 0.01);
        assert!(description.rect.height() > description.style.line_height);
        assert_eq!(description.style.color, theme.palette.placeholder);
        assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
        assert!(
            (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
                < 0.75
        );
    }

    #[test]
    fn density_modes_resize_composite_status_widgets() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(
                ActionCard::new("Paint", "Pixel canvas workspace")
                    .theme(compact)
                    .icon(crate::IconGlyph::Brush)
            )
            .frame
            .viewport
            .height
                < render(
                    ActionCard::new("Paint", "Pixel canvas workspace")
                        .theme(touch)
                        .icon(crate::IconGlyph::Brush)
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                StatusBar::new()
                    .theme(compact)
                    .text_segment("Ready")
                    .text_segment("100%")
            )
            .frame
            .viewport
            .height
                < render(
                    StatusBar::new()
                        .theme(touch)
                        .text_segment("Ready")
                        .text_segment("100%")
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                StatusBadge::new("Synced")
                    .theme(compact)
                    .tone(SemanticTone::Success)
                    .icon(crate::IconGlyph::Storage)
            )
            .frame
            .viewport
            .height
                < render(
                    StatusBadge::new("Synced")
                        .theme(touch)
                        .tone(SemanticTone::Success)
                        .icon(crate::IconGlyph::Storage)
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                CoverageDots::new("Replicas", 2, 3)
                    .theme(compact)
                    .tone(SemanticTone::Success)
            )
            .frame
            .viewport
            .height
                < render(
                    CoverageDots::new("Replicas", 2, 3)
                        .theme(touch)
                        .tone(SemanticTone::Success)
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(
                ProgressBar::new("Export progress")
                    .theme(compact)
                    .value(0.42)
            )
            .frame
            .viewport
            .height
                < render(ProgressBar::new("Export progress").theme(touch).value(0.42))
                    .frame
                    .viewport
                    .height
        );
    }

    #[test]
    fn composite_focus_rings_use_theme_motion() -> Result<(), String> {
        assert_focus_ring_uses_theme_motion(
            crate::SizedBox::new()
                .size(Size::new(112.0, 44.0))
                .with_child(
                    ToolPalette::horizontal("Tools")
                        .items([
                            ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush"),
                            ToolPaletteItem::new(crate::IconGlyph::Eraser, "Erase"),
                        ])
                        .selected(0),
                ),
            Point::new(18.0, 18.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            crate::SizedBox::new()
                .size(Size::new(260.0, 92.0))
                .with_child(ActionCard::new("Paint", "Pixel canvas workspace")),
            Point::new(18.0, 18.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            crate::SizedBox::new()
                .size(Size::new(240.0, 40.0))
                .with_child(PresetStrip::new("Brush presets").presets(["8 px", "18 px"])),
            Point::new(24.0, 18.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            crate::SizedBox::new()
                .size(Size::new(260.0, 92.0))
                .with_child(
                    PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                        .collapsible(true)
                        .collapsed(),
                ),
            Point::new(24.0, 18.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            Menu::new("App menu").items([MenuItem::new("New File"), MenuItem::new("Open...")]),
            Point::new(24.0, 24.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .activation_button(PointerButton::Primary)
                .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
            Point::new(24.0, 24.0),
        )?;

        assert_focus_ring_uses_theme_motion(
            crate::SizedBox::new()
                .size(Size::new(640.0, 420.0))
                .with_child(Dialog::new(
                    "Confirm",
                    crate::Label::new("Apply the change?"),
                )),
            Point::new(320.0, 210.0),
        )
    }

    #[test]
    fn density_modes_resize_form_and_panel_widgets() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(PropertyRow::new("Opacity", crate::Slider::new("Opacity")).theme(compact))
                .frame
                .viewport
                .height
                < render(PropertyRow::new("Opacity", crate::Slider::new("Opacity")).theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(
                FieldGroup::new()
                    .theme(compact)
                    .with_child(crate::Label::new("First"))
                    .with_child(crate::Label::new("Second"))
            )
            .frame
            .viewport
            .height
                < render(
                    FieldGroup::new()
                        .theme(touch)
                        .with_child(crate::Label::new("First"))
                        .with_child(crate::Label::new("Second"))
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(FormSection::new("Providers", crate::Label::new("Configured")).theme(compact))
                .frame
                .viewport
                .height
                < render(
                    FormSection::new("Providers", crate::Label::new("Configured")).theme(touch)
                )
                .frame
                .viewport
                .height
        );
        assert!(
            render(PanelSection::new("Brush", crate::Label::new("Opacity")).theme(compact))
                .frame
                .viewport
                .height
                < render(PanelSection::new("Brush", crate::Label::new("Opacity")).theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(
                DockPanel::new("Tool properties", crate::Label::new("Brush size")).theme(compact)
            )
            .frame
            .viewport
            .height
                < render(
                    DockPanel::new("Tool properties", crate::Label::new("Brush size")).theme(touch)
                )
                .frame
                .viewport
                .height
        );
    }

    #[test]
    fn detail_row_wraps_metadata_and_exposes_value_semantics() {
        let theme = DefaultTheme::default();
        let value = "replicated across atlas, keep, and wren with one pending repair";
        let narrow = render(
            crate::SizedBox::new()
                .width(140.0)
                .with_child(DetailRow::new("Placement", value).theme(theme)),
        );
        let wide = render(
            crate::SizedBox::new()
                .width(360.0)
                .with_child(DetailRow::new("Placement", value).theme(theme)),
        );

        assert!(
            narrow.frame.viewport.height > wide.frame.viewport.height,
            "narrow detail rows should wrap their metadata value"
        );
        let label = text_run_for(&wide, "PLACEMENT");
        assert_eq!(label.style.color, theme.palette.text_muted);
        let node = wide
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Placement"))
            .expect("detail row semantics should exist");
        assert_eq!(node.value, Some(SemanticsValue::Text(value.to_string())));
    }

    struct DetailRowHeightProbe {
        theme: DefaultTheme,
        measured: Rc<Cell<f32>>,
        painted: Rc<Cell<f32>>,
    }

    impl Widget for DetailRowHeightProbe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(140.0, 96.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let value = "replicated across atlas, keep, and wren with one pending repair";
            self.measured.set(super::detail_row_height_for_value(
                ctx,
                &self.theme,
                ctx.bounds().width(),
                value,
                Some(2),
            ));
            self.painted.set(super::paint_detail_row_at(
                ctx,
                &self.theme,
                Point::new(ctx.bounds().x(), ctx.bounds().y()),
                ctx.bounds().width(),
                "Placement",
                value,
                Some(2),
            ));
        }
    }

    #[test]
    fn detail_row_height_helper_matches_painter() {
        let measured = Rc::new(Cell::new(0.0));
        let painted = Rc::new(Cell::new(0.0));
        let _ = render(DetailRowHeightProbe {
            theme: DefaultTheme::default(),
            measured: Rc::clone(&measured),
            painted: Rc::clone(&painted),
        });

        assert!(measured.get() > 0.0);
        assert!((measured.get() - painted.get()).abs() < 0.01);
    }

    #[test]
    fn section_label_uses_micro_label_token_and_text_semantics() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new().width(120.0).height(18.0).with_child(
                SectionLabel::new("file tasks")
                    .semantic_name("File tasks")
                    .theme(theme),
            ),
        );

        let label = text_run_for(&output, "FILE TASKS");
        assert_text_run_uses_token(&label, theme.text.xs);
        assert_eq!(label.style.color, theme.surfaces.text_faint);
        assert_eq!(label.style.weight, FontWeight::SEMIBOLD);

        let node = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("File tasks"))
            .expect("section label semantics should exist");
        assert_eq!(node.role, SemanticsRole::Text);
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("File tasks".to_string()))
        );
    }

    struct SectionLabelPaintFixture {
        theme: DefaultTheme,
        color: Color,
    }

    impl Widget for SectionLabelPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 18.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_section_label(
                ctx,
                &self.theme,
                ctx.bounds(),
                "placement",
                SectionLabelPaint::new().color(self.color),
            );
        }
    }

    #[test]
    fn section_label_paint_matches_widget_style_with_color_override() {
        let theme = DefaultTheme::default();
        let color = theme.palette.warning;
        let output = render(SectionLabelPaintFixture { theme, color });

        let label = text_run_for(&output, "PLACEMENT");
        assert_text_run_uses_token(&label, theme.text.xs);
        assert_eq!(label.style.color, color);
        assert_eq!(label.style.weight, FontWeight::SEMIBOLD);
    }

    struct SectionLabelDetailPaintFixture {
        theme: DefaultTheme,
        color: Color,
    }

    impl Widget for SectionLabelDetailPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 18.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_section_label_detail(
                ctx,
                &self.theme,
                ctx.bounds(),
                "input",
                "command payload",
                SectionLabelPaint::new().color(self.color),
            );
        }
    }

    #[test]
    fn section_label_detail_paint_preserves_detail_text() {
        let theme = DefaultTheme::default();
        let color = theme.palette.warning;
        let output = render(SectionLabelDetailPaintFixture { theme, color });

        let label = text_run_for(&output, "INPUT · command payload");
        assert_text_run_uses_token(&label, theme.text.xs);
        assert_eq!(label.style.color, color);
        assert_eq!(label.style.weight, FontWeight::SEMIBOLD);
    }

    struct GenericPaintPrimitiveFixture {
        fill: Color,
        panel_border: Color,
        right_hairline: Color,
        full_border: Color,
    }

    impl Widget for GenericPaintPrimitiveFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(96.0, 64.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_rounded_panel(
                ctx,
                Rect::new(4.0, 4.0, 36.0, 20.0),
                self.fill,
                self.panel_border,
                0.0,
            );
            paint_hairline(
                ctx,
                Rect::new(50.0, 6.0, 24.0, 18.0),
                HairlineEdge::Right,
                self.right_hairline,
            );
            paint_border(ctx, Rect::new(10.0, 34.0, 18.0, 12.0), self.full_border);
        }
    }

    #[test]
    fn generic_paint_primitives_emit_expected_rects() {
        let fill = Color::rgba(0.1, 0.2, 0.3, 1.0);
        let panel_border = Color::rgba(0.4, 0.5, 0.6, 1.0);
        let right_hairline = Color::rgba(0.7, 0.2, 0.1, 1.0);
        let full_border = Color::rgba(0.2, 0.8, 0.4, 1.0);
        let output = render(GenericPaintPrimitiveFixture {
            fill,
            panel_border,
            right_hairline,
            full_border,
        });

        assert_eq!(
            solid_fill_rects_for_color(&output, fill),
            vec![Rect::new(5.0, 5.0, 34.0, 18.0)]
        );
        assert_eq!(
            solid_fill_rects_for_color(&output, panel_border),
            vec![Rect::new(4.0, 4.0, 36.0, 20.0)]
        );
        assert_eq!(
            solid_fill_rects_for_color(&output, right_hairline),
            vec![Rect::new(73.0, 6.0, 1.0, 18.0)]
        );
        assert_eq!(
            solid_fill_rects_for_color(&output, full_border),
            vec![
                Rect::new(10.0, 34.0, 18.0, 1.0),
                Rect::new(27.0, 34.0, 1.0, 12.0),
                Rect::new(10.0, 45.0, 18.0, 1.0),
                Rect::new(10.0, 34.0, 1.0, 12.0),
            ]
        );
    }

    #[test]
    fn semantic_tones_drive_composite_status_colors() {
        let theme = DefaultTheme::default();

        let action_card = render(
            ActionCard::new("Deploy", "Publish release artifacts")
                .theme(theme)
                .tone(SemanticTone::Success),
        );
        assert!(solid_fill_colors(&action_card).contains(&theme.palette.success.with_alpha(0.78)));

        let status_bar = render(
            StatusBar::new()
                .theme(theme)
                .segment(StatusBarSegment::new("Offline").tone(SemanticTone::Warning)),
        );
        assert!(solid_fill_colors(&status_bar).contains(&theme.palette.warning.with_alpha(0.12)));

        let badge = render(
            StatusBadge::new("Replicated")
                .theme(theme)
                .tone(SemanticTone::Success)
                .icon(crate::IconGlyph::Storage),
        );
        // Mesh badges fill with the soft status wash and draw no border.
        assert!(solid_fill_colors(&badge).contains(&theme.palette.success_soft));
        assert!(!solid_stroke_colors(&badge).contains(&theme.palette.success.with_alpha(0.52)));

        let progress_bar = render(
            ProgressBar::new("Delete progress")
                .theme(theme)
                .tone(SemanticTone::Danger)
                .value(0.5),
        );
        assert!(solid_fill_colors(&progress_bar).contains(&theme.palette.danger));
    }

    struct CommandButtonPaintFixture {
        theme: DefaultTheme,
        style: CommandButtonPaint,
    }

    impl Widget for CommandButtonPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(150.0, 28.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_command_button(
                ctx,
                &self.theme,
                ctx.bounds(),
                "Repair",
                Some(crate::IconGlyph::Storage),
                self.style,
            );
        }
    }

    #[test]
    fn command_button_paint_uses_theme_surfaces_and_tones() {
        let theme = DefaultTheme::default();

        let neutral = render(CommandButtonPaintFixture {
            theme,
            style: CommandButtonPaint::neutral().icon_tone(SemanticTone::Accent),
        });
        assert!(solid_fill_colors(&neutral).contains(&theme.surfaces.field));
        assert!(solid_stroke_colors(&neutral).contains(&theme.palette.border));

        let hovered = render(CommandButtonPaintFixture {
            theme,
            style: CommandButtonPaint::neutral()
                .icon_tone(SemanticTone::Accent)
                .hovered(true),
        });
        assert!(solid_fill_colors(&hovered).contains(&theme.palette.control_hover));

        let accent = render(CommandButtonPaintFixture {
            theme,
            style: CommandButtonPaint::filled(SemanticTone::Accent),
        });
        assert!(solid_fill_colors(&accent).contains(&theme.palette.accent));
        assert!(solid_stroke_colors(&accent).contains(&theme.palette.accent));

        let pressed = render(CommandButtonPaintFixture {
            theme,
            style: CommandButtonPaint::filled(SemanticTone::Accent).pressed(true),
        });
        assert!(solid_fill_colors(&pressed).contains(&theme.palette.accent_pressed));

        let danger = render(CommandButtonPaintFixture {
            theme,
            style: CommandButtonPaint::tonal(SemanticTone::Danger),
        });
        assert!(solid_fill_colors(&danger).contains(&theme.surfaces.field));
        assert!(solid_stroke_colors(&danger).contains(&theme.palette.danger.with_alpha(0.72)));
    }

    struct DisclosureButtonPaintFixture {
        theme: DefaultTheme,
        expanded: bool,
    }

    impl Widget for DisclosureButtonPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(126.0, 24.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_disclosure_button(
                ctx,
                &self.theme,
                ctx.bounds(),
                if self.expanded {
                    "Show less"
                } else {
                    "Show more"
                },
                self.expanded,
                DisclosureButtonPaint::new(),
            );
        }
    }

    #[test]
    fn disclosure_button_paint_uses_accent_command_button_style() {
        let theme = DefaultTheme::default();
        let collapsed = render(DisclosureButtonPaintFixture {
            theme,
            expanded: false,
        });
        let expanded = render(DisclosureButtonPaintFixture {
            theme,
            expanded: true,
        });

        assert!(solid_fill_colors(&collapsed).contains(&theme.surfaces.field));
        assert!(solid_stroke_colors(&collapsed).contains(&theme.palette.accent.with_alpha(0.72)));
        assert_eq!(
            text_run_for(&collapsed, "Show more").style.color,
            theme.palette.accent
        );
        assert_eq!(
            text_run_for(&expanded, "Show less").style.color,
            theme.palette.accent
        );
    }

    struct CompactCommandButtonPaintFixture {
        theme: DefaultTheme,
    }

    impl Widget for CompactCommandButtonPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(200.0, 24.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let mut natural_style =
                text_token_style(&self.theme, self.theme.text.sm, self.theme.palette.accent);
            natural_style.weight = FontWeight::SEMIBOLD;
            let natural_width = ctx
                .measure_text("Hide details", natural_style)
                .expect("command button label should be measurable")
                .width;
            let target_label_width = natural_width * 0.95;
            // Compact command buttons reserve 8px leading padding, a 12px icon,
            // a 6px gap, and 6px trailing padding around the label slot.
            let button_rect = Rect::new(
                ctx.bounds().x(),
                ctx.bounds().y(),
                target_label_width + 32.0,
                ctx.bounds().height(),
            );
            paint_command_button(
                ctx,
                &self.theme,
                button_rect,
                "Hide details",
                Some(crate::IconGlyph::ChevronUp),
                CommandButtonPaint::tonal(SemanticTone::Accent).icon_tone(SemanticTone::Accent),
            );
        }
    }

    #[test]
    fn command_button_paint_scales_label_in_compact_slots() {
        let theme = DefaultTheme::default();
        let output = render(CompactCommandButtonPaintFixture { theme });
        let label = text_run_for(&output, "Hide details");
        let layout = text_layout_for(&output, "Hide details");

        assert!(
            label.style.font_size < theme.text.sm.size,
            "compact command button should reduce label font size"
        );
        assert!(label.style.font_size >= 10.0);
        assert_eq!(layout.lines().len(), 1, "label should not wrap");
        assert!(
            layout.measurement().width <= layout.box_size().width + 0.01,
            "compact command button label should fit its slot: measurement={:?} box={:?}",
            layout.measurement(),
            layout.box_size()
        );
    }

    struct ActionTilePaintFixture {
        theme: DefaultTheme,
        style: ActionTilePaint,
    }

    impl Widget for ActionTilePaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(178.0, 58.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_action_tile(
                ctx,
                &self.theme,
                ctx.bounds(),
                "Check cluster",
                Some("background-agent"),
                Some(crate::IconGlyph::Storage),
                self.style,
            );
        }
    }

    struct ActionTileReservedSlotFixture {
        theme: DefaultTheme,
        style: ActionTilePaint,
    }

    impl Widget for ActionTileReservedSlotFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(178.0, 58.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_action_tile(
                ctx,
                &self.theme,
                ctx.bounds(),
                "Check cluster",
                Some("background-agent"),
                None,
                self.style,
            );
        }
    }

    #[test]
    fn action_tile_paint_supports_highlight_hover_and_press_states() {
        let theme = DefaultTheme::default();

        let neutral = render(ActionTilePaintFixture {
            theme,
            style: ActionTilePaint::neutral(),
        });
        assert!(solid_fill_colors(&neutral).contains(&theme.palette.control));
        assert!(solid_stroke_colors(&neutral).contains(&theme.palette.border));

        let highlighted = render(ActionTilePaintFixture {
            theme,
            style: ActionTilePaint::tonal(SemanticTone::Accent),
        });
        assert!(solid_stroke_colors(&highlighted).contains(&theme.palette.accent.with_alpha(0.84)));

        let hovered = render(ActionTilePaintFixture {
            theme,
            style: ActionTilePaint::neutral().hovered(true),
        });
        assert!(solid_fill_colors(&hovered).contains(&theme.palette.control_hover));

        let pressed = render(ActionTilePaintFixture {
            theme,
            style: ActionTilePaint::neutral().pressed(true),
        });
        assert!(solid_fill_colors(&pressed).contains(&theme.palette.control_active));
    }

    #[test]
    fn action_tile_paint_supports_surface_overrides_and_reserved_slots() {
        let theme = DefaultTheme::default();
        let output = render(ActionTileReservedSlotFixture {
            theme,
            style: ActionTilePaint::neutral()
                .background(theme.surfaces.panel)
                .border(theme.palette.warning)
                .title_color(theme.palette.text)
                .subtitle_color(theme.surfaces.text_faint)
                .icon_color(theme.palette.accent)
                .radius(theme.radius.xl)
                .padding_x(12.0)
                .leading_width(18.0)
                .trailing_width(48.0),
        });

        assert!(solid_fill_colors(&output).contains(&theme.surfaces.panel));
        assert!(solid_stroke_colors(&output).contains(&theme.palette.warning));

        let title = text_run_for(&output, "Check cluster");
        let subtitle = text_run_for(&output, "background-agent");
        assert_eq!(title.style.color, theme.palette.text);
        assert_eq!(subtitle.style.color, theme.surfaces.text_faint);
        assert!(
            title.rect.x() >= 30.0,
            "reserved leading slot should move title after the status slot: {:?}",
            title.rect
        );
        let title_clip = clip_rect_for_text(&output, "Check cluster");
        assert!(
            title_clip.max_x() <= 118.0,
            "reserved trailing slot should keep title clip clear of action area: {:?}",
            title_clip
        );
    }

    #[test]
    fn action_tile_paint_supports_leading_semantic_status_dot() {
        let theme = DefaultTheme::default();
        let output = render(ActionTileReservedSlotFixture {
            theme,
            style: ActionTilePaint::neutral()
                .padding_x(12.0)
                .leading_tone_dot(SemanticTone::Success)
                .leading_width(18.0),
        });

        assert!(
            solid_fill_colors(&output).contains(&theme.semantic_tone_color(SemanticTone::Success)),
            "leading status dot should use semantic tone color"
        );
        let title = text_run_for(&output, "Check cluster");
        assert!(
            title.rect.x() >= 30.0,
            "leading status dot should reserve the standard leading slot: {:?}",
            title.rect
        );
    }

    struct CalloutPaintFixture {
        theme: DefaultTheme,
    }

    impl Widget for CalloutPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(260.0, 104.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_callout(
                ctx,
                &self.theme,
                ctx.bounds(),
                Some(crate::IconGlyph::Alert),
                Some("Conflict"),
                "notes.md changed on two devices while offline. Resolve before syncing.",
                CalloutPaint::new(SemanticTone::Warning).reserved_bottom(24.0),
            );
        }
    }

    #[test]
    fn callout_paint_uses_tone_rail_wrapped_text_and_reserved_bottom() {
        let theme = DefaultTheme::default();
        let (tone, _) = theme.semantic_tone_colors(SemanticTone::Warning);
        let (tone_soft, _) = theme.semantic_tone_soft_colors(SemanticTone::Warning);
        let output = render(CalloutPaintFixture { theme });

        assert!(solid_fill_colors(&output).contains(&tone_soft));
        assert!(solid_fill_colors(&output).contains(&tone));
        assert!(solid_stroke_colors(&output).contains(&theme.palette.border));

        let title = text_run_for(&output, "Conflict");
        assert_text_run_uses_token(&title, theme.text.sm);
        assert_eq!(title.style.color, theme.palette.text);
        assert_eq!(title.style.weight, FontWeight::SEMIBOLD);

        let body_text = "notes.md changed on two devices while offline. Resolve before syncing.";
        let body = text_run_for(&output, body_text);
        assert_text_run_uses_token(&body, theme.text.sm);
        assert_eq!(body.style.color, theme.palette.text_muted);
        let body_clip = clip_rect_for_text(&output, body_text);
        assert!(body_clip.height() <= 104.0 - 20.0 - 24.0 + 0.01);
    }

    struct CodePanelPaintFixture {
        theme: DefaultTheme,
        style: CodePanelPaint,
        content: Rc<Cell<Rect>>,
    }

    impl Widget for CodePanelPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 84.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let content = paint_code_panel(ctx, &self.theme, ctx.bounds(), "rust", self.style);
            self.content.set(content);
            ctx.fill_rect(content, self.theme.palette.accent.with_alpha(0.08));
        }
    }

    #[test]
    fn code_panel_paint_uses_compact_header_and_returns_content_rect() {
        let theme = DefaultTheme::default();
        let content = Rc::new(Cell::new(Rect::ZERO));
        let output = render(CodePanelPaintFixture {
            theme,
            style: CodePanelPaint::new(),
            content: Rc::clone(&content),
        });

        assert!(solid_fill_colors(&output).contains(&theme.surfaces.field));
        assert!(solid_fill_colors(&output).contains(&theme.surfaces.titlebar));
        assert!(solid_stroke_colors(&output).contains(&theme.surfaces.border));

        let label = text_run_for(&output, "rust");
        assert_text_run_uses_token(&label, theme.text.xs);
        assert_eq!(label.style.color, theme.surfaces.text_faint);
        assert_eq!(label.style.weight, FontWeight::SEMIBOLD);

        let content = content.get();
        assert!((content.x() - 8.0).abs() < 0.01);
        assert!((content.y() - 30.0).abs() < 0.01);
        assert!((content.width() - 164.0).abs() < 0.01);
        assert!(content.height() > 0.0);
    }

    struct CodeLinesPaintFixture {
        theme: DefaultTheme,
    }

    impl Widget for CodeLinesPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 46.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let first_spans = [
                CodeTextSpan::new("let").color(self.theme.palette.accent),
                CodeTextSpan::new(" value"),
            ];
            let second_spans = [CodeTextSpan::new("+ added").color(self.theme.palette.success)];
            let lines = [
                CodeTextLine::new(&first_spans),
                CodeTextLine::new(&second_spans)
                    .background(self.theme.palette.success.with_alpha(0.12)),
            ];
            paint_code_lines(
                ctx,
                &self.theme,
                ctx.bounds(),
                &lines,
                CodeTextPaint::new()
                    .color(self.theme.palette.text)
                    .line_height(18.0),
            );
        }
    }

    #[test]
    fn code_lines_paint_supports_span_colors_and_line_backgrounds() {
        let theme = DefaultTheme::default();
        let output = render(CodeLinesPaintFixture { theme });

        assert!(
            solid_fill_colors(&output).contains(&theme.palette.success.with_alpha(0.12)),
            "line background should be drawn before text"
        );
        let keyword = text_run_for(&output, "let");
        assert_eq!(keyword.style.color, theme.palette.accent);
        let fallback = text_run_for(&output, " value");
        assert_eq!(fallback.style.color, theme.palette.text);
        let added = text_run_for(&output, "+ added");
        assert_eq!(added.style.color, theme.palette.success);
    }

    struct SectionPanelPaintFixture {
        theme: DefaultTheme,
        content: Rc<Cell<Rect>>,
        title: Rc<Cell<Rect>>,
    }

    impl Widget for SectionPanelPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(220.0, 90.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let geometry = paint_section_panel(
                ctx,
                &self.theme,
                ctx.bounds(),
                "Metadata",
                SectionPanelPaint::new()
                    .header_height(38.0)
                    .title_token(self.theme.text.base)
                    .trailing_width(42.0),
            );
            self.content.set(geometry.content_rect);
            self.title.set(geometry.title_rect);
            ctx.fill_rect(
                geometry.content_rect,
                self.theme.palette.accent.with_alpha(0.08),
            );
        }
    }

    #[test]
    fn section_panel_paint_reserves_header_action_space_and_returns_content_rect() {
        let theme = DefaultTheme::default();
        let content = Rc::new(Cell::new(Rect::ZERO));
        let title_rect = Rc::new(Cell::new(Rect::ZERO));
        let output = render(SectionPanelPaintFixture {
            theme,
            content: Rc::clone(&content),
            title: Rc::clone(&title_rect),
        });

        assert!(solid_fill_colors(&output).contains(&theme.surfaces.panel));
        assert!(solid_stroke_colors(&output).contains(&theme.surfaces.border));

        let title = text_run_for(&output, "Metadata");
        assert_text_run_uses_token(&title, theme.text.base);
        assert_eq!(title.style.color, theme.surfaces.text);
        assert_eq!(title.style.weight, FontWeight::SEMIBOLD);

        let title_rect = title_rect.get();
        assert!((title_rect.x() - 12.0).abs() < 0.01);
        assert!((title_rect.width() - 154.0).abs() < 0.01);

        let content = content.get();
        assert!((content.x() - 12.0).abs() < 0.01);
        assert!((content.y() - 38.0).abs() < 0.01);
        assert!((content.width() - 196.0).abs() < 0.01);
        assert!(content.height() > 0.0);
    }

    #[test]
    fn placement_badge_combines_status_and_replica_coverage() {
        let theme = DefaultTheme::default();
        let output = render(
            PlacementBadge::new("synced")
                .theme(theme)
                .icon(crate::IconGlyph::Storage)
                .tone(SemanticTone::Success)
                .coverage(2, 3)
                .min_width(136.0),
        );

        assert!(solid_fill_colors(&output).contains(&theme.palette.success_soft));
        assert!(
            solid_fill_colors(&output)
                .iter()
                .filter(|color| **color == theme.palette.success)
                .count()
                >= 2,
            "coverage dots should paint filled replica dots"
        );
        let node = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("synced"))
            .expect("placement badge semantics should exist");
        assert_eq!(
            node.description.as_deref(),
            Some("2 of 3 replicas available")
        );
    }

    struct PlacementBadgePaintFixture {
        theme: DefaultTheme,
    }

    impl Widget for PlacementBadgePaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(156.0, 34.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_placement_badge_with(
                ctx,
                &self.theme,
                ctx.bounds(),
                "synced",
                Some(crate::IconGlyph::Storage),
                SemanticTone::Success,
                Some((2, 3)),
                PlacementBadgePaint::new().padding(8.0, 5.0, 8.0, 5.0),
            );
        }
    }

    #[test]
    fn placement_badge_paint_applies_cell_padding() {
        let theme = DefaultTheme::default();
        let output = render(PlacementBadgePaintFixture { theme });

        let status = text_run_for(&output, "synced");
        assert!(
            status.rect.x() >= 8.0,
            "status label should stay inside left padding: {:?}",
            status.rect
        );
        assert!(
            status.rect.y() >= 5.0,
            "status label should stay inside top padding: {:?}",
            status.rect
        );
        assert!(
            status.rect.max_x() <= 148.0,
            "status label should stay inside right padding: {:?}",
            status.rect
        );

        let coverage = text_run_for(&output, "2/3");
        assert!(
            coverage.rect.max_x() <= 148.0,
            "coverage label should stay inside right padding: {:?}",
            coverage.rect
        );
        assert!(
            solid_fill_colors(&output).contains(&theme.palette.success_soft),
            "padded placement badge should still paint the status fill"
        );
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
    fn action_card_text_visual_centers_match_title_and_description_slots() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 104.0))
                .with_child(
                    ActionCard::new("Paint", "Pixel canvas workspace")
                        .theme(theme)
                        .icon(crate::IconGlyph::Brush),
                ),
        );
        let card = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Paint")
            })
            .expect("action card should expose button semantics");

        let title = text_run_for(&output, "Paint");
        let title_layout = text_run_layout(&title);
        let title_line = title_layout
            .lines()
            .first()
            .expect("action card title should contain one line");
        let title_visual_center = title.rect.y()
            + title_line.baseline
            + optical_visual_center(title_layout.measurement());

        let description = text_run_for(&output, "Pixel canvas workspace");
        let description_layout = text_run_layout(&description);
        let description_line = description_layout
            .lines()
            .first()
            .expect("action card description should contain one line");
        let description_visual_center = description.rect.y()
            + description_line.baseline
            + optical_visual_center(description_layout.measurement());

        let metrics = theme.metrics;
        let content = super::inset_rect(card.bounds, metrics.action_card_padding);
        let icon_extent = metrics.action_card_icon_box_size + metrics.action_card_icon_gap;
        let text_bounds = Rect::new(
            content.x() + icon_extent,
            content.y(),
            (content.width() - icon_extent - metrics.action_card_trailing_gap).max(0.0),
            content.height(),
        );
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_height =
            (text_bounds.height() - title_height - metrics.action_card_text_gap)
                .max(description.style.line_height)
                .min(description.style.line_height * 2.0);
        let text_block_height = title_height + metrics.action_card_text_gap + description_height;
        let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
        let description_slot = Rect::new(
            text_bounds.x(),
            title_slot.max_y() + metrics.action_card_text_gap,
            text_bounds.width(),
            description_height,
        );

        assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
        assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
    }

    #[test]
    fn action_card_multiline_description_stays_inside_clip_slot() {
        let description = "Catalog of controls, containers, media, and text surfaces.";
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(360.0, 104.0))
                .with_child(
                    ActionCard::new("Widget book", description)
                        .icon(crate::IconGlyph::MoreHorizontal),
                ),
        );
        let run = text_run_for(&output, description);
        let clip = clip_rect_for_text(&output, description);
        let layout = TextSystem::new()
            .shape_text_run(&run, &FontRegistry::new())
            .expect("action card description should shape");

        assert!(
            layout.lines().len() > 1,
            "test description should wrap in the dev picker-sized card"
        );
        assert!(
            run.rect.y() >= clip.y() - 0.01,
            "description should start inside its clip: rect={:?}, clip={:?}",
            run.rect,
            clip
        );
        assert!(
            run.rect.max_y() <= clip.max_y() + 0.01,
            "description should end inside its clip: rect={:?}, clip={:?}",
            run.rect,
            clip
        );
    }

    #[test]
    fn action_card_text_preserves_tall_measurements_in_compact_line_boxes() {
        let mut theme = DefaultTheme::default();
        theme.text.base = ThemeTextToken {
            size: 32.0,
            line_height: 12.0,
        };
        theme.text.sm = ThemeTextToken {
            size: 32.0,
            line_height: 12.0,
        };
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(360.0, 148.0))
                .with_child(
                    ActionCard::new("Paint", "Glyph box")
                        .theme(theme)
                        .icon(crate::IconGlyph::Brush),
                ),
        );
        let card = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Paint")
            })
            .expect("action card should expose button semantics");
        let description = text_run_for(&output, "Glyph box");
        let description_layout = TextSystem::new()
            .shape_text_run(&description, &FontRegistry::new())
            .expect("action card description should shape");
        let title = text_run_for(&output, "Paint");
        let title_layout = TextSystem::new()
            .shape_text_run(&title, &FontRegistry::new())
            .expect("action card title should shape");
        let metrics = theme.metrics;
        let content = super::inset_rect(card.bounds, metrics.action_card_padding);
        let icon_extent = metrics.action_card_icon_box_size + metrics.action_card_icon_gap;
        let text_bounds = Rect::new(
            content.x() + icon_extent,
            content.y(),
            (content.width() - icon_extent - metrics.action_card_trailing_gap).max(0.0),
            content.height(),
        );
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_min_height = description
            .style
            .line_height
            .max(description_layout.measurement().height);
        let description_height =
            (text_bounds.height() - title_height - metrics.action_card_text_gap)
                .max(description_min_height)
                .min((description.style.line_height * 2.0).max(description_min_height));
        let text_block_height = title_height + metrics.action_card_text_gap + description_height;
        let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
        let description_slot = Rect::new(
            text_bounds.x(),
            text_y + title_height + metrics.action_card_text_gap,
            text_bounds.width(),
            description_height,
        );

        assert_text_run_uses_token(&title, theme.text.base);
        assert!(
            title.rect.height() >= title_layout.measurement().height - 0.01,
            "action card title rect should preserve measured glyph height: rect={:?}, measurement={:?}",
            title.rect,
            title_layout.measurement()
        );
        assert!(
            title.rect.height() > title.style.line_height,
            "test theme should exercise a title measurement taller than line-height"
        );
        assert!(
            (text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75,
            "title text should remain optically centered in its slot"
        );
        assert_text_run_uses_token(&description, theme.text.sm);
        assert!(
            description.rect.height() >= description_layout.measurement().height - 0.01,
            "action card description rect should preserve measured glyph height: rect={:?}, measurement={:?}",
            description.rect,
            description_layout.measurement()
        );
        assert!(
            description.rect.height() > description.style.line_height * 2.0,
            "test theme should exercise measured-height preservation beyond the old two-line cap"
        );
        assert!(
            (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
                < 0.75,
            "description text should remain optically centered in its slot"
        );
        assert!(
            description.rect.y() >= text_bounds.y(),
            "description should stay inside action card text bounds"
        );
        assert!(
            description.rect.max_y() <= text_bounds.max_y() + 0.75,
            "description should stay inside action card text bounds"
        );
    }

    #[test]
    fn composite_default_text_styles_follow_theme_text_tokens() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 11.0,
            line_height: 15.0,
        };
        theme.text.sm = ThemeTextToken {
            size: 15.0,
            line_height: 23.0,
        };
        theme.text.base = ThemeTextToken {
            size: 17.0,
            line_height: 25.0,
        };
        theme.sync_derived_fields();

        let action_card = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 112.0))
                .with_child(ActionCard::new("Token action", "Token action detail").theme(theme)),
        );
        assert_text_run_uses_token(&text_run_for(&action_card, "Token action"), theme.text.base);
        assert_text_run_uses_token(
            &text_run_for(&action_card, "Token action detail"),
            theme.text.sm,
        );

        let property_row = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 64.0))
                .with_child(
                    PropertyRow::new("Token property", crate::Button::new("Edit"))
                        .theme(theme)
                        .inline(),
                ),
        );
        assert_text_run_uses_token(
            &text_run_for(&property_row, "Token property"),
            theme.text.sm,
        );

        let form_section = render(
            crate::SizedBox::new()
                .size(Size::new(360.0, 140.0))
                .with_child(
                    FormSection::new("Token section", crate::Button::new("Apply"))
                        .theme(theme)
                        .description("Token section detail"),
                ),
        );
        assert_text_run_uses_token(&text_run_for(&form_section, "Token section"), theme.text.sm);
        assert_text_run_uses_token(
            &text_run_for(&form_section, "Token section detail"),
            theme.text.xs,
        );

        let preset_strip = render(
            crate::SizedBox::new()
                .size(Size::new(240.0, 44.0))
                .with_child(
                    PresetStrip::new("Brush")
                        .theme(theme)
                        .preset("Token preset"),
                ),
        );
        assert_text_run_uses_token(&text_run_for(&preset_strip, "Token preset"), theme.text.xs);

        let status_bar = render(
            crate::SizedBox::new()
                .size(Size::new(240.0, 32.0))
                .with_child(StatusBar::new().theme(theme).text_segment("Token status")),
        );
        assert_text_run_uses_token(&text_run_for(&status_bar, "Token status"), theme.text.xs);
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
    fn preset_strip_label_clips_to_padded_item_slot() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(220.0, 40.0))
                .with_child(
                    PresetStrip::new("Brush presets")
                        .item_width(180.0)
                        .preset("Soft"),
                ),
        );
        let preset = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Soft"))
            .expect("preset button semantics should exist");
        let text = text_run_for(&output, "Soft");
        let clip = clip_rect_for_text(&output, "Soft");
        let expected_clip =
            super::inset_rect(preset.bounds, theme.metrics.preset_strip_label_padding);

        assert!(
            clip.width() > text.rect.width(),
            "clip should cover the padded item slot rather than the measured text rect"
        );
        assert!((clip.x() - expected_clip.x()).abs() < 0.75);
        assert!((clip.y() - expected_clip.y()).abs() < 0.75);
        assert!((clip.width() - expected_clip.width()).abs() < 0.75);
        assert!((clip.height() - expected_clip.height()).abs() < 0.75);
    }

    #[test]
    fn preset_strip_label_preserves_tall_measurements_and_item_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 28.0,
            line_height: 12.0,
        };
        theme.sync_derived_fields();

        let output = render_isolated(
            crate::SizedBox::new()
                .size(Size::new(240.0, 56.0))
                .with_child(
                    PresetStrip::new("Brush presets")
                        .theme(theme)
                        .item_height(56.0)
                        .item_width(180.0)
                        .preset("Soft"),
                ),
        );
        let preset = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Soft"))
            .expect("preset button semantics should exist");
        let text = text_run_for(&output, "Soft");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("preset label should shape");

        assert_text_run_uses_token(&text, theme.text.xs);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!(
            (text_visual_center_for(&output, "Soft") - super::rect_center(preset.bounds).y).abs()
                < 0.75
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
    fn preset_strip_hover_and_press_use_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.surface,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(220.0, 32.0))
                .with_child(
                    PresetStrip::new("Brush presets")
                        .theme(theme)
                        .presets(["8 px", "18 px", "36 px"]),
                ),
        );
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let preset = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("18 px")
            })
            .expect("target preset button should exist");
        let position = super::rect_center(preset.bounds);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    #[test]
    fn status_bar_exposes_dynamic_segment_semantics() {
        let theme = DefaultTheme::default();
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
        assert_eq!(status.bounds.height(), theme.metrics.status_bar_height);
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Zoom 35%")
        }));
    }

    #[test]
    fn status_bar_description_is_exposed_to_semantics() {
        let summary = Rc::new(RefCell::new(
            "SIFS online | Path /services | idle".to_string(),
        ));
        let summary_reader = Rc::clone(&summary);
        let output = render(
            StatusBar::new()
                .name("Files status")
                .description_when(move || summary_reader.borrow().clone())
                .segment(StatusBarSegment::new("Path /services").expand(true)),
        );

        let status = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Files status")
            })
            .expect("status bar container semantics should exist");
        assert_eq!(
            status.value,
            Some(SemanticsValue::Text(
                "SIFS online | Path /services | idle".to_string(),
            ))
        );
        assert_eq!(
            status.description.as_deref(),
            Some("SIFS online | Path /services | idle")
        );
    }

    #[test]
    fn status_badge_publishes_text_semantics_and_theme_token_text() {
        let theme = DefaultTheme::default();
        let output = render(
            StatusBadge::new("Primary on atlas")
                .theme(theme)
                .tone(SemanticTone::Accent)
                .icon(crate::IconGlyph::Storage),
        );
        let node = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Primary on atlas")
            })
            .expect("status badge should publish text semantics");
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("Primary on atlas".to_string()))
        );
        assert_text_run_uses_token(&text_run_for(&output, "Primary on atlas"), theme.text.xs);
    }

    #[test]
    fn coverage_dots_publish_replica_like_coverage_semantics_and_token_text() {
        let theme = DefaultTheme::default();
        let output = render(
            CoverageDots::new("Replicas", 2, 3)
                .theme(theme)
                .tone(SemanticTone::Warning),
        );
        let node = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Replicas")
            })
            .expect("coverage dots should publish text semantics");
        assert_eq!(node.value, Some(SemanticsValue::Text("2/3".to_string())));
        assert_eq!(node.description.as_deref(), Some("2 of 3 covered"));
        assert_text_run_uses_token(&text_run_for(&output, "2/3"), theme.text.xs);
        assert!(contains_approx_color(
            &solid_fill_colors(&output),
            theme.semantic_tone_colors(SemanticTone::Warning).0,
        ));
    }

    #[test]
    fn status_bar_sizes_segments_from_measured_text() {
        let theme = DefaultTheme::default();
        let text = "Layer Paint / Normal / 100% / Unlocked";
        let output = render(
            StatusBar::new()
                .name("Editor status")
                .segment(StatusBarSegment::new(text))
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
        let measured_width = text_layout_for(&output, text).measurement().width;
        let expected_width = (measured_width + theme.metrics.status_bar_segment_padding * 2.0)
            .ceil()
            .max(theme.metrics.status_bar_segment_min_width);
        assert!(
            (layer.bounds.width() - expected_width).abs() < 0.01,
            "expected status segment width {expected_width} from text measurement, got {:?}",
            layer.bounds
        );
    }

    #[test]
    fn status_bar_numeric_segments_use_tabular_figures_without_forcing_plain_labels() {
        let output = render(
            StatusBar::new()
                .name("Editor status")
                .segment(StatusBarSegment::new("Ready").min_width(80.0))
                .segment(StatusBarSegment::new("Zoom 35%").min_width(120.0)),
        );

        let ready = text_run_for(&output, "Ready");
        let zoom = text_run_for(&output, "Zoom 35%");

        assert!(
            !ready
                .style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!(
            zoom.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
    }

    #[test]
    fn status_bar_segment_text_visual_center_matches_segment_center() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(220.0, 40.0))
                .with_child(
                    StatusBar::new()
                        .height(40.0)
                        .segment(StatusBarSegment::new("Ready").min_width(96.0)),
                ),
        );
        let text = text_run_for(&output, "Ready");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("status segment text should shape");
        let line = layout
            .lines()
            .first()
            .expect("status segment text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let segment = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Ready"))
            .expect("status segment semantics should exist");
        let segment_center = segment.bounds.y() + (segment.bounds.height() * 0.5);

        assert!((actual_visual_center - segment_center).abs() < 0.75);
    }

    #[test]
    fn status_bar_segments_preserve_tall_measurements_and_numeric_features() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.metrics.status_bar_height = 52.0;
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(360.0, 52.0))
                .with_child(
                    StatusBar::new()
                        .theme(theme)
                        .height(52.0)
                        .segment(StatusBarSegment::new("Ready").min_width(120.0))
                        .segment(StatusBarSegment::new("Zoom 35%").min_width(140.0)),
                ),
        );
        for label in ["Ready", "Zoom 35%"] {
            let text = text_run_for(&output, label);
            let layout = TextSystem::new()
                .shape_text_run(&text, &FontRegistry::new())
                .expect("status segment text should shape");
            let segment = output
                .semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Text && node.name.as_deref() == Some(label)
                })
                .expect("status segment semantics should exist");
            let segment_center = segment.bounds.y() + (segment.bounds.height() * 0.5);

            assert_text_run_uses_token(&text, theme.text.xs);
            assert!(text.rect.height() >= layout.measurement().height - 0.01);
            assert!(text.rect.height() > text.style.line_height);
            assert!((text_run_visual_center(&text) - segment_center).abs() < 0.75);
        }

        let ready = text_run_for(&output, "Ready");
        let zoom = text_run_for(&output, "Zoom 35%");
        assert!(
            !ready
                .style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!(
            zoom.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
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
        let theme = DefaultTheme::default();
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
        let button_size = 28.0_f32.max(theme.metrics.icon_button_size);
        let expected_width = button_size * 2.0
            + theme.metrics.command_group_spacing
            + theme.metrics.command_group_padding.left
            + theme.metrics.command_group_padding.right;
        let expected_height = button_size
            + theme.metrics.command_group_padding.top
            + theme.metrics.command_group_padding.bottom;
        assert!(
            (group.bounds.width() - expected_width).abs() < 0.01,
            "expected command group width {expected_width}, got {}",
            group.bounds.width()
        );
        assert!(
            (group.bounds.height() - expected_height).abs() < 0.01,
            "expected command group height {expected_height}, got {}",
            group.bounds.height()
        );

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
    fn tool_palette_hover_and_press_use_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.surface,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(64.0, 180.0))
                .with_child(ToolPalette::vertical("Paint tools").theme(theme).items([
                    ToolPaletteItem::new(crate::IconGlyph::Brush, "Brush tool"),
                    ToolPaletteItem::new(crate::IconGlyph::Eraser, "Eraser tool"),
                    ToolPaletteItem::new(crate::IconGlyph::PaintBucket, "Fill tool"),
                ])),
        );
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let eraser = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
            })
            .expect("eraser tool button semantics should exist");
        let position = super::rect_center(eraser.bounds);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "tool hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "tool press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

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
        let theme = DefaultTheme::default();
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
        let expected_width = 320.0 - 96.0 - theme.metrics.property_row_inline_gap;
        assert!((control.bounds.width() - expected_width).abs() < 0.01);
    }

    #[test]
    fn property_row_inline_label_visual_center_matches_row_center() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 36.0))
                .with_child(
                    PropertyRow::new("Opacity", crate::Slider::new("Opacity"))
                        .layout(PropertyRowLayout::Inline)
                        .label_width(96.0),
                ),
        );
        let text = text_run_for(&output, "Opacity");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("property row label should shape");
        let line = layout
            .lines()
            .first()
            .expect("property row label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let row_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    #[test]
    fn property_row_numeric_control_aligns_value_to_control_edge() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 36.0))
                .with_child(
                    PropertyRow::new(
                        "Brush size",
                        crate::NumberInput::new("Brush size")
                            .precision(0)
                            .value(128.0),
                    )
                    .layout(PropertyRowLayout::Inline)
                    .label_width(96.0)
                    .control_width(120.0),
                ),
        );
        let value = text_run_for(&output, "128");
        let label = text_run_for(&output, "Brush size");
        let control = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
            })
            .expect("number input semantics should exist");
        let expected_right = control.bounds.max_x()
            - theme.metrics.number_input_stepper_width
            - theme.metrics.text_input_padding.right;

        assert!(
            value
                .style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((value.rect.max_x() - expected_right).abs() < 1.0);
        assert!(
            (text_run_visual_center(&value) - (control.bounds.y() + control.bounds.height() * 0.5))
                .abs()
                < 0.75,
            "property row numeric value should remain optically centered in the control"
        );
        assert!(
            (text_run_visual_center(&value) - text_run_visual_center(&label)).abs() < 0.75,
            "property row label and numeric value should share a visual baseline"
        );
    }

    #[test]
    fn property_row_inline_label_preserves_tall_metrics_with_numeric_control() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();
        theme.metrics.min_height = 56.0;
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(380.0, 64.0))
                .with_child(
                    PropertyRow::new(
                        "Brush size",
                        crate::NumberInput::new("Brush size")
                            .theme(theme)
                            .precision(0)
                            .value(128.0),
                    )
                    .theme(theme)
                    .layout(PropertyRowLayout::Inline)
                    .label_width(132.0)
                    .control_width(150.0),
                ),
        );
        let label = text_run_for(&output, "Brush size");
        let label_layout = TextSystem::new()
            .shape_text_run(&label, &FontRegistry::new())
            .expect("property row label should shape");
        let value = text_run_for(&output, "128");
        let row_center = output.frame.viewport.height * 0.5;
        let control = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Brush size")
            })
            .expect("number input semantics should exist");
        let expected_right = control.bounds.max_x()
            - theme.metrics.number_input_stepper_width
            - theme.metrics.text_input_padding.right;

        assert_text_run_uses_token(&label, theme.text.sm);
        assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
        assert!(label.rect.height() > label.style.line_height);
        assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
        assert!((value.rect.max_x() - expected_right).abs() < 1.0);
        assert!(
            (text_run_visual_center(&value) - text_run_visual_center(&label)).abs() < 0.75,
            "property row label and numeric value should share a visual baseline for tall metrics"
        );
    }

    #[test]
    fn property_row_label_id_is_javascript_safe() {
        let id = super::property_row_label_id(WidgetId::new(402)).get();

        assert!(id < (1_u64 << 53));
    }

    #[test]
    fn form_section_bounds_grouped_rows_and_exposes_semantics() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(900.0, 180.0))
                .with_child(
                    FormSection::new(
                        "Providers",
                        FieldGroup::new()
                            .with_child(FormRow::new("API key", crate::Label::new("Configured")))
                            .with_child(FormRow::new(
                                "Default model",
                                crate::Label::new("Provider default"),
                            )),
                    )
                    .description("Credentials and model defaults"),
                ),
        );

        let theme = DefaultTheme::default();
        assert!(
            solid_fill_colors(&output).contains(&theme.surfaces.panel),
            "form section card fill should use the surface panel token"
        );

        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Providers")
            })
            .expect("form section semantics should exist");
        let padding = theme.metrics.form_section_padding;
        assert!(
            section.bounds.width()
                <= theme.metrics.form_section_max_width + padding.left + padding.right
        );
        assert!(
            section.bounds.x() > 100.0,
            "wide parent should center a max-width form section"
        );
        assert_eq!(
            section.description.as_deref(),
            Some("Credentials and model defaults")
        );
        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Default model"))
        );
    }

    #[test]
    fn form_section_header_text_block_centers_against_tall_header_action() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(420.0, 140.0))
                .with_child(
                    FormSection::new("Providers", crate::Label::new("Configured"))
                        .theme(theme)
                        .description("Credentials and defaults")
                        .header_action(
                            crate::SizedBox::new()
                                .size(Size::new(76.0, 52.0))
                                .with_child(crate::Label::new("Sync")),
                        ),
                ),
        );
        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Providers")
            })
            .expect("form section semantics should exist");

        let title = text_run_for(&output, "Providers");
        let title_layout = text_run_layout(&title);
        let title_line = title_layout
            .lines()
            .first()
            .expect("form section title should contain one line");
        let title_visual_center = title.rect.y()
            + title_line.baseline
            + optical_visual_center(title_layout.measurement());

        let description = text_run_for(&output, "Credentials and defaults");
        let description_layout = text_run_layout(&description);
        let description_line = description_layout
            .lines()
            .first()
            .expect("form section description should contain one line");
        let description_visual_center = description.rect.y()
            + description_line.baseline
            + optical_visual_center(description_layout.measurement());

        let metrics = theme.metrics;
        let content = super::inset_rect(section.bounds, metrics.form_section_padding);
        let header_gap = metrics.form_section_header_gap;
        let action_width = (76.0 + header_gap).min(content.width());
        let text_width = (content.width() - action_width).max(0.0);
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_height = description
            .style
            .line_height
            .max(description_layout.measurement().height);
        let text_block_height =
            title_height + metrics.form_section_description_gap + description_height;
        let header_height = text_block_height.max(52.0);
        let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
        let description_slot = Rect::new(
            content.x(),
            title_slot.max_y() + metrics.form_section_description_gap,
            text_width,
            description_height,
        );

        assert!((title_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
        assert!((description_visual_center - super::rect_center(description_slot).y).abs() < 0.75);
    }

    #[test]
    fn form_section_header_text_preserves_tall_measurements_in_compact_line_boxes() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 30.0,
            line_height: 10.0,
        };
        theme.text.xs = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();

        let output = render(
            crate::SizedBox::new()
                .size(Size::new(460.0, 190.0))
                .with_child(
                    FormSection::new("Providers", crate::Label::new("Configured"))
                        .theme(theme)
                        .description("Credentials and defaults")
                        .header_action(
                            crate::SizedBox::new()
                                .size(Size::new(76.0, 52.0))
                                .with_child(crate::Label::new("Sync")),
                        ),
                ),
        );
        let section = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Providers")
            })
            .expect("form section semantics should exist");
        let title = text_run_for(&output, "Providers");
        let title_layout = TextSystem::new()
            .shape_text_run(&title, &FontRegistry::new())
            .expect("form section title should shape");
        let description = text_run_for(&output, "Credentials and defaults");
        let description_layout = TextSystem::new()
            .shape_text_run(&description, &FontRegistry::new())
            .expect("form section description should shape");
        let metrics = theme.metrics;
        let content = super::inset_rect(section.bounds, metrics.form_section_padding);
        let action_width = (76.0 + metrics.form_section_header_gap).min(content.width());
        let text_width = (content.width() - action_width).max(0.0);
        let title_height = title
            .style
            .line_height
            .max(title_layout.measurement().height);
        let description_height = description
            .style
            .line_height
            .max(description_layout.measurement().height);
        let text_block_height =
            title_height + metrics.form_section_description_gap + description_height;
        let header_height = text_block_height.max(52.0);
        let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
        let description_slot = Rect::new(
            content.x(),
            title_slot.max_y() + metrics.form_section_description_gap,
            text_width,
            description_height,
        );

        assert_text_run_uses_token(&title, theme.text.sm);
        assert_text_run_uses_token(&description, theme.text.xs);
        assert!(title.rect.height() >= title_layout.measurement().height - 0.01);
        assert!(title.rect.height() > title.style.line_height);
        assert!(description.rect.height() >= description_layout.measurement().height - 0.01);
        assert!(description.rect.height() > description.style.line_height);
        assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
        assert!(
            (text_run_visual_center(&description) - super::rect_center(description_slot).y).abs()
                < 0.75
        );
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
    fn panel_section_title_visual_center_matches_title_slot_center() {
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
        let title_slot = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(section.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layers")
            })
            .expect("panel section title semantics should exist")
            .bounds;
        let text = text_run_for(&output, "Layers");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("panel section title should shape");
        let line = layout
            .lines()
            .first()
            .expect("panel section title should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());

        assert!((actual_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
    }

    #[test]
    fn panel_section_title_preserves_tall_measurement_and_header_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 30.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();

        let output = render(
            crate::SizedBox::new()
                .size(Size::new(280.0, 120.0))
                .with_child(
                    PanelSection::new("Layers", crate::Label::new("Paint"))
                        .theme(theme)
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
        let title_slot = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(section.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layers")
            })
            .expect("panel section title semantics should exist")
            .bounds;
        let title = text_run_for(&output, "Layers");
        let layout = TextSystem::new()
            .shape_text_run(&title, &FontRegistry::new())
            .expect("panel section title should shape");

        assert_text_run_uses_token(&title, theme.text.xs);
        assert!(title.rect.height() >= layout.measurement().height - 0.01);
        assert!(title.rect.height() > title.style.line_height);
        assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
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
    fn collapsible_panel_section_header_motion_uses_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = theme
            .palette
            .accent
            .with_alpha((theme.interaction.hover_blend * 0.07).min(0.08));
        let expected_press = theme
            .palette
            .accent
            .with_alpha((theme.interaction.selected_blend * 0.48).min(0.14));
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(240.0, 120.0))
                .with_child(
                    PanelSection::new("Advanced color", crate::Label::new("RGB sliders"))
                        .theme(theme)
                        .collapsible(true)
                        .collapsed(),
                ),
        );
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let title = text_run_for(&output, "Advanced color");
        let position = super::rect_center(title.rect);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "panel header hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "panel header press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

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

        let theme = DefaultTheme::default();
        assert!(title.bounds.max_y() <= theme.metrics.dock_panel_header_height);
        assert!(
            child.bounds.y()
                >= theme.metrics.dock_panel_header_height + theme.metrics.dock_panel_padding.top
        );
    }

    #[test]
    fn empty_state_exposes_content_and_action_semantics() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(320.0, 220.0))
                .with_child(
                    EmptyState::new("Empty directory", "Nothing here yet.")
                        .theme(theme.clone())
                        .icon(crate::IconGlyph::Folder)
                        .detail("No files matched the active filter.")
                        .action(crate::Button::new("Create file").theme(theme.clone())),
                ),
        );

        let state = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Empty directory")
            })
            .expect("empty-state semantics should exist");
        assert_eq!(
            state.description.as_deref(),
            Some("Nothing here yet. No files matched the active filter.")
        );
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Create file")
        }));
        assert_eq!(
            text_run_for(&output, "Empty directory").style.color,
            theme.surfaces.text_muted
        );
        assert_eq!(
            text_run_for(&output, "Nothing here yet.").style.color,
            theme.surfaces.text_faint
        );
        assert_eq!(
            text_run_for(&output, "No files matched the active filter.")
                .style
                .color,
            theme.surfaces.text_muted
        );
    }

    #[test]
    fn dock_panel_title_visual_center_matches_header_title_slot_center() {
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
        let title_slot = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(panel.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Tool properties")
            })
            .expect("dock panel title semantics should exist")
            .bounds;
        let text = text_run_for(&output, "Tool properties");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("dock panel title should shape");
        let line = layout
            .lines()
            .first()
            .expect("dock panel title should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());

        assert!((actual_visual_center - super::rect_center(title_slot).y).abs() < 0.75);
    }

    #[test]
    fn dock_panel_title_preserves_tall_measurement_and_header_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 30.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();
        theme.metrics.dock_panel_header_height = 52.0;

        let output = render(
            crate::SizedBox::new()
                .size(Size::new(300.0, 180.0))
                .with_child(
                    DockPanel::new("Tool properties", crate::Label::new("Brush size"))
                        .theme(theme)
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
        let title_slot = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(panel.id)
                    && node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Tool properties")
            })
            .expect("dock panel title semantics should exist")
            .bounds;
        let title = text_run_for(&output, "Tool properties");
        let layout = TextSystem::new()
            .shape_text_run(&title, &FontRegistry::new())
            .expect("dock panel title should shape");

        assert_text_run_uses_token(&title, theme.text.sm);
        assert!(title.rect.height() >= layout.measurement().height - 0.01);
        assert!(title.rect.height() > title.style.line_height);
        assert!((text_run_visual_center(&title) - super::rect_center(title_slot).y).abs() < 0.75);
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

        let theme = DefaultTheme::default();
        assert_eq!(
            status.bounds,
            Rect::new(
                0.0,
                160.0 - theme.metrics.status_bar_height,
                320.0,
                theme.metrics.status_bar_height,
            )
        );
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

    fn text_run_from_shaped(
        output: &RenderOutput,
        run: &sui_text::ShapedText,
    ) -> Option<sui_text::TextRun> {
        run.resolve(output.frame.text_layout_registry.as_ref())
            .map(|layout| {
                let mut style = layout.style().clone();
                if let Some(color) = run.color_override {
                    style.color = color;
                }
                sui_text::TextRun {
                    rect: shaped_text_run_rect(run.origin, layout),
                    text: layout.text().to_string(),
                    style,
                }
            })
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

    fn first_text_run(output: &RenderOutput) -> sui_text::TextRun {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                sui_scene::SceneCommand::DrawText(text) => Some(text.clone()),
                sui_scene::SceneCommand::DrawShapedText(text) => text_run_from_shaped(output, text),
                _ => None,
            })
            .expect("text draw command present")
    }

    fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            found = match command {
                sui_scene::SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
                sui_scene::SceneCommand::DrawShapedText(run) => {
                    text_run_from_shaped(output, run).filter(|resolved| resolved.text == text)
                }
                _ => None,
            };
        });
        found.expect("text draw command present")
    }

    fn text_layout_for(output: &RenderOutput, text: &str) -> sui_text::TextLayout {
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            if let sui_scene::SceneCommand::DrawShapedText(run) = command
                && let Some(layout) = run.resolve(output.frame.text_layout_registry.as_ref())
                && layout.text() == text
            {
                found = Some(layout.clone());
            }
        });
        found.expect("shaped text layout present")
    }

    fn slow_normal_motion_theme() -> DefaultTheme {
        let mut theme = DefaultTheme::default();
        theme.motion.duration_fast = 0.0;
        theme.motion.duration_normal = 0.6;
        theme
    }

    fn text_transform_dx_for(output: &RenderOutput, text: &str) -> Option<f32> {
        fn find_in_commands(
            output: &RenderOutput,
            text: &str,
            commands: &[SceneCommand],
            inherited_dx: f32,
            stack: &mut Vec<f32>,
        ) -> Option<f32> {
            let mut current_dx = inherited_dx;
            for command in commands {
                match command {
                    SceneCommand::PushTransform { transform } => {
                        stack.push(current_dx);
                        current_dx += transform.dx;
                    }
                    SceneCommand::PopTransform => {
                        current_dx = stack.pop().unwrap_or(inherited_dx);
                    }
                    SceneCommand::DrawText(run) if run.text == text => {
                        return Some(current_dx);
                    }
                    SceneCommand::DrawShapedText(run) => {
                        if run
                            .resolve(output.frame.text_layout_registry.as_ref())
                            .is_some_and(|layout| layout.text() == text)
                        {
                            return Some(current_dx);
                        }
                    }
                    SceneCommand::Layer(layer) => {
                        if let Some(dx) = find_in_commands(
                            output,
                            text,
                            layer.scene.commands(),
                            current_dx + layer.descriptor.properties.translation.x,
                            stack,
                        ) {
                            return Some(dx);
                        }
                    }
                    _ => {}
                }
            }

            None
        }

        find_in_commands(
            output,
            text,
            output.frame.scene.commands(),
            0.0,
            &mut Vec::new(),
        )
    }

    fn clip_rect_for_text(output: &RenderOutput, text: &str) -> Rect {
        let mut stack = Vec::new();
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            match command {
                sui_scene::SceneCommand::PushClip { rect } => stack.push(*rect),
                sui_scene::SceneCommand::PopClip => {
                    stack.pop();
                }
                sui_scene::SceneCommand::DrawText(run) if run.text == text => {
                    found = stack.last().copied();
                }
                sui_scene::SceneCommand::DrawShapedText(run)
                    if run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .is_some_and(|layout| layout.text() == text) =>
                {
                    found = stack.last().copied();
                }
                _ => {}
            }
        });
        found.expect("text draw command should have an active clip")
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
    }

    fn text_run_layout(run: &sui_text::TextRun) -> sui_text::TextLayout {
        TextSystem::new()
            .shape_text(
                run.text.clone(),
                Size::new(f32::INFINITY, run.rect.height().max(1.0)),
                run.style.clone(),
                &FontRegistry::new(),
            )
            .expect("text run should shape")
    }

    fn text_run_visual_center(run: &sui_text::TextRun) -> f32 {
        let layout = text_run_layout(run);
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
                sui_scene::SceneCommand::DrawText(run) if run.text == text => {
                    Some(text_run_visual_center(run))
                }
                sui_scene::SceneCommand::DrawShapedText(run) => {
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

    fn assert_focus_ring_uses_theme_motion<W>(root: W, position: Point) -> Result<(), String>
    where
        W: Widget + 'static,
    {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;
        let _ = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;

        runtime.tick(focus_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid), theme.palette.focus_ring),
            "focus ring should not snap to the settled focus color"
        );

        runtime.tick(focus_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            contains_approx_color(&solid_stroke_colors(&settled), theme.palette.focus_ring),
            "focus ring should settle to the theme focus color"
        );

        Ok(())
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

    fn solid_fill_rects_for_color(output: &RenderOutput, expected: Color) -> Vec<Rect> {
        let mut rects = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    rect,
                    brush: Brush::Solid(color),
                } if *color == expected => rects.push(*rect),
                _ => {}
            });
        rects
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

    fn contains_approx_color(colors: &[Color], expected: Color) -> bool {
        const CHANNEL_TOLERANCE: f32 = 1.0 / 255.0;

        colors.iter().any(|color| {
            color.space == expected.space
                && (color.red - expected.red).abs() <= CHANNEL_TOLERANCE
                && (color.green - expected.green).abs() <= CHANNEL_TOLERANCE
                && (color.blue - expected.blue).abs() <= CHANNEL_TOLERANCE
                && (color.alpha - expected.alpha).abs() <= CHANNEL_TOLERANCE
        })
    }

    fn non_hit_test_layer_descriptors(output: &RenderOutput) -> Vec<SceneLayerDescriptor> {
        let mut descriptors = Vec::new();
        output.frame.scene.visit_layers(&mut |layer| {
            if !layer.descriptor.hit_test {
                descriptors.push(layer.descriptor.clone());
            }
        });
        descriptors
    }

    fn non_hit_test_layer_owners(output: &RenderOutput) -> Vec<WidgetId> {
        let mut owners = Vec::new();
        output.frame.scene.visit_layers(&mut |layer| {
            if !layer.descriptor.hit_test {
                owners.push(layer.widget_id());
            }
        });
        owners
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
    fn selected_tab_chrome_uses_interaction_token() {
        let mut theme = DefaultTheme::default();
        theme.interaction.tab_selected_blend = 0.31;
        let selected_fill = super::mix_color(
            theme.palette.surface_raised,
            theme.palette.accent,
            theme.interaction.tab_selected_blend,
        );

        let tab_bar = render_isolated(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected(1),
        );
        assert!(solid_fill_colors(&tab_bar).contains(&selected_fill));
        assert!(
            !solid_stroke_colors(&tab_bar).contains(&theme.palette.border_focus),
            "selected tab bar chrome should not use the focus border color"
        );
        assert!(
            !solid_stroke_colors(&tab_bar).contains(&theme.palette.focus_ring),
            "unfocused selected tab bar chrome should not paint a focus ring"
        );

        let tabs = render_isolated(
            Tabs::new("Main tabs")
                .theme(theme)
                .selected(1)
                .tab("Design", crate::Label::new("Design"))
                .tab("Inspect", crate::Label::new("Inspect")),
        );
        assert!(solid_fill_colors(&tabs).contains(&selected_fill));
        assert!(
            !solid_stroke_colors(&tabs).contains(&theme.palette.border_focus),
            "selected tabs chrome should not use the focus border color"
        );
        assert!(
            !solid_stroke_colors(&tabs).contains(&theme.palette.focus_ring),
            "unfocused selected tabs chrome should not paint a focus ring"
        );
    }

    #[test]
    fn tab_widgets_focus_highlights_selected_tab_button() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let tab_bar_point = Point::new(
            theme.metrics.tab_min_width + theme.metrics.tab_gap + 8.0,
            18.0,
        );
        let (mut tab_bar_runtime, tab_bar_window) = build_runtime(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected(1),
        );
        let _ = tab_bar_runtime
            .render(tab_bar_window)
            .map_err(|error| error.to_string())?;
        tab_bar_runtime
            .handle_event(
                tab_bar_window,
                primary_pointer(PointerEventKind::Down, tab_bar_point, true),
            )
            .map_err(|error| error.to_string())?;
        tab_bar_runtime.tick(focus_duration + 0.01);
        assert!(handle_ready_events(&mut tab_bar_runtime)? >= 1);
        let tab_bar_focused = tab_bar_runtime
            .render(tab_bar_window)
            .map_err(|error| error.to_string())?;
        let tab_bar_strokes = solid_stroke_colors(&tab_bar_focused);
        assert!(
            contains_approx_color(&tab_bar_strokes, theme.palette.focus_ring),
            "focused selected tab button should paint a focus ring; strokes={tab_bar_strokes:?}"
        );
        assert!(
            !contains_approx_color(&tab_bar_strokes, theme.palette.border_focus),
            "focused tab bar should keep neutral selected strokes; strokes={tab_bar_strokes:?}"
        );

        let tabs_point = Point::new(
            theme.metrics.tab_min_width + theme.metrics.tab_gap + 8.0,
            18.0,
        );
        let (mut tabs_runtime, tabs_window) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(260.0, 120.0))
                .with_child(
                    Tabs::new("Main tabs")
                        .theme(theme)
                        .selected(1)
                        .tab("Design", crate::Label::new("Design"))
                        .tab("Inspect", crate::Label::new("Inspect")),
                ),
        );
        let _ = tabs_runtime
            .render(tabs_window)
            .map_err(|error| error.to_string())?;
        tabs_runtime
            .handle_event(
                tabs_window,
                primary_pointer(PointerEventKind::Down, tabs_point, true),
            )
            .map_err(|error| error.to_string())?;
        tabs_runtime.tick(focus_duration + 0.01);
        assert!(handle_ready_events(&mut tabs_runtime)? >= 1);
        let tabs_focused = tabs_runtime
            .render(tabs_window)
            .map_err(|error| error.to_string())?;
        let tabs_strokes = solid_stroke_colors(&tabs_focused);
        assert!(
            contains_approx_color(&tabs_strokes, theme.palette.focus_ring),
            "focused selected tab button should paint a focus ring; strokes={tabs_strokes:?}"
        );
        assert!(
            !contains_approx_color(&tabs_strokes, theme.palette.border_focus),
            "focused tabs should keep neutral selected strokes; strokes={tabs_strokes:?}"
        );

        Ok(())
    }

    #[test]
    fn selected_tab_labels_preserve_body_text_metrics() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 15.5,
            line_height: 22.0,
        };
        theme.sync_derived_fields();

        let tab_bar = render_isolated(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected(1),
        );
        let tab_bar_label = text_run_for(&tab_bar, "Inspect");
        assert_text_run_uses_token(&tab_bar_label, theme.text.sm);
        assert_eq!(tab_bar_label.style.color, theme.palette.text);
        assert!(
            (text_run_visual_center(&tab_bar_label) - (tab_bar.frame.viewport.height * 0.5)).abs()
                < 0.75
        );

        let tabs = render(
            Tabs::new("Main tabs")
                .theme(theme)
                .selected(1)
                .tab("Design", crate::Label::new("Design"))
                .tab("Inspect", crate::Label::new("Inspect")),
        );
        let tabs_label = text_run_for(&tabs, "Inspect");
        assert_text_run_uses_token(&tabs_label, theme.text.sm);
        assert_eq!(tabs_label.style.color, theme.palette.text);
        assert!(
            (text_run_visual_center(&tabs_label) - (theme.metrics.tab_height * 0.5)).abs() < 0.75
        );
    }

    #[test]
    fn selected_tab_labels_preserve_tall_measurements_and_exact_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 28.0,
            line_height: 12.0,
        };
        theme.sync_derived_fields();
        theme.metrics.tab_height = 48.0;

        let tab_bar = render_isolated(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected(1),
        );
        let tab_bar_label = text_run_for(&tab_bar, "Inspect");
        let tab_bar_layout = TextSystem::new()
            .shape_text_run(&tab_bar_label, &FontRegistry::new())
            .expect("selected tab bar label should shape");

        assert_text_run_uses_token(&tab_bar_label, theme.text.sm);
        assert!(tab_bar_label.rect.height() >= tab_bar_layout.measurement().height - 0.01);
        assert!(tab_bar_label.rect.height() > tab_bar_label.style.line_height);
        assert!(
            (text_visual_center_for(&tab_bar, "Inspect") - (tab_bar.frame.viewport.height * 0.5))
                .abs()
                < 0.75
        );

        let tabs = render_isolated(
            Tabs::new("Main tabs")
                .theme(theme)
                .selected(1)
                .tab("Design", crate::Label::new("Design panel"))
                .tab("Inspect", crate::Label::new("Selected panel")),
        );
        let tabs_label = text_run_for(&tabs, "Inspect");
        let tabs_layout = TextSystem::new()
            .shape_text_run(&tabs_label, &FontRegistry::new())
            .expect("selected tabs label should shape");

        assert_text_run_uses_token(&tabs_label, theme.text.sm);
        assert!(tabs_label.rect.height() >= tabs_layout.measurement().height - 0.01);
        assert!(tabs_label.rect.height() > tabs_label.style.line_height);
        assert!(
            (text_visual_center_for(&tabs, "Inspect") - (theme.metrics.tab_height * 0.5)).abs()
                < 0.75
        );
    }

    #[test]
    fn tab_widgets_share_pressed_tab_border() -> Result<(), String> {
        let theme = DefaultTheme::default();

        let (mut runtime, window_id) = build_runtime(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"]),
        );
        let initial_tab_bar = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let press_point = super::rect_center(text_run_for(&initial_tab_bar, "Inspect").rect);
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, press_point, true),
            )
            .map_err(|error| error.to_string())?;
        let tab_bar = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(contains_approx_color(
            &solid_stroke_colors(&tab_bar),
            theme.palette.border_hover,
        ));

        let (mut runtime, window_id) = build_runtime(
            Tabs::new("Main tabs")
                .theme(theme)
                .tab("Design", crate::Label::new("Design"))
                .tab("Inspect", crate::Label::new("Inspect")),
        );
        let initial_tabs = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let press_point = super::rect_center(text_run_for(&initial_tabs, "Inspect").rect);
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, press_point, true),
            )
            .map_err(|error| error.to_string())?;
        let tabs = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(contains_approx_color(
            &solid_stroke_colors(&tabs),
            theme.palette.border_hover,
        ));

        Ok(())
    }

    #[test]
    fn tab_hover_and_press_chrome_use_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.control,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );

        fn assert_tab_header_motion<W>(
            root: W,
            hover_duration: f64,
            press_duration: f64,
            expected_hover: Color,
            expected_press: Color,
        ) -> Result<(), String>
        where
            W: Widget + 'static,
        {
            let (mut runtime, window_id) = build_runtime(root);
            let initial = runtime
                .render(window_id)
                .map_err(|error| error.to_string())?;
            let second_tab_point = super::rect_center(text_run_for(&initial, "Inspect").rect);

            let mut move_event = PointerEvent::new(PointerEventKind::Move, second_tab_point);
            move_event.pointer_id = 1;
            runtime
                .handle_event(window_id, Event::Pointer(move_event))
                .map_err(|error| error.to_string())?;

            runtime.tick(hover_duration * 0.5);
            assert_eq!(handle_ready_events(&mut runtime)?, 1);
            let mid_hover = runtime
                .render(window_id)
                .map_err(|error| error.to_string())?;
            assert!(
                !solid_fill_colors(&mid_hover).contains(&expected_hover),
                "tab hover fill should not snap to the settled hover color"
            );

            runtime.tick(hover_duration);
            assert_eq!(handle_ready_events(&mut runtime)?, 1);
            let settled_hover = runtime
                .render(window_id)
                .map_err(|error| error.to_string())?;
            assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

            runtime
                .handle_event(
                    window_id,
                    primary_pointer(PointerEventKind::Down, second_tab_point, true),
                )
                .map_err(|error| error.to_string())?;

            runtime.tick(hover_duration + press_duration * 0.5);
            assert_eq!(handle_ready_events(&mut runtime)?, 1);
            let mid_press = runtime
                .render(window_id)
                .map_err(|error| error.to_string())?;
            assert!(
                !solid_fill_colors(&mid_press).contains(&expected_press),
                "tab press fill should not snap to the settled pressed color"
            );

            runtime.tick(hover_duration + press_duration);
            assert_eq!(handle_ready_events(&mut runtime)?, 1);
            let settled_press = runtime
                .render(window_id)
                .map_err(|error| error.to_string())?;
            assert!(solid_fill_colors(&settled_press).contains(&expected_press));

            Ok(())
        }

        assert_tab_header_motion(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"]),
            hover_duration,
            press_duration,
            expected_hover,
            expected_press,
        )?;
        assert_tab_header_motion(
            Tabs::new("Main tabs")
                .theme(theme)
                .tab("Design", crate::Label::new("Design"))
                .tab("Inspect", crate::Label::new("Inspect")),
            hover_duration,
            press_duration,
            expected_hover,
            expected_press,
        )?;

        Ok(())
    }

    #[test]
    fn tab_switch_animation_uses_theme_motion() -> Result<(), String> {
        let mut theme = DefaultTheme::default();
        theme.motion.duration_fast = 0.6;
        theme.motion.duration_normal = 0.0;
        let switch_duration = theme.motion.tab_switch_duration();

        assert_keyboard_tab_switch_uses_duration(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"]),
            SemanticsRole::TabBar,
            switch_duration,
        )?;
        let selected = Rc::new(RefCell::new(0_usize));
        let selected_reader = Rc::clone(&selected);
        let selected_writer = Rc::clone(&selected);
        assert_keyboard_tab_switch_uses_duration(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected_when(move || Some(*selected_reader.borrow()))
                .on_change(move |index, _| *selected_writer.borrow_mut() = index),
            SemanticsRole::TabBar,
            switch_duration,
        )?;
        assert_keyboard_tab_switch_uses_duration(
            BrowserTabBar::new("Open tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"]),
            SemanticsRole::TabBar,
            switch_duration,
        )?;
        assert_keyboard_tab_switch_uses_duration(
            Tabs::new("Main tabs")
                .theme(theme)
                .tab("Design", crate::Label::new("Design panel"))
                .tab("Inspect", crate::Label::new("Inspect panel")),
            SemanticsRole::Tabs,
            switch_duration,
        )?;

        Ok(())
    }

    #[test]
    fn tab_bar_external_selected_state_animates_without_rebuild() -> Result<(), String> {
        let mut theme = DefaultTheme::default();
        theme.motion.duration_fast = 0.6;
        theme.motion.duration_normal = 0.0;
        let switch_duration = theme.motion.tab_switch_duration();
        let selected = Rc::new(RefCell::new(0_usize));
        let selected_reader = Rc::clone(&selected);
        let (mut runtime, window_id) = build_runtime(
            TabBar::new("Main tabs")
                .theme(theme)
                .tabs(["Design", "Inspect"])
                .selected_when(move || Some(*selected_reader.borrow())),
        );

        runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        *selected.borrow_mut() = 1;
        runtime
            .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
            .map_err(|error| error.to_string())?;

        runtime.tick(switch_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some(),
            "externally selected tab switch should still be animating at half duration"
        );

        runtime.tick(switch_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let tabs = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TabBar)
            .expect("tab semantics present");
        assert_eq!(
            tabs.value,
            Some(SemanticsValue::Text("Inspect".to_string()))
        );

        Ok(())
    }

    #[test]
    fn browser_tab_bar_semantics_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(17);
        let mut ids = BTreeSet::new();
        for tab_index in 0..13 {
            for id in [
                super::browser_tab_semantics_id(parent, tab_index).get(),
                super::browser_tab_close_semantics_id(parent, tab_index).get(),
            ] {
                assert!(id <= (1_u64 << 53) - 1, "{id} should be JS-safe");
                assert!(ids.insert(id), "{id} should be unique");
            }
        }
    }

    fn assert_keyboard_tab_switch_uses_duration<W>(
        widget: W,
        role: SemanticsRole,
        switch_duration: f64,
    ) -> Result<(), String>
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(widget);

        runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;
        runtime.tick(0.0);
        let _ = handle_ready_events(&mut runtime)?;
        assert!(
            runtime
                .focused_widget(window_id)
                .map_err(|error| error.to_string())?
                .is_some(),
            "tab widget should receive keyboard focus before arrow-key switching"
        );

        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(switch_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some(),
            "tab switch should still be animating at half of the custom theme duration"
        );

        runtime.tick(switch_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let tabs = output
            .semantics
            .iter()
            .find(|node| node.role == role)
            .expect("tab semantics present");
        assert_eq!(
            tabs.value,
            Some(SemanticsValue::Text("Inspect".to_string()))
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
    fn tabs_panel_body_slides_during_theme_switch_motion() -> Result<(), String> {
        let mut theme = DefaultTheme::default();
        theme.motion.duration_fast = 0.6;
        theme.motion.duration_normal = 0.0;
        let switch_duration = theme.motion.tab_switch_duration();
        let (mut runtime, window_id) = build_runtime(
            Tabs::new("Main tabs")
                .theme(theme)
                .tab("Design", crate::Label::new("Design panel"))
                .tab("Inspect", crate::Label::new("Inspect panel")),
        );

        runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;
        runtime.tick(0.0);
        let _ = handle_ready_events(&mut runtime)?;
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(switch_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_dx = text_transform_dx_for(&mid, "Inspect panel")
            .expect("incoming panel text should be painted");
        assert!(
            mid_dx > 0.0,
            "forward tab switch should slide the incoming panel in from the right; dx={mid_dx}"
        );

        runtime.tick(switch_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_dx = text_transform_dx_for(&settled, "Inspect panel")
            .expect("settled panel text should be painted");
        assert_eq!(settled_dx, 0.0);

        Ok(())
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
        assert_eq!(
            output.frame.viewport.height,
            DefaultTheme::default().metrics.tab_height
        );

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
        let theme = DefaultTheme::default();
        let padding = theme.metrics.menu_padding;
        let row_height = (output.frame.viewport.height - padding.top - padding.bottom) / 2.0;
        let row_center = padding.top + (row_height * 0.5);

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    #[test]
    fn menu_row_hover_and_press_use_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.control,
            theme.palette.accent,
            theme.interaction.selected_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) = build_runtime(
            Menu::new("App menu")
                .theme(theme)
                .items([MenuItem::new("New File"), MenuItem::new("Open...")]),
        );
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let item = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
            })
            .expect("menu item semantics should exist");
        let position = super::rect_center(item.bounds);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "menu hover fill should not snap to the settled highlighted color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "menu press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    #[test]
    fn menu_shortcuts_align_to_trailing_edge_and_row_center() {
        let theme = DefaultTheme::default();
        let output = render(Menu::new("App menu").items([
            MenuItem::new("New File").shortcut("Ctrl+N"),
            MenuItem::new("Open...").shortcut("Ctrl+Shift+O"),
        ]));
        let first_row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
            })
            .expect("first menu item semantics present")
            .bounds;
        let second_row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Open...")
            })
            .expect("second menu item semantics present")
            .bounds;
        let first_shortcut = text_run_for(&output, "Ctrl+N");
        let second_shortcut = text_run_for(&output, "Ctrl+Shift+O");
        let first_label_clip = clip_rect_for_text(&output, "New File");
        let second_label_clip = clip_rect_for_text(&output, "Open...");
        let first_edge = first_row.max_x() - theme.metrics.menu_item_padding.right;
        let second_edge = second_row.max_x() - theme.metrics.menu_item_padding.right;
        let first_label_edge = first_row.max_x()
            - theme.metrics.menu_item_padding.right
            - theme.metrics.menu_shortcut_width;
        let second_label_edge = second_row.max_x()
            - theme.metrics.menu_item_padding.right
            - theme.metrics.menu_shortcut_width;

        assert_eq!(
            first_shortcut.style.color,
            theme.placeholder_text_style().color
        );
        assert!((first_label_clip.max_x() - first_label_edge).abs() < 0.75);
        assert!((second_label_clip.max_x() - second_label_edge).abs() < 0.75);
        assert!((first_shortcut.rect.max_x() - first_edge).abs() < 0.75);
        assert!((second_shortcut.rect.max_x() - second_edge).abs() < 0.75);
        assert!((first_shortcut.rect.max_x() - second_shortcut.rect.max_x()).abs() < 0.75);
        assert!(
            (text_run_visual_center(&first_shortcut) - (first_row.y() + first_row.height() * 0.5))
                .abs()
                < 0.75
        );
    }

    #[test]
    fn menu_shortcuts_preserve_tall_measurements_and_row_center() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.menu_row_height = 64.0;
        let metrics = theme.metrics;
        let output = render_isolated(
            Menu::new("App menu")
                .theme(theme)
                .items([MenuItem::new("New File").shortcut("Ctrl+N")]),
        );
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New File")
            })
            .expect("menu item semantics present")
            .bounds;
        let label = text_run_for(&output, "New File");
        let shortcut = text_run_for(&output, "Ctrl+N");
        let label_layout = TextSystem::new()
            .shape_text_run(&label, &FontRegistry::new())
            .expect("menu item text should shape");
        let shortcut_layout = TextSystem::new()
            .shape_text_run(&shortcut, &FontRegistry::new())
            .expect("menu shortcut text should shape");
        let shortcut_edge = row.max_x() - metrics.menu_item_padding.right;
        let row_center = row.y() + (row.height() * 0.5);

        assert_eq!(label.style.font_size, 28.0);
        assert_eq!(label.style.line_height, 12.0);
        assert_eq!(shortcut.style.font_size, 28.0);
        assert_eq!(shortcut.style.line_height, 12.0);
        assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
        assert!(shortcut.rect.height() >= shortcut_layout.measurement().height - 0.01);
        assert!(label.rect.height() > label.style.line_height);
        assert!(shortcut.rect.height() > shortcut.style.line_height);
        assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
        assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
        assert!((text_run_visual_center(&shortcut) - row_center).abs() < 0.75);
    }

    #[test]
    fn context_menu_row_label_visual_center_matches_row_center() -> Result<(), String> {
        // Dropdown anchoring keeps the row geometry derivable from the
        // trigger bounds; pointer anchoring is covered separately.
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .anchor_to_pointer(false)
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
        let theme = DefaultTheme::default();
        let padding = theme.metrics.menu_padding;
        let gap = theme.metrics.popover_gap;
        let menu_height = context.bounds.height() - trigger.height() - gap;
        let row_height = (menu_height - padding.top - padding.bottom) / 2.0;
        let row_center = trigger.max_y() + gap + padding.top + (row_height * 0.5);

        assert!((actual_visual_center - row_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn context_menu_opens_at_the_right_click_position() -> Result<(), String> {
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new(
                "Surface menu",
                crate::containers::SizedBox::new()
                    .width(600.0)
                    .height(400.0),
            )
            .items([MenuItem::new("Copy"), MenuItem::new("Select All")]),
        );

        runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let press = Point::new(220.0, 140.0);
        let mut down = PointerEvent::new(PointerEventKind::Down, press);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Secondary);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .map_err(|error| error.to_string())?;

        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let first_item = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Copy")
            })
            .expect("menu item semantics present");
        let theme = DefaultTheme::default();
        let padding = theme.metrics.menu_padding;
        assert!(
            (first_item.bounds.x() - (press.x + padding.left)).abs() < 1.0,
            "menu should open at the press x, got item at {:?}",
            first_item.bounds
        );
        assert!(
            (first_item.bounds.y() - (press.y + padding.top)).abs() < 1.0,
            "menu should open at the press y, got item at {:?}",
            first_item.bounds
        );

        // The menu must also actually paint at the anchored position.
        let label = text_run_for(&output, "Copy");
        assert!(
            (label.rect.y() - press.y).abs() < theme.metrics.menu_padding.top + 16.0,
            "menu label should paint near the press position, got {:?}",
            label.rect
        );
        Ok(())
    }

    #[test]
    fn context_menu_shortcut_aligns_to_trailing_edge() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .anchor_to_pointer(false)
                .items([MenuItem::new("Rename").shortcut("F2")]),
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
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename")
            })
            .expect("context menu item semantics present")
            .bounds;
        let label_clip = clip_rect_for_text(&output, "Rename");
        let shortcut = text_run_for(&output, "F2");
        let label_edge =
            row.max_x() - theme.metrics.menu_item_padding.right - theme.metrics.menu_shortcut_width;
        let shortcut_edge = row.max_x() - theme.metrics.menu_item_padding.right;

        assert_eq!(shortcut.style.color, theme.placeholder_text_style().color);
        assert!((label_clip.max_x() - label_edge).abs() < 0.75);
        assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
        assert!((text_run_visual_center(&shortcut) - (row.y() + row.height() * 0.5)).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn context_menu_shortcuts_preserve_tall_measurements_and_row_center() -> Result<(), String> {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.menu_row_height = 64.0;
        let metrics = theme.metrics;
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .theme(theme)
                .items([MenuItem::new("Rename").shortcut("F2")]),
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
        let row = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Rename")
            })
            .expect("context menu item semantics present")
            .bounds;
        let label = text_run_for(&output, "Rename");
        let shortcut = text_run_for(&output, "F2");
        let label_layout = TextSystem::new()
            .shape_text_run(&label, &FontRegistry::new())
            .expect("context menu item text should shape");
        let shortcut_layout = TextSystem::new()
            .shape_text_run(&shortcut, &FontRegistry::new())
            .expect("context menu shortcut text should shape");
        let shortcut_edge = row.max_x() - metrics.menu_item_padding.right;
        let row_center = row.y() + (row.height() * 0.5);

        assert_eq!(label.style.font_size, 28.0);
        assert_eq!(label.style.line_height, 12.0);
        assert_eq!(shortcut.style.font_size, 28.0);
        assert_eq!(shortcut.style.line_height, 12.0);
        assert!(label.rect.height() >= label_layout.measurement().height - 0.01);
        assert!(shortcut.rect.height() >= shortcut_layout.measurement().height - 0.01);
        assert!(label.rect.height() > label.style.line_height);
        assert!(shortcut.rect.height() > shortcut.style.line_height);
        assert!((shortcut.rect.max_x() - shortcut_edge).abs() < 0.75);
        assert!((text_run_visual_center(&label) - row_center).abs() < 0.75);
        assert!((text_run_visual_center(&shortcut) - row_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn context_menu_entrance_uses_theme_motion_layer_properties() -> Result<(), String> {
        let theme = slow_normal_motion_theme();
        let duration = theme.motion.entrance_duration();
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .theme(theme)
                .items([MenuItem::new("Rename"), MenuItem::new("Duplicate")]),
        );

        let closed = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(overlay_layer_descriptor(&closed).is_none());
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

        let start = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let context = start
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ContextMenu)
            .expect("context menu semantics present");
        let start_descriptor =
            overlay_layer_descriptor(&start).expect("context menu overlay layer should appear");
        let menu_owner = overlay_layer_owner(&start).expect("context menu overlay owner present");
        assert_eq!(start_descriptor.properties.opacity, 0.0);
        assert!(start_descriptor.properties.translation.y < 0.0);
        assert!(
            layer_descriptor_for(&start, context.id).is_none(),
            "the context menu owner should not fade or translate the trigger"
        );

        runtime.tick(duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_descriptor =
            overlay_layer_descriptor(&mid).expect("context menu overlay layer should stay active");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert!(mid_descriptor.properties.translation.y < 0.0);
        assert!(
            mid_descriptor.properties.translation.y.abs()
                < start_descriptor.properties.translation.y.abs()
        );
        assert!(
            mid.frame.layer_updates.iter().any(|update| {
                update.owner == menu_owner
                    && matches!(
                        update.kind,
                        SceneLayerUpdateKind::Transform | SceneLayerUpdateKind::Effect
                    )
            }),
            "context menu entrance should update retained layer properties"
        );
        assert!(
            !mid.frame.layer_updates.iter().any(|update| {
                update.owner == menu_owner && update.kind == SceneLayerUpdateKind::Content
            }),
            "context menu entrance should not repaint menu content"
        );
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some()
        );

        runtime.tick(duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_descriptor =
            overlay_layer_descriptor(&settled).expect("context menu overlay layer should remain");
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
    fn context_menu_focus_ring_uses_non_hit_test_retained_layer() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .theme(theme)
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

        let opened = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let menu_owner = overlay_layer_owner(&opened).expect("context menu overlay owner present");
        let overlay =
            overlay_layer_descriptor(&opened).expect("context menu overlay layer should appear");
        assert!(overlay.hit_test);
        let focus_layers = non_hit_test_layer_descriptors(&opened);
        assert_eq!(
            focus_layers.len(),
            1,
            "context menu focus chrome should be the only non-hit-test layer"
        );
        assert_eq!(
            focus_layers[0].composition_mode,
            LayerCompositionMode::Normal
        );
        let focus_owner = non_hit_test_layer_owners(&opened)
            .into_iter()
            .next()
            .expect("context menu focus layer owner present");

        runtime.tick(focus_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_focus = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
            "context menu focus ring should not snap to the settled focus color"
        );
        assert!(
            !mid_focus.frame.layer_updates.iter().any(|update| {
                update.owner == menu_owner && update.kind == SceneLayerUpdateKind::Content
            }),
            "context menu rows should stay retained during focus chrome animation"
        );
        assert!(
            mid_focus
                .frame
                .layer_updates
                .iter()
                .any(|update| update.owner == focus_owner),
            "context menu focus layer should receive the animation update"
        );

        runtime.tick(focus_duration + 0.01);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_focus = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_strokes = solid_stroke_colors(&settled_focus);
        assert!(
            contains_approx_color(&settled_strokes, theme.palette.focus_ring),
            "context menu focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
        );

        Ok(())
    }

    #[test]
    fn context_menu_row_hover_and_press_use_theme_motion() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.control,
            theme.palette.accent,
            theme.interaction.selected_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) = build_runtime(
            ContextMenu::new("Canvas menu", crate::Button::new("Open menu"))
                .theme(theme)
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
        let trigger_center = super::rect_center(trigger);
        let mut secondary_down = PointerEvent::new(PointerEventKind::Down, trigger_center);
        secondary_down.pointer_id = 1;
        secondary_down.button = Some(PointerButton::Secondary);
        secondary_down.buttons = PointerButtons::new(2);
        runtime
            .handle_event(window_id, Event::Pointer(secondary_down))
            .map_err(|error| error.to_string())?;

        let opened = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let duplicate = opened
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("Duplicate")
            })
            .expect("duplicate menu item semantics should exist");
        let position = super::rect_center(duplicate.bounds);

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "context menu hover fill should not snap to the settled highlighted color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .map_err(|error| error.to_string())?;

        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "context menu press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    #[test]
    fn progress_bar_value_text_visual_center_matches_control_center() {
        let output = render_isolated(
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

        assert!(
            text.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn progress_bar_value_text_preserves_tall_measurements_and_exact_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 28.0,
            line_height: 12.0,
        };
        theme.sync_derived_fields();

        let output = render_isolated(
            ProgressBar::new("Export progress")
                .theme(theme)
                .range(0.0, 100.0)
                .value(42.0)
                .height(48.0)
                .show_value(true),
        );
        let text = text_run_for(&output, "42%");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("progress bar label should shape");

        assert_text_run_uses_token(&text, theme.text.sm);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!(
            (text_visual_center_for(&output, "42%") - (output.frame.viewport.height * 0.5)).abs()
                < 0.75
        );
    }

    #[test]
    fn spinner_label_visual_center_matches_indicator_center() {
        let output = render(Spinner::new("Background work").label("Uploading textures"));
        let text = text_run_for(&output, "Uploading textures");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("spinner label should shape");
        let line = layout
            .lines()
            .first()
            .expect("spinner label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let indicator_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - indicator_center).abs() < 0.75);
    }

    #[test]
    fn spinner_label_preserves_tall_measurement_and_indicator_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();
        let output = render(
            Spinner::new("Background work")
                .theme(theme)
                .label("Uploading"),
        );
        let text = text_run_for(&output, "Uploading");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("spinner label should shape");
        let busy = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::BusyIndicator)
            .expect("spinner semantics should exist");
        let center_y = busy.bounds.y() + busy.bounds.height() * 0.5;

        assert_text_run_uses_token(&text, theme.text.sm);
        assert!(busy.bounds.height() > 20.0);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text_run_visual_center(&text) - center_y).abs() < 0.75);
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
    fn tooltip_paints_with_surface_tokens() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            crate::Tooltip::new(
                "Quick access to common commands",
                crate::Button::new("Hover for shortcuts").min_width(180.0),
            )
            .theme(theme),
        ));

        let initial = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
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

        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(solid_fill_colors(&output).contains(&theme.surfaces.tooltip));
        let mut painted_tooltip_border = false;
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
                } if *color == theme.surfaces.tooltip_border => {
                    painted_tooltip_border = true;
                }
                _ => {}
            });
        assert!(painted_tooltip_border);
        Ok(())
    }

    #[test]
    fn tooltip_text_visual_center_matches_padded_bubble_center() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let tooltip_text = "Quick access to common commands";
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            crate::Tooltip::new(
                tooltip_text,
                crate::Button::new("Hover for shortcuts").min_width(180.0),
            )
            .theme(theme),
        ));

        let initial = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
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

        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let tooltip = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Tooltip && node.name.as_deref() == Some(tooltip_text)
            })
            .expect("tooltip semantics present");
        let text = text_run_for(&output, tooltip_text);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("tooltip text should shape");
        let line = layout
            .lines()
            .first()
            .expect("tooltip text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let text_slot = super::inset_rect(tooltip.bounds, theme.metrics.tooltip_padding);

        assert!((actual_visual_center - super::rect_center(text_slot).y).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn tooltip_text_preserves_tall_measurement_in_padded_bubble() -> Result<(), String> {
        let mut theme = DefaultTheme::default();
        theme.text.sm = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.sync_derived_fields();
        let tooltip_text = "Quick commands";
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            crate::Tooltip::new(
                tooltip_text,
                crate::Button::new("Hover for shortcuts").min_width(180.0),
            )
            .theme(theme),
        ));

        let initial = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
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

        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let tooltip = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Tooltip && node.name.as_deref() == Some(tooltip_text)
            })
            .expect("tooltip semantics present");
        let text = text_run_for(&output, tooltip_text);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("tooltip text should shape");
        let text_slot = super::inset_rect(tooltip.bounds, theme.metrics.tooltip_padding);

        assert_text_run_uses_token(&text, theme.text.sm);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!(
            (text_run_visual_center(&text) - super::rect_center(text_slot).y).abs() < 0.75,
            "tooltip text should remain visually centered in the padded bubble; rect={:?}, slot={:?}, measurement={:?}",
            text.rect,
            text_slot,
            layout.measurement()
        );
        Ok(())
    }

    #[test]
    fn tooltip_reveal_animation_updates_layer_properties_until_complete() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let entrance_duration = theme.motion.entrance_duration();

        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            crate::Tooltip::new(
                "Quick access to common commands",
                crate::Button::new("Hover for shortcuts").min_width(180.0),
            )
            .theme(theme),
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
        assert!(
            !start_descriptor.hit_test,
            "tooltip overlay should not intercept pointer hit testing"
        );
        assert_eq!(
            start_descriptor.properties.translation.y.signum(),
            -1.0,
            "tooltip reveal should start offset upward"
        );
        assert_eq!(
            start_descriptor.properties.translation.y.abs(),
            theme.metrics.tooltip_reveal_offset
        );
        assert_eq!(start_descriptor.properties.opacity, 0.0);

        runtime.tick(entrance_duration * 0.5);
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

        runtime.tick(entrance_duration);
        assert!(handle_ready_events(&mut runtime)? >= 1);
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
        let theme = slow_normal_motion_theme();
        let entrance_duration = theme.motion.entrance_duration();

        let content = Rc::new(RefCell::new(PanelCounters::default()));
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            Popover::new(
                "Inline inspector",
                crate::Button::new("Open inspector").min_width(180.0),
                SpyPanel::new("popover-content", Rc::clone(&content)),
            )
            .theme(theme),
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

        runtime.tick(entrance_duration * 0.5);
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

        runtime.tick(entrance_duration);
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
    fn popover_focus_ring_animates_without_repainting_retained_content() -> Result<(), String> {
        let theme = DefaultTheme::default();
        let entrance_duration = theme.motion.entrance_duration();
        let focus_duration = theme.motion.focus_duration();

        let content = Rc::new(RefCell::new(PanelCounters::default()));
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            16.0,
            Popover::new(
                "Inline inspector",
                crate::Button::new("Open inspector").min_width(180.0),
                SpyPanel::new("popover-content", Rc::clone(&content)),
            )
            .theme(theme),
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
        assert!(open_descriptor.hit_test);
        let open_focus_layers = non_hit_test_layer_descriptors(&opened);
        assert_eq!(
            open_focus_layers.len(),
            1,
            "popover focus chrome should be the only non-hit-test layer"
        );
        assert_eq!(
            open_focus_layers[0].composition_mode,
            LayerCompositionMode::Normal
        );
        assert_eq!(content.borrow().paint, 1);

        runtime.tick(focus_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_focus = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
            "popover focus ring should not snap to the settled focus color"
        );
        assert_eq!(
            content.borrow().paint,
            1,
            "popover content should stay retained while focus chrome repaints"
        );

        runtime.tick(entrance_duration.max(focus_duration) + 0.01);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_focus = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_strokes = solid_stroke_colors(&settled_focus);
        assert!(
            contains_approx_color(&settled_strokes, theme.palette.focus_ring),
            "popover focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
        );
        assert_eq!(
            content.borrow().paint,
            1,
            "popover content should not repaint on focus-only animation frames"
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
        assert!(
            solid_fill_colors(&output).contains(&DefaultTheme::default().surfaces.overlay_scrim)
        );
    }

    #[test]
    fn modal_dialog_entrance_uses_theme_motion_effect_layer_properties() -> Result<(), String> {
        let theme = slow_normal_motion_theme();
        let duration = theme.motion.entrance_duration();
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(640.0, 420.0))
                .with_child(
                    Dialog::new("Confirm", crate::Label::new("Apply the change?")).theme(theme),
                ),
        );

        let start = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let dialog = start
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present");
        let start_descriptor =
            layer_descriptor_for(&start, dialog.id).expect("dialog layer descriptor present");
        assert_eq!(
            start_descriptor.composition_mode,
            LayerCompositionMode::Effect
        );
        assert_eq!(start_descriptor.properties.opacity, 0.0);
        assert_eq!(start_descriptor.properties.translation, Vector::ZERO);
        assert!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?
                .is_some()
        );

        runtime.tick(duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_descriptor =
            layer_descriptor_for(&mid, dialog.id).expect("dialog layer descriptor still present");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert_eq!(mid_descriptor.properties.translation, Vector::ZERO);

        runtime.tick(duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_descriptor = layer_descriptor_for(&settled, dialog.id)
            .expect("dialog layer descriptor still present after settling");
        assert_eq!(settled_descriptor.properties.opacity, 1.0);
        assert_eq!(settled_descriptor.properties.translation, Vector::ZERO);
        assert_eq!(
            runtime
                .next_wakeup_time(window_id)
                .map_err(|error| error.to_string())?,
            None
        );
        Ok(())
    }

    #[test]
    fn dialog_entrance_animates_without_repainting_retained_body() -> Result<(), String> {
        let theme = slow_normal_motion_theme();
        let entrance_duration = theme.motion.entrance_duration();
        let body = Rc::new(RefCell::new(PanelCounters::default()));
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(640.0, 420.0))
                .with_child(
                    Dialog::new("Confirm", SpyPanel::new("dialog-body", Rc::clone(&body)))
                        .theme(theme),
                ),
        );

        let start = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let dialog = start
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present");
        let start_descriptor =
            layer_descriptor_for(&start, dialog.id).expect("dialog layer descriptor present");
        assert_eq!(start_descriptor.properties.opacity, 0.0);
        assert_eq!(body.borrow().paint, 1);

        runtime.tick(entrance_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mid_descriptor =
            layer_descriptor_for(&mid, dialog.id).expect("dialog layer descriptor still present");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert_eq!(
            body.borrow().paint,
            1,
            "dialog body should stay retained while entrance only changes layer properties"
        );

        runtime.tick(entrance_duration + 0.01);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let settled_descriptor = layer_descriptor_for(&settled, dialog.id)
            .expect("dialog layer descriptor still present after settling");
        assert_eq!(
            settled_descriptor.properties.opacity, 1.0,
            "dialog entrance should settle to full layer opacity"
        );
        assert_eq!(
            body.borrow().paint,
            1,
            "dialog body should not repaint on retained-only entrance frames"
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
    fn non_modal_dialog_entrance_uses_overlay_translation() {
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(640.0, 420.0))
                .with_child(
                    Dialog::new("Inspector", crate::Label::new("Layer settings")).modal(false),
                ),
        );

        let dialog = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Dialog)
            .expect("dialog semantics present");
        let descriptor =
            layer_descriptor_for(&output, dialog.id).expect("dialog layer descriptor present");

        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
        assert_eq!(descriptor.properties.opacity, 0.0);
        assert!(descriptor.properties.translation.y > 0.0);
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
