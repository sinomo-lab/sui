use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

use sui_core::{
    Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Path, Point,
    PointerButton, PointerEvent, PointerEventKind, PointerKind, Rect, ScrollDelta, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector, WakeEvent,
    WidgetId, WindowEvent,
};
use sui_layout::{
    Alignment, Axis, Constraints, FlexAlignContent, FlexItem, FlexJustify, FlexStyle, FlexWrap,
    Padding as Insets, arrange_flex, flex_layout,
};
use sui_reactive::Observable;
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    REACTIVE_CHANGE_KIND, SemanticsCtx, SingleChild, Widget, WidgetChildren, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Brush, LayerCompositionMode, StrokeStyle};

use crate::{DefaultTheme, MotionScalar};

pub struct Padding {
    insets: Insets,
    fill_child_width: bool,
    fill_child_height: bool,
    child: SingleChild,
}

impl Padding {
    pub fn new<W>(insets: Insets, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            insets,
            fill_child_width: false,
            fill_child_height: false,
            child: SingleChild::new(child),
        }
    }

    pub fn all<W>(value: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(Insets::all(value), child)
    }

    pub fn horizontal<W>(value: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let value = value.max(0.0);
        Self::new(
            Insets {
                left: value,
                top: 0.0,
                right: value,
                bottom: 0.0,
            },
            child,
        )
    }

    pub fn vertical<W>(value: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let value = value.max(0.0);
        Self::new(
            Insets {
                left: 0.0,
                top: value,
                right: 0.0,
                bottom: value,
            },
            child,
        )
    }

    pub fn symmetric<W>(horizontal: f32, vertical: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(
            Insets {
                left: horizontal.max(0.0),
                top: vertical.max(0.0),
                right: horizontal.max(0.0),
                bottom: vertical.max(0.0),
            },
            child,
        )
    }

    pub fn insets(&self) -> Insets {
        self.insets
    }

    pub fn fill_child_width(mut self) -> Self {
        self.fill_child_width = true;
        self
    }

    pub fn fill_child_height(mut self) -> Self {
        self.fill_child_height = true;
        self
    }

    pub fn fill_child(mut self) -> Self {
        self.fill_child_width = true;
        self.fill_child_height = true;
        self
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }
}

impl Widget for Padding {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = inset_constraints(constraints, self.insets);
        let child_size = self.child.measure(ctx, child_constraints);

        constraints.clamp(expand_size(child_size, self.insets))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let content = inset_rect(bounds, self.insets);
        let measured = self.child.child().measured_size();
        let child_size = Size::new(
            if self.fill_child_width {
                content.width()
            } else {
                measured.width.min(content.width())
            },
            if self.fill_child_height {
                content.height()
            } else {
                measured.height.min(content.height())
            },
        );
        self.child
            .arrange(ctx, Rect::from_origin_size(content.origin, child_size));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub struct Align {
    horizontal: Alignment,
    vertical: Alignment,
    child: SingleChild,
}

impl Align {
    pub fn new<W>(horizontal: Alignment, vertical: Alignment, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            horizontal,
            vertical,
            child: SingleChild::new(child),
        }
    }

    pub fn center<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(Alignment::Center, Alignment::Center, child)
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }
}

impl Widget for Align {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints =
            aligned_child_constraints(constraints, self.horizontal, self.vertical);
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            stretched_dimension(self.horizontal, constraints.max.width, child_size.width),
            stretched_dimension(self.vertical, constraints.max.height, child_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let child_size = self.child.child().measured_size();
        self.child.arrange(
            ctx,
            Rect::from_origin_size(
                Point::new(
                    bounds.x() + aligned_offset(self.horizontal, bounds.width() - child_size.width),
                    bounds.y() + aligned_offset(self.vertical, bounds.height() - child_size.height),
                ),
                child_size,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub struct Background {
    brush: Brush,
    brush_reader: Option<Box<dyn Fn() -> Brush>>,
    child: SingleChild,
}

impl Background {
    pub fn new<W>(brush: impl Into<Brush>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            brush: brush.into(),
            brush_reader: None,
            child: SingleChild::new(child),
        }
    }

    pub fn brush_when<F, B>(mut self, brush: F) -> Self
    where
        F: Fn() -> B + 'static,
        B: Into<Brush>,
    {
        self.brush_reader = Some(Box::new(move || brush().into()));
        self
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }
}

impl Widget for Background {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let brush = self
            .brush_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.brush.clone());
        ctx.fill_bounds(brush);
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub struct SemanticRegion {
    name: String,
    name_reader: Option<Box<dyn Fn() -> String>>,
    description: Option<String>,
    description_reader: Option<Box<dyn Fn() -> String>>,
    role: SemanticsRole,
    child: SingleChild,
}

impl SemanticRegion {
    pub fn new<W>(name: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            name: name.into(),
            name_reader: None,
            description: None,
            description_reader: None,
            role: SemanticsRole::GenericContainer,
            child: SingleChild::new(child),
        }
    }

    pub fn name_when<F>(mut self, name: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.name_reader = Some(Box::new(name));
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

    pub fn role(mut self, role: SemanticsRole) -> Self {
        self.role = role;
        self
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    fn name_text(&self) -> String {
        self.name_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.name.clone())
    }

    fn description_text(&self) -> Option<String> {
        self.description_reader
            .as_ref()
            .map(|reader| reader())
            .or_else(|| self.description.clone())
    }
}

impl Widget for SemanticRegion {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), self.role.clone(), ctx.bounds());
        node.name = Some(self.name_text());
        node.description = self.description_text();
        ctx.push(node);
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub struct SizedBox {
    width: Option<f32>,
    height: Option<f32>,
    child: Option<SingleChild>,
}

impl SizedBox {
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            child: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.width = Some(size.width.max(0.0));
        self.height = Some(size.height.max(0.0));
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.child = Some(SingleChild::new(child));
        self
    }

    pub fn child(&self) -> Option<&WidgetPod> {
        self.child.as_ref().map(SingleChild::child)
    }

    pub fn child_mut(&mut self) -> Option<&mut WidgetPod> {
        self.child.as_mut().map(SingleChild::child_mut)
    }
}

impl Default for SizedBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SizedBox {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = sized_box_constraints(constraints, self.width, self.height);
        let child_size = if let Some(child) = &mut self.child {
            child.measure(ctx, child_constraints)
        } else {
            Size::ZERO
        };

        constraints.clamp(Size::new(
            self.width.unwrap_or(child_size.width),
            self.height.unwrap_or(child_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if let Some(child) = &mut self.child {
            let measured = child.child().measured_size();
            let child_size = Size::new(
                measured.width.min(bounds.width()),
                measured.height.min(bounds.height()),
            );
            child.arrange(ctx, Rect::from_origin_size(bounds.origin, child_size));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if let Some(child) = &self.child {
            child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(child) = &self.child {
            child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(child) = &self.child {
            child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(child) = &mut self.child {
            child.visit_children_mut(visitor);
        }
    }
}

pub struct Stack {
    axis: Axis,
    spacing: f32,
    alignment: Alignment,
    children: WidgetChildren,
}

impl Stack {
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            spacing: 0.0,
            alignment: Alignment::Start,
            children: WidgetChildren::new(),
        }
    }

    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
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

    pub fn children(&self) -> &[WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [WidgetPod] {
        self.children.as_mut_slice()
    }
}

impl Widget for Stack {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_main = axis_main(self.axis, constraints.max);
        let max_cross = axis_cross(self.axis, constraints.max);
        let stretch_cross = self.alignment == Alignment::Stretch && max_cross.is_finite();
        let mut main_extent = 0.0;
        let mut cross_extent: f32 = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            let spacing_before = if index == 0 { 0.0 } else { self.spacing };
            let remaining_main = (max_main - main_extent - spacing_before).max(0.0);
            let child_constraints =
                stack_child_constraints(self.axis, remaining_main, max_cross, stretch_cross);
            let child_size = child.measure(ctx, child_constraints);
            main_extent += spacing_before + axis_main(self.axis, child_size);
            cross_extent = cross_extent.max(axis_cross(self.axis, child_size));
        }

        constraints.clamp(axis_size(self.axis, main_extent, cross_extent))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let cross_available = axis_cross(self.axis, bounds.size);
        let mut main_offset = 0.0;

        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                main_offset += self.spacing;
            }

            let child_size = stack_arranged_child_size(
                self.axis,
                self.alignment,
                child.measured_size(),
                cross_available,
            );
            let cross_offset = aligned_offset(
                self.alignment,
                cross_available - axis_cross(self.axis, child_size),
            );
            child.arrange(
                ctx,
                Rect::from_origin_size(
                    Point::new(bounds.x(), bounds.y())
                        + axis_point(self.axis, main_offset, cross_offset).to_vector(),
                    child_size,
                ),
            );
            main_offset += axis_main(self.axis, child_size);
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

pub struct TrailingSlotRow {
    body: SingleChild,
    trailing: SingleChild,
    trailing_width: f32,
    trailing_height: f32,
    gap: f32,
}

impl TrailingSlotRow {
    pub fn new<B, T>(body: B, trailing: T) -> Self
    where
        B: Widget + 'static,
        T: Widget + 'static,
    {
        Self {
            body: SingleChild::new(body),
            trailing: SingleChild::new(trailing),
            trailing_width: 0.0,
            trailing_height: 0.0,
            gap: 0.0,
        }
    }

    pub fn trailing_width(mut self, width: f32) -> Self {
        self.trailing_width = width.max(0.0);
        self
    }

    pub fn trailing_height(mut self, height: f32) -> Self {
        self.trailing_height = height.max(0.0);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn body(&self) -> &WidgetPod {
        self.body.child()
    }

    pub fn body_mut(&mut self) -> &mut WidgetPod {
        self.body.child_mut()
    }

    pub fn trailing(&self) -> &WidgetPod {
        self.trailing.child()
    }

    pub fn trailing_mut(&mut self) -> &mut WidgetPod {
        self.trailing.child_mut()
    }

    fn layout_rects(&self, bounds: Rect) -> (Rect, Rect) {
        let trailing_height = self
            .trailing_height
            .min(bounds.width())
            .min(bounds.height())
            .max(0.0);
        let trailing_width = self.trailing_width.min(bounds.width()).max(0.0);
        let gap = if trailing_width > 0.0 && bounds.width() > trailing_width {
            self.gap.min(bounds.width() - trailing_width).max(0.0)
        } else {
            0.0
        };
        let body_width = (bounds.width() - trailing_width - gap).max(0.0);
        let body = Rect::new(bounds.x(), bounds.y(), body_width, bounds.height());
        let trailing = Rect::new(
            bounds.x() + body_width + gap,
            bounds.y() + ((bounds.height() - trailing_height) * 0.5).max(0.0),
            trailing_width,
            trailing_height,
        );
        (body, trailing)
    }
}

impl Widget for TrailingSlotRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(self.trailing_width)
        };
        if !constraints.max.height.is_finite() {
            let provisional_height = constraints.min.height.max(self.trailing_height);
            let bounds = Rect::new(0.0, 0.0, width, provisional_height);
            let (body, trailing) = self.layout_rects(bounds);
            let body_size = self.body.measure(
                ctx,
                Constraints::new(
                    Size::new(body.width(), 0.0),
                    Size::new(body.width(), f32::INFINITY),
                ),
            );
            self.trailing.measure(
                ctx,
                Constraints::tight(Size::new(trailing.width(), self.trailing_height)),
            );
            let height = body_size
                .height
                .max(self.trailing_height)
                .max(constraints.min.height);
            return constraints.clamp(Size::new(width, height));
        }

        let height = constraints.max.height.max(constraints.min.height);
        let bounds = Rect::new(0.0, 0.0, width, height);
        let (body, trailing) = self.layout_rects(bounds);
        self.body.measure(
            ctx,
            Constraints::tight(Size::new(body.width(), body.height())),
        );
        self.trailing.measure(
            ctx,
            Constraints::tight(Size::new(trailing.width(), trailing.height())),
        );
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let (body, trailing) = self.layout_rects(bounds);
        self.body.arrange(ctx, body);
        self.trailing.arrange(ctx, trailing);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.body.paint(ctx);
        self.trailing.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.body.semantics(ctx);
        self.trailing.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.body.visit_children(visitor);
        self.trailing.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.body.visit_children_mut(visitor);
        self.trailing.visit_children_mut(visitor);
    }
}

/// Vertical dock layout with optional fixed-height top and bottom slots.
///
/// The body fills the remaining height. This is a layout primitive, not a themed panel: callers
/// provide their own surfaces, separators, status bars, and scroll views.
pub struct Dock {
    top: Option<SingleChild>,
    bottom: Option<SingleChild>,
    body: SingleChild,
    top_height: f32,
    bottom_height: f32,
    fallback_width: f32,
    fallback_body_height: f32,
}

impl Dock {
    pub fn new<W>(body: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            top: None,
            bottom: None,
            body: SingleChild::new(body),
            top_height: 0.0,
            bottom_height: 0.0,
            fallback_width: 320.0,
            fallback_body_height: 240.0,
        }
    }

    pub fn top<W>(mut self, height: f32, widget: W) -> Self
    where
        W: Widget + 'static,
    {
        self.top = Some(SingleChild::new(widget));
        self.top_height = height.max(0.0);
        self
    }

    pub fn bottom<W>(mut self, height: f32, widget: W) -> Self
    where
        W: Widget + 'static,
    {
        self.bottom = Some(SingleChild::new(widget));
        self.bottom_height = height.max(0.0);
        self
    }

    pub fn fallback_width(mut self, width: f32) -> Self {
        self.fallback_width = width.max(0.0);
        self
    }

    pub fn fallback_body_height(mut self, height: f32) -> Self {
        self.fallback_body_height = height.max(0.0);
        self
    }

    pub fn top_child(&self) -> Option<&WidgetPod> {
        self.top.as_ref().map(SingleChild::child)
    }

    pub fn top_child_mut(&mut self) -> Option<&mut WidgetPod> {
        self.top.as_mut().map(SingleChild::child_mut)
    }

    pub fn body(&self) -> &WidgetPod {
        self.body.child()
    }

    pub fn body_mut(&mut self) -> &mut WidgetPod {
        self.body.child_mut()
    }

    pub fn bottom_child(&self) -> Option<&WidgetPod> {
        self.bottom.as_ref().map(SingleChild::child)
    }

    pub fn bottom_child_mut(&mut self) -> Option<&mut WidgetPod> {
        self.bottom.as_mut().map(SingleChild::child_mut)
    }

    fn width_for_measure(&self, constraints: Constraints) -> f32 {
        if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(self.fallback_width)
        }
    }
}

impl Widget for Dock {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = self.width_for_measure(constraints);
        let top_h = self.top_height.max(0.0);
        let bottom_h = self.bottom_height.max(0.0);

        let top_size = if let Some(top) = &mut self.top {
            top.measure(ctx, Constraints::tight(Size::new(width, top_h)))
        } else {
            Size::ZERO
        };
        let bottom_size = if let Some(bottom) = &mut self.bottom {
            bottom.measure(ctx, Constraints::tight(Size::new(width, bottom_h)))
        } else {
            Size::ZERO
        };

        let available_body_h = if constraints.max.height.is_finite() {
            (constraints.max.height - top_h - bottom_h).max(0.0)
        } else {
            self.fallback_body_height
        };
        let body_size = self.body.measure(
            ctx,
            Constraints::new(
                Size::new(width, 0.0),
                Size::new(width, available_body_h.max(0.0)),
            ),
        );

        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            top_h + body_size.height + bottom_h
        };
        let width = width
            .max(top_size.width)
            .max(body_size.width)
            .max(bottom_size.width);
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let top_h = self.top_height.min(bounds.height()).max(0.0);
        let bottom_h = self
            .bottom_height
            .min((bounds.height() - top_h).max(0.0))
            .max(0.0);
        let body_h = (bounds.height() - top_h - bottom_h).max(0.0);

        if let Some(top) = &mut self.top {
            top.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), bounds.width(), top_h),
            );
        }
        self.body.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y() + top_h, bounds.width(), body_h),
        );
        if let Some(bottom) = &mut self.bottom {
            bottom.arrange(
                ctx,
                Rect::new(
                    bounds.x(),
                    bounds.max_y() - bottom_h,
                    bounds.width(),
                    bottom_h,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.body.paint(ctx);
        if let Some(top) = &self.top {
            top.paint(ctx);
        }
        if let Some(bottom) = &self.bottom {
            bottom.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(top) = &self.top {
            top.semantics(ctx);
        }
        self.body.semantics(ctx);
        if let Some(bottom) = &self.bottom {
            bottom.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(top) = &self.top {
            top.visit_children(visitor);
        }
        self.body.visit_children(visitor);
        if let Some(bottom) = &self.bottom {
            bottom.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(top) = &mut self.top {
            top.visit_children_mut(visitor);
        }
        self.body.visit_children_mut(visitor);
        if let Some(bottom) = &mut self.bottom {
            bottom.visit_children_mut(visitor);
        }
    }
}

/// Vertical dock layout where the bottom child is measured to its natural height.
///
/// This is useful for composer/status panels that grow with content while the body takes whatever
/// height remains above them.
pub struct MeasuredBottomDock {
    body: SingleChild,
    bottom: SingleChild,
    fallback_size: Size,
}

impl MeasuredBottomDock {
    pub fn new<B, T>(body: B, bottom: T) -> Self
    where
        B: Widget + 'static,
        T: Widget + 'static,
    {
        Self {
            body: SingleChild::new(body),
            bottom: SingleChild::new(bottom),
            fallback_size: Size::new(640.0, 640.0),
        }
    }

    pub fn fallback_size(mut self, size: Size) -> Self {
        self.fallback_size = Size::new(size.width.max(0.0), size.height.max(0.0));
        self
    }

    pub fn body(&self) -> &WidgetPod {
        self.body.child()
    }

    pub fn body_mut(&mut self) -> &mut WidgetPod {
        self.body.child_mut()
    }

    pub fn bottom(&self) -> &WidgetPod {
        self.bottom.child()
    }

    pub fn bottom_mut(&mut self) -> &mut WidgetPod {
        self.bottom.child_mut()
    }
}

impl Widget for MeasuredBottomDock {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(self.fallback_size.width)
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints.min.height.max(self.fallback_size.height)
        };
        let bottom_size = self.bottom.measure(
            ctx,
            Constraints::new(Size::new(width, 0.0), Size::new(width, height)),
        );
        let bottom_h = bottom_size.height.min(height).max(0.0);
        let body_h = (height - bottom_h).max(0.0);
        self.body
            .measure(ctx, Constraints::tight(Size::new(width, body_h)));
        constraints.clamp(Size::new(width, body_h + bottom_h))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let bottom_h = self
            .bottom
            .child()
            .measured_size()
            .height
            .min(bounds.height())
            .max(0.0);
        let body_h = (bounds.height() - bottom_h).max(0.0);
        self.body.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), bounds.width(), body_h),
        );
        self.bottom.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.max_y() - bottom_h,
                bounds.width(),
                bottom_h,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.body.paint(ctx);
        self.bottom.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.body.semantics(ctx);
        self.bottom.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.body.visit_children(visitor);
        self.bottom.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.body.visit_children_mut(visitor);
        self.bottom.visit_children_mut(visitor);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixedPane {
    First,
    Second,
}

pub struct FixedPaneSplit {
    axis: Axis,
    fixed_pane: FixedPane,
    fixed_extent: f32,
    divider_extent: f32,
    fallback_flexible_extent: f32,
    first: SingleChild,
    divider: SingleChild,
    second: SingleChild,
}

impl FixedPaneSplit {
    pub fn new<F, D, S>(axis: Axis, first: F, divider: D, second: S) -> Self
    where
        F: Widget + 'static,
        D: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            axis,
            fixed_pane: FixedPane::First,
            fixed_extent: 0.0,
            divider_extent: 1.0,
            fallback_flexible_extent: 320.0,
            first: SingleChild::new(first),
            divider: SingleChild::new(divider),
            second: SingleChild::new(second),
        }
    }

    pub fn horizontal<F, D, S>(first: F, divider: D, second: S) -> Self
    where
        F: Widget + 'static,
        D: Widget + 'static,
        S: Widget + 'static,
    {
        Self::new(Axis::Horizontal, first, divider, second)
    }

    pub fn vertical<F, D, S>(first: F, divider: D, second: S) -> Self
    where
        F: Widget + 'static,
        D: Widget + 'static,
        S: Widget + 'static,
    {
        Self::new(Axis::Vertical, first, divider, second)
    }

    pub fn fixed_first(mut self, extent: f32) -> Self {
        self.fixed_pane = FixedPane::First;
        self.fixed_extent = extent.max(0.0);
        self
    }

    pub fn fixed_second(mut self, extent: f32) -> Self {
        self.fixed_pane = FixedPane::Second;
        self.fixed_extent = extent.max(0.0);
        self
    }

    pub fn divider_extent(mut self, extent: f32) -> Self {
        self.divider_extent = extent.max(0.0);
        self
    }

    pub fn fallback_flexible_extent(mut self, extent: f32) -> Self {
        self.fallback_flexible_extent = extent.max(0.0);
        self
    }

    pub fn first(&self) -> &WidgetPod {
        self.first.child()
    }

    pub fn first_mut(&mut self) -> &mut WidgetPod {
        self.first.child_mut()
    }

    pub fn divider(&self) -> &WidgetPod {
        self.divider.child()
    }

    pub fn divider_mut(&mut self) -> &mut WidgetPod {
        self.divider.child_mut()
    }

    pub fn second(&self) -> &WidgetPod {
        self.second.child()
    }

    pub fn second_mut(&mut self) -> &mut WidgetPod {
        self.second.child_mut()
    }

    fn split_extents(&self, total_main: f32) -> (f32, f32, f32) {
        let divider = self.divider_extent.min(total_main).max(0.0);
        let available = (total_main - divider).max(0.0);
        match self.fixed_pane {
            FixedPane::First => {
                let first = self.fixed_extent.min(available).max(0.0);
                (first, divider, (available - first).max(0.0))
            }
            FixedPane::Second => {
                let second = self.fixed_extent.min(available).max(0.0);
                ((available - second).max(0.0), divider, second)
            }
        }
    }

    fn child_constraints(&self, main: f32, cross: Option<f32>) -> Constraints {
        Constraints::new(
            axis_size(self.axis, main, cross.unwrap_or(0.0)),
            axis_size(self.axis, main, cross.unwrap_or(f32::INFINITY)),
        )
    }

    fn child_rect(&self, bounds: Rect, main_offset: f32, main: f32) -> Rect {
        Rect::from_origin_size(
            bounds.origin + axis_point(self.axis, main_offset, 0.0).to_vector(),
            axis_size(self.axis, main, axis_cross(self.axis, bounds.size)),
        )
    }
}

impl Widget for FixedPaneSplit {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let total_main = if axis_main(self.axis, constraints.max).is_finite() {
            axis_main(self.axis, constraints.max)
        } else {
            self.fixed_extent + self.divider_extent + self.fallback_flexible_extent
        };
        let cross_max = axis_cross(self.axis, constraints.max);
        let tight_cross = cross_max.is_finite().then_some(cross_max);
        let (first_main, divider_main, second_main) = self.split_extents(total_main);

        let first_size = self
            .first
            .measure(ctx, self.child_constraints(first_main, tight_cross));
        let divider_size = self
            .divider
            .measure(ctx, self.child_constraints(divider_main, tight_cross));
        let second_size = self
            .second
            .measure(ctx, self.child_constraints(second_main, tight_cross));

        let cross = tight_cross.unwrap_or_else(|| {
            axis_cross(self.axis, first_size)
                .max(axis_cross(self.axis, divider_size))
                .max(axis_cross(self.axis, second_size))
                .max(axis_cross(self.axis, constraints.min))
        });
        constraints.clamp(axis_size(self.axis, total_main, cross))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let (first_main, divider_main, second_main) =
            self.split_extents(axis_main(self.axis, bounds.size));
        self.first
            .arrange(ctx, self.child_rect(bounds, 0.0, first_main));
        self.divider
            .arrange(ctx, self.child_rect(bounds, first_main, divider_main));
        self.second.arrange(
            ctx,
            self.child_rect(bounds, first_main + divider_main, second_main),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.first.paint(ctx);
        self.divider.paint(ctx);
        self.second.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.first.semantics(ctx);
        self.divider.semantics(ctx);
        self.second.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.first.visit_children(visitor);
        self.divider.visit_children(visitor);
        self.second.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.first.visit_children_mut(visitor);
        self.divider.visit_children_mut(visitor);
        self.second.visit_children_mut(visitor);
    }
}

pub struct Flex {
    style: FlexStyle,
    items: Vec<FlexItem>,
    children: WidgetChildren,
}

impl Flex {
    pub fn new(axis: Axis) -> Self {
        Self {
            style: FlexStyle::new(axis),
            items: Vec::new(),
            children: WidgetChildren::new(),
        }
    }

    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn with_style(mut self, style: FlexStyle) -> Self {
        self.style = style;
        self
    }

    pub fn wrap(mut self, wrap: FlexWrap) -> Self {
        self.style = self.style.wrap(wrap);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style = self.style.gap(gap);
        self
    }

    pub fn main_gap(mut self, gap: f32) -> Self {
        self.style = self.style.main_gap(gap);
        self
    }

    pub fn cross_gap(mut self, gap: f32) -> Self {
        self.style = self.style.cross_gap(gap);
        self
    }

    pub fn justify(mut self, justify: FlexJustify) -> Self {
        self.style = self.style.justify(justify);
        self
    }

    pub fn align_items(mut self, alignment: Alignment) -> Self {
        self.style = self.style.align_items(alignment);
        self
    }

    pub fn align_content(mut self, alignment: FlexAlignContent) -> Self {
        self.style = self.style.align_content(alignment);
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.items.push(FlexItem::new());
        self
    }

    pub fn with_item<W>(mut self, child: W, item: FlexItem) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.items.push(item);
        self
    }

    pub fn spacer(mut self) -> Self {
        self.push_spacer();
        self
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.items.push(FlexItem::new());
    }

    pub fn push_item<W>(&mut self, child: W, item: FlexItem)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.items.push(item);
    }

    pub fn push_spacer(&mut self) {
        self.children.push(SizedBox::new());
        self.items.push(FlexItem::fill());
    }

    pub const fn style(&self) -> FlexStyle {
        self.style
    }

    pub fn items(&self) -> &[FlexItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [FlexItem] {
        &mut self.items
    }

    pub fn item(&self, index: usize) -> Option<FlexItem> {
        self.items.get(index).copied()
    }

    pub fn set_item(&mut self, index: usize, item: FlexItem) -> bool {
        let Some(slot) = self.items.get_mut(index) else {
            return false;
        };
        *slot = item;
        true
    }

    pub fn children(&self) -> &[WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [WidgetPod] {
        self.children.as_mut_slice()
    }
}

impl Widget for Flex {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let items = self.items.clone();
        let layout = flex_layout(
            self.style,
            &items,
            constraints,
            |index, child_constraints| self.children.measure_child(index, ctx, child_constraints),
        );

        layout.size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured_sizes = self
            .children
            .as_slice()
            .iter()
            .map(WidgetPod::measured_size)
            .collect::<Vec<_>>();
        let layout = arrange_flex(self.style, &self.items, bounds.size, &measured_sizes);

        for (index, item_layout) in layout.items.iter().enumerate() {
            self.children.arrange_child(
                index,
                ctx,
                item_layout.rect.translate(bounds.origin.to_vector()),
            );
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

pub struct SwitchView {
    selected: usize,
    selected_reader: Option<Box<dyn Fn() -> usize>>,
    selected_source: Option<Arc<dyn Observable<usize>>>,
    children: WidgetChildren,
}

impl SwitchView {
    pub fn new() -> Self {
        Self {
            selected: 0,
            selected_reader: None,
            selected_source: None,
            children: WidgetChildren::new(),
        }
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self.selected_reader = None;
        self.selected_source = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> usize + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self.selected_source = None;
        self
    }

    /// Bind the active child index to an observable value while retaining all
    /// child widget pods and their local state.
    pub fn selected_from<O>(mut self, selected: O) -> Self
    where
        O: Observable<usize> + 'static,
    {
        self.selected = selected.get();
        self.selected_reader = None;
        self.selected_source = Some(Arc::new(selected));
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

    pub fn selected_index(&self) -> Option<usize> {
        self.active_index()
    }

    pub fn children(&self) -> &[WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [WidgetPod] {
        self.children.as_mut_slice()
    }

    fn active_index(&self) -> Option<usize> {
        let selected = self
            .selected_source
            .as_ref()
            .map(|source| source.get())
            .or_else(|| self.selected_reader.as_ref().map(|reader| reader()))
            .unwrap_or(self.selected);
        (selected < self.children.as_slice().len()).then_some(selected)
    }
}

impl Default for SwitchView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SwitchView {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(source) = &self.selected_source {
            self.selected = ctx.observe(source.as_ref());
        }
        let Some(index) = self.active_index() else {
            return constraints.clamp(Size::ZERO);
        };

        self.children.as_mut_slice()[index].measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if let Some(index) = self.active_index() {
            self.children.as_mut_slice()[index].arrange(ctx, bounds);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if let Some(index) = self.active_index() {
            self.children.as_slice()[index].paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(index) = self.active_index() {
            self.children.as_slice()[index].semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(index) = self.active_index() {
            visitor.visit(&self.children.as_slice()[index]);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(index) = self.active_index() {
            visitor.visit(&mut self.children.as_mut_slice()[index]);
        }
    }
}

/// Hosts a child subtree that is rebuilt whenever a caller-supplied key changes.
///
/// This is useful for composed widgets whose child list is fixed at construction time, such as
/// segmented views, breadcrumbs, and virtualized tables. The container checks the key during
/// measure, on pointer release, and on redraw. When the key changes it replaces the child pod and
/// requests a fresh layout/paint pass.
pub struct RebuildOnChange<K: PartialEq + Clone> {
    key_fn: Option<Box<dyn Fn() -> K>>,
    key_source: Option<Arc<dyn Observable<K>>>,
    build: Box<dyn Fn(&K) -> WidgetPod>,
    last_key: K,
    child: WidgetPod,
}

impl<K: PartialEq + Clone> RebuildOnChange<K> {
    pub fn new<KF, BF>(key_fn: KF, build: BF) -> Self
    where
        KF: Fn() -> K + 'static,
        BF: Fn(&K) -> WidgetPod + 'static,
    {
        let last_key = key_fn();
        let child = build(&last_key);
        Self {
            key_fn: Some(Box::new(key_fn)),
            key_source: None,
            build: Box::new(build),
            last_key,
            child,
        }
    }

    /// Rebuild from an observable structural key.
    ///
    /// Unlike [`Self::new`], this form wakes the runtime and targets this host
    /// automatically when the key changes.
    pub fn new_observable<O, BF>(key_source: O, build: BF) -> Self
    where
        O: Observable<K> + 'static,
        BF: Fn(&K) -> WidgetPod + 'static,
        K: 'static,
    {
        let last_key = key_source.get();
        let child = build(&last_key);
        Self {
            key_fn: None,
            key_source: Some(Arc::new(key_source)),
            build: Box::new(build),
            last_key,
            child,
        }
    }

    fn current_key(&self) -> K {
        self.key_source
            .as_ref()
            .map(|source| source.get())
            .or_else(|| self.key_fn.as_ref().map(|key_fn| key_fn()))
            .expect("RebuildOnChange requires a key reader or observable")
    }

    pub fn refresh(&mut self) -> bool {
        let key = self.current_key();
        if key != self.last_key {
            self.child = (self.build)(&key);
            self.last_key = key;
            true
        } else {
            false
        }
    }

    pub fn child(&self) -> &WidgetPod {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        &mut self.child
    }
}

impl<K: PartialEq + Clone + 'static> Widget for RebuildOnChange<K> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Some(source) = &self.key_source {
            let _ = ctx.observe(source.as_ref(), InvalidationKind::Measure);
        }
        let pointer_up = matches!(
            event,
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Up)
        );
        let redraw = matches!(event, Event::Window(WindowEvent::RedrawRequested));
        let reactive = matches!(
            event,
            Event::Custom(custom) if custom.kind == REACTIVE_CHANGE_KIND
        );
        if (pointer_up || redraw || reactive) && self.refresh() {
            ctx.record_rebuild(
                std::any::type_name::<Self>(),
                "caller-supplied structural key changed",
            );
            ctx.request_measure();
            ctx.request_arrange();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(source) = &self.key_source {
            let _ = ctx.observe(source.as_ref());
        }
        if self.refresh() {
            ctx.record_rebuild(
                std::any::type_name::<Self>(),
                "caller-supplied structural key changed during measure",
            );
        }
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.child);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.child);
    }
}

/// Hosts a child subtree that is rebuilt whenever layout constraints resolve to a new key.
///
/// This is useful for breakpoint-driven layouts where the available width or height changes which
/// control set should be mounted. The container owns the child replacement and invalidation; the
/// caller only provides the key derivation and child builder.
pub struct RebuildOnConstraints<K: PartialEq + Clone> {
    key_fn: Box<dyn Fn(Constraints) -> K>,
    build: Box<dyn Fn(&K) -> WidgetPod>,
    last_key: K,
    child: WidgetPod,
}

impl<K: PartialEq + Clone> RebuildOnConstraints<K> {
    pub fn new<KF, BF>(initial_key: K, key_fn: KF, build: BF) -> Self
    where
        KF: Fn(Constraints) -> K + 'static,
        BF: Fn(&K) -> WidgetPod + 'static,
    {
        let child = build(&initial_key);
        Self {
            key_fn: Box::new(key_fn),
            build: Box::new(build),
            last_key: initial_key,
            child,
        }
    }

    pub fn refresh_for_constraints(&mut self, constraints: Constraints) -> bool {
        let key = (self.key_fn)(constraints);
        if key != self.last_key {
            self.child = (self.build)(&key);
            self.last_key = key;
            true
        } else {
            false
        }
    }

    pub fn child(&self) -> &WidgetPod {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        &mut self.child
    }
}

impl<K: PartialEq + Clone + 'static> Widget for RebuildOnConstraints<K> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let pointer_up = matches!(
            event,
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Up)
        );
        let redraw = matches!(event, Event::Window(WindowEvent::RedrawRequested));
        let bounds = ctx.bounds();
        if (pointer_up || redraw)
            && self.refresh_for_constraints(Constraints::tight(Size::new(
                bounds.width(),
                bounds.height(),
            )))
        {
            ctx.record_rebuild(
                std::any::type_name::<Self>(),
                "constraint-derived structural key changed",
            );
            ctx.request_measure();
            ctx.request_arrange();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.refresh_for_constraints(constraints) {
            ctx.record_rebuild(
                std::any::type_name::<Self>(),
                "constraint-derived structural key changed during measure",
            );
        }
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.child);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.child);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxes {
    None,
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxes {
    const fn allows_horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    const fn allows_vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

const TOUCH_SCROLL_DRAG_THRESHOLD: f32 = 4.0;

#[derive(Debug, Clone, Copy)]
struct TouchScrollGesture {
    pointer_id: u64,
    start_position: Point,
    dragging: bool,
}

impl TouchScrollGesture {
    fn new(pointer: &PointerEvent) -> Self {
        Self {
            pointer_id: pointer.pointer_id,
            start_position: pointer.position,
            dragging: false,
        }
    }

    fn matches(self, pointer: &PointerEvent) -> bool {
        self.pointer_id == pointer.pointer_id
    }

    fn passed_threshold(self, pointer: &PointerEvent, axes: ScrollAxes) -> bool {
        let distance = pointer.position - self.start_position;
        let distance_squared = match axes {
            ScrollAxes::None => 0.0,
            ScrollAxes::Vertical => distance.y * distance.y,
            ScrollAxes::Horizontal => distance.x * distance.x,
            ScrollAxes::Both => distance.x * distance.x + distance.y * distance.y,
        };
        distance_squared >= TOUCH_SCROLL_DRAG_THRESHOLD * TOUCH_SCROLL_DRAG_THRESHOLD
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Visible,
    Clip,
    Scroll,
    Auto,
}

impl Overflow {
    const fn is_scrollable(self) -> bool {
        matches!(self, Self::Scroll | Self::Auto)
    }

    const fn clips_paint(self) -> bool {
        !matches!(self, Self::Visible)
    }
}

#[derive(Debug, Clone)]
pub struct ScrollState {
    inner: Rc<RefCell<ScrollStateInner>>,
}

#[derive(Debug, Clone)]
struct ScrollStateInner {
    axes: ScrollAxes,
    viewport: Size,
    content_size: Size,
    offset: Vector,
    pending_virtual_item: Option<usize>,
    scroll_view_id: Option<WidgetId>,
    scroll_content_id: Option<WidgetId>,
    scroll_bar_ids: Vec<WidgetId>,
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viewport_size(&self) -> Size {
        self.inner.borrow().viewport
    }

    pub fn content_size(&self) -> Size {
        self.inner.borrow().content_size
    }

    pub fn current_offset(&self) -> Vector {
        let inner = self.inner.borrow();
        clamp_shared_offset(inner.axes, inner.viewport, inner.content_size, inner.offset)
    }

    pub fn max_offset(&self) -> Vector {
        let inner = self.inner.borrow();
        shared_max_offset(inner.viewport, inner.content_size)
    }

    fn bind_scroll_view(&self, scroll_view_id: WidgetId, scroll_content_id: WidgetId) {
        let mut inner = self.inner.borrow_mut();
        inner.scroll_view_id = Some(scroll_view_id);
        inner.scroll_content_id = Some(scroll_content_id);
    }

    fn bind_scroll_bar(&self, scroll_bar_id: WidgetId) {
        let mut inner = self.inner.borrow_mut();
        if !inner.scroll_bar_ids.contains(&scroll_bar_id) {
            inner.scroll_bar_ids.push(scroll_bar_id);
        }
    }

    fn sync_metrics(&self, axes: ScrollAxes, viewport: Size, content_size: Size) -> bool {
        let mut inner = self.inner.borrow_mut();
        let next_offset = clamp_shared_offset(axes, viewport, content_size, inner.offset);
        let changed = inner.axes != axes
            || inner.viewport != viewport
            || inner.content_size != content_size
            || inner.offset != next_offset;
        inner.axes = axes;
        inner.viewport = viewport;
        inner.content_size = content_size;
        inner.offset = next_offset;
        changed
    }

    fn sync_unmeasured_axes(&self, axes: ScrollAxes) {
        let mut inner = self.inner.borrow_mut();
        if inner.viewport == Size::ZERO && inner.content_size == Size::ZERO {
            inner.axes = axes;
            inner.offset = axis_limited_offset(axes, inner.offset);
        }
    }

    /// Updates the shared scroll offset, clamped to the current viewport and
    /// content metrics. Returns whether the effective offset changed.
    ///
    /// Widgets bound to this state observe the new offset on their next layout
    /// pass; event handlers should request the corresponding invalidation.
    pub fn set_offset(&self, offset: Vector) -> bool {
        let mut inner = self.inner.borrow_mut();
        let next_offset = if inner.viewport == Size::ZERO && inner.content_size == Size::ZERO {
            axis_limited_offset(inner.axes, offset)
        } else {
            clamp_shared_offset(inner.axes, inner.viewport, inner.content_size, offset)
        };
        if inner.offset == next_offset {
            return false;
        }
        inner.offset = next_offset;
        true
    }

    /// Queues a virtual-scroll item to align with the viewport start on the
    /// next layout pass. Returns whether the pending request changed.
    ///
    /// Resolving the request during layout keeps the target aligned with the
    /// item's latest measured position, including requests made before the
    /// first render.
    pub fn scroll_to_item(&self, index: usize) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_virtual_item == Some(index) {
            return false;
        }
        inner.pending_virtual_item = Some(index);
        true
    }

    /// Queues a virtual-scroll item and invalidates the bound viewport so the
    /// jump is laid out and repainted in the frame requested by an event.
    pub fn scroll_to_item_with_ctx(&self, index: usize, ctx: &mut EventCtx) -> bool {
        if !self.scroll_to_item(index) {
            return false;
        }

        let subscribers = self.subscribers();
        if let Some(scroll_view_id) = subscribers.scroll_view_id {
            for kind in [
                InvalidationKind::Measure,
                InvalidationKind::Paint,
                InvalidationKind::HitTest,
                InvalidationKind::Semantics,
            ] {
                ctx.request(InvalidationRequest::new(
                    InvalidationTarget::Widget(scroll_view_id),
                    kind,
                ));
            }
        }
        for scroll_bar_id in subscribers.scroll_bar_ids {
            request_scroll_bar_refresh(ctx, scroll_bar_id);
        }
        true
    }

    fn take_pending_virtual_item(&self) -> Option<usize> {
        self.inner.borrow_mut().pending_virtual_item.take()
    }

    fn pending_virtual_item(&self) -> Option<usize> {
        self.inner.borrow().pending_virtual_item
    }

    fn subscribers(&self) -> ScrollStateSubscribers {
        let inner = self.inner.borrow();
        ScrollStateSubscribers {
            scroll_view_id: inner.scroll_view_id,
            scroll_content_id: inner.scroll_content_id,
            scroll_bar_ids: inner.scroll_bar_ids.clone(),
        }
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ScrollStateInner {
                axes: ScrollAxes::Vertical,
                viewport: Size::ZERO,
                content_size: Size::ZERO,
                offset: Vector::ZERO,
                pending_virtual_item: None,
                scroll_view_id: None,
                scroll_content_id: None,
                scroll_bar_ids: Vec::new(),
            })),
        }
    }
}

#[derive(Debug, Clone)]
struct ScrollStateSubscribers {
    scroll_view_id: Option<WidgetId>,
    scroll_content_id: Option<WidgetId>,
    scroll_bar_ids: Vec<WidgetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollBarAxis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollBarAppearance {
    Gutter,
    Overlay,
}

#[derive(Debug, Clone, Copy)]
struct ScrollBarMetrics {
    track: Rect,
    thumb: Rect,
    max_scroll: f32,
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

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    crate::animation::Interpolate::interpolate(from, to, amount).clamped()
}

pub struct ScrollBar {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    state: ScrollState,
    axis: ScrollBarAxis,
    name: Option<String>,
    width: Option<f32>,
    hovered: bool,
    dragging: bool,
    hover_animation: AnimatedScalar,
    drag_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    pointer_id: Option<u64>,
    drag_thumb_offset: f32,
    appearance: ScrollBarAppearance,
    focusable: bool,
}

impl ScrollBar {
    pub fn vertical(state: ScrollState) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            state,
            axis: ScrollBarAxis::Vertical,
            name: None,
            width: None,
            hovered: false,
            dragging: false,
            hover_animation: AnimatedScalar::new(0.0),
            drag_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            pointer_id: None,
            drag_thumb_offset: 0.0,
            appearance: ScrollBarAppearance::Gutter,
            focusable: true,
        }
    }

    pub fn horizontal(state: ScrollState) -> Self {
        Self {
            axis: ScrollBarAxis::Horizontal,
            ..Self::vertical(state)
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

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(8.0));
        self
    }

    fn overlay(mut self) -> Self {
        self.appearance = ScrollBarAppearance::Overlay;
        self.focusable = false;
        self
    }

    fn resolved_width(&self) -> f32 {
        let theme = self.resolved_theme();
        self.width.unwrap_or(theme.metrics.scroll_bar_thickness)
    }

    fn resolved_min_thumb_length(&self) -> f32 {
        self.resolved_theme().metrics.scroll_bar_min_thumb_length
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    fn track_rect(&self, bounds: Rect) -> Rect {
        let width = self.resolved_width();
        match self.axis {
            ScrollBarAxis::Vertical => {
                let horizontal_inset = ((bounds.width() - width) * 0.5).max(0.0);
                Rect::new(
                    bounds.x() + horizontal_inset,
                    bounds.y(),
                    width.min(bounds.width()),
                    bounds.height(),
                )
            }
            ScrollBarAxis::Horizontal => {
                let vertical_inset = ((bounds.height() - width) * 0.5).max(0.0);
                Rect::new(
                    bounds.x(),
                    bounds.y() + vertical_inset,
                    bounds.width(),
                    width.min(bounds.height()),
                )
            }
        }
    }

    fn metrics(&self, bounds: Rect) -> Option<ScrollBarMetrics> {
        let viewport = self.state.viewport_size();
        let content = self.state.content_size();
        let viewport_extent = self.axis_size(viewport);
        let content_extent = self.axis_size(content);
        let bounds_extent = self.axis_rect_length(bounds);
        if viewport_extent <= 0.0 || content_extent <= viewport_extent || bounds_extent <= 0.0 {
            return None;
        }

        let track = self.track_rect(bounds);
        let track_extent = self.axis_rect_length(track);
        let ratio = (viewport_extent / content_extent).clamp(0.08, 1.0);
        let thumb_extent = (track_extent * ratio)
            .max(self.resolved_min_thumb_length())
            .min(track_extent);
        let max_scroll = (content_extent - viewport_extent).max(0.0);
        let travel = (track_extent - thumb_extent).max(0.0);
        let offset = self
            .axis_offset(self.state.current_offset())
            .clamp(0.0, max_scroll.max(0.0));
        let thumb_start = self.axis_rect_start(track)
            + if travel <= f32::EPSILON || max_scroll <= f32::EPSILON {
                0.0
            } else {
                travel * (offset / max_scroll)
            };
        let thumb = match self.axis {
            ScrollBarAxis::Vertical => {
                Rect::new(track.x(), thumb_start, track.width(), thumb_extent)
            }
            ScrollBarAxis::Horizontal => {
                Rect::new(thumb_start, track.y(), thumb_extent, track.height())
            }
        };

        Some(ScrollBarMetrics {
            track,
            thumb,
            max_scroll,
        })
    }

    fn axis_size(&self, size: Size) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => size.height,
            ScrollBarAxis::Horizontal => size.width,
        }
    }

    fn axis_offset(&self, offset: Vector) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => offset.y,
            ScrollBarAxis::Horizontal => offset.x,
        }
    }

    fn axis_rect_start(&self, rect: Rect) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => rect.y(),
            ScrollBarAxis::Horizontal => rect.x(),
        }
    }

    fn axis_rect_length(&self, rect: Rect) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => rect.height(),
            ScrollBarAxis::Horizontal => rect.width(),
        }
    }

    fn axis_rect_max(&self, rect: Rect) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => rect.max_y(),
            ScrollBarAxis::Horizontal => rect.max_x(),
        }
    }

    fn pointer_position(&self, position: Point) -> f32 {
        match self.axis {
            ScrollBarAxis::Vertical => position.y,
            ScrollBarAxis::Horizontal => position.x,
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn set_dragging(&mut self, dragging: bool, ctx: &mut EventCtx) {
        if self.dragging != dragging {
            let theme = self.resolved_theme();
            self.dragging = dragging;
            set_press_animation_target(
                &mut self.drag_animation,
                dragging as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64, ctx: &mut EventCtx) {
        let previous_hover = self.hover_animation.value;
        let previous_drag = self.drag_animation.value;
        let previous_focus = self.focus_animation.value;
        let animating = self.hover_animation.advance(time)
            | self.drag_animation.advance(time)
            | self.focus_animation.advance(time);
        let changed = self.hover_animation.changed_since(previous_hover)
            || self.drag_animation.changed_since(previous_drag)
            || self.focus_animation.changed_since(previous_focus);

        if changed {
            ctx.request_paint();
        }
        if animating {
            ctx.request_animation_frame();
        }
    }

    fn request_dependents<C>(&self, ctx: &mut C, source_widget_id: WidgetId)
    where
        C: ScrollInvalidationCtx,
    {
        let subscribers = self.state.subscribers();
        if let Some(scroll_view_id) = subscribers.scroll_view_id
            && scroll_view_id != source_widget_id
        {
            request_scroll_view_refresh(ctx, scroll_view_id, subscribers.scroll_content_id);
        }
        for scroll_bar_id in subscribers.scroll_bar_ids {
            if scroll_bar_id != source_widget_id {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
    }

    fn set_axis_offset<C>(
        &mut self,
        ctx: &mut C,
        source_widget_id: WidgetId,
        axis_offset: f32,
    ) -> bool
    where
        C: ScrollInvalidationCtx,
    {
        let current = self.state.current_offset();
        let next = match self.axis {
            ScrollBarAxis::Vertical => Vector::new(current.x, axis_offset),
            ScrollBarAxis::Horizontal => Vector::new(axis_offset, current.y),
        };
        if !self.state.set_offset(next) {
            return false;
        }
        self.request_dependents(ctx, source_widget_id);
        true
    }

    fn set_from_pointer_position<C>(
        &mut self,
        ctx: &mut C,
        source_widget_id: WidgetId,
        metrics: ScrollBarMetrics,
        pointer_position: f32,
        drag_anchor: f32,
    ) -> bool
    where
        C: ScrollInvalidationCtx,
    {
        let thumb_extent = self.axis_rect_length(metrics.thumb);
        let travel = (self.axis_rect_length(metrics.track) - thumb_extent).max(0.0);
        let thumb_start = (pointer_position - drag_anchor).clamp(
            self.axis_rect_start(metrics.track),
            self.axis_rect_max(metrics.track) - thumb_extent,
        );
        let fraction = if travel <= f32::EPSILON {
            0.0
        } else {
            (thumb_start - self.axis_rect_start(metrics.track)) / travel
        };
        self.set_axis_offset(ctx, source_widget_id, metrics.max_scroll * fraction)
    }

    fn page_step(&self) -> f32 {
        let viewport = self.state.viewport_size();
        (self.axis_size(viewport) * 0.85).max(40.0)
    }
}

impl Widget for ScrollBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let metrics = self.metrics(ctx.bounds());

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
                if self.dragging
                    && self.pointer_id == Some(pointer.pointer_id)
                    && let Some(metrics) = metrics
                    && self.set_from_pointer_position(
                        ctx,
                        ctx.widget_id(),
                        metrics,
                        self.pointer_position(pointer.position),
                        self.drag_thumb_offset,
                    )
                {
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if !self.dragging || self.pointer_id != Some(pointer.pointer_id) {
                    self.set_hovered(false, ctx);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let Some(metrics) = metrics else {
                    return;
                };

                self.set_hovered(true, ctx);
                self.set_dragging(true, ctx);
                self.pointer_id = Some(pointer.pointer_id);
                self.drag_thumb_offset = if metrics.thumb.contains(pointer.position) {
                    self.pointer_position(pointer.position) - self.axis_rect_start(metrics.thumb)
                } else {
                    self.axis_rect_length(metrics.thumb) * 0.5
                };
                let _ = self.set_from_pointer_position(
                    ctx,
                    ctx.widget_id(),
                    metrics,
                    self.pointer_position(pointer.position),
                    self.drag_thumb_offset,
                );
                ctx.request_pointer_capture(pointer.pointer_id);
                if self.focusable {
                    ctx.request_focus();
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pointer_id == Some(pointer.pointer_id) =>
            {
                if self.dragging
                    && let Some(metrics) = metrics
                {
                    let _ = self.set_from_pointer_position(
                        ctx,
                        ctx.widget_id(),
                        metrics,
                        self.pointer_position(pointer.position),
                        self.drag_thumb_offset,
                    );
                }
                self.set_dragging(false, ctx);
                self.pointer_id = None;
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.pointer_id == Some(pointer.pointer_id) =>
            {
                self.set_dragging(false, ctx);
                self.pointer_id = None;
                self.set_hovered(false, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let current = f64::from(self.axis_offset(self.state.current_offset()));
                let next = match &semantics.action {
                    SemanticsActionRequest::Increment => Some(current + 40.0),
                    SemanticsActionRequest::Decrement => Some(current - 40.0),
                    SemanticsActionRequest::SetValue(SemanticsValue::Number(value)) => Some(*value),
                    SemanticsActionRequest::SetValue(SemanticsValue::Range { value, .. }) => {
                        Some(*value)
                    }
                    _ => None,
                };
                let Some(next) = next.filter(|value| value.is_finite()) else {
                    return;
                };
                let next = next as f32;
                if !next.is_finite() {
                    return;
                }
                if self.set_axis_offset(ctx, ctx.widget_id(), next) {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let max_scroll = self.axis_offset(self.state.max_offset());
                if max_scroll <= f32::EPSILON {
                    return;
                }

                let current = self.axis_offset(self.state.current_offset());
                let next = match key.key.as_str() {
                    "ArrowUp" if self.axis == ScrollBarAxis::Vertical => Some(current - 40.0),
                    "ArrowDown" if self.axis == ScrollBarAxis::Vertical => Some(current + 40.0),
                    "ArrowLeft" if self.axis == ScrollBarAxis::Horizontal => Some(current - 40.0),
                    "ArrowRight" if self.axis == ScrollBarAxis::Horizontal => Some(current + 40.0),
                    "PageUp" => Some(current - self.page_step()),
                    "PageDown" => Some(current + self.page_step()),
                    "Home" => Some(0.0),
                    "End" => Some(max_scroll),
                    _ => None,
                };

                if let Some(next) = next {
                    if self.set_axis_offset(ctx, ctx.widget_id(), next) {
                        ctx.request_paint();
                        ctx.request_semantics();
                    }
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_animations(*time, ctx);
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.state.bind_scroll_bar(ctx.widget_id());
        let width = self.resolved_width();
        let desired = match self.axis {
            ScrollBarAxis::Vertical => Size::new(
                width,
                if constraints.max.height.is_finite() {
                    constraints.max.height.max(0.0)
                } else {
                    40.0
                }
                .max(40.0),
            ),
            ScrollBarAxis::Horizontal => Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width.max(0.0)
                } else {
                    40.0
                }
                .max(40.0),
                width,
            ),
        };
        constraints.clamp(desired)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(metrics) = self.metrics(ctx.bounds()) else {
            return;
        };

        let theme = self.resolved_theme();
        let palette = theme.palette;
        let track = metrics.track;
        let thumb = metrics.thumb;
        let track_radius = (track.width() * 0.5).min(track.height() * 0.5);
        let thumb_radius = (thumb.width() * 0.5).min(thumb.height() * 0.5);
        let interaction = self
            .hover_animation
            .value
            .max(self.drag_animation.value)
            .max(self.focus_animation.value);
        let track_alpha = match self.appearance {
            ScrollBarAppearance::Gutter => 0.7,
            ScrollBarAppearance::Overlay => 0.08 + 0.32 * interaction,
        };
        let thumb_alpha = match self.appearance {
            ScrollBarAppearance::Gutter => 0.95,
            ScrollBarAppearance::Overlay => 0.68 + 0.27 * interaction,
        };
        let border_alpha = match self.appearance {
            ScrollBarAppearance::Gutter => 0.9,
            ScrollBarAppearance::Overlay => 0.42 + 0.36 * interaction,
        };
        ctx.fill(
            Path::rounded_rect(track, track_radius),
            palette.control_active.with_alpha(track_alpha),
        );
        ctx.fill(
            Path::rounded_rect(thumb, thumb_radius),
            mix_color(
                mix_color(
                    palette.border_hover,
                    palette.accent_hover,
                    self.hover_animation.value.max(self.focus_animation.value),
                ),
                palette.accent_pressed,
                self.drag_animation.value,
            )
            .with_alpha(thumb_alpha),
        );
        ctx.stroke(
            Path::rounded_rect(thumb, thumb_radius),
            mix_color(
                palette.border.with_alpha(border_alpha),
                palette.focus_ring,
                self.focus_animation.value,
            ),
            StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width).max(1.0)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let max_scroll = self.axis_offset(self.state.max_offset()).max(0.0);
        let current = self
            .axis_offset(self.state.current_offset())
            .clamp(0.0, max_scroll.max(0.0));
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Slider, ctx.bounds());
        node.name = self.name.clone();
        node.value = Some(SemanticsValue::Range {
            value: f64::from(current),
            min: 0.0,
            max: f64::from(max_scroll),
        });
        node.state.disabled = max_scroll <= f32::EPSILON;
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = if self.focusable {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::Increment,
                SemanticsAction::Decrement,
                SemanticsAction::SetValue,
            ]
        } else {
            vec![
                SemanticsAction::Increment,
                SemanticsAction::Decrement,
                SemanticsAction::SetValue,
            ]
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.focusable
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

const OVERLAY_SCROLL_BAR_INSET: f32 = 3.0;

struct OverlayScrollBars {
    vertical: Option<SingleChild>,
    horizontal: Option<SingleChild>,
    show_vertical: bool,
    show_horizontal: bool,
}

impl OverlayScrollBars {
    fn new(
        state: ScrollState,
        theme: Rc<RefCell<DefaultTheme>>,
        name: Option<&str>,
        axes: ScrollAxes,
    ) -> Self {
        let vertical = axes.allows_vertical().then(|| {
            let vertical_theme = Rc::clone(&theme);
            let vertical_name = name
                .map(|name| format!("{name} vertical scroll bar"))
                .unwrap_or_else(|| "Vertical scroll bar".to_string());
            SingleChild::new_with_paint_boundary(
                ScrollBar::vertical(state.clone())
                    .theme_when(move || *vertical_theme.borrow())
                    .name(vertical_name)
                    .overlay(),
            )
        });

        let horizontal = axes.allows_horizontal().then(|| {
            let horizontal_theme = Rc::clone(&theme);
            let horizontal_name = name
                .map(|name| format!("{name} horizontal scroll bar"))
                .unwrap_or_else(|| "Horizontal scroll bar".to_string());
            SingleChild::new_with_paint_boundary(
                ScrollBar::horizontal(state)
                    .theme_when(move || *horizontal_theme.borrow())
                    .name(horizontal_name)
                    .overlay(),
            )
        });

        Self {
            vertical,
            horizontal,
            show_vertical: false,
            show_horizontal: false,
        }
    }

    fn set_visibility(&mut self, axes: ScrollAxes, max_offset: Vector) {
        let show_vertical =
            self.vertical.is_some() && axes.allows_vertical() && max_offset.y > f32::EPSILON;
        let show_horizontal =
            self.horizontal.is_some() && axes.allows_horizontal() && max_offset.x > f32::EPSILON;
        self.show_vertical = show_vertical;
        self.show_horizontal = show_horizontal;
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, viewport: Size, thickness: f32) {
        if let Some(vertical) = &mut self.vertical {
            vertical.measure(
                ctx,
                Constraints::tight(Size::new(thickness, viewport.height.max(0.0))),
            );
        }
        if let Some(horizontal) = &mut self.horizontal {
            horizontal.measure(
                ctx,
                Constraints::tight(Size::new(viewport.width.max(0.0), thickness)),
            );
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect, thickness: f32) {
        let inset = OVERLAY_SCROLL_BAR_INSET
            .min((bounds.width() * 0.5).min(bounds.height() * 0.5).max(0.0));
        let vertical_height = (bounds.height()
            - inset * 2.0
            - if self.show_horizontal {
                thickness + inset
            } else {
                0.0
            })
        .max(0.0);
        let vertical_bounds = if self.show_vertical {
            Rect::new(
                (bounds.max_x() - inset - thickness).max(bounds.x()),
                bounds.y() + inset,
                thickness.min(bounds.width()),
                vertical_height,
            )
        } else {
            Rect::from_origin_size(Point::new(bounds.max_x(), bounds.max_y()), Size::ZERO)
        };
        if let Some(vertical) = &mut self.vertical {
            vertical.arrange(ctx, vertical_bounds);
        }

        if let Some(horizontal) = &mut self.horizontal {
            let horizontal_width = (bounds.width()
                - inset * 2.0
                - if self.show_vertical {
                    thickness + inset
                } else {
                    0.0
                })
            .max(0.0);
            let horizontal_bounds = if self.show_horizontal {
                Rect::new(
                    bounds.x() + inset,
                    (bounds.max_y() - inset - thickness).max(bounds.y()),
                    horizontal_width,
                    thickness.min(bounds.height()),
                )
            } else {
                Rect::from_origin_size(Point::new(bounds.max_x(), bounds.max_y()), Size::ZERO)
            };
            horizontal.arrange(ctx, horizontal_bounds);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if let Some(vertical) = &self.vertical {
            vertical.paint(ctx);
        }
        if let Some(horizontal) = &self.horizontal {
            horizontal.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.show_vertical
            && let Some(vertical) = &self.vertical
        {
            vertical.semantics(ctx);
        }
        if self.show_horizontal
            && let Some(horizontal) = &self.horizontal
        {
            horizontal.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(vertical) = &self.vertical {
            vertical.visit_children(visitor);
        }
        if let Some(horizontal) = &self.horizontal {
            horizontal.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(vertical) = &mut self.vertical {
            vertical.visit_children_mut(visitor);
        }
        if let Some(horizontal) = &mut self.horizontal {
            horizontal.visit_children_mut(visitor);
        }
    }
}

pub struct ScrollView {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Rc<dyn Fn() -> DefaultTheme>>,
    overlay_theme: Rc<RefCell<DefaultTheme>>,
    name: Option<String>,
    state: ScrollState,
    overflow_x: Overflow,
    overflow_y: Overflow,
    viewport_width_hint: bool,
    viewport_height_hint: bool,
    offset: Vector,
    content_size: Size,
    focus_animation: AnimatedScalar,
    retain_content: bool,
    overlay_scroll_bars: bool,
    overlay_bars: Option<OverlayScrollBars>,
    touch_scroll: Option<TouchScrollGesture>,
    child: SingleChild,
}

impl ScrollView {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        let theme = DefaultTheme::default();
        Self {
            theme: Box::new(theme),
            theme_reader: None,
            overlay_theme: Rc::new(RefCell::new(theme)),
            name: None,
            state: ScrollState::new(),
            overflow_x: Overflow::Clip,
            overflow_y: Overflow::Auto,
            viewport_width_hint: false,
            viewport_height_hint: false,
            offset: Vector::ZERO,
            content_size: Size::ZERO,
            focus_animation: AnimatedScalar::new(0.0),
            retain_content: false,
            overlay_scroll_bars: true,
            overlay_bars: None,
            touch_scroll: None,
            child: SingleChild::new(child),
        }
    }

    pub fn vertical<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(child)
    }

    pub fn horizontal<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(child).axes(ScrollAxes::Horizontal)
    }

    pub fn both<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(child).axes(ScrollAxes::Both)
    }

    pub fn axes(mut self, axes: ScrollAxes) -> Self {
        self.overflow_x = if axes.allows_horizontal() {
            Overflow::Auto
        } else {
            Overflow::Clip
        };
        self.overflow_y = if axes.allows_vertical() {
            Overflow::Auto
        } else {
            Overflow::Clip
        };
        self.viewport_width_hint = false;
        self.viewport_height_hint = false;
        self.state.sync_unmeasured_axes(self.scroll_axes());
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow_x = overflow;
        self.overflow_y = overflow;
        self.viewport_width_hint = overflow.is_scrollable();
        self.state.sync_unmeasured_axes(self.scroll_axes());
        self
    }

    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        self.overflow_x = overflow;
        self.viewport_width_hint = overflow.is_scrollable();
        self.state.sync_unmeasured_axes(self.scroll_axes());
        self
    }

    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        self.overflow_y = overflow;
        self.state.sync_unmeasured_axes(self.scroll_axes());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        *self.overlay_theme.borrow_mut() = theme;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Rc::new(theme));
        self
    }

    pub fn state(mut self, state: ScrollState) -> Self {
        self.state = state;
        self.state.sync_unmeasured_axes(self.scroll_axes());
        self.overlay_bars = None;
        self
    }

    /// Controls the built-in scroll bars painted over overflowing content.
    /// Disable this when composing a standalone [`ScrollBar`] with the same
    /// [`ScrollState`].
    pub fn overlay_scroll_bars(mut self, enabled: bool) -> Self {
        self.overlay_scroll_bars = enabled;
        if !enabled {
            self.overlay_bars = None;
        }
        self
    }

    pub const fn viewport_size_hint(mut self, enabled: bool) -> Self {
        self.viewport_width_hint = enabled;
        self.viewport_height_hint = enabled;
        self
    }

    pub const fn viewport_width_hint(mut self, enabled: bool) -> Self {
        self.viewport_width_hint = enabled;
        self
    }

    pub const fn viewport_height_hint(mut self, enabled: bool) -> Self {
        self.viewport_height_hint = enabled;
        self
    }

    /// Retains the complete child subtree behind a paint boundary so scrolling
    /// can move it with a composition-only transform.
    ///
    /// This is best for content that is expensive to repaint but cheap to
    /// composite. Very large text or deeply layered scenes may render faster
    /// with the default flattened path.
    pub fn retain_content_layer(mut self) -> Self {
        self.retain_content = true;
        let child = self.child;
        self.child = child.with_paint_boundary();
        self
    }

    pub const fn current_offset(&self) -> Vector {
        self.offset
    }

    pub fn set_offset(&mut self, offset: Vector) {
        self.state.sync_unmeasured_axes(self.scroll_axes());
        let _ = self.state.set_offset(offset);
        self.offset = self.state.current_offset();
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    pub fn replace_child<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.child = if self.retain_content {
            SingleChild::new_with_paint_boundary(child)
        } else {
            SingleChild::new(child)
        };
    }

    fn clamp_offset(&self, viewport: Size, offset: Vector) -> Vector {
        let max_x = (self.content_size.width - viewport.width).max(0.0);
        let max_y = (self.content_size.height - viewport.height).max(0.0);

        Vector::new(
            if self.overflow_x.is_scrollable() {
                offset.x.clamp(0.0, max_x)
            } else {
                0.0
            },
            if self.overflow_y.is_scrollable() {
                offset.y.clamp(0.0, max_y)
            } else {
                0.0
            },
        )
    }

    fn scroll_axes(&self) -> ScrollAxes {
        match (
            self.overflow_x.is_scrollable(),
            self.overflow_y.is_scrollable(),
        ) {
            (true, true) => ScrollAxes::Both,
            (true, false) => ScrollAxes::Horizontal,
            (false, true) => ScrollAxes::Vertical,
            (false, false) => ScrollAxes::None,
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    fn sync_overlay_theme(&self) -> DefaultTheme {
        let theme = self.resolved_theme();
        *self.overlay_theme.borrow_mut() = theme;
        theme
    }

    fn ensure_overlay_bars(&mut self) {
        let axes = self.scroll_axes();
        if self.overlay_scroll_bars && axes != ScrollAxes::None && self.overlay_bars.is_none() {
            self.overlay_bars = Some(OverlayScrollBars::new(
                self.state.clone(),
                Rc::clone(&self.overlay_theme),
                self.name.as_deref(),
                axes,
            ));
        }
    }

    fn touch_delta(&self, pointer: &PointerEvent) -> Vector {
        let delta = Vector::new(-pointer.delta.x, -pointer.delta.y);
        match self.scroll_axes() {
            ScrollAxes::None => Vector::ZERO,
            ScrollAxes::Vertical => Vector::new(0.0, delta.y),
            ScrollAxes::Horizontal => Vector::new(delta.x, 0.0),
            ScrollAxes::Both => delta,
        }
    }

    fn has_touch_overflow(&self) -> bool {
        let max_offset = self.state.max_offset();
        (self.scroll_axes().allows_horizontal() && max_offset.x > f32::EPSILON)
            || (self.scroll_axes().allows_vertical() && max_offset.y > f32::EPSILON)
    }

    fn handle_touch_pointer(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent, viewport: Size) {
        if pointer.pointer_kind != PointerKind::Touch {
            return;
        }

        match pointer.kind {
            PointerEventKind::Down
                if pointer.is_primary
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position)
                    && self.has_touch_overflow() =>
            {
                self.touch_scroll = Some(TouchScrollGesture::new(pointer));
            }
            PointerEventKind::Move => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if !gesture.matches(pointer) || ctx.phase() == EventPhase::Capture {
                    return;
                }
                if !gesture.dragging && !gesture.passed_threshold(pointer, self.scroll_axes()) {
                    return;
                }

                let delta = self.touch_delta(pointer);
                let relevant_delta = delta.x.abs().max(delta.y.abs());
                if relevant_delta <= f32::EPSILON {
                    if gesture.dragging {
                        ctx.set_handled();
                    }
                    return;
                }

                if self.scroll_by(viewport, delta, ctx) {
                    if !gesture.dragging {
                        if let Some(gesture) = &mut self.touch_scroll {
                            gesture.dragging = true;
                        }
                        ctx.request_pointer_capture(pointer.pointer_id);
                    }
                    ctx.set_handled();
                } else if !gesture.dragging {
                    if let Some(gesture) = &mut self.touch_scroll {
                        gesture.dragging = true;
                    }
                    // Claim the pan even at a hard boundary so controls beneath it receive
                    // Cancel and the gesture can reverse direction. Leave the event unhandled
                    // so an enclosing scroll view can still take ownership and move instead.
                    ctx.request_pointer_capture(pointer.pointer_id);
                }
            }
            PointerEventKind::Up | PointerEventKind::Cancel => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if !gesture.matches(pointer) {
                    return;
                }
                self.touch_scroll = None;
                if gesture.dragging && ctx.phase() != EventPhase::Capture {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn should_clip_paint(&self) -> bool {
        self.overflow_x.clips_paint() || self.overflow_y.clips_paint()
    }

    fn clip_rect(&self, bounds: Rect) -> Rect {
        let large = 1_000_000.0;
        let x = if self.overflow_x.clips_paint() {
            bounds.x()
        } else {
            -large
        };
        let width = if self.overflow_x.clips_paint() {
            bounds.width()
        } else {
            large * 2.0
        };
        let y = if self.overflow_y.clips_paint() {
            bounds.y()
        } else {
            -large
        };
        let height = if self.overflow_y.clips_paint() {
            bounds.height()
        } else {
            large * 2.0
        };
        Rect::new(x, y, width, height)
    }

    fn scroll_by(&mut self, viewport: Size, delta: Vector, ctx: &mut EventCtx) -> bool {
        let next = self.clamp_offset(viewport, self.offset + delta);
        if next != self.offset {
            self.offset = next;
            self.publish_state(ctx, viewport);
            ctx.request_arrange();
            if !self.retain_content {
                ctx.request_paint();
            }
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Widget(self.child.child().id()),
                InvalidationKind::Transform,
            ));
            ctx.request_semantics();
            true
        } else {
            false
        }
    }

    fn sync_state<C>(&mut self, ctx: &mut C, viewport: Size)
    where
        C: ScrollInvalidationCtx + ScrollWidgetCtx,
    {
        let state = &self.state;

        state.bind_scroll_view(ctx.widget_id(), self.child.child().id());
        if state.sync_metrics(self.scroll_axes(), viewport, self.content_size) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
        self.offset = self.clamp_offset(viewport, state.current_offset());
        if state.set_offset(self.offset) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
    }

    fn publish_state(&self, ctx: &mut EventCtx, viewport: Size) {
        let state = &self.state;

        state.bind_scroll_view(ctx.widget_id(), self.child.child().id());
        let _ = state.sync_metrics(self.scroll_axes(), viewport, self.content_size);
        if state.set_offset(self.offset) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                if scroll_bar_id != ctx.widget_id() {
                    request_scroll_bar_refresh(ctx, scroll_bar_id);
                }
            }
        }
    }

    fn advance_focus_animation(&mut self, time: f64, ctx: &mut EventCtx) {
        let previous = self.focus_animation.value;
        let animating = self.focus_animation.advance(time);
        if self.focus_animation.changed_since(previous) {
            ctx.request_paint();
        }
        if animating {
            ctx.request_animation_frame();
        }
    }
}

pub struct VirtualScrollView {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Rc<dyn Fn() -> DefaultTheme>>,
    overlay_theme: Rc<RefCell<DefaultTheme>>,
    name: Option<String>,
    padding: Insets,
    spacing: f32,
    state: ScrollState,
    offset_y: f32,
    last_arranged_offset_y: f32,
    content_height: f32,
    item_offsets: Vec<f32>,
    visible_range: Range<usize>,
    focus_animation: AnimatedScalar,
    overlay_scroll_bars: bool,
    overlay_bars: Option<OverlayScrollBars>,
    touch_scroll: Option<TouchScrollGesture>,
    children: WidgetChildren,
}

impl VirtualScrollView {
    pub fn new() -> Self {
        let theme = DefaultTheme::default();
        Self {
            theme: Box::new(theme),
            theme_reader: None,
            overlay_theme: Rc::new(RefCell::new(theme)),
            name: None,
            padding: Insets::ZERO,
            spacing: 0.0,
            state: ScrollState::new(),
            offset_y: 0.0,
            last_arranged_offset_y: 0.0,
            content_height: 0.0,
            item_offsets: Vec::new(),
            visible_range: 0..0,
            focus_animation: AnimatedScalar::new(0.0),
            overlay_scroll_bars: true,
            overlay_bars: None,
            touch_scroll: None,
            children: WidgetChildren::new(),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        *self.overlay_theme.borrow_mut() = theme;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Rc::new(theme));
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

    pub fn state(mut self, state: ScrollState) -> Self {
        self.state = state;
        self.state.sync_unmeasured_axes(ScrollAxes::Vertical);
        self.overlay_bars = None;
        self
    }

    /// Controls the built-in vertical scroll bar painted over overflowing
    /// content. Disable this when composing a standalone [`ScrollBar`] with
    /// the same [`ScrollState`].
    pub fn overlay_scroll_bars(mut self, enabled: bool) -> Self {
        self.overlay_scroll_bars = enabled;
        if !enabled {
            self.overlay_bars = None;
        }
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

    pub const fn current_offset(&self) -> Vector {
        Vector::new(0.0, self.offset_y)
    }

    pub fn set_offset(&mut self, offset: Vector) {
        self.offset_y = offset.y.max(0.0);
        let _ = self.state.set_offset(Vector::new(0.0, self.offset_y));
        self.offset_y = self.state.current_offset().y;
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    fn sync_overlay_theme(&self) -> DefaultTheme {
        let theme = self.resolved_theme();
        *self.overlay_theme.borrow_mut() = theme;
        theme
    }

    fn ensure_overlay_bars(&mut self) {
        if self.overlay_scroll_bars && self.overlay_bars.is_none() {
            self.overlay_bars = Some(OverlayScrollBars::new(
                self.state.clone(),
                Rc::clone(&self.overlay_theme),
                self.name.as_deref(),
                ScrollAxes::Vertical,
            ));
        }
    }

    fn has_touch_overflow(&self) -> bool {
        self.state.max_offset().y > f32::EPSILON
    }

    fn handle_touch_pointer(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent, viewport: Rect) {
        if pointer.pointer_kind != PointerKind::Touch {
            return;
        }

        match pointer.kind {
            PointerEventKind::Down
                if pointer.is_primary
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position)
                    && self.has_touch_overflow() =>
            {
                self.touch_scroll = Some(TouchScrollGesture::new(pointer));
            }
            PointerEventKind::Move => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if !gesture.matches(pointer) || ctx.phase() == EventPhase::Capture {
                    return;
                }
                if !gesture.dragging && !gesture.passed_threshold(pointer, ScrollAxes::Vertical) {
                    return;
                }

                let delta_y = -pointer.delta.y;
                if delta_y.abs() <= f32::EPSILON {
                    if gesture.dragging {
                        ctx.set_handled();
                    }
                    return;
                }

                if self.scroll_by(viewport, delta_y, ctx) {
                    if !gesture.dragging {
                        if let Some(gesture) = &mut self.touch_scroll {
                            gesture.dragging = true;
                        }
                        ctx.request_pointer_capture(pointer.pointer_id);
                    }
                    ctx.set_handled();
                } else if !gesture.dragging {
                    if let Some(gesture) = &mut self.touch_scroll {
                        gesture.dragging = true;
                    }
                    ctx.request_pointer_capture(pointer.pointer_id);
                }
            }
            PointerEventKind::Up | PointerEventKind::Cancel => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if !gesture.matches(pointer) {
                    return;
                }
                self.touch_scroll = None;
                if gesture.dragging && ctx.phase() != EventPhase::Capture {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }

    fn clamp_offset(&self, viewport_height: f32, offset_y: f32) -> f32 {
        let max_scroll = (self.content_height - viewport_height).max(0.0);
        offset_y.clamp(0.0, max_scroll)
    }

    fn visible_range_for_offset(&self, viewport_height: f32, offset_y: f32) -> Range<usize> {
        let overdraw = viewport_height * 0.75;
        let visible_top = (offset_y - overdraw).max(0.0);
        let visible_bottom = offset_y + viewport_height + overdraw;
        let child_count = self.children.len();
        let mut start = 0;
        while start < child_count {
            let row_top = self.item_offsets[start];
            let row_bottom = row_top + self.children.as_slice()[start].measured_size().height;
            if row_bottom >= visible_top {
                break;
            }
            start += 1;
        }

        let mut end = start;
        while end < child_count {
            if self.item_offsets[end] > visible_bottom {
                break;
            }
            end += 1;
        }

        start..end
    }

    fn exposed_viewport_strip(&self, viewport: Rect, previous_offset_y: f32) -> Option<Rect> {
        let delta_y = self.offset_y - previous_offset_y;
        if delta_y.abs() <= f32::EPSILON || viewport.is_empty() {
            return None;
        }

        let strip_height = delta_y.abs().min(viewport.height());
        if strip_height <= 0.0 {
            return None;
        }

        if delta_y > 0.0 {
            Some(Rect::new(
                viewport.x(),
                viewport.max_y() - strip_height,
                viewport.width(),
                strip_height,
            ))
        } else {
            Some(Rect::new(
                viewport.x(),
                viewport.y(),
                viewport.width(),
                strip_height,
            ))
        }
    }

    fn scroll_by(&mut self, viewport: Rect, delta_y: f32, ctx: &mut EventCtx) -> bool {
        let previous_offset_y = self.offset_y;
        let next = self.clamp_offset(viewport.height(), previous_offset_y + delta_y);
        if (next - previous_offset_y).abs() > f32::EPSILON {
            self.offset_y = next;
            self.publish_state(ctx, viewport.size);
            ctx.request_arrange();
            if let Some(exposed_strip) = self.exposed_viewport_strip(viewport, previous_offset_y) {
                ctx.request_paint_rect(exposed_strip);
            }
            for child in self.visible_children() {
                ctx.request(InvalidationRequest::new(
                    InvalidationTarget::Widget(child.id()),
                    InvalidationKind::Transform,
                ));
            }
            ctx.request_semantics();
            true
        } else {
            false
        }
    }

    fn sync_state<C>(&mut self, ctx: &mut C, viewport: Size)
    where
        C: ScrollInvalidationCtx + ScrollWidgetCtx,
    {
        let state = &self.state;

        state.bind_scroll_view(ctx.widget_id(), ctx.widget_id());
        let content_size = Size::new(viewport.width, self.content_height);
        let mut state_changed = state.sync_metrics(ScrollAxes::Vertical, viewport, content_size);
        if let Some(index) = state.take_pending_virtual_item()
            && let Some(item_offset) = self.item_offsets.get(index).copied()
        {
            let target_offset = self.clamp_offset(viewport.height, item_offset);
            state_changed |= state.set_offset(Vector::new(0.0, target_offset));
        }
        self.offset_y = self.clamp_offset(viewport.height, state.current_offset().y);
        state_changed |= state.set_offset(Vector::new(0.0, self.offset_y));
        if state_changed {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
    }

    fn publish_state(&self, ctx: &mut EventCtx, viewport: Size) {
        let state = &self.state;

        state.bind_scroll_view(ctx.widget_id(), ctx.widget_id());
        let content_size = Size::new(viewport.width, self.content_height);
        let _ = state.sync_metrics(ScrollAxes::Vertical, viewport, content_size);
        if state.set_offset(Vector::new(0.0, self.offset_y)) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                if scroll_bar_id != ctx.widget_id() {
                    request_scroll_bar_refresh(ctx, scroll_bar_id);
                }
            }
        }
    }

    fn update_visible_range(&mut self, viewport_height: f32) {
        // Extend the visible window by a buffer zone so small scrolls keep a
        // stable widget set and avoid unnecessary repaint churn.
        self.visible_range = self.visible_range_for_offset(viewport_height, self.offset_y);
    }

    fn visible_children(&self) -> &[WidgetPod] {
        &self.children.as_slice()[self.visible_range.clone()]
    }

    fn pending_visible_range(&self) -> Option<Range<usize>> {
        let index = self.state.pending_virtual_item()?;
        let offset_y = self.item_offsets.get(index).copied()?;
        Some(self.visible_range_for_offset(self.state.viewport_size().height, offset_y))
    }

    fn advance_focus_animation(&mut self, time: f64, ctx: &mut EventCtx) {
        let previous = self.focus_animation.value;
        let animating = self.focus_animation.advance(time);
        if self.focus_animation.changed_since(previous) {
            ctx.request_paint();
        }
        if animating {
            ctx.request_animation_frame();
        }
    }
}

impl Default for VirtualScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_overlay_theme();
        let viewport = ctx.bounds().size;
        if let Event::Pointer(pointer) = event {
            self.handle_touch_pointer(ctx, pointer, viewport);
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Scroll
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                if self.scroll_by(viewport, Vector::new(-delta.x, -delta.y), ctx) {
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if ctx.phase() != EventPhase::Capture
                    && ctx.is_focused()
                    && key.state == sui_core::KeyState::Pressed =>
            {
                let line = 40.0;
                let page = (viewport.height * 0.85).max(line);
                let delta = match key.key.as_str() {
                    "ArrowUp" => Some(Vector::new(0.0, -line)),
                    "ArrowDown" => Some(Vector::new(0.0, line)),
                    "ArrowLeft" => Some(Vector::new(-line, 0.0)),
                    "ArrowRight" => Some(Vector::new(line, 0.0)),
                    "PageUp" => Some(Vector::new(0.0, -page)),
                    "PageDown" => Some(Vector::new(0.0, page)),
                    "Home" => Some(Vector::new(-self.offset.x, -self.offset.y)),
                    "End" => Some(Vector::new(
                        self.content_size.width - viewport.width - self.offset.x,
                        self.content_size.height - viewport.height - self.offset.y,
                    )),
                    _ => None,
                };

                if let Some(delta) = delta
                    && self.scroll_by(viewport, delta, ctx)
                {
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Down
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                ctx.request_focus();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_focus_animation(*time, ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.sync_overlay_theme();
        self.ensure_overlay_bars();
        let viewport_hint = Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                constraints.min.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                constraints.min.height
            },
        );
        let mut child_constraints = constraints.loosen();
        if self.overflow_x.is_scrollable() {
            child_constraints.max.width = if self.viewport_width_hint {
                viewport_hint.width
            } else {
                f32::INFINITY
            };
        } else if constraints.max.width.is_finite() {
            child_constraints.min.width = constraints.max.width;
            child_constraints.max.width = constraints.max.width;
        }

        if self.overflow_y.is_scrollable() {
            child_constraints.max.height = if self.viewport_height_hint {
                viewport_hint.height
            } else {
                f32::INFINITY
            };
        } else if constraints.max.height.is_finite() {
            child_constraints.min.height = constraints.max.height;
            child_constraints.max.height = constraints.max.height;
        }

        let child_size = self.child.measure(ctx, child_constraints);
        self.content_size = child_size;

        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                child_size.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                child_size.height
            },
        ));
        self.sync_state(ctx, viewport);
        let axes = self.scroll_axes();
        let max_offset = self.state.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(axes, max_offset);
            overlay_bars.measure(ctx, viewport, theme.metrics.scroll_bar_thickness);
        }

        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.sync_overlay_theme();
        let previous_offset = self.offset;
        self.sync_state(ctx, bounds.size);
        // A bound scroll bar writes through shared state. Keep flattened content
        // dirty when redraw handling eagerly consumes this arrange pass.
        if self.offset != previous_offset && !self.retain_content {
            ctx.request_paint();
        }
        let measured = self.child.child().measured_size();
        let child_size = Size::new(
            if self.overflow_x == Overflow::Clip {
                bounds.width()
            } else {
                measured.width
            },
            if self.overflow_y == Overflow::Clip {
                bounds.height()
            } else {
                measured.height
            },
        );
        self.child.arrange(
            ctx,
            Rect::from_origin_size(
                Point::new(bounds.x() - self.offset.x, bounds.y() - self.offset.y),
                child_size,
            ),
        );
        let axes = self.scroll_axes();
        let max_offset = self.state.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(axes, max_offset);
            overlay_bars.arrange(ctx, bounds, theme.metrics.scroll_bar_thickness);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.sync_overlay_theme();
        if self.should_clip_paint() {
            ctx.push_clip_rect(self.clip_rect(ctx.bounds()));
            self.child.paint(ctx);
            ctx.pop_clip();
        } else {
            self.child.paint(ctx);
        }
        if let Some(overlay_bars) = &self.overlay_bars {
            ctx.push_clip_rect(ctx.bounds());
            overlay_bars.paint(ctx);
            ctx.pop_clip();
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.scroll_axes() == ScrollAxes::None {
                LayerCompositionMode::Normal
            } else {
                LayerCompositionMode::Scroll
            },
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
        node.name = self.name.clone();
        node.actions = vec![SemanticsAction::Focus];
        node.state.focused = ctx.is_focused();
        ctx.push(node);
        self.child.semantics(ctx);
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.sync_overlay_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.visit_children_mut(visitor);
        }
    }
}

impl Widget for VirtualScrollView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_overlay_theme();
        let viewport = self.viewport_rect(ctx.bounds());
        if let Event::Pointer(pointer) = event {
            self.handle_touch_pointer(ctx, pointer, viewport);
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Scroll
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                if self.scroll_by(viewport, -delta.y, ctx) {
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if ctx.phase() != EventPhase::Capture
                    && ctx.is_focused()
                    && key.state == sui_core::KeyState::Pressed =>
            {
                let line = 40.0;
                let page = (viewport.height() * 0.85).max(line);
                let delta = match key.key.as_str() {
                    "ArrowUp" => Some(-line),
                    "ArrowDown" => Some(line),
                    "PageUp" => Some(-page),
                    "PageDown" => Some(page),
                    "Home" => Some(-self.offset_y),
                    "End" => Some(self.content_height - viewport.height() - self.offset_y),
                    _ => None,
                };

                if let Some(delta) = delta
                    && self.scroll_by(viewport, delta, ctx)
                {
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Down
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                ctx.request_focus();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_focus_animation(*time, ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.sync_overlay_theme();
        self.ensure_overlay_bars();
        let previous_content_height = self.content_height;
        let previous_item_offsets = std::mem::take(&mut self.item_offsets);
        let previous_visible_range = self.visible_range.clone();
        let available_width = if constraints.max.width.is_finite() {
            (constraints.max.width - (self.padding.left + self.padding.right)).max(0.0)
        } else {
            f32::INFINITY
        };
        let child_constraints = Constraints::new(
            Size::new(
                if available_width.is_finite() {
                    available_width
                } else {
                    0.0
                },
                0.0,
            ),
            Size::new(available_width, f32::INFINITY),
        );

        self.item_offsets.reserve(self.children.len());
        let mut content_width: f32 = 0.0;
        let mut content_height = 0.0;
        for child in self.children.as_mut_slice() {
            let child_size = child.measure(ctx, child_constraints);
            self.item_offsets.push(content_height);
            content_width = content_width.max(child_size.width);
            content_height += child_size.height;
            content_height += self.spacing;
        }
        if !self.item_offsets.is_empty() {
            content_height -= self.spacing;
        }
        self.content_height = content_height;

        let desired = Size::new(
            content_width + self.padding.left + self.padding.right,
            content_height + self.padding.top + self.padding.bottom,
        );
        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                desired.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                desired.height
            },
        ));

        let viewport = self.viewport_rect(Rect::from_origin_size(Point::ZERO, size));
        self.sync_state(ctx, viewport.size);
        self.update_visible_range(viewport.height());
        let max_offset = self.state.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(ScrollAxes::Vertical, max_offset);
            overlay_bars.measure(ctx, size, theme.metrics.scroll_bar_thickness);
        }
        if previous_content_height != self.content_height
            || previous_item_offsets != self.item_offsets
            || previous_visible_range != self.visible_range
        {
            ctx.request_paint();
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.sync_overlay_theme();
        let viewport = self.viewport_rect(bounds);
        let previous_offset_y = self.offset_y;
        self.sync_state(ctx, viewport.size);
        // Shared-state movement changes both row positions and the visible window.
        if (self.offset_y - previous_offset_y).abs() > f32::EPSILON {
            ctx.request_paint();
        }
        let previous_visible_range = self.visible_range.clone();
        let previous_arranged_offset_y = self.last_arranged_offset_y;
        self.update_visible_range(viewport.height());
        if self.visible_range != previous_visible_range
            && (self.offset_y - previous_arranged_offset_y).abs() <= f32::EPSILON
        {
            ctx.request_paint();
        }
        self.last_arranged_offset_y = self.offset_y;
        let viewport_width = viewport.width();
        let visible_range = self.visible_range.clone();
        let offset_y = self.offset_y;
        let item_offsets = self.item_offsets[visible_range.clone()].to_vec();
        for (relative_index, child) in self.children.as_mut_slice()[visible_range]
            .iter_mut()
            .enumerate()
        {
            let child_size = child.measured_size();
            child.arrange(
                ctx,
                Rect::from_origin_size(
                    Point::new(
                        viewport.x(),
                        viewport.y() + item_offsets[relative_index] - offset_y,
                    ),
                    Size::new(
                        if viewport_width.is_finite() {
                            viewport_width
                        } else {
                            child_size.width
                        },
                        child_size.height,
                    ),
                ),
            );
        }
        let max_offset = self.state.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(ScrollAxes::Vertical, max_offset);
            overlay_bars.arrange(ctx, bounds, theme.metrics.scroll_bar_thickness);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.sync_overlay_theme();
        let viewport = self.viewport_rect(ctx.bounds());
        ctx.push_clip_rect(viewport);
        for child in self.visible_children() {
            child.paint(ctx);
        }
        ctx.pop_clip();
        if let Some(overlay_bars) = &self.overlay_bars {
            ctx.push_clip_rect(ctx.bounds());
            overlay_bars.paint(ctx);
            ctx.pop_clip();
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Scroll,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
        node.name = self.name.clone();
        node.actions = vec![SemanticsAction::Focus];
        node.state.focused = ctx.is_focused();
        ctx.push(node);
        for child in self.visible_children() {
            child.semantics(ctx);
        }
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.sync_overlay_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        let pending_range = self.pending_visible_range();
        for (index, child) in self.children.as_slice().iter().enumerate() {
            if self.visible_range.contains(&index)
                || pending_range
                    .as_ref()
                    .is_some_and(|range| range.contains(&index))
            {
                visitor.visit(child);
            }
        }
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let visible_range = self.visible_range.clone();
        let pending_range = self.pending_visible_range();
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if visible_range.contains(&index)
                || pending_range
                    .as_ref()
                    .is_some_and(|range| range.contains(&index))
            {
                visitor.visit(child);
            }
        }
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.visit_children_mut(visitor);
        }
    }
}

fn scroll_delta_to_offset(delta: ScrollDelta) -> Vector {
    match delta {
        ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        ScrollDelta::Pixels(delta) => delta,
    }
}

fn axis_limited_offset(axes: ScrollAxes, offset: Vector) -> Vector {
    Vector::new(
        if axes.allows_horizontal() {
            offset.x.max(0.0)
        } else {
            0.0
        },
        if axes.allows_vertical() {
            offset.y.max(0.0)
        } else {
            0.0
        },
    )
}

fn shared_max_offset(viewport: Size, content_size: Size) -> Vector {
    Vector::new(
        (content_size.width - viewport.width).max(0.0),
        (content_size.height - viewport.height).max(0.0),
    )
}

fn clamp_shared_offset(
    axes: ScrollAxes,
    viewport: Size,
    content_size: Size,
    offset: Vector,
) -> Vector {
    let max = shared_max_offset(viewport, content_size);
    Vector::new(
        if axes.allows_horizontal() {
            offset.x.clamp(0.0, max.x)
        } else {
            0.0
        },
        if axes.allows_vertical() {
            offset.y.clamp(0.0, max.y)
        } else {
            0.0
        },
    )
}

trait ScrollInvalidationCtx {
    fn push_invalidation(&mut self, request: InvalidationRequest);
}

trait ScrollWidgetCtx {
    fn widget_id(&self) -> WidgetId;
}

impl ScrollInvalidationCtx for EventCtx {
    fn push_invalidation(&mut self, request: InvalidationRequest) {
        self.request(request);
    }
}

impl ScrollInvalidationCtx for MeasureCtx {
    fn push_invalidation(&mut self, request: InvalidationRequest) {
        self.request(request);
    }
}

impl ScrollInvalidationCtx for ArrangeCtx {
    fn push_invalidation(&mut self, request: InvalidationRequest) {
        self.request(request);
    }
}

impl ScrollWidgetCtx for EventCtx {
    fn widget_id(&self) -> WidgetId {
        self.widget_id()
    }
}

impl ScrollWidgetCtx for MeasureCtx {
    fn widget_id(&self) -> WidgetId {
        self.widget_id()
    }
}

impl ScrollWidgetCtx for ArrangeCtx {
    fn widget_id(&self) -> WidgetId {
        self.widget_id()
    }
}

fn request_scroll_bar_refresh<C>(ctx: &mut C, widget_id: WidgetId)
where
    C: ScrollInvalidationCtx,
{
    for kind in [
        InvalidationKind::Paint,
        InvalidationKind::HitTest,
        InvalidationKind::Semantics,
    ] {
        ctx.push_invalidation(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            kind,
        ));
    }
}

fn request_scroll_view_refresh<C>(ctx: &mut C, widget_id: WidgetId, content_id: Option<WidgetId>)
where
    C: ScrollInvalidationCtx,
{
    for kind in [InvalidationKind::Arrange, InvalidationKind::Semantics] {
        ctx.push_invalidation(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            kind,
        ));
    }
    if let Some(content_id) = content_id {
        ctx.push_invalidation(InvalidationRequest::new(
            InvalidationTarget::Widget(content_id),
            InvalidationKind::Transform,
        ));
    }
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    ctx.dpi().physical_pixels_to_logical(value)
}

fn inset_constraints(constraints: Constraints, insets: Insets) -> Constraints {
    let horizontal = insets.left + insets.right;
    let vertical = insets.top + insets.bottom;

    Constraints::new(
        Size::new(
            (constraints.min.width - horizontal).max(0.0),
            (constraints.min.height - vertical).max(0.0),
        ),
        Size::new(
            (constraints.max.width - horizontal).max(0.0),
            (constraints.max.height - vertical).max(0.0),
        ),
    )
}

fn expand_size(size: Size, insets: Insets) -> Size {
    Size::new(
        size.width + insets.left + insets.right,
        size.height + insets.top + insets.bottom,
    )
}

fn inset_rect(rect: Rect, insets: Insets) -> Rect {
    Rect::new(
        rect.x() + insets.left,
        rect.y() + insets.top,
        (rect.width() - (insets.left + insets.right)).max(0.0),
        (rect.height() - (insets.top + insets.bottom)).max(0.0),
    )
}

fn aligned_child_constraints(
    constraints: Constraints,
    horizontal: Alignment,
    vertical: Alignment,
) -> Constraints {
    Constraints::new(
        Size::new(
            if horizontal == Alignment::Stretch && constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                0.0
            },
            if vertical == Alignment::Stretch && constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                0.0
            },
        ),
        constraints.max,
    )
}

fn stretched_dimension(alignment: Alignment, max: f32, fallback: f32) -> f32 {
    if alignment == Alignment::Stretch && max.is_finite() {
        max
    } else {
        fallback
    }
}

fn aligned_offset(alignment: Alignment, remaining: f32) -> f32 {
    let remaining = remaining.max(0.0);
    match alignment {
        Alignment::Start | Alignment::Stretch => 0.0,
        Alignment::Center => remaining * 0.5,
        Alignment::End => remaining,
    }
}

fn sized_box_constraints(
    constraints: Constraints,
    width: Option<f32>,
    height: Option<f32>,
) -> Constraints {
    Constraints::new(
        Size::new(width.unwrap_or(0.0), height.unwrap_or(0.0)),
        Size::new(
            width.unwrap_or(constraints.max.width),
            height.unwrap_or(constraints.max.height),
        ),
    )
}

fn axis_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn axis_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn axis_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

fn stack_arranged_child_size(
    axis: Axis,
    alignment: Alignment,
    measured: Size,
    cross_available: f32,
) -> Size {
    let main = axis_main(axis, measured);
    let measured_cross = axis_cross(axis, measured);
    let cross = if alignment == Alignment::Stretch {
        cross_available
    } else {
        measured_cross.min(cross_available)
    };
    axis_size(axis, main, cross)
}

fn axis_point(axis: Axis, main: f32, cross: f32) -> Point {
    match axis {
        Axis::Horizontal => Point::new(main, cross),
        Axis::Vertical => Point::new(cross, main),
    }
}

fn stack_child_constraints(
    axis: Axis,
    remaining_main: f32,
    max_cross: f32,
    stretch_cross: bool,
) -> Constraints {
    let min_cross = if stretch_cross { max_cross } else { 0.0 };

    match axis {
        Axis::Horizontal => Constraints::new(
            Size::new(0.0, min_cross),
            Size::new(remaining_main, max_cross),
        ),
        Axis::Vertical => Constraints::new(
            Size::new(min_cross, 0.0),
            Size::new(max_cross, remaining_main),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use super::{
        Align, Background, Dock, FixedPaneSplit, Flex, MeasuredBottomDock, Overflow, Padding,
        RebuildOnChange, RebuildOnConstraints, ScrollAxes, ScrollBar, ScrollState, ScrollView,
        SemanticRegion, SizedBox, Stack, SwitchView, TrailingSlotRow, VirtualScrollView,
    };
    use crate::{DefaultTheme, SplitView};
    use sui_core::{
        Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point,
        PointerButton, PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect,
        ScrollDelta, SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole,
        SemanticsValue, Size, Vector, WidgetId, WindowEvent,
    };
    use sui_layout::{Alignment, Axis, Constraints, FlexItem, FlexWrap, Padding as Insets};
    use sui_reactive::Signal;
    use sui_runtime::{
        Application, ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode,
        PaintCtx, RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetGraphSnapshot,
        WidgetNodeSnapshot, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
    };
    use sui_scene::{Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor};

    struct FixedBox {
        size: Size,
        color: Color,
    }

    impl FixedBox {
        fn new(size: Size, color: Color) -> Self {
            Self { size, color }
        }
    }

    impl Widget for FixedBox {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.size)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(self.color);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            ctx.push(SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            ));
        }
    }

    #[test]
    fn rebuild_on_change_rebuilds_child_only_when_key_changes() {
        let key = Rc::new(Cell::new(1usize));
        let builds = Rc::new(RefCell::new(Vec::new()));
        let key_reader = Rc::clone(&key);
        let build_log = Rc::clone(&builds);

        let mut host = RebuildOnChange::new(
            move || key_reader.get(),
            move |value| {
                build_log.borrow_mut().push(*value);
                WidgetPod::new(FixedBox::new(
                    Size::new(*value as f32, 10.0),
                    Color::srgba(0.0, 0.0, 0.0, 1.0),
                ))
            },
        );

        assert_eq!(*builds.borrow(), vec![1]);
        assert!(!host.refresh());
        assert_eq!(*builds.borrow(), vec![1]);

        key.set(2);
        assert!(host.refresh());
        assert_eq!(*builds.borrow(), vec![1, 2]);
    }

    #[test]
    fn rebuild_on_change_reports_structural_reason() -> sui_core::Result<()> {
        let key = Rc::new(Cell::new(1usize));
        let key_reader = Rc::clone(&key);
        let (mut runtime, window_id) = build_runtime(RebuildOnChange::new(
            move || key_reader.get(),
            |value| {
                WidgetPod::new(FixedBox::new(
                    Size::new(*value as f32 * 20.0, 20.0),
                    Color::BLACK,
                ))
            },
        ));

        runtime.render(window_id)?;
        key.set(2);
        runtime.handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))?;
        let output = runtime.render(window_id)?;

        assert!(output.diagnostics.widget_rebuilds.iter().any(|sample| {
            sample
                .reason
                .contains("caller-supplied structural key changed")
        }));
        Ok(())
    }

    #[test]
    fn rebuild_on_change_observable_rebuilds_without_redraw_polling() -> sui_core::Result<()> {
        let key = Signal::named("structural_mode", 1usize);
        let (mut runtime, window_id) =
            build_runtime(RebuildOnChange::new_observable(key.clone(), |value| {
                WidgetPod::new(FixedBox::new(
                    Size::new(*value as f32 * 20.0, 20.0),
                    Color::BLACK,
                ))
            }));

        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(20.0, 20.0));

        assert!(key.set(2));
        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(40.0, 20.0));
        assert!(output.diagnostics.widget_rebuilds.iter().any(|sample| {
            sample
                .reason
                .contains("caller-supplied structural key changed")
        }));
        Ok(())
    }

    #[test]
    fn rebuild_on_constraints_rebuilds_child_when_breakpoint_changes() {
        let builds = Rc::new(RefCell::new(Vec::new()));
        let build_log = Rc::clone(&builds);
        let mut host = RebuildOnConstraints::new(
            false,
            |constraints| constraints.max.width >= 420.0,
            move |wide| {
                build_log.borrow_mut().push(*wide);
                let width = if *wide { 24.0 } else { 12.0 };
                WidgetPod::new(FixedBox::new(
                    Size::new(width, 10.0),
                    Color::srgba(0.0, 0.0, 0.0, 1.0),
                ))
            },
        );

        assert_eq!(*builds.borrow(), vec![false]);
        let constraints = Constraints::tight(Size::new(520.0, 80.0));
        assert!(host.refresh_for_constraints(constraints));
        assert_eq!(*builds.borrow(), vec![false, true]);

        let constraints = Constraints::tight(Size::new(560.0, 80.0));
        assert!(!host.refresh_for_constraints(constraints));
        assert_eq!(*builds.borrow(), vec![false, true]);
    }

    struct OverflowingBox {
        size: Size,
        color: Color,
    }

    impl OverflowingBox {
        fn new(size: Size, color: Color) -> Self {
            Self { size, color }
        }
    }

    impl Widget for OverflowingBox {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
            self.size
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(self.color);
        }
    }

    struct PaintCounterBox {
        size: Size,
        color: Color,
        counts: Rc<RefCell<Vec<usize>>>,
        index: usize,
    }

    impl PaintCounterBox {
        fn new(size: Size, color: Color, counts: Rc<RefCell<Vec<usize>>>, index: usize) -> Self {
            Self {
                size,
                color,
                counts,
                index,
            }
        }
    }

    impl Widget for PaintCounterBox {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.size)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counts.borrow_mut()[self.index] += 1;
            ctx.fill_bounds(self.color);
        }
    }

    struct LayeredScrollPaintBox {
        size: Size,
        color: Color,
        paints: Rc<RefCell<usize>>,
    }

    impl Widget for LayeredScrollPaintBox {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if ctx.phase() == EventPhase::Target
                && matches!(event, Event::Pointer(pointer) if pointer.kind == PointerEventKind::Scroll)
            {
                ctx.request_paint();
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.size)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            *self.paints.borrow_mut() += 1;
            ctx.fill_bounds(self.color);
        }

        fn layer_options(&self) -> LayerOptions {
            LayerOptions {
                paint_boundary: PaintBoundaryMode::Explicit,
                composition_mode: LayerCompositionMode::Normal,
            }
        }
    }

    struct HitTestBox {
        size: Size,
        presses: Rc<RefCell<Vec<usize>>>,
        index: usize,
    }

    impl HitTestBox {
        fn new(size: Size, presses: Rc<RefCell<Vec<usize>>>, index: usize) -> Self {
            Self {
                size,
                presses,
                index,
            }
        }
    }

    impl Widget for HitTestBox {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if ctx.phase() == EventPhase::Target
                && matches!(
                    event,
                    Event::Pointer(pointer)
                        if pointer.kind == PointerEventKind::Down
                            && ctx.bounds().contains(pointer.position)
                )
            {
                self.presses.borrow_mut()[self.index] += 1;
                ctx.set_handled();
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.size)
        }
    }

    struct ConstraintProbe {
        size: Size,
        seen: Rc<RefCell<Vec<Constraints>>>,
    }

    impl ConstraintProbe {
        fn new(size: Size, seen: Rc<RefCell<Vec<Constraints>>>) -> Self {
            Self { size, seen }
        }
    }

    impl Widget for ConstraintProbe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            self.seen.borrow_mut().push(constraints);
            constraints.clamp(self.size)
        }
    }

    struct ExpandingLayerBox {
        collapsed_size: Size,
        expanded_size: Size,
        color: Color,
        counts: Rc<RefCell<Vec<usize>>>,
        index: usize,
        expanded: bool,
    }

    impl ExpandingLayerBox {
        fn new(
            collapsed_size: Size,
            expanded_size: Size,
            color: Color,
            counts: Rc<RefCell<Vec<usize>>>,
            index: usize,
        ) -> Self {
            Self {
                collapsed_size,
                expanded_size,
                color,
                counts,
                index,
                expanded: false,
            }
        }
    }

    impl Widget for ExpandingLayerBox {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Pointer(pointer) = event
                && pointer.kind == PointerEventKind::Down
                && ctx.bounds().contains(pointer.position)
                && !self.expanded
            {
                self.expanded = true;
                ctx.request_measure();
                ctx.request_paint();
                ctx.set_handled();
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(if self.expanded {
                self.expanded_size
            } else {
                self.collapsed_size
            })
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counts.borrow_mut()[self.index] += 1;
            ctx.fill_bounds(self.color);
        }

        fn layer_options(&self) -> LayerOptions {
            LayerOptions {
                paint_boundary: PaintBoundaryMode::Explicit,
                composition_mode: LayerCompositionMode::Normal,
            }
        }
    }

    struct ScrollViewNoPaint {
        axes: ScrollAxes,
        name: Option<String>,
        offset: Vector,
        content_size: Size,
        child: SingleChild,
    }

    impl ScrollViewNoPaint {
        fn vertical<W>(child: W) -> Self
        where
            W: Widget + 'static,
        {
            Self {
                axes: ScrollAxes::Vertical,
                name: None,
                offset: Vector::ZERO,
                content_size: Size::ZERO,
                child: SingleChild::new(child),
            }
        }

        fn name(mut self, name: impl Into<String>) -> Self {
            self.name = Some(name.into());
            self
        }

        fn clamp_offset(&self, viewport: Size, offset: Vector) -> Vector {
            let max_x = (self.content_size.width - viewport.width).max(0.0);
            let max_y = (self.content_size.height - viewport.height).max(0.0);

            Vector::new(
                if self.axes.allows_horizontal() {
                    offset.x.clamp(0.0, max_x)
                } else {
                    0.0
                },
                if self.axes.allows_vertical() {
                    offset.y.clamp(0.0, max_y)
                } else {
                    0.0
                },
            )
        }
    }

    impl Widget for ScrollViewNoPaint {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            let viewport = ctx.bounds().size;

            match event {
                Event::Pointer(pointer)
                    if pointer.kind == sui_core::PointerEventKind::Scroll
                        && ctx.phase() != EventPhase::Capture
                        && ctx.bounds().contains(pointer.position) =>
                {
                    let delta = pointer
                        .scroll_delta
                        .map(super::scroll_delta_to_offset)
                        .unwrap_or(pointer.delta);
                    let next =
                        self.clamp_offset(viewport, self.offset + Vector::new(-delta.x, -delta.y));
                    if next != self.offset {
                        self.offset = next;
                        ctx.request_arrange();
                        ctx.request(InvalidationRequest::new(
                            InvalidationTarget::Widget(self.child.child().id()),
                            InvalidationKind::Transform,
                        ));
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                }
                _ => {}
            }
        }

        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let mut child_constraints = constraints.loosen();
            if self.axes.allows_horizontal() {
                child_constraints.max.width = f32::INFINITY;
            } else if constraints.max.width.is_finite() {
                child_constraints.min.width = constraints.max.width;
                child_constraints.max.width = constraints.max.width;
            }

            if self.axes.allows_vertical() {
                child_constraints.max.height = f32::INFINITY;
            } else if constraints.max.height.is_finite() {
                child_constraints.min.height = constraints.max.height;
                child_constraints.max.height = constraints.max.height;
            }

            let child_size = self.child.measure(ctx, child_constraints);
            self.content_size = child_size;

            let viewport = constraints.clamp(Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width
                } else {
                    child_size.width
                },
                if constraints.max.height.is_finite() {
                    constraints.max.height
                } else {
                    child_size.height
                },
            ));
            self.offset = self.clamp_offset(viewport, self.offset);

            viewport
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
            self.child.arrange(
                ctx,
                Rect::from_origin_size(
                    Point::new(bounds.x() - self.offset.x, bounds.y() - self.offset.y),
                    self.child.child().measured_size(),
                ),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.push_clip_rect(ctx.bounds());
            self.child.paint(ctx);
            ctx.pop_clip();
        }

        fn layer_options(&self) -> LayerOptions {
            LayerOptions {
                paint_boundary: PaintBoundaryMode::Explicit,
                composition_mode: LayerCompositionMode::Scroll,
            }
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
            node.name = self.name.clone();
            node.actions = vec![sui_core::SemanticsAction::Focus];
            node.state.focused = ctx.is_focused();
            ctx.push(node);
            self.child.semantics(ctx);
        }

        fn accepts_focus(&self) -> bool {
            true
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.child.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            self.child.visit_children_mut(visitor);
        }
    }

    struct ScrollBarHost {
        spacing: f32,
        content: SingleChild,
        scroll_bar: SingleChild,
    }

    impl ScrollBarHost {
        fn new<W, S>(content: W, scroll_bar: S) -> Self
        where
            W: Widget + 'static,
            S: Widget + 'static,
        {
            Self {
                spacing: 0.0,
                content: SingleChild::new(content),
                scroll_bar: SingleChild::new(scroll_bar),
            }
        }
    }

    impl Widget for ScrollBarHost {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let scroll_bar_size = self.scroll_bar.measure(
                ctx,
                Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
            );
            let content_constraints = Constraints::new(
                Size::new(
                    (constraints.min.width - scroll_bar_size.width - self.spacing).max(0.0),
                    constraints.min.height,
                ),
                Size::new(
                    (constraints.max.width - scroll_bar_size.width - self.spacing).max(0.0),
                    constraints.max.height,
                ),
            );
            let content_size = self.content.measure(ctx, content_constraints);
            constraints.clamp(Size::new(
                content_size.width + scroll_bar_size.width + self.spacing,
                content_size.height.max(scroll_bar_size.height),
            ))
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
            let scroll_bar_size = self.scroll_bar.child().measured_size();
            let content_width = (bounds.width() - scroll_bar_size.width - self.spacing).max(0.0);
            self.content.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), content_width, bounds.height()),
            );
            self.scroll_bar.arrange(
                ctx,
                Rect::new(
                    bounds.max_x() - scroll_bar_size.width,
                    bounds.y(),
                    scroll_bar_size.width,
                    bounds.height(),
                ),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.content.paint(ctx);
            self.scroll_bar.paint(ctx);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            self.content.semantics(ctx);
            self.scroll_bar.semantics(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.content.visit_children(visitor);
            self.scroll_bar.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            self.content.visit_children_mut(visitor);
            self.scroll_bar.visit_children_mut(visitor);
        }
    }

    fn render_root<W>(root: W) -> (RenderOutput, WidgetGraphSnapshot)
    where
        W: Widget + 'static,
    {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Containers").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();
        (output, graph)
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

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Containers").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn touch_pointer(
        kind: PointerEventKind,
        pointer_id: u64,
        position: Point,
        delta: Vector,
    ) -> PointerEvent {
        let mut pointer = PointerEvent::new(kind, position);
        pointer.pointer_id = pointer_id;
        pointer.pointer_kind = PointerKind::Touch;
        pointer.delta = delta;
        pointer.is_primary = true;
        match kind {
            PointerEventKind::Down => {
                pointer.button = Some(PointerButton::Primary);
                pointer.buttons = PointerButtons::new(1);
            }
            PointerEventKind::Move => {
                pointer.buttons = PointerButtons::new(1);
            }
            PointerEventKind::Up => {
                pointer.button = Some(PointerButton::Primary);
            }
            _ => {}
        }
        pointer
    }

    fn drag_vertical_scroll_bar(
        runtime: &mut Runtime,
        window_id: sui_core::WindowId,
        bounds: Rect,
        pointer_id: u64,
        delta_y: f32,
    ) {
        let start = Point::new(bounds.x() + bounds.width() * 0.5, bounds.y() + 8.0);
        let end = Point::new(start.x, start.y + delta_y);
        let mut down = PointerEvent::new(PointerEventKind::Down, start);
        down.pointer_id = pointer_id;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .unwrap();

        let mut moved = PointerEvent::new(PointerEventKind::Move, end);
        moved.pointer_id = pointer_id;
        moved.delta = Vector::new(0.0, delta_y);
        moved.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(moved))
            .unwrap();
        runtime
            .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
            .unwrap();
    }

    fn scroll_view_content_with_height(
        graph: &WidgetGraphSnapshot,
        content_height: f32,
    ) -> &WidgetNodeSnapshot {
        graph
            .nodes
            .iter()
            .find(|node| {
                (node.measured_size.height - content_height).abs() <= 0.5
                    && node
                        .parent
                        .and_then(|parent_id| {
                            graph.nodes.iter().find(|parent| parent.id == parent_id)
                        })
                        .is_some_and(|parent| parent.paint_boundary == PaintBoundaryMode::Explicit)
            })
            .expect("scroll content present")
    }

    fn parent_node<'a>(
        graph: &'a WidgetGraphSnapshot,
        node: &WidgetNodeSnapshot,
    ) -> &'a WidgetNodeSnapshot {
        let parent_id = node.parent.expect("node has parent");
        graph
            .nodes
            .iter()
            .find(|parent| parent.id == parent_id)
            .expect("parent node present")
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

    fn handle_ready_events(runtime: &mut Runtime) -> usize {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window, event) in ready {
            runtime
                .handle_event(ready_window, event)
                .expect("ready event should be handled");
        }
        count
    }

    #[test]
    fn switch_view_only_exposes_selected_child_semantics() {
        let (output, _) = render_root(
            SwitchView::new()
                .selected(1)
                .with_child(crate::Label::new("Brush options"))
                .with_child(crate::Label::new("Pan options")),
        );

        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Pan options")
        }));
        assert!(!output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush options")
        }));
    }

    #[test]
    fn switch_view_selected_when_updates_active_child() -> sui_core::Result<()> {
        let selected = Rc::new(RefCell::new(0_usize));
        let selected_reader = Rc::clone(&selected);
        let (mut runtime, window_id) = build_runtime(
            SwitchView::new()
                .selected_when(move || *selected_reader.borrow())
                .with_child(
                    SizedBox::new()
                        .size(Size::new(80.0, 24.0))
                        .with_child(crate::Label::new("Brush options")),
                )
                .with_child(
                    SizedBox::new()
                        .size(Size::new(120.0, 36.0))
                        .with_child(crate::Label::new("Fill options")),
                ),
        );

        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(80.0, 24.0));
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush options")
        }));

        *selected.borrow_mut() = 1;
        runtime.handle_event(
            window_id,
            Event::Window(sui_core::WindowEvent::Resized(Size::new(120.0, 36.0))),
        )?;
        let output = runtime.render(window_id)?;

        assert_eq!(output.frame.viewport, Size::new(120.0, 36.0));
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Fill options")
        }));
        assert!(!output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Brush options")
        }));
        Ok(())
    }

    #[test]
    fn switch_view_observable_selection_relayouts_retained_child() -> sui_core::Result<()> {
        let selected = Signal::named("active_panel", 0_usize);
        let (mut runtime, window_id) = build_runtime(
            SwitchView::new()
                .selected_from(selected.clone())
                .with_child(
                    SizedBox::new()
                        .size(Size::new(80.0, 24.0))
                        .with_child(crate::Label::new("Brush options")),
                )
                .with_child(
                    SizedBox::new()
                        .size(Size::new(120.0, 36.0))
                        .with_child(crate::Label::new("Fill options")),
                ),
        );

        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(80.0, 24.0));

        assert!(selected.set(1));
        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(120.0, 36.0));
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Fill options")
        }));
        assert!(
            output
                .diagnostics
                .reactive_invalidations
                .iter()
                .any(|sample| sample.source_name == "active_panel")
        );
        Ok(())
    }

    #[test]
    fn padding_expands_child_size_and_offsets_bounds() {
        let (output, graph) = render_root(Padding::new(
            Insets {
                left: 8.0,
                top: 6.0,
                right: 4.0,
                bottom: 2.0,
            },
            FixedBox::new(Size::new(40.0, 20.0), Color::rgba(0.2, 0.3, 0.4, 1.0)),
        ));

        assert_eq!(output.frame.viewport, Size::new(52.0, 28.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(8.0, 6.0, 40.0, 20.0));
    }

    #[test]
    fn padding_horizontal_constructor_offsets_x_only() {
        let (output, graph) = render_root(Padding::horizontal(
            9.0,
            FixedBox::new(Size::new(40.0, 20.0), Color::rgba(0.2, 0.3, 0.4, 1.0)),
        ));

        assert_eq!(output.frame.viewport, Size::new(58.0, 20.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(9.0, 0.0, 40.0, 20.0));
    }

    #[test]
    fn padding_can_stretch_child_to_arranged_content_height() {
        let (_output, graph) = render_root(
            SizedBox::new().size(Size::new(100.0, 80.0)).with_child(
                Padding::new(
                    Insets {
                        left: 8.0,
                        top: 5.0,
                        right: 12.0,
                        bottom: 7.0,
                    },
                    FixedBox::new(Size::new(20.0, 10.0), Color::rgba(0.2, 0.3, 0.4, 1.0)),
                )
                .fill_child_height(),
            ),
        );

        assert_eq!(graph.nodes[2].bounds, Rect::new(8.0, 5.0, 80.0, 68.0));
    }

    #[test]
    fn align_centers_child_within_available_space() {
        let (output, graph) = render_root(SizedBox::new().size(Size::new(100.0, 60.0)).with_child(
            Align::center(FixedBox::new(
                Size::new(20.0, 10.0),
                Color::rgba(0.4, 0.3, 0.2, 1.0),
            )),
        ));

        assert_eq!(output.frame.viewport, Size::new(100.0, 60.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(40.0, 25.0, 20.0, 10.0));
    }

    #[test]
    fn sized_box_applies_explicit_dimensions_to_child() {
        let (output, graph) = render_root(SizedBox::new().width(40.0).height(24.0).with_child(
            FixedBox::new(Size::new(12.0, 8.0), Color::rgba(0.1, 0.7, 0.2, 1.0)),
        ));

        assert_eq!(output.frame.viewport, Size::new(40.0, 24.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(0.0, 0.0, 40.0, 24.0));
    }

    #[test]
    fn trailing_slot_row_places_fixed_trailing_slot() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(200.0, 80.0)).with_child(
                TrailingSlotRow::new(
                    FixedBox::new(Size::new(120.0, 64.0), Color::rgba(0.1, 0.7, 0.2, 1.0)),
                    FixedBox::new(Size::new(40.0, 40.0), Color::rgba(0.7, 0.2, 0.2, 1.0)),
                )
                .trailing_width(50.0)
                .trailing_height(40.0)
                .gap(8.0),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(200.0, 80.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 142.0, 80.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(150.0, 20.0, 50.0, 40.0));
    }

    #[test]
    fn dock_places_fixed_top_bottom_and_fills_body() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(200.0, 120.0)).with_child(
                Dock::new(FixedBox::new(
                    Size::new(50.0, 50.0),
                    Color::rgba(0.1, 0.7, 0.2, 1.0),
                ))
                .top(
                    24.0,
                    FixedBox::new(Size::new(200.0, 24.0), Color::rgba(0.2, 0.2, 0.8, 1.0)),
                )
                .bottom(
                    18.0,
                    FixedBox::new(Size::new(200.0, 18.0), Color::rgba(0.8, 0.2, 0.2, 1.0)),
                ),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(200.0, 120.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 200.0, 24.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(0.0, 24.0, 200.0, 78.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(0.0, 102.0, 200.0, 18.0));
    }

    #[test]
    fn measured_bottom_dock_places_natural_bottom_at_bottom_edge() {
        let (output, graph) =
            render_root(SizedBox::new().size(Size::new(200.0, 120.0)).with_child(
                MeasuredBottomDock::new(
                    FixedBox::new(Size::new(50.0, 50.0), Color::rgba(0.1, 0.7, 0.2, 1.0)),
                    FixedBox::new(Size::new(60.0, 26.0), Color::rgba(0.8, 0.2, 0.2, 1.0)),
                ),
            ));

        assert_eq!(output.frame.viewport, Size::new(200.0, 120.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 200.0, 94.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(0.0, 94.0, 200.0, 26.0));
    }

    #[test]
    fn fixed_pane_split_preserves_fixed_first_and_stretches_cross_axis() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(200.0, 80.0)).with_child(
                FixedPaneSplit::horizontal(
                    FixedBox::new(Size::new(80.0, 12.0), Color::rgba(0.1, 0.7, 0.2, 1.0)),
                    FixedBox::new(Size::new(1.0, 80.0), Color::rgba(0.7, 0.7, 0.7, 1.0)),
                    FixedBox::new(Size::new(120.0, 12.0), Color::rgba(0.7, 0.2, 0.2, 1.0)),
                )
                .fixed_first(64.0)
                .divider_extent(1.0),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(200.0, 80.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 64.0, 80.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(64.0, 0.0, 1.0, 80.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(65.0, 0.0, 135.0, 80.0));
    }

    #[test]
    fn fixed_pane_split_preserves_fixed_second_and_shrinks_to_fit() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(90.0, 40.0)).with_child(
                FixedPaneSplit::horizontal(
                    FixedBox::new(Size::new(80.0, 12.0), Color::rgba(0.1, 0.7, 0.2, 1.0)),
                    FixedBox::new(Size::new(1.0, 40.0), Color::rgba(0.7, 0.7, 0.7, 1.0)),
                    FixedBox::new(Size::new(120.0, 12.0), Color::rgba(0.7, 0.2, 0.2, 1.0)),
                )
                .fixed_second(120.0)
                .divider_extent(1.0),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(90.0, 40.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 0.0, 40.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(0.0, 0.0, 1.0, 40.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(1.0, 0.0, 89.0, 40.0));
    }

    #[test]
    fn stack_positions_children_with_spacing_and_alignment() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(100.0, 40.0)).with_child(
                Stack::horizontal()
                    .spacing(5.0)
                    .alignment(Alignment::Center)
                    .with_child(FixedBox::new(
                        Size::new(30.0, 10.0),
                        Color::rgba(0.6, 0.2, 0.2, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(20.0, 20.0),
                        Color::rgba(0.2, 0.2, 0.6, 1.0),
                    )),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(100.0, 40.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 15.0, 30.0, 10.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(35.0, 10.0, 20.0, 20.0));
    }

    #[test]
    fn flex_grows_child_to_fill_available_main_axis_space() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(100.0, 20.0)).with_child(
                Flex::horizontal()
                    .with_child(FixedBox::new(
                        Size::new(20.0, 10.0),
                        Color::rgba(0.7, 0.2, 0.2, 1.0),
                    ))
                    .with_item(
                        FixedBox::new(Size::new(10.0, 10.0), Color::rgba(0.2, 0.2, 0.7, 1.0)),
                        FlexItem::new().grow(1.0),
                    ),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(100.0, 20.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 20.0, 10.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(20.0, 0.0, 80.0, 10.0));
    }

    #[test]
    fn flex_remeasures_wrapping_label_at_resolved_width() {
        const TEXT: &str =
            "Provider verification status and usage limits are shared across the cluster.";
        let (output, _) = render_root(SizedBox::new().width(240.0).with_child(
            Flex::horizontal().with_item(crate::Label::new(TEXT), FlexItem::flex(1.0)),
        ));

        let label = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(TEXT))
            .expect("flex label semantics present");
        assert!((label.bounds.width() - 240.0).abs() < 0.01);

        let mut shaped_width = None;
        let mut line_count = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(run) = command
                && let Some(layout) = run.resolve(output.frame.text_layout_registry.as_ref())
                && layout.text() == TEXT
            {
                shaped_width = Some(layout.box_size().width);
                line_count = Some(layout.lines().len());
            }
        });

        assert!(shaped_width.is_some_and(|width| width >= 239.0));
        assert!(line_count.is_some_and(|lines| (2..=4).contains(&lines)));
        assert!(output.frame.viewport.height < 100.0);
    }

    #[test]
    fn flex_spacer_pushes_following_children_to_remaining_edge() {
        let (_, graph) = render_root(
            SizedBox::new().size(Size::new(100.0, 10.0)).with_child(
                Flex::horizontal()
                    .with_child(FixedBox::new(
                        Size::new(10.0, 10.0),
                        Color::rgba(0.7, 0.2, 0.2, 1.0),
                    ))
                    .spacer()
                    .with_child(FixedBox::new(
                        Size::new(10.0, 10.0),
                        Color::rgba(0.2, 0.2, 0.7, 1.0),
                    )),
            ),
        );

        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(90.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn flex_wraps_children_and_applies_cross_gap() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(70.0, 25.0)).with_child(
                Flex::horizontal()
                    .wrap(FlexWrap::Wrap)
                    .main_gap(5.0)
                    .cross_gap(5.0)
                    .with_child(FixedBox::new(
                        Size::new(30.0, 10.0),
                        Color::rgba(0.7, 0.2, 0.2, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(30.0, 10.0),
                        Color::rgba(0.2, 0.7, 0.2, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(30.0, 10.0),
                        Color::rgba(0.2, 0.2, 0.7, 1.0),
                    )),
            ),
        );

        assert_eq!(output.frame.viewport, Size::new(70.0, 25.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 30.0, 10.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(35.0, 0.0, 30.0, 10.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(0.0, 15.0, 30.0, 10.0));
    }

    #[test]
    fn flex_stretches_children_on_cross_axis() {
        let (_, graph) = render_root(
            SizedBox::new().size(Size::new(60.0, 20.0)).with_child(
                Flex::horizontal()
                    .align_items(Alignment::Stretch)
                    .with_child(FixedBox::new(
                        Size::new(20.0, 8.0),
                        Color::rgba(0.4, 0.3, 0.2, 1.0),
                    )),
            ),
        );

        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 0.0, 20.0, 20.0));
    }

    #[test]
    fn background_paints_before_child_content() {
        let (output, _) = render_root(Background::new(
            Brush::Solid(Color::rgba(0.1, 0.1, 0.1, 1.0)),
            FixedBox::new(Size::new(16.0, 12.0), Color::rgba(0.9, 0.2, 0.1, 1.0)),
        ));

        assert_eq!(output.frame.scene.commands().len(), 2);
        assert_eq!(
            output.frame.scene.commands()[0],
            SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 16.0, 12.0),
                brush: Brush::Solid(Color::rgba(0.1, 0.1, 0.1, 1.0)),
            }
        );
        assert_eq!(
            output.frame.scene.commands()[1],
            SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 16.0, 12.0),
                brush: Brush::Solid(Color::rgba(0.9, 0.2, 0.1, 1.0)),
            }
        );
    }

    #[test]
    fn semantic_region_wraps_child_with_accessible_summary() {
        let description = Rc::new(RefCell::new("4 active tasks".to_string()));
        let description_reader = Rc::clone(&description);
        let (output, graph) = render_root(
            SemanticRegion::new(
                "File task strip",
                FixedBox::new(Size::new(96.0, 28.0), Color::rgba(0.2, 0.4, 0.6, 1.0)),
            )
            .role(SemanticsRole::Text)
            .description_when(move || description_reader.borrow().clone()),
        );

        let region = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("File task strip")
            })
            .expect("semantic region node should exist");
        assert_eq!(region.description.as_deref(), Some("4 active tasks"));
        assert_eq!(graph.nodes[0].bounds, Rect::new(0.0, 0.0, 96.0, 28.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(0.0, 0.0, 96.0, 28.0));
    }

    #[test]
    fn stack_vertical_axis_is_available() {
        let (output, graph) = render_root(
            Stack::new(Axis::Vertical)
                .spacing(4.0)
                .with_child(FixedBox::new(
                    Size::new(18.0, 10.0),
                    Color::rgba(0.5, 0.5, 0.1, 1.0),
                ))
                .with_child(FixedBox::new(
                    Size::new(12.0, 8.0),
                    Color::rgba(0.1, 0.5, 0.5, 1.0),
                )),
        );

        assert_eq!(output.frame.viewport, Size::new(18.0, 22.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(0.0, 0.0, 18.0, 10.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, 14.0, 12.0, 8.0));
    }

    #[test]
    fn nested_containers_preserve_global_child_bounds() {
        let (output, graph) = render_root(Padding::all(
            24.0,
            Background::new(
                Brush::Solid(Color::rgba(0.1, 0.1, 0.1, 1.0)),
                Padding::all(
                    18.0,
                    Stack::vertical()
                        .spacing(10.0)
                        .with_child(FixedBox::new(
                            Size::new(50.0, 12.0),
                            Color::rgba(0.7, 0.2, 0.2, 1.0),
                        ))
                        .with_child(FixedBox::new(
                            Size::new(30.0, 8.0),
                            Color::rgba(0.2, 0.7, 0.2, 1.0),
                        )),
                ),
            ),
        ));

        assert_eq!(output.frame.viewport, Size::new(134.0, 114.0));
        assert_eq!(graph.nodes[1].bounds, Rect::new(24.0, 24.0, 86.0, 66.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(24.0, 24.0, 86.0, 66.0));
        assert_eq!(graph.nodes[3].bounds, Rect::new(42.0, 42.0, 50.0, 30.0));
        assert_eq!(graph.nodes[4].bounds, Rect::new(42.0, 42.0, 50.0, 12.0));
        assert_eq!(graph.nodes[5].bounds, Rect::new(42.0, 64.0, 30.0, 8.0));
    }

    #[test]
    fn scroll_view_updates_child_bounds_after_scroll_input() {
        let (mut runtime, window_id) =
            build_runtime(SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(FixedBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                )),
            ));

        let _ = runtime.render(window_id).unwrap();
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        assert_eq!(graph.nodes[1].bounds, Rect::new(0.0, 0.0, 80.0, 40.0));
        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("scroll content present");
        assert_eq!(content.bounds, Rect::new(0.0, -32.0, 80.0, 120.0));

        let semantic_content = output
            .semantics
            .iter()
            .find(|node| node.id == content.id)
            .expect("scroll content semantics present");
        assert_eq!(semantic_content.bounds, content.bounds);

        let scroll_view = parent_node(&graph, content);
        let scroll_scene = output
            .frame
            .scene
            .layer_scene(scroll_view.id)
            .expect("scroll viewport layer present");
        assert!(scroll_scene.commands().iter().any(|command| {
            matches!(
                command,
                SceneCommand::PushClip { rect }
                    if *rect == Rect::new(0.0, 0.0, 80.0, 40.0)
            )
        }));
    }

    #[test]
    fn scroll_view_translates_retained_content_without_repainting() {
        let counts = Rc::new(RefCell::new(vec![0usize; 2]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 60.0)).with_child(
                ScrollView::vertical(
                    Stack::vertical()
                        .with_child(PaintCounterBox::new(
                            Size::new(80.0, 60.0),
                            Color::rgba(0.8, 0.2, 0.2, 1.0),
                            Rc::clone(&counts),
                            0,
                        ))
                        .with_child(PaintCounterBox::new(
                            Size::new(80.0, 60.0),
                            Color::rgba(0.2, 0.6, 0.8, 1.0),
                            Rc::clone(&counts),
                            1,
                        )),
                )
                .retain_content_layer(),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1, 1]);
        let graph = runtime.widget_graph(window_id).unwrap();
        let content_id = scroll_view_content_with_height(&graph, 120.0).id;

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![1, 1]);
        assert!(output.frame.layer_updates.iter().any(|update| {
            update.owner == content_id && update.kind == sui_scene::SceneLayerUpdateKind::Transform
        }));
        assert!(!output.frame.layer_updates.iter().any(|update| {
            update.owner == content_id && update.kind == sui_scene::SceneLayerUpdateKind::Content
        }));
    }

    #[test]
    fn scroll_view_repaints_content_by_default_after_scroll_input() {
        let counts = Rc::new(RefCell::new(vec![0usize; 2]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(80.0, 60.0))
                .with_child(ScrollView::vertical(
                    Stack::vertical()
                        .with_child(PaintCounterBox::new(
                            Size::new(80.0, 60.0),
                            Color::rgba(0.8, 0.2, 0.2, 1.0),
                            Rc::clone(&counts),
                            0,
                        ))
                        .with_child(PaintCounterBox::new(
                            Size::new(80.0, 60.0),
                            Color::rgba(0.2, 0.6, 0.8, 1.0),
                            Rc::clone(&counts),
                            1,
                        )),
                )),
        );

        let _ = runtime.render(window_id).unwrap();
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![2, 2]);
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| { update.kind == sui_scene::SceneLayerUpdateKind::Content })
        );
    }

    #[test]
    fn retained_scroll_repaints_ancestor_when_nested_layer_is_dirty() {
        let nested_paints = Rc::new(RefCell::new(0usize));
        let sibling_paints = Rc::new(RefCell::new(vec![0usize]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 60.0)).with_child(
                ScrollView::vertical(
                    Stack::vertical()
                        .with_child(LayeredScrollPaintBox {
                            size: Size::new(80.0, 60.0),
                            color: Color::rgba(0.8, 0.2, 0.2, 1.0),
                            paints: Rc::clone(&nested_paints),
                        })
                        .with_child(PaintCounterBox::new(
                            Size::new(80.0, 60.0),
                            Color::rgba(0.2, 0.6, 0.8, 1.0),
                            Rc::clone(&sibling_paints),
                            0,
                        )),
                )
                .retain_content_layer(),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        let initial_graph = runtime.widget_graph(window_id).unwrap();
        let content = scroll_view_content_with_height(&initial_graph, 120.0);
        let content_id = content.id;
        let nested_id = content.children[0];

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();
        let nested = graph
            .nodes
            .iter()
            .find(|node| node.id == nested_id)
            .expect("nested retained layer remains in the graph");
        let descriptor = layer_descriptor_for(&output, nested_id)
            .expect("nested retained layer remains in the scene");

        assert_eq!(*nested_paints.borrow(), 2);
        assert_eq!(*sibling_paints.borrow(), vec![2]);
        assert_eq!(descriptor.bounds, nested.bounds);
        assert!(output.frame.layer_updates.iter().any(|update| {
            update.owner == content_id && update.kind == sui_scene::SceneLayerUpdateKind::Content
        }));
        assert!(!output.frame.layer_updates.iter().any(|update| {
            update.owner == content_id && update.kind == sui_scene::SceneLayerUpdateKind::Transform
        }));
    }

    #[test]
    fn scroll_view_hit_testing_tracks_retained_content_translation() {
        let presses = Rc::new(RefCell::new(vec![0usize; 2]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(
                    Stack::vertical()
                        .with_child(HitTestBox::new(
                            Size::new(80.0, 40.0),
                            Rc::clone(&presses),
                            0,
                        ))
                        .with_child(HitTestBox::new(
                            Size::new(80.0, 40.0),
                            Rc::clone(&presses),
                            1,
                        )),
                )
                .retain_content_layer(),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -40.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();

        let pointer = PointerEvent::new(PointerEventKind::Down, Point::new(20.0, 20.0));
        runtime
            .handle_event(window_id, Event::Pointer(pointer))
            .unwrap();

        assert_eq!(*presses.borrow(), vec![0, 1]);
    }

    #[test]
    fn scroll_view_uses_scroll_layer_metadata() {
        let (output, _) = render_root(SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
            ScrollView::vertical(FixedBox::new(
                Size::new(80.0, 120.0),
                Color::rgba(0.2, 0.3, 0.7, 1.0),
            )),
        ));

        let scroll_id = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ScrollView)
            .expect("scroll view semantics present")
            .id;
        let descriptor =
            layer_descriptor_for(&output, scroll_id).expect("scroll view layer present");

        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Scroll);
    }

    #[test]
    fn scroll_view_overlays_scroll_bar_only_when_content_overflows() {
        let theme = DefaultTheme::default();
        let (overflowing, graph) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .name("Results")
                .theme(theme),
            ),
        );

        let scroll_view = overflowing
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ScrollView)
            .expect("scroll view semantics present");
        let scroll_bar = overflowing
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some("Results vertical scroll bar")
            })
            .expect("overflowing content should expose an overlay scroll bar");
        assert_eq!(
            scroll_bar.bounds.width(),
            theme.metrics.scroll_bar_thickness
        );
        assert!(!scroll_bar.actions.contains(&SemanticsAction::Focus));
        assert!(scroll_bar.actions.contains(&SemanticsAction::SetValue));
        assert!(scroll_view.bounds.contains(scroll_bar.bounds.origin));
        assert!(scroll_bar.bounds.max_x() <= scroll_view.bounds.max_x());
        assert!(scroll_bar.bounds.max_y() <= scroll_view.bounds.max_y());
        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.height() == 120.0 && node.bounds.width() == 80.0)
            .expect("scroll content keeps the full viewport width");
        assert_eq!(content.bounds.width(), scroll_view.bounds.width());

        let (fitting, _) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(FixedBox::new(
                    Size::new(80.0, 40.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .name("Results"),
            ),
        );
        assert!(
            fitting
                .semantics
                .iter()
                .all(|node| node.role != SemanticsRole::Slider),
            "fitting content should not expose scroll-bar chrome"
        );
    }

    #[test]
    fn embedded_overlay_scroll_bar_drags_the_shared_view_state() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 60.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 180.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .name("Results")
                .state(state.clone()),
            ),
        );
        let output = runtime.render(window_id).unwrap();
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Results vertical scroll bar"))
            .expect("embedded scroll bar present");
        let point = Point::new(
            scroll_bar.bounds.x() + scroll_bar.bounds.width() * 0.5,
            scroll_bar.bounds.max_y() - 1.0,
        );
        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.pointer_id = 41;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .unwrap();

        assert!(state.current_offset().y > 0.0);
        let _ = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();
        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.size == Size::new(80.0, 180.0))
            .expect("scroll content present");
        assert_eq!(content.bounds.y(), -state.current_offset().y);
    }

    #[test]
    fn overlay_scroll_bar_drag_repaints_non_retained_content_immediately() {
        let state = ScrollState::new();
        let counts = Rc::new(RefCell::new(vec![0usize]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 60.0)).with_child(
                ScrollView::vertical(PaintCounterBox::new(
                    Size::new(80.0, 180.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                    Rc::clone(&counts),
                    0,
                ))
                .name("Results")
                .state(state.clone()),
            ),
        );
        let output = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1]);
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Results vertical scroll bar"))
            .expect("embedded scroll bar present");

        drag_vertical_scroll_bar(&mut runtime, window_id, scroll_bar.bounds, 42, 20.0);
        assert!(state.current_offset().y > 0.0);
        let _ = runtime.render(window_id).unwrap();

        assert_eq!(
            *counts.borrow(),
            vec![2],
            "the first frame after a scroll-bar drag must repaint moved flattened content"
        );
    }

    #[test]
    fn overlay_scroll_bar_drag_keeps_retained_content_transform_only() {
        let state = ScrollState::new();
        let counts = Rc::new(RefCell::new(vec![0usize]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 60.0)).with_child(
                ScrollView::vertical(PaintCounterBox::new(
                    Size::new(80.0, 180.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                    Rc::clone(&counts),
                    0,
                ))
                .name("Retained results")
                .state(state.clone())
                .retain_content_layer(),
            ),
        );
        let output = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1]);
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Retained results vertical scroll bar"))
            .expect("embedded scroll bar present");

        drag_vertical_scroll_bar(&mut runtime, window_id, scroll_bar.bounds, 43, 20.0);
        assert!(state.current_offset().y > 0.0);
        let output = runtime.render(window_id).unwrap();

        assert_eq!(
            *counts.borrow(),
            vec![1],
            "retained content should move without rebuilding its paint commands"
        );
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| { update.kind == sui_scene::SceneLayerUpdateKind::Transform })
        );
    }

    #[test]
    fn virtual_overlay_scroll_bar_drag_repaints_visible_content_immediately() {
        let state = ScrollState::new();
        let counts = Rc::new(RefCell::new(vec![0usize; 4]));
        let mut scroll = VirtualScrollView::new()
            .name("Virtual results")
            .state(state.clone());
        for (index, color) in [
            Color::rgba(0.8, 0.2, 0.2, 1.0),
            Color::rgba(0.2, 0.8, 0.2, 1.0),
            Color::rgba(0.2, 0.2, 0.8, 1.0),
            Color::rgba(0.8, 0.8, 0.2, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            scroll.push(PaintCounterBox::new(
                Size::new(80.0, 30.0),
                color,
                Rc::clone(&counts),
                index,
            ));
        }
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(80.0, 80.0))
                .with_child(scroll),
        );
        let output = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1, 1, 1, 1]);
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Virtual results vertical scroll bar"))
            .expect("embedded scroll bar present");

        drag_vertical_scroll_bar(&mut runtime, window_id, scroll_bar.bounds, 44, 20.0);
        assert!(state.current_offset().y > 0.0);
        let _ = runtime.render(window_id).unwrap();

        assert_eq!(
            *counts.borrow(),
            vec![2, 2, 2, 2],
            "the first frame after a virtual scroll-bar drag must repaint visible rows"
        );
    }

    #[test]
    fn both_axis_overlay_scroll_bars_share_the_corner_without_reserving_space() {
        let (output, graph) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::both(OverflowingBox::new(
                    Size::new(160.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .name("Canvas"),
            ),
        );

        let vertical = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Canvas vertical scroll bar"))
            .expect("vertical overlay scroll bar present");
        let horizontal = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Canvas horizontal scroll bar"))
            .expect("horizontal overlay scroll bar present");
        assert!(vertical.bounds.max_y() <= horizontal.bounds.y());
        assert!(horizontal.bounds.max_x() <= vertical.bounds.x());

        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.size == Size::new(160.0, 120.0))
            .expect("two-axis content present");
        assert_eq!(content.bounds.origin, Point::ZERO);
    }

    #[test]
    fn virtual_scroll_view_overlays_a_synchronized_vertical_scroll_bar() {
        let state = ScrollState::new();
        let (output, _) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .name("Timeline")
                    .state(state.clone())
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.7, 0.3, 0.2, 1.0),
                    )),
            ),
        );

        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Timeline vertical scroll bar"))
            .expect("virtual scroll bar present");
        assert_eq!(
            scroll_bar.value,
            Some(SemanticsValue::Range {
                value: 0.0,
                min: 0.0,
                max: 40.0,
            })
        );
        assert_eq!(state.max_offset(), Vector::new(0.0, 40.0));
    }

    #[test]
    fn scroll_view_auto_overflow_uses_finite_width_and_natural_height() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (output, _) = render_root(
            SizedBox::new().size(Size::new(120.0, 60.0)).with_child(
                ScrollView::both(ConstraintProbe::new(
                    Size::new(90.0, 180.0),
                    Rc::clone(&seen),
                ))
                .overflow_x(Overflow::Auto)
                .overflow_y(Overflow::Auto),
            ),
        );

        let constraints = seen
            .borrow()
            .last()
            .copied()
            .expect("probe should be measured");
        assert_eq!(constraints.max.width, 120.0);
        assert!(constraints.max.height.is_infinite());
        assert_eq!(output.frame.viewport, Size::new(120.0, 60.0));
    }

    #[test]
    fn vertical_scroll_view_clamps_cross_axis_after_split_arrange() {
        let (_, graph) = render_root(
            SizedBox::new().size(Size::new(240.0, 80.0)).with_child(
                SplitView::horizontal(
                    FixedBox::new(Size::new(40.0, 80.0), Color::rgba(0.1, 0.2, 0.3, 1.0)),
                    ScrollView::vertical(Padding::all(
                        8.0,
                        Stack::vertical()
                            .alignment(Alignment::Stretch)
                            .with_child(FixedBox::new(
                                Size::new(400.0, 32.0),
                                Color::rgba(0.4, 0.5, 0.6, 1.0),
                            )),
                    )),
                )
                .ratio(0.5)
                .min_first(40.0)
                .min_second(40.0)
                .divider_thickness(8.0),
            ),
        );

        let content = graph
            .nodes
            .iter()
            .find(|node| {
                (node.bounds.x() - 132.0).abs() < 0.001
                    && (node.bounds.y() - 8.0).abs() < 0.001
                    && (node.bounds.height() - 32.0).abs() < 0.001
            })
            .expect("scroll content child should be arranged inside the narrow pane");
        assert_eq!(content.bounds.width(), 100.0);
    }

    #[test]
    fn scroll_view_visible_overflow_does_not_scroll_or_use_scroll_layer() {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .overflow(Overflow::Visible),
            ),
        );

        let output = runtime.render(window_id).unwrap();
        let scroll_id = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ScrollView)
            .expect("scroll view semantics present")
            .id;
        let descriptor =
            layer_descriptor_for(&output, scroll_id).expect("scroll view layer present");
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Normal);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();
        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("visible overflow content present");
        assert_eq!(content.bounds, Rect::new(0.0, 0.0, 80.0, 120.0));
    }

    #[test]
    fn virtual_scroll_view_uses_scroll_layer_metadata() {
        let (output, _) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.7, 0.3, 0.2, 1.0),
                    )),
            ),
        );

        let scroll_id = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ScrollView)
            .expect("virtual scroll view semantics present")
            .id;
        let descriptor =
            layer_descriptor_for(&output, scroll_id).expect("virtual scroll view layer present");

        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Scroll);
    }

    #[test]
    fn virtual_scroll_state_scroll_to_item_aligns_item_at_viewport_top() {
        let state = ScrollState::new();
        assert!(state.scroll_to_item(2));

        let (output, _) = render_root(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .state(state.clone())
                    .with_child(SemanticRegion::new(
                        "Item 0",
                        FixedBox::new(Size::new(80.0, 20.0), Color::rgba(0.8, 0.2, 0.2, 1.0)),
                    ))
                    .with_child(SemanticRegion::new(
                        "Item 1",
                        FixedBox::new(Size::new(80.0, 20.0), Color::rgba(0.2, 0.8, 0.2, 1.0)),
                    ))
                    .with_child(SemanticRegion::new(
                        "Item 2",
                        FixedBox::new(Size::new(80.0, 40.0), Color::rgba(0.2, 0.2, 0.8, 1.0)),
                    )),
            ),
        );

        let viewport = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ScrollView)
            .expect("virtual scroll view semantics present");
        let target = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Item 2"))
            .expect("target item semantics present");

        assert_eq!(state.current_offset(), Vector::new(0.0, 40.0));
        assert_eq!(target.bounds.y(), viewport.bounds.y());
    }

    #[test]
    fn virtual_scroll_view_paints_only_visible_children() {
        let counts = Rc::new(RefCell::new(vec![0usize; 4]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.8, 0.2, 0.2, 1.0),
                        Rc::clone(&counts),
                        0,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.8, 0.2, 1.0),
                        Rc::clone(&counts),
                        1,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.2, 0.8, 1.0),
                        Rc::clone(&counts),
                        2,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.8, 0.8, 0.2, 1.0),
                        Rc::clone(&counts),
                        3,
                    )),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        // The visible window extends by 75% of the viewport above and below,
        // so the third item is included in the initial overdraw buffer.
        assert_eq!(*counts.borrow(), vec![1, 1, 1, 0]);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();

        // After scrolling 32px, the overdraw buffer still includes item 0 and
        // extends far enough to bring item 3 into the painted range.
        assert_eq!(*counts.borrow(), vec![2, 2, 2, 1]);
    }

    #[test]
    fn scroll_view_emits_transform_updates_after_scroll_offset_changes() {
        let counts = Rc::new(RefCell::new(vec![0usize; 1]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(PaintCounterBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                    Rc::clone(&counts),
                    0,
                ))
                .retain_content_layer(),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1]);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![1]);
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| update.kind == sui_scene::SceneLayerUpdateKind::Transform)
        );
    }

    #[test]
    fn split_view_scroll_without_paint_still_updates_runtime_scene() {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(360.0, 220.0)).with_child(
                SplitView::horizontal(
                    ScrollViewNoPaint::vertical(
                        Stack::vertical()
                            .with_child(FixedBox::new(
                                Size::new(220.0, 120.0),
                                Color::rgba(0.82, 0.36, 0.18, 1.0),
                            ))
                            .with_child(FixedBox::new(
                                Size::new(220.0, 120.0),
                                Color::rgba(0.18, 0.54, 0.82, 1.0),
                            ))
                            .with_child(FixedBox::new(
                                Size::new(220.0, 120.0),
                                Color::rgba(0.24, 0.72, 0.36, 1.0),
                            )),
                    )
                    .name("NoPaint scroll"),
                    SizedBox::new().size(Size::new(120.0, 220.0)),
                )
                .ratio(0.68),
            ),
        );

        let before = runtime.render(window_id).unwrap();

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(48.0, 96.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -72.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();

        let after = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        assert!(graph.nodes.iter().any(|node| node.bounds.y() < 0.0));
        assert_ne!(before.frame.scene, after.frame.scene);
        assert!(!after.frame.layer_updates.is_empty());
    }

    #[test]
    fn virtual_scroll_view_repaints_exposed_strip_while_visible_range_is_unchanged() {
        // Use a wider viewport (80px) so the overdraw buffer (35% = 28px)
        // is large enough that a small 4px scroll does not bring new items
        // into the visible range.
        let counts = Rc::new(RefCell::new(vec![0usize; 4]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 80.0)).with_child(
                VirtualScrollView::new()
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.8, 0.2, 0.2, 1.0),
                        Rc::clone(&counts),
                        0,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.8, 0.2, 1.0),
                        Rc::clone(&counts),
                        1,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.2, 0.8, 1.0),
                        Rc::clone(&counts),
                        2,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.8, 0.8, 0.2, 1.0),
                        Rc::clone(&counts),
                        3,
                    )),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        // All 4 items fit within the 80px viewport + 28px overdraw = 108px.
        assert_eq!(*counts.borrow(), vec![1, 1, 1, 1]);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -4.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        // A small 4px scroll should not change the visible range when
        // the overdraw buffer covers all items, but it still needs to repaint
        // the newly exposed strip so equivalent final offsets stay history-independent.
        assert_eq!(*counts.borrow(), vec![2, 2, 2, 2]);
        assert!(output.frame.layer_updates.iter().any(|update| {
            update.kind == sui_scene::SceneLayerUpdateKind::Content
                && update.damage == Some(Rect::new(0.0, 76.0, 80.0, 4.0))
        }));
    }

    #[test]
    fn virtual_scroll_view_repaints_when_a_visible_layered_child_changes_height() {
        let counts = Rc::new(RefCell::new(vec![0usize; 2]));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 80.0)).with_child(
                VirtualScrollView::new()
                    .with_child(ExpandingLayerBox::new(
                        Size::new(80.0, 30.0),
                        Size::new(80.0, 60.0),
                        Color::rgba(0.7, 0.3, 0.2, 1.0),
                        Rc::clone(&counts),
                        0,
                    ))
                    .with_child(PaintCounterBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.5, 0.8, 1.0),
                        Rc::clone(&counts),
                        1,
                    )),
            ),
        );

        let _ = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1, 1]);

        let pointer = PointerEvent::new(PointerEventKind::Down, Point::new(20.0, 20.0));
        runtime
            .handle_event(window_id, Event::Pointer(pointer))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![2, 2]);
    }

    #[test]
    fn nested_scroll_views_scroll_the_inner_region_first() {
        let (mut runtime, window_id) =
            build_runtime(
                SizedBox::new()
                    .size(Size::new(80.0, 80.0))
                    .with_child(ScrollView::vertical(
                        Stack::vertical()
                            .spacing(8.0)
                            .with_child(FixedBox::new(
                                Size::new(80.0, 32.0),
                                Color::rgba(0.8, 0.2, 0.2, 1.0),
                            ))
                            .with_child(SizedBox::new().height(40.0).with_child(
                                ScrollView::vertical(FixedBox::new(
                                    Size::new(80.0, 120.0),
                                    Color::rgba(0.2, 0.7, 0.3, 1.0),
                                )),
                            ))
                            .with_child(FixedBox::new(
                                Size::new(80.0, 140.0),
                                Color::rgba(0.2, 0.3, 0.8, 1.0),
                            )),
                    )),
            );

        let _ = runtime.render(window_id).unwrap();
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 52.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -24.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        let outer_content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 228.0)
            .expect("outer scroll content present");
        let inner_content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("inner scroll content present");

        assert_eq!(outer_content.bounds.y(), 0.0);
        assert_eq!(inner_content.bounds.y(), 16.0);
        assert!(!output.frame.layer_updates.is_empty());
    }

    #[test]
    fn nested_scroll_views_fall_back_to_parent_at_inner_limit() {
        let (mut runtime, window_id) =
            build_runtime(
                SizedBox::new()
                    .size(Size::new(80.0, 80.0))
                    .with_child(ScrollView::vertical(
                        Stack::vertical()
                            .spacing(8.0)
                            .with_child(FixedBox::new(
                                Size::new(80.0, 32.0),
                                Color::rgba(0.8, 0.2, 0.2, 1.0),
                            ))
                            .with_child(SizedBox::new().height(40.0).with_child(
                                ScrollView::vertical(FixedBox::new(
                                    Size::new(80.0, 120.0),
                                    Color::rgba(0.2, 0.7, 0.3, 1.0),
                                )),
                            ))
                            .with_child(FixedBox::new(
                                Size::new(80.0, 140.0),
                                Color::rgba(0.2, 0.3, 0.8, 1.0),
                            )),
                    )),
            );

        let _ = runtime.render(window_id).unwrap();

        let mut first = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 52.0));
        first.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -200.0)));
        runtime
            .handle_event(window_id, Event::Pointer(first))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();

        let mut second = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 52.0));
        second.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -24.0)));
        runtime
            .handle_event(window_id, Event::Pointer(second))
            .unwrap();
        let _ = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        let outer_content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 228.0)
            .expect("outer scroll content present");
        let inner_content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("inner scroll content present");

        assert_eq!(inner_content.bounds.y(), -64.0);
        assert_eq!(outer_content.bounds.y(), -24.0);
    }

    #[test]
    fn scroll_view_touch_drag_scrolls_content_after_drag_threshold() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .state(state.clone()),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Down,
                    7,
                    Point::new(20.0, 30.0),
                    Vector::ZERO,
                )),
            )
            .unwrap();
        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Move,
                    7,
                    Point::new(20.0, 6.0),
                    Vector::new(0.0, -24.0),
                )),
            )
            .unwrap();

        assert_eq!(state.current_offset(), Vector::new(0.0, 24.0));
        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Up,
                    7,
                    Point::new(20.0, 6.0),
                    Vector::ZERO,
                )),
            )
            .unwrap();
    }

    #[test]
    fn touch_move_below_threshold_or_from_another_pointer_does_not_scroll() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .state(state.clone()),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Down,
                    1,
                    Point::new(20.0, 20.0),
                    Vector::ZERO,
                )),
            )
            .unwrap();
        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Move,
                    1,
                    Point::new(20.0, 17.0),
                    Vector::new(0.0, -3.0),
                )),
            )
            .unwrap();
        runtime
            .handle_event(
                window_id,
                Event::Pointer(touch_pointer(
                    PointerEventKind::Move,
                    2,
                    Point::new(20.0, 4.0),
                    Vector::new(0.0, -16.0),
                )),
            )
            .unwrap();

        assert_eq!(state.current_offset(), Vector::ZERO);
    }

    #[test]
    fn virtual_scroll_view_supports_touch_drag_and_cancelled_gesture_recovery() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .state(state.clone())
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.7, 0.3, 0.2, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.3, 0.7, 0.2, 1.0),
                    )),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                3,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                3,
                Point::new(20.0, 10.0),
                Vector::new(0.0, -20.0),
            ),
            touch_pointer(
                PointerEventKind::Cancel,
                3,
                Point::new(20.0, 10.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Down,
                4,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                4,
                Point::new(20.0, 18.0),
                Vector::new(0.0, -12.0),
            ),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }

        assert_eq!(state.current_offset(), Vector::new(0.0, 32.0));
    }

    #[test]
    fn boundary_touch_gestures_keep_capture_for_reversal_and_accept_fresh_down() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(OverflowingBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .state(state.clone()),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                20,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                20,
                Point::new(20.0, 18.0),
                Vector::new(0.0, -200.0),
            ),
            touch_pointer(
                PointerEventKind::Up,
                20,
                Point::new(20.0, -170.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Down,
                21,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                21,
                Point::new(20.0, 18.0),
                Vector::new(0.0, -12.0),
            ),
            touch_pointer(
                PointerEventKind::Move,
                21,
                Point::new(20.0, 30.0),
                Vector::new(0.0, 12.0),
            ),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(
            state.current_offset(),
            Vector::new(0.0, 68.0),
            "a captured pan should reverse away from a hard boundary"
        );

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                21,
                Point::new(20.0, 18.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                21,
                Point::new(20.0, 30.0),
                Vector::new(0.0, 12.0),
            ),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(state.current_offset(), Vector::new(0.0, 56.0));

        let virtual_state = ScrollState::new();
        let (mut virtual_runtime, virtual_window) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .state(virtual_state.clone())
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.7, 0.3, 0.2, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 40.0),
                        Color::rgba(0.3, 0.7, 0.2, 1.0),
                    )),
            ),
        );
        let _ = virtual_runtime.render(virtual_window).unwrap();

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                30,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                30,
                Point::new(20.0, 18.0),
                Vector::new(0.0, -200.0),
            ),
            touch_pointer(
                PointerEventKind::Up,
                30,
                Point::new(20.0, -170.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Down,
                31,
                Point::new(20.0, 30.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                31,
                Point::new(20.0, 18.0),
                Vector::new(0.0, -12.0),
            ),
            touch_pointer(
                PointerEventKind::Move,
                31,
                Point::new(20.0, 30.0),
                Vector::new(0.0, 12.0),
            ),
        ] {
            virtual_runtime
                .handle_event(virtual_window, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(virtual_state.current_offset(), Vector::new(0.0, 68.0));

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                31,
                Point::new(20.0, 18.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                31,
                Point::new(20.0, 30.0),
                Vector::new(0.0, 12.0),
            ),
        ] {
            virtual_runtime
                .handle_event(virtual_window, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(virtual_state.current_offset(), Vector::new(0.0, 56.0));
    }

    #[test]
    fn nested_touch_scroll_hands_off_from_inner_limit_to_parent() {
        let outer_state = ScrollState::new();
        let inner_state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 80.0)).with_child(
                ScrollView::vertical(
                    Stack::vertical()
                        .spacing(8.0)
                        .with_child(FixedBox::new(
                            Size::new(80.0, 32.0),
                            Color::rgba(0.8, 0.2, 0.2, 1.0),
                        ))
                        .with_child(
                            SizedBox::new().height(40.0).with_child(
                                ScrollView::vertical(FixedBox::new(
                                    Size::new(80.0, 120.0),
                                    Color::rgba(0.2, 0.7, 0.3, 1.0),
                                ))
                                .state(inner_state.clone()),
                            ),
                        )
                        .with_child(FixedBox::new(
                            Size::new(80.0, 140.0),
                            Color::rgba(0.2, 0.3, 0.8, 1.0),
                        )),
                )
                .state(outer_state.clone()),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                9,
                Point::new(20.0, 52.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                9,
                Point::new(20.0, 44.0),
                Vector::new(0.0, -8.0),
            ),
            touch_pointer(
                PointerEventKind::Move,
                9,
                Point::new(20.0, -56.0),
                Vector::new(0.0, -100.0),
            ),
            touch_pointer(
                PointerEventKind::Move,
                9,
                Point::new(20.0, -80.0),
                Vector::new(0.0, -24.0),
            ),
            touch_pointer(
                PointerEventKind::Up,
                9,
                Point::new(20.0, -80.0),
                Vector::ZERO,
            ),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }

        assert_eq!(inner_state.current_offset(), Vector::new(0.0, 80.0));
        assert_eq!(outer_state.current_offset(), Vector::new(0.0, 24.0));
        assert_eq!(runtime.pointer_capture_target(window_id, 9).unwrap(), None);
    }

    #[test]
    fn touch_pan_starting_on_button_cancels_click_but_tap_still_activates() {
        let pans = Rc::new(Cell::new(0usize));
        let pan_count = Rc::clone(&pans);
        let pan_state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(100.0, 48.0)).with_child(
                ScrollView::vertical(
                    Stack::vertical()
                        .with_child(crate::Button::new("Open").on_press(move || {
                            pan_count.set(pan_count.get() + 1);
                        }))
                        .with_child(FixedBox::new(
                            Size::new(100.0, 120.0),
                            Color::rgba(0.2, 0.3, 0.7, 1.0),
                        )),
                )
                .state(pan_state.clone()),
            ),
        );
        let _ = runtime.render(window_id).unwrap();

        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                12,
                Point::new(20.0, 20.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Move,
                12,
                Point::new(20.0, 4.0),
                Vector::new(0.0, -16.0),
            ),
            touch_pointer(
                PointerEventKind::Up,
                12,
                Point::new(20.0, 4.0),
                Vector::ZERO,
            ),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(pans.get(), 0, "a pan must not activate the button");
        assert_eq!(pan_state.current_offset(), Vector::new(0.0, 16.0));

        let taps = Rc::new(Cell::new(0usize));
        let tap_count = Rc::clone(&taps);
        let (mut tap_runtime, tap_window) = build_runtime(
            SizedBox::new()
                .size(Size::new(100.0, 48.0))
                .with_child(ScrollView::vertical(
                    Stack::vertical()
                        .with_child(crate::Button::new("Open").on_press(move || {
                            tap_count.set(tap_count.get() + 1);
                        }))
                        .with_child(FixedBox::new(
                            Size::new(100.0, 120.0),
                            Color::rgba(0.2, 0.3, 0.7, 1.0),
                        )),
                )),
        );
        let _ = tap_runtime.render(tap_window).unwrap();
        for pointer in [
            touch_pointer(
                PointerEventKind::Down,
                13,
                Point::new(20.0, 20.0),
                Vector::ZERO,
            ),
            touch_pointer(
                PointerEventKind::Up,
                13,
                Point::new(20.0, 20.0),
                Vector::ZERO,
            ),
        ] {
            tap_runtime
                .handle_event(tap_window, Event::Pointer(pointer))
                .unwrap();
        }
        assert_eq!(taps.get(), 1, "a tap should still activate exactly once");
    }

    #[test]
    fn touch_pan_at_scroll_boundary_cancels_underlying_button() {
        let activations = Rc::new(Cell::new(0usize));
        let activation_count = Rc::clone(&activations);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(100.0, 48.0))
                .with_child(ScrollView::vertical(
                    Stack::vertical()
                        .with_child(FixedBox::new(
                            Size::new(100.0, 120.0),
                            Color::rgba(0.2, 0.3, 0.7, 1.0),
                        ))
                        .with_child(crate::Button::new("Bottom").on_press(move || {
                            activation_count.set(activation_count.get() + 1);
                        })),
                )),
        );
        let _ = runtime.render(window_id).unwrap();

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -500.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();
        let button = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Bottom")
            })
            .expect("bottom button visible at the scroll boundary");
        let down_position = Point::new(
            button.bounds.x() + button.bounds.width() * 0.5,
            button.bounds.y() + button.bounds.height() * 0.5,
        );
        let move_position = Point::new(down_position.x, down_position.y - 12.0);

        for pointer in [
            touch_pointer(PointerEventKind::Down, 14, down_position, Vector::ZERO),
            touch_pointer(
                PointerEventKind::Move,
                14,
                move_position,
                Vector::new(0.0, -12.0),
            ),
            touch_pointer(PointerEventKind::Up, 14, move_position, Vector::ZERO),
        ] {
            runtime
                .handle_event(window_id, Event::Pointer(pointer))
                .unwrap();
        }

        assert_eq!(
            activations.get(),
            0,
            "a boundary pan must cancel the pressed control even when content cannot move"
        );
    }

    #[test]
    fn shared_scroll_state_accepts_programmatic_offsets() {
        let state = ScrollState::new();
        state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );

        assert!(state.set_offset(Vector::new(20.0, 32.0)));
        assert_eq!(state.current_offset(), Vector::new(0.0, 32.0));
        assert!(!state.set_offset(Vector::new(0.0, 32.0)));
        assert!(state.set_offset(Vector::new(0.0, 200.0)));
        assert_eq!(state.current_offset(), Vector::new(0.0, 80.0));
    }

    #[test]
    fn scroll_bar_updates_after_wheel_scrolling_bound_scroll_view() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(92.0, 40.0))
                .with_child(ScrollBarHost::new(
                    ScrollView::vertical(FixedBox::new(
                        Size::new(80.0, 120.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .state(state.clone())
                    .overlay_scroll_bars(false)
                    .name("Scrollable content"),
                    ScrollBar::vertical(state).name("Scroll bar"),
                )),
        );

        let _ = runtime.render(window_id).unwrap();

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();

        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        let content = scroll_view_content_with_height(&graph, 120.0);
        let scroll_view = parent_node(&graph, content);
        assert_eq!(
            content.bounds,
            Rect::new(0.0, -32.0, scroll_view.bounds.width(), 120.0)
        );
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider && node.name.as_deref() == Some("Scroll bar")
            })
            .expect("scroll bar semantics present");
        assert_eq!(
            scroll_bar.value,
            Some(SemanticsValue::Range {
                value: 32.0,
                min: 0.0,
                max: 80.0,
            })
        );
    }

    #[test]
    fn embedded_overlay_scroll_bar_applies_semantic_actions() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(FixedBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .state(state.clone())
                .name("Semantic content"),
            ),
        );
        let scroll_bar_id = runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some("Semantic content vertical scroll bar")
            })
            .expect("overlay scroll bar semantics present")
            .id;

        assert!(
            runtime
                .handle_semantics_action(
                    window_id,
                    scroll_bar_id,
                    SemanticsActionRequest::Increment,
                )
                .unwrap()
        );
        assert_eq!(state.current_offset(), Vector::new(0.0, 40.0));

        assert!(
            runtime
                .handle_semantics_action(
                    window_id,
                    scroll_bar_id,
                    SemanticsActionRequest::SetValue(SemanticsValue::Range {
                        value: 72.0,
                        min: 0.0,
                        max: 80.0,
                    }),
                )
                .unwrap()
        );
        assert_eq!(state.current_offset(), Vector::new(0.0, 72.0));

        assert!(
            runtime
                .handle_semantics_action(
                    window_id,
                    scroll_bar_id,
                    SemanticsActionRequest::Decrement,
                )
                .unwrap()
        );
        assert_eq!(state.current_offset(), Vector::new(0.0, 32.0));
        assert!(
            !runtime
                .handle_semantics_action(
                    window_id,
                    scroll_bar_id,
                    SemanticsActionRequest::SetValue(SemanticsValue::Text("invalid".to_string())),
                )
                .unwrap()
        );
    }

    #[test]
    fn scroll_bar_defaults_follow_theme_density_and_width_override() {
        let compact_state = ScrollState::new();
        compact_state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let (compact, _) = render_root(
            ScrollBar::vertical(compact_state)
                .theme(DefaultTheme::compact())
                .name("Compact scroll bar"),
        );

        let touch_state = ScrollState::new();
        touch_state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let (touch, _) = render_root(
            ScrollBar::vertical(touch_state)
                .theme(DefaultTheme::touch())
                .name("Touch scroll bar"),
        );

        assert_eq!(
            compact.frame.viewport.width,
            DefaultTheme::compact().metrics.scroll_bar_thickness
        );
        assert_eq!(
            touch.frame.viewport.width,
            DefaultTheme::touch().metrics.scroll_bar_thickness
        );
        assert!(touch.frame.viewport.width > compact.frame.viewport.width);

        let override_state = ScrollState::new();
        override_state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let (overridden, _) = render_root(
            ScrollBar::vertical(override_state)
                .theme(DefaultTheme::touch())
                .width(9.0),
        );
        assert_eq!(overridden.frame.viewport.width, 9.0);
    }

    #[test]
    fn scroll_bar_theme_when_reads_current_theme() {
        let state = ScrollState::new();
        state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let theme = Rc::new(RefCell::new(DefaultTheme::touch()));
        let theme_reader = Rc::clone(&theme);
        let (output, _) = render_root(
            ScrollBar::vertical(state)
                .theme_when(move || *theme_reader.borrow())
                .name("Themed scroll bar"),
        );

        assert_eq!(
            output.frame.viewport.width,
            DefaultTheme::touch().metrics.scroll_bar_thickness
        );
    }

    #[test]
    fn scroll_bar_hover_and_drag_use_theme_motion() {
        let theme = DefaultTheme::default();
        let state = ScrollState::new();
        state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let point = Point::new(theme.metrics.scroll_bar_thickness * 0.5, 8.0);
        let expected_hover =
            super::mix_color(theme.palette.border_hover, theme.palette.accent_hover, 1.0)
                .with_alpha(0.95);
        let expected_drag = theme.palette.accent_pressed.with_alpha(0.95);
        let (mut runtime, window_id) =
            build_runtime(ScrollBar::vertical(state).theme(theme).name("Scroll bar"));

        let _ = runtime.render(window_id).expect("render should succeed");
        runtime
            .handle_event(
                window_id,
                Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)),
            )
            .expect("hover event should be handled");

        runtime.tick(theme.motion.hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let mid_hover = runtime.render(window_id).expect("render should succeed");
        assert!(!solid_fill_colors(&mid_hover).contains(&expected_hover));

        runtime.tick(theme.motion.hover_duration());
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let settled_hover = runtime.render(window_id).expect("render should succeed");
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("drag event should be handled");

        runtime.tick(theme.motion.hover_duration() + theme.motion.press_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let mid_drag = runtime.render(window_id).expect("render should succeed");
        assert!(!solid_fill_colors(&mid_drag).contains(&expected_drag));

        runtime.tick(theme.motion.hover_duration() + theme.motion.press_duration());
        assert_eq!(handle_ready_events(&mut runtime), 1);
        let settled_drag = runtime.render(window_id).expect("render should succeed");
        assert!(solid_fill_colors(&settled_drag).contains(&expected_drag));
    }

    #[test]
    fn scroll_bar_focus_stroke_uses_theme_motion() {
        let theme = DefaultTheme::default();
        let state = ScrollState::new();
        state.sync_metrics(
            ScrollAxes::Vertical,
            Size::new(80.0, 40.0),
            Size::new(80.0, 120.0),
        );
        let point = Point::new(theme.metrics.scroll_bar_thickness * 0.5, 8.0);
        let (mut runtime, window_id) =
            build_runtime(ScrollBar::vertical(state).theme(theme).name("Scroll bar"));

        let _ = runtime.render(window_id).expect("render should succeed");
        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("focus event should be handled");
        let _ = runtime.render(window_id).expect("render should succeed");

        runtime.tick(theme.motion.focus_duration() * 0.5);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let mid_focus = runtime.render(window_id).expect("render should succeed");
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
            "scroll bar focus stroke should not snap to the settled focus color"
        );

        runtime.tick(theme.motion.focus_duration() + 0.01);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let settled_focus = runtime.render(window_id).expect("render should succeed");
        let settled_strokes = solid_stroke_colors(&settled_focus);
        assert!(
            contains_approx_color(&settled_strokes, theme.palette.focus_ring),
            "scroll bar focus stroke should settle to the theme focus color; strokes={settled_strokes:?}"
        );
    }

    #[test]
    fn scroll_view_focus_keeps_viewport_chrome_neutral() {
        let theme = DefaultTheme::default();
        let point = Point::new(20.0, 20.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(FixedBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                ))
                .theme(theme),
            ),
        );

        let _ = runtime.render(window_id).expect("render should succeed");
        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("focus event should be handled");

        let focused = runtime.render(window_id).expect("render should succeed");
        let focused_strokes = solid_stroke_colors(&focused);
        assert!(
            !contains_approx_color(&focused_strokes, theme.palette.focus_ring),
            "scroll view focus should not paint an activation ring; strokes={focused_strokes:?}"
        );
    }

    #[test]
    fn virtual_scroll_view_focus_keeps_viewport_chrome_neutral() {
        let theme = DefaultTheme::default();
        let point = Point::new(20.0, 20.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                VirtualScrollView::new()
                    .theme(theme)
                    .with_child(FixedBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.3, 0.4, 0.8, 1.0),
                    ))
                    .with_child(FixedBox::new(
                        Size::new(80.0, 30.0),
                        Color::rgba(0.4, 0.5, 0.9, 1.0),
                    )),
            ),
        );

        let _ = runtime.render(window_id).expect("render should succeed");
        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("focus event should be handled");

        let focused = runtime.render(window_id).expect("render should succeed");
        let focused_strokes = solid_stroke_colors(&focused);
        assert!(
            !contains_approx_color(&focused_strokes, theme.palette.focus_ring),
            "virtual scroll view focus should not paint an activation ring; strokes={focused_strokes:?}"
        );
    }

    #[test]
    fn scroll_bar_pointer_input_moves_bound_scroll_view() {
        let state = ScrollState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(92.0, 40.0))
                .with_child(ScrollBarHost::new(
                    ScrollView::vertical(FixedBox::new(
                        Size::new(80.0, 120.0),
                        Color::rgba(0.2, 0.3, 0.7, 1.0),
                    ))
                    .state(state.clone())
                    .overlay_scroll_bars(false),
                    ScrollBar::vertical(state),
                )),
        );

        let _ = runtime.render(window_id).unwrap();
        let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(86.0, 36.0));
        down.button = Some(PointerButton::Primary);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .unwrap();
        let mut up = PointerEvent::new(PointerEventKind::Up, Point::new(86.0, 36.0));
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up)).unwrap();

        let _ = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        let content = scroll_view_content_with_height(&graph, 120.0);
        let scroll_view = parent_node(&graph, content);
        assert_eq!(
            content.bounds,
            Rect::new(0.0, -80.0, scroll_view.bounds.width(), 120.0)
        );
    }
}
