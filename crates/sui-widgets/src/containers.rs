use std::ops::Range;

use sui_core::{
    Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point, Rect, ScrollDelta,
    SemanticsAction, SemanticsNode, SemanticsRole, Size, Vector,
};
use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintCtx, SemanticsCtx,
    SingleChild, Widget, WidgetChildren, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Brush, LayerCompositionMode};

pub struct Padding {
    insets: Insets,
    child: SingleChild,
}

impl Padding {
    pub fn new<W>(insets: Insets, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            insets,
            child: SingleChild::new(child),
        }
    }

    pub fn all<W>(value: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(Insets::all(value), child)
    }

    pub fn insets(&self) -> Insets {
        self.insets
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
        let child_size = self.child.child().measured_size();
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin + self.insets.offset().to_vector(), child_size),
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
    child: SingleChild,
}

impl Background {
    pub fn new<W>(brush: impl Into<Brush>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            brush: brush.into(),
            child: SingleChild::new(child),
        }
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
        ctx.fill_bounds(self.brush.clone());
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
            child.arrange(
                ctx,
                Rect::from_origin_size(bounds.origin, child.child().measured_size()),
            );
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

            let child_size = child.measured_size();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxes {
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

pub struct ScrollView {
    axes: ScrollAxes,
    name: Option<String>,
    offset: Vector,
    content_size: Size,
    child: SingleChild,
}

impl ScrollView {
    pub fn new<W>(child: W) -> Self
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
        self.axes = axes;
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub const fn current_offset(&self) -> Vector {
        self.offset
    }

    pub fn set_offset(&mut self, offset: Vector) {
        self.offset = offset;
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
        self.child = SingleChild::new(child);
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

    fn scroll_by(&mut self, viewport: Size, delta: Vector, ctx: &mut EventCtx) -> bool {
        let next = self.clamp_offset(viewport, self.offset + delta);
        if next != self.offset {
            self.offset = next;
            ctx.request_arrange();
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
}

pub struct VirtualScrollView {
    name: Option<String>,
    padding: Insets,
    spacing: f32,
    offset_y: f32,
    last_arranged_offset_y: f32,
    content_height: f32,
    item_offsets: Vec<f32>,
    visible_range: Range<usize>,
    children: WidgetChildren,
}

impl VirtualScrollView {
    pub fn new() -> Self {
        Self {
            name: None,
            padding: Insets::ZERO,
            spacing: 0.0,
            offset_y: 0.0,
            last_arranged_offset_y: 0.0,
            content_height: 0.0,
            item_offsets: Vec::new(),
            visible_range: 0..0,
            children: WidgetChildren::new(),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
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

    fn update_visible_range(&mut self, viewport_height: f32) {
        // Extend the visible window by a buffer zone so small scrolls keep a
        // stable widget set and avoid unnecessary repaint churn.
        self.visible_range = self.visible_range_for_offset(viewport_height, self.offset_y);
    }

    fn visible_children(&self) -> &[WidgetPod] {
        &self.children.as_slice()[self.visible_range.clone()]
    }

    fn visible_children_mut(&mut self) -> &mut [WidgetPod] {
        let range = self.visible_range.clone();
        &mut self.children.as_mut_slice()[range]
    }
}

impl Default for VirtualScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollView {
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

                if let Some(delta) = delta {
                    if self.scroll_by(viewport, delta, ctx) {
                        ctx.set_handled();
                    }
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                ctx.request_focus();
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
            composition_mode: LayerCompositionMode::Scroll,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
        node.name = self.name.clone();
        node.actions = vec![SemanticsAction::Focus];
        node.state.focused = ctx.is_focused();
        ctx.push(node);
        self.child.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

impl Widget for VirtualScrollView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let viewport = self.viewport_rect(ctx.bounds());

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

                if let Some(delta) = delta {
                    if self.scroll_by(viewport, delta, ctx) {
                        ctx.set_handled();
                    }
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                ctx.request_focus();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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
        self.offset_y = self.clamp_offset(viewport.height(), self.offset_y);
        self.update_visible_range(viewport.height());
        if previous_content_height != self.content_height
            || previous_item_offsets != self.item_offsets
            || previous_visible_range != self.visible_range
        {
            ctx.request_paint();
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport = self.viewport_rect(bounds);
        self.offset_y = self.clamp_offset(viewport.height(), self.offset_y);
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
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.push_clip_rect(ctx.bounds());
        for child in self.visible_children() {
            child.paint(ctx);
        }
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
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
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for child in self.visible_children() {
            visitor.visit(child);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for child in self.visible_children_mut() {
            visitor.visit(child);
        }
    }
}

fn scroll_delta_to_offset(delta: ScrollDelta) -> Vector {
    match delta {
        ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        ScrollDelta::Pixels(delta) => delta,
    }
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
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Align, Background, Padding, ScrollAxes, ScrollView, SizedBox, Stack, VirtualScrollView,
    };
    use crate::SplitView;
    use sui_core::{
        Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point,
        PointerEvent, PointerEventKind, Rect, ScrollDelta, SemanticsNode, SemanticsRole, Size,
        Vector, WidgetId,
    };
    use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
    use sui_runtime::{
        Application, ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintCtx,
        RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetGraphSnapshot,
        WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
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
            match &output.frame.scene.commands()[1] {
                SceneCommand::Layer(layer) => {
                    assert_eq!(layer.descriptor.bounds, Rect::new(0.0, 0.0, 16.0, 12.0));
                    assert_eq!(layer.scene.commands().len(), 1);
                    assert_eq!(
                        layer.scene.commands()[0],
                        SceneCommand::FillRect {
                            rect: Rect::new(0.0, 0.0, 16.0, 12.0),
                            brush: Brush::Solid(Color::rgba(0.9, 0.2, 0.1, 1.0)),
                        }
                    );
                    output.frame.scene.commands()[1].clone()
                }
                other => panic!("expected child layer command, found {other:?}"),
            }
        );
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
        let _ = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        assert_eq!(graph.nodes[1].bounds, Rect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, -32.0, 80.0, 120.0));
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
        let (mut runtime, window_id) =
            build_runtime(SizedBox::new().size(Size::new(80.0, 40.0)).with_child(
                ScrollView::vertical(PaintCounterBox::new(
                    Size::new(80.0, 120.0),
                    Color::rgba(0.2, 0.3, 0.7, 1.0),
                    Rc::clone(&counts),
                    0,
                )),
            ));

        let _ = runtime.render(window_id).unwrap();
        assert_eq!(*counts.borrow(), vec![1]);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![1]);
        assert!(output.frame.layer_updates.iter().any(|update| {
            update.kind == sui_scene::SceneLayerUpdateKind::Transform
        }));
        assert!(output.frame.layer_updates.iter().all(|update| {
            update.kind != sui_scene::SceneLayerUpdateKind::Content
        }));
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
        assert!(
            after
                .frame
                .layer_updates
                .iter()
                .any(|update| { update.kind == sui_scene::SceneLayerUpdateKind::Transform })
        );
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
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| { update.kind == sui_scene::SceneLayerUpdateKind::Transform })
        );
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| {
                    update.kind == sui_scene::SceneLayerUpdateKind::Content
                        && update.damage == Some(Rect::new(0.0, 76.0, 80.0, 4.0))
                })
        );
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
        assert!(
            output
                .frame
                .layer_updates
                .iter()
                .any(|update| update.owner == inner_content.id)
        );
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
}
