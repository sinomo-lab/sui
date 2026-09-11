use crate::ControlMetrics;
use crate::DefaultTheme;
use crate::IconGlyph;
use crate::SemanticTone;
use crate::composites::forms::{
    set_focus_animation_target, set_hover_animation_target, set_press_animation_target,
};
use crate::composites::indicators::{
    draw_control_frame, inset_rect, mix_color, physical_pixels, rect_center, rounded_rect_path,
};
use crate::composites::popups::AnimatedScalar;
use crate::controls::draw_icon_glyph;
use sui_core::Color;
use sui_core::Event;
use sui_core::KeyState;
use sui_core::Point;
use sui_core::PointerButton;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsAction;
use sui_core::SemanticsNode;
use sui_core::SemanticsPopupKind;
use sui_core::SemanticsRole;
use sui_core::SemanticsValue;
use sui_core::Size;
use sui_core::WakeEvent;
use sui_core::WidgetId;
use sui_layout::Alignment;
use sui_layout::Axis;
use sui_layout::Constraints;
use sui_layout::FlexAlignContent;
use sui_layout::FlexItem;
use sui_layout::FlexJustify;
use sui_layout::FlexLayout;
use sui_layout::FlexStyle;
use sui_layout::FlexWrap;
use sui_layout::Padding as Insets;
use sui_layout::arrange_flex;
use sui_layout::flex_layout;
use sui_runtime::ArrangeCtx;
use sui_runtime::EventCtx;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::Widget;
use sui_runtime::WidgetChildren;
use sui_runtime::WidgetPod;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::StrokeStyle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub(super) label: String,
    pub(super) shortcut: Option<String>,
    pub(super) enabled: bool,
    pub(super) destructive: bool,
    pub(super) separator_before: bool,
    pub(super) submenu: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            enabled: true,
            destructive: false,
            separator_before: false,
            submenu: Vec::new(),
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

    /// Add the nested actions presented when this item is opened by a
    /// [`ContextMenu`].
    pub fn submenu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.submenu.extend(items);
        self
    }

    /// The nested actions owned by this item.
    pub fn submenu_items(&self) -> &[MenuItem] {
        &self.submenu
    }

    /// Whether this item opens a nested menu.
    pub fn has_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }

    pub(super) fn text_color(&self, theme: &DefaultTheme) -> Color {
        if !self.enabled {
            theme.palette.placeholder
        } else if self.destructive {
            theme.semantic_tone_color(SemanticTone::Danger)
        } else {
            theme.palette.text
        }
    }
}

pub(super) fn virtual_menu_item_id(parent: WidgetId, index: usize) -> WidgetId {
    virtual_menu_item_path_id(parent, &[index])
}

pub(super) fn virtual_menu_item_path_id(parent: WidgetId, path: &[usize]) -> WidgetId {
    let value = path.iter().fold(parent.get(), |value, index| {
        value.wrapping_mul(257).wrapping_add(*index as u64 + 1)
    });
    WidgetId::new((1_u64 << 63) | value)
}

pub(super) fn menu_row_height(theme: &DefaultTheme) -> f32 {
    theme.metrics.menu_row_height
}

pub(super) fn themed_menu_height_for_rows(
    theme: &DefaultTheme,
    row_height: f32,
    rows: usize,
) -> f32 {
    theme.metrics.menu_padding.top + theme.metrics.menu_padding.bottom + (row_height * rows as f32)
}

pub(super) fn menu_submenu_indicator_width(theme: &DefaultTheme) -> f32 {
    menu_row_height(theme) * 0.55
}

pub(super) fn menu_item_semantics_node(
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

pub(super) fn context_menu_item_semantics_node(
    root: WidgetId,
    parent: WidgetId,
    path: &[usize],
    item: &MenuItem,
    bounds: Rect,
    highlighted: bool,
    expanded: bool,
) -> SemanticsNode {
    let mut node = SemanticsNode::new(
        virtual_menu_item_path_id(root, path),
        SemanticsRole::MenuItem,
        bounds,
    );
    node.parent = Some(parent);
    node.name = Some(item.label.clone());
    node.state.disabled = !item.enabled;
    node.state.selected = highlighted;
    if item.has_submenu() {
        node.state.expanded = Some(expanded);
        node.popup = Some(SemanticsPopupKind::Menu);
    }
    if item.enabled {
        node.actions = if item.has_submenu() {
            vec![
                SemanticsAction::Activate,
                SemanticsAction::Expand,
                SemanticsAction::Collapse,
            ]
        } else {
            vec![SemanticsAction::Activate]
        };
    }
    node
}

pub struct Toolbar {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) axis: Axis,
    pub(super) name: Option<String>,
    pub(super) extent: Option<f32>,
    pub(super) padding: Option<Insets>,
    pub(super) spacing: Option<f32>,
    pub(super) line_spacing: Option<f32>,
    pub(super) wrap: FlexWrap,
    pub(super) background: Option<Color>,
    pub(super) divider: bool,
    pub(super) children: WidgetChildren,
    pub(super) layout: Option<FlexLayout>,
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
            line_spacing: None,
            wrap: FlexWrap::NoWrap,
            background: None,
            divider: true,
            children: WidgetChildren::new(),
            layout: None,
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

    /// Allow toolbar items to flow into additional rows or columns while
    /// retaining their original widget identities and navigation order.
    pub fn wrap(mut self, wrap: FlexWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn wrapping(self) -> Self {
        self.wrap(FlexWrap::Wrap)
    }

    pub fn line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = Some(spacing.max(0.0));
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

    pub(super) fn resolved_extent(&self, metrics: ControlMetrics) -> f32 {
        self.extent.unwrap_or(metrics.toolbar_extent)
    }

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.toolbar_padding)
    }

    pub(super) fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.toolbar_spacing)
    }

    pub(super) fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let padding = self.resolved_padding(metrics);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom).max(0.0),
        )
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
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
        if self.wrap == FlexWrap::Wrap {
            let line_spacing = self.line_spacing.unwrap_or(spacing);
            let max_content = Size::new(
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
            );
            let min_cross = match self.axis {
                Axis::Horizontal => (extent - padding.top - padding.bottom).max(0.0),
                Axis::Vertical => (extent - padding.left - padding.right).max(0.0),
            };
            let min_content = match self.axis {
                Axis::Horizontal => Size::new(
                    if max_content.width.is_finite() {
                        max_content.width
                    } else {
                        0.0
                    },
                    min_cross,
                ),
                Axis::Vertical => Size::new(
                    min_cross,
                    if max_content.height.is_finite() {
                        max_content.height
                    } else {
                        0.0
                    },
                ),
            };
            let style = FlexStyle::new(self.axis)
                .wrap(FlexWrap::Wrap)
                .main_gap(spacing)
                .cross_gap(line_spacing)
                .justify(FlexJustify::Start)
                .align_items(Alignment::Center)
                .align_content(FlexAlignContent::Start);
            let items = vec![FlexItem::new(); self.children.len()];
            let layout = flex_layout(
                style,
                &items,
                Constraints::new(min_content, max_content),
                |index, child_constraints| {
                    self.children.measure_child(index, ctx, child_constraints)
                },
            );
            let natural = Size::new(
                layout.size.width + padding.left + padding.right,
                layout.size.height + padding.top + padding.bottom,
            );
            self.layout = Some(layout);
            return constraints.clamp(natural);
        }
        self.layout = None;
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
        if self.wrap == FlexWrap::Wrap {
            let style = FlexStyle::new(self.axis)
                .wrap(FlexWrap::Wrap)
                .main_gap(spacing)
                .cross_gap(self.line_spacing.unwrap_or(spacing))
                .justify(FlexJustify::Start)
                .align_items(Alignment::Center)
                .align_content(FlexAlignContent::Start);
            let items = vec![FlexItem::new(); self.children.len()];
            let measured = self
                .children
                .as_slice()
                .iter()
                .map(WidgetPod::measured_size)
                .collect::<Vec<_>>();
            let layout = arrange_flex(style, &items, content.size, &measured);
            for (index, item) in layout.items.iter().enumerate() {
                self.children.arrange_child(
                    index,
                    ctx,
                    item.rect.translate(content.origin.to_vector()),
                );
            }
            self.layout = Some(layout);
            return;
        }
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
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) axis: Axis,
    pub(super) name: Option<String>,
    pub(super) padding: Option<Insets>,
    pub(super) spacing: Option<f32>,
    pub(super) corner_radius: Option<f32>,
    pub(super) background: Option<Color>,
    pub(super) border: Option<Color>,
    pub(super) children: WidgetChildren,
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

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.command_group_padding)
    }

    pub(super) fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.command_group_spacing)
    }

    pub(super) fn resolved_corner_radius(&self, metrics: ControlMetrics) -> f32 {
        self.corner_radius.unwrap_or(metrics.command_group_radius)
    }

    pub(super) fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        inset_rect(bounds, self.resolved_padding(metrics))
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
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

pub(super) fn toolbar_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

pub(super) fn toolbar_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

pub(super) fn toolbar_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPaletteItem {
    pub(super) icon: IconGlyph,
    pub(super) label: String,
    pub(super) enabled: bool,
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
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) axis: Axis,
    pub(super) name: String,
    pub(super) items: Vec<ToolPaletteItem>,
    pub(super) selected: Option<usize>,
    pub(super) selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    pub(super) hovered: Option<usize>,
    pub(super) hover_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) extent: Option<f32>,
    pub(super) padding: Option<Insets>,
    pub(super) spacing: Option<f32>,
    pub(super) item_size: Option<f32>,
    pub(super) icon_size: Option<f32>,
    pub(super) background: Option<Color>,
    pub(super) divider: bool,
    pub(super) on_change: Option<Box<dyn FnMut(usize, String)>>,
    pub(super) on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String)>>,
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

    pub(super) fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.items.len())
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_extent(&self, metrics: ControlMetrics) -> f32 {
        self.extent.unwrap_or(metrics.toolbar_extent)
    }

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.toolbar_padding)
    }

    pub(super) fn resolved_spacing(&self, metrics: ControlMetrics) -> f32 {
        self.spacing.unwrap_or(metrics.toolbar_spacing)
    }

    pub(super) fn resolved_item_size(&self, metrics: ControlMetrics) -> f32 {
        self.item_size.unwrap_or(metrics.tool_palette_item_size)
    }

    pub(super) fn resolved_icon_size(&self, metrics: ControlMetrics) -> f32 {
        self.icon_size.unwrap_or(metrics.tool_palette_icon_size)
    }

    pub(super) fn content_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let padding = self.resolved_padding(metrics);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + padding.top,
            (bounds.width() - padding.left - padding.right).max(0.0),
            (bounds.height() - padding.top - padding.bottom).max(0.0),
        )
    }

    pub(super) fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
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

    pub(super) fn hit_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        (0..self.items.len()).find(|index| {
            self.items[*index].enabled
                && self
                    .item_rect(bounds, *index)
                    .is_some_and(|rect| rect.contains(position))
        })
    }

    pub(super) fn select(&mut self, ctx: &mut EventCtx, index: usize) {
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

    pub(super) fn move_selection(&mut self, ctx: &mut EventCtx, delta: isize) {
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

    pub(super) fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
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

    pub(super) fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
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

    pub(super) fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
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
                palette.selection
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
            } else if selected_item {
                palette.selection_border
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

pub(super) fn tool_palette_item_id(parent: WidgetId, index: usize) -> WidgetId {
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
