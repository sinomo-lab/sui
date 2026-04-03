use sui_core::{
    Event, Point, Rect, ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, Size, Vector,
};
use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
use sui_runtime::{
    EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, SingleChild, Widget, WidgetChildren, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::Brush;

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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let child_constraints = inset_constraints(constraints, self.insets);
        let child_size = self
            .child
            .layout_at(ctx, child_constraints, self.insets.offset());

        constraints.clamp(expand_size(child_size, self.insets))
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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let child_constraints =
            aligned_child_constraints(constraints, self.horizontal, self.vertical);
        let child_size = self.child.layout(ctx, child_constraints);
        let size = constraints.clamp(Size::new(
            stretched_dimension(self.horizontal, constraints.max.width, child_size.width),
            stretched_dimension(self.vertical, constraints.max.height, child_size.height),
        ));

        self.child.set_bounds(Rect::from_origin_size(
            Point::new(
                aligned_offset(self.horizontal, size.width - child_size.width),
                aligned_offset(self.vertical, size.height - child_size.height),
            ),
            child_size,
        ));

        size
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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        self.child.layout_at(ctx, constraints, Point::ZERO)
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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let child_constraints = sized_box_constraints(constraints, self.width, self.height);
        let child_size = if let Some(child) = &mut self.child {
            child.layout_at(ctx, child_constraints, Point::ZERO)
        } else {
            Size::ZERO
        };

        constraints.clamp(Size::new(
            self.width.unwrap_or(child_size.width),
            self.height.unwrap_or(child_size.height),
        ))
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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let mut child_sizes = Vec::with_capacity(self.children.len());
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
            let child_size = child.layout(ctx, child_constraints);
            main_extent += spacing_before + axis_main(self.axis, child_size);
            cross_extent = cross_extent.max(axis_cross(self.axis, child_size));
            child_sizes.push(child_size);
        }

        let size = constraints.clamp(axis_size(self.axis, main_extent, cross_extent));
        let cross_available = axis_cross(self.axis, size);
        let mut main_offset = 0.0;

        for (index, (child, child_size)) in self
            .children
            .as_mut_slice()
            .iter_mut()
            .zip(child_sizes.into_iter())
            .enumerate()
        {
            if index > 0 {
                main_offset += self.spacing;
            }

            let cross_offset = aligned_offset(
                self.alignment,
                cross_available - axis_cross(self.axis, child_size),
            );
            child.set_bounds(Rect::from_origin_size(
                axis_point(self.axis, main_offset, cross_offset),
                child_size,
            ));
            main_offset += axis_main(self.axis, child_size);
        }

        size
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

    fn scroll_by(&mut self, viewport: Size, delta: Vector, ctx: &mut EventCtx) {
        let next = self.clamp_offset(viewport, self.offset + delta);
        if next != self.offset {
            self.offset = next;
            ctx.request_layout();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for ScrollView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let viewport = ctx.bounds().size;

        match event {
            Event::Pointer(pointer)
                if pointer.kind == sui_core::PointerEventKind::Scroll
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                self.scroll_by(viewport, Vector::new(-delta.x, -delta.y), ctx);
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == sui_core::KeyState::Pressed =>
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
                    self.scroll_by(viewport, delta, ctx);
                    ctx.set_handled();
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

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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

        let child_size = self.child.layout(ctx, child_constraints);
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
        self.child.set_bounds(Rect::from_origin_size(
            Point::new(-self.offset.x, -self.offset.y),
            child_size,
        ));

        viewport
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.push_clip_rect(ctx.bounds());
        self.child.paint(ctx);
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
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
    use super::{Align, Background, Padding, ScrollView, SizedBox, Stack};
    use sui_core::{
        Color, Event, Point, PointerEvent, PointerEventKind, Rect, ScrollDelta, SemanticsNode,
        SemanticsRole, Size, Vector,
    };
    use sui_layout::{Alignment, Axis, Constraints, Padding as Insets};
    use sui_runtime::{
        Application, LayoutCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx, Widget,
        WidgetGraphSnapshot, WindowBuilder,
    };
    use sui_scene::{Brush, SceneCommand};

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
        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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
        let (output, _graph) = render_root(Background::new(
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
        assert_eq!(graph.nodes[2].bounds, Rect::new(0.0, -32.0, 80.0, 120.0));
    }
}
