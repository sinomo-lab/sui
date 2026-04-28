use std::{cell::RefCell, ops::Range, rc::Rc};

use sui_core::{
    Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Path, Point,
    PointerButton, PointerEventKind, Rect, ScrollDelta, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsValue, Size, Vector, WidgetId,
};
use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    SemanticsCtx, SingleChild, Widget, WidgetChildren, WidgetPod, WidgetPodMutVisitor,
    WidgetPodVisitor,
};
use sui_scene::{Brush, LayerCompositionMode, StrokeStyle};

use crate::DefaultTheme;

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

    fn set_offset(&self, offset: Vector) -> bool {
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

#[derive(Debug, Clone, Copy)]
struct ScrollBarMetrics {
    track: Rect,
    thumb: Rect,
    max_scroll: f32,
}

pub struct ScrollBar {
    theme: Box<DefaultTheme>,
    state: ScrollState,
    axis: ScrollBarAxis,
    name: Option<String>,
    width: f32,
    min_thumb_length: f32,
    hovered: bool,
    dragging: bool,
    pointer_id: Option<u64>,
    drag_thumb_offset: f32,
}

impl ScrollBar {
    pub fn vertical(state: ScrollState) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            state,
            axis: ScrollBarAxis::Vertical,
            name: None,
            width: 12.0,
            min_thumb_length: 28.0,
            hovered: false,
            dragging: false,
            pointer_id: None,
            drag_thumb_offset: 0.0,
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
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(8.0);
        self
    }

    fn track_rect(&self, bounds: Rect) -> Rect {
        match self.axis {
            ScrollBarAxis::Vertical => {
                let horizontal_inset = ((bounds.width() - self.width) * 0.5).max(0.0);
                Rect::new(
                    bounds.x() + horizontal_inset,
                    bounds.y(),
                    self.width.min(bounds.width()),
                    bounds.height(),
                )
            }
            ScrollBarAxis::Horizontal => {
                let vertical_inset = ((bounds.height() - self.width) * 0.5).max(0.0);
                Rect::new(
                    bounds.x(),
                    bounds.y() + vertical_inset,
                    bounds.width(),
                    self.width.min(bounds.height()),
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
            .max(self.min_thumb_length)
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
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
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

                self.dragging = true;
                self.pointer_id = Some(pointer.pointer_id);
                self.hovered = true;
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
                ctx.request_focus();
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
                self.dragging = false;
                self.pointer_id = None;
                self.hovered = ctx.bounds().contains(pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.pointer_id == Some(pointer.pointer_id) =>
            {
                self.dragging = false;
                self.pointer_id = None;
                self.hovered = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
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
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.state.bind_scroll_bar(ctx.widget_id());
        let desired = match self.axis {
            ScrollBarAxis::Vertical => Size::new(
                self.width,
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
                self.width,
            ),
        };
        constraints.clamp(desired)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(metrics) = self.metrics(ctx.bounds()) else {
            return;
        };

        let palette = self.theme.palette;
        let track = metrics.track;
        let thumb = metrics.thumb;
        let track_radius = (track.width() * 0.5).min(track.height() * 0.5);
        let thumb_radius = (thumb.width() * 0.5).min(thumb.height() * 0.5);
        ctx.fill(
            Path::rounded_rect(track, track_radius),
            palette.surface_pressed.with_alpha(0.7),
        );
        ctx.fill(
            Path::rounded_rect(thumb, thumb_radius),
            if self.dragging {
                palette.accent_pressed
            } else if self.hovered || ctx.is_focused() {
                palette.accent_hover
            } else {
                palette.border_hover
            }
            .with_alpha(0.95),
        );
        ctx.stroke(
            Path::rounded_rect(thumb, thumb_radius),
            if ctx.is_focused() {
                palette.focus_ring
            } else {
                palette.border.with_alpha(0.9)
            },
            StrokeStyle::new(physical_pixels(ctx, self.theme.metrics.border_width).max(1.0)),
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
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
            SemanticsAction::SetValue,
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

pub struct ScrollView {
    axes: ScrollAxes,
    name: Option<String>,
    state: Option<ScrollState>,
    viewport_size_hint: bool,
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
            state: None,
            viewport_size_hint: false,
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

    pub fn state(mut self, state: ScrollState) -> Self {
        self.state = Some(state);
        self
    }

    pub const fn viewport_size_hint(mut self, enabled: bool) -> Self {
        self.viewport_size_hint = enabled;
        self
    }

    pub const fn current_offset(&self) -> Vector {
        self.offset
    }

    pub fn set_offset(&mut self, offset: Vector) {
        if let Some(state) = &self.state {
            let _ = state.set_offset(offset);
            self.offset = state.current_offset();
        } else {
            self.offset = offset;
        }
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
            self.publish_state(ctx, viewport);
            ctx.request_arrange();
            ctx.request_paint();
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
        let Some(state) = &self.state else {
            self.offset = self.clamp_offset(viewport, self.offset);
            return;
        };

        state.bind_scroll_view(ctx.widget_id(), self.child.child().id());
        if state.sync_metrics(self.axes, viewport, self.content_size) {
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
        let Some(state) = &self.state else {
            return;
        };

        state.bind_scroll_view(ctx.widget_id(), self.child.child().id());
        let _ = state.sync_metrics(self.axes, viewport, self.content_size);
        if state.set_offset(self.offset) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                if scroll_bar_id != ctx.widget_id() {
                    request_scroll_bar_refresh(ctx, scroll_bar_id);
                }
            }
        }
    }
}

pub struct VirtualScrollView {
    name: Option<String>,
    padding: Insets,
    spacing: f32,
    state: Option<ScrollState>,
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
            state: None,
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

    pub fn state(mut self, state: ScrollState) -> Self {
        self.state = Some(state);
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
        if let Some(state) = &self.state {
            let _ = state.set_offset(Vector::new(0.0, self.offset_y));
            self.offset_y = state.current_offset().y;
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
        let Some(state) = &self.state else {
            self.offset_y = self.clamp_offset(viewport.height, self.offset_y);
            return;
        };

        state.bind_scroll_view(ctx.widget_id(), ctx.widget_id());
        let content_size = Size::new(viewport.width, self.content_height);
        if state.sync_metrics(ScrollAxes::Vertical, viewport, content_size) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
        self.offset_y = self.clamp_offset(viewport.height, state.current_offset().y);
        if state.set_offset(Vector::new(0.0, self.offset_y)) {
            for scroll_bar_id in state.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
    }

    fn publish_state(&self, ctx: &mut EventCtx, viewport: Size) {
        let Some(state) = &self.state else {
            return;
        };

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
        if self.axes.allows_horizontal() {
            child_constraints.max.width = if self.viewport_size_hint {
                viewport_hint.width
            } else {
                f32::INFINITY
            };
        } else if constraints.max.width.is_finite() {
            child_constraints.min.width = constraints.max.width;
            child_constraints.max.width = constraints.max.width;
        }

        if self.axes.allows_vertical() {
            child_constraints.max.height = if self.viewport_size_hint {
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

        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_state(ctx, bounds.size);
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
        self.sync_state(ctx, viewport.size);
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
        self.sync_state(ctx, viewport.size);
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
        Align, Background, Padding, ScrollAxes, ScrollBar, ScrollState, ScrollView, SizedBox,
        Stack, VirtualScrollView,
    };
    use crate::SplitView;
    use sui_core::{
        Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point,
        PointerButton, PointerEvent, PointerEventKind, Rect, ScrollDelta, SemanticsNode,
        SemanticsRole, SemanticsValue, Size, Vector, WidgetId,
    };
    use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
    use sui_runtime::{
        Application, ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode,
        PaintCtx, RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetGraphSnapshot,
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
            SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 16.0, 12.0),
                brush: Brush::Solid(Color::rgba(0.9, 0.2, 0.1, 1.0)),
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
        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("scroll content present");
        assert_eq!(content.bounds, Rect::new(0.0, -32.0, 80.0, 120.0));
    }

    #[test]
    fn scroll_view_repaints_visible_content_after_scroll_input() {
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
        assert_eq!(*counts.borrow(), vec![1, 1]);

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(20.0, 20.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -32.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let output = runtime.render(window_id).unwrap();

        assert_eq!(*counts.borrow(), vec![2, 2]);
        assert!(output.frame.layer_updates.iter().any(|update| {
            update.kind == sui_scene::SceneLayerUpdateKind::Content
                && update.damage == Some(Rect::new(0.0, 0.0, 80.0, 60.0))
        }));
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

        assert_eq!(*counts.borrow(), vec![2]);
        assert!(!output.frame.layer_updates.is_empty());
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

        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("scroll content present");
        assert_eq!(content.bounds, Rect::new(0.0, -32.0, 80.0, 120.0));
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
                    .state(state.clone()),
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

        let content = graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 80.0 && node.bounds.height() == 120.0)
            .expect("scroll content present");
        assert_eq!(content.bounds, Rect::new(0.0, -80.0, 80.0, 120.0));
    }
}
