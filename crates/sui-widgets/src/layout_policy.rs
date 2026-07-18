use sui_animation::{Easing, Transition};
use sui_core::{
    Event, Point, Rect, SafeAreaInsets, SemanticsNode, SemanticsRole, Size, Vector, WakeEvent,
};
use sui_layout::{
    Alignment, Axis, Constraints, GridItem, GridLayout, GridPlacement, GridStyle, GridTrack,
    Padding, grid_layout,
};
use sui_runtime::{
    ArrangeCtx, EventCtx, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx, SemanticsCtx,
    SingleChild, Widget, WidgetChildren, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{LayerCompositionMode, LayerProperties};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridCell {
    pub placement: GridPlacement,
    pub horizontal_alignment: Alignment,
    pub vertical_alignment: Alignment,
}

impl GridCell {
    pub const fn new(row: usize, column: usize) -> Self {
        Self {
            placement: GridPlacement::new(row, column),
            horizontal_alignment: Alignment::Stretch,
            vertical_alignment: Alignment::Stretch,
        }
    }

    pub const fn span(mut self, rows: usize, columns: usize) -> Self {
        self.placement = self.placement.span(rows, columns);
        self
    }

    pub const fn align(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.horizontal_alignment = horizontal;
        self.vertical_alignment = vertical;
        self
    }
}

/// Retained explicit grid container backed by the renderer-independent
/// `sui-layout` track solver.
pub struct Grid {
    style: GridStyle,
    cells: Vec<GridCell>,
    children: WidgetChildren,
    layout: GridLayout,
    name: Option<String>,
}

impl Grid {
    pub fn new(columns: impl IntoIterator<Item = GridTrack>) -> Self {
        Self {
            style: GridStyle::new(columns),
            cells: Vec::new(),
            children: WidgetChildren::new(),
            layout: grid_layout(&GridStyle::default(), &[], Constraints::default()),
            name: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = GridTrack>) -> Self {
        self.style = self.style.rows(rows);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style = self.style.gap(gap);
        self
    }

    pub fn column_gap(mut self, gap: f32) -> Self {
        self.style = self.style.column_gap(gap);
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style = self.style.row_gap(gap);
        self
    }

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let columns = self.style.columns.len().max(1);
        let index = self.children.len();
        self.children.push(child);
        self.cells
            .push(GridCell::new(index / columns, index % columns));
        self
    }

    pub fn with_cell<W>(mut self, cell: GridCell, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.cells.push(cell);
        self
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        let columns = self.style.columns.len().max(1);
        let index = self.children.len();
        self.children.push(child);
        self.cells
            .push(GridCell::new(index / columns, index % columns));
    }

    pub fn push_cell<W>(&mut self, cell: GridCell, child: W)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self.cells.push(cell);
    }

    pub fn layout(&self) -> &GridLayout {
        &self.layout
    }

    fn layout_items(&self, natural_sizes: &[Size], minimum_widths: &[f32]) -> Vec<GridItem> {
        self.cells
            .iter()
            .zip(natural_sizes)
            .zip(minimum_widths)
            .map(|((cell, natural_size), minimum_width)| {
                GridItem::new(cell.placement, *natural_size)
                    .minimum_size(Size::new(*minimum_width, natural_size.height))
                    .align(cell.horizontal_alignment, cell.vertical_alignment)
            })
            .collect()
    }
}

impl Widget for Grid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let mut natural_sizes = Vec::with_capacity(self.children.len());
        let mut minimum_widths = Vec::with_capacity(self.children.len());
        for child in self.children.as_mut_slice() {
            let natural = child.measure(ctx, Constraints::UNBOUNDED);
            let intrinsic = child.intrinsic_size(ctx, Axis::Horizontal, natural.height);
            natural_sizes.push(natural);
            minimum_widths.push(intrinsic.minimum);
        }

        let provisional = grid_layout(
            &self.style,
            &self.layout_items(&natural_sizes, &minimum_widths),
            constraints,
        );
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            let cell = provisional
                .items
                .get(index)
                .map(|item| item.cell)
                .unwrap_or(Rect::ZERO);
            natural_sizes[index] = child.measure(
                ctx,
                Constraints::new(Size::ZERO, Size::new(cell.width(), f32::INFINITY)),
            );
        }
        self.layout = grid_layout(
            &self.style,
            &self.layout_items(&natural_sizes, &minimum_widths),
            constraints,
        );
        self.layout.size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let translation = bounds.origin.to_vector();
        for (index, item) in self.layout.items.iter().enumerate() {
            self.children
                .arrange_child(index, ctx, item.rect.translate(translation));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AspectRatioFit {
    #[default]
    Contain,
    Cover,
}

/// Constrains one retained child to an aspect ratio without specializing it as
/// an image or media widget.
pub struct AspectRatio {
    ratio: f32,
    fit: AspectRatioFit,
    alignment: (Alignment, Alignment),
    child: SingleChild,
}

impl AspectRatio {
    pub fn new<W>(ratio: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            ratio: normalize_ratio(ratio),
            fit: AspectRatioFit::Contain,
            alignment: (Alignment::Center, Alignment::Center),
            child: SingleChild::new(child),
        }
    }

    pub fn fit(mut self, fit: AspectRatioFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn align(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.alignment = (horizontal, vertical);
        self
    }

    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = normalize_ratio(ratio);
        self
    }

    fn child_rect(&self, bounds: Rect) -> Rect {
        let size = aspect_size(bounds.size, self.ratio, self.fit);
        let x = aligned_start(self.alignment.0, bounds.x(), bounds.width(), size.width);
        let y = aligned_start(self.alignment.1, bounds.y(), bounds.height(), size.height);
        Rect::new(x, y, size.width, size.height)
    }
}

impl Widget for AspectRatio {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let natural = self.child.measure(ctx, constraints.loosen());
        let desired = if natural.width > 0.0 || natural.height > 0.0 {
            aspect_size(natural, self.ratio, AspectRatioFit::Contain)
        } else {
            Size::ZERO
        };
        constraints.clamp(desired)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.child_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.fit == AspectRatioFit::Cover {
            ctx.push_clip_rect(ctx.bounds());
            self.child.paint(ctx);
            ctx.pop_clip();
        } else {
            self.child.paint(ctx);
        }
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

fn normalize_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.max(0.01)
    } else {
        1.0
    }
}

fn aspect_size(available: Size, ratio: f32, fit: AspectRatioFit) -> Size {
    let width = available.width.max(0.0);
    let height = available.height.max(0.0);
    if width == 0.0 || height == 0.0 {
        return if width > 0.0 {
            Size::new(width, width / ratio)
        } else if height > 0.0 {
            Size::new(height * ratio, height)
        } else {
            Size::ZERO
        };
    }
    let available_ratio = width / height;
    let use_width = match fit {
        AspectRatioFit::Contain => available_ratio <= ratio,
        AspectRatioFit::Cover => available_ratio >= ratio,
    };
    if use_width {
        Size::new(width, width / ratio)
    } else {
        Size::new(height * ratio, height)
    }
}

/// Selects which edges of [`SafeArea`] consume the platform-provided insets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeAreaEdges(u8);

impl SafeAreaEdges {
    pub const NONE: Self = Self(0);
    pub const LEFT: Self = Self(1 << 0);
    pub const TOP: Self = Self(1 << 1);
    pub const RIGHT: Self = Self(1 << 2);
    pub const BOTTOM: Self = Self(1 << 3);
    pub const HORIZONTAL: Self = Self(Self::LEFT.0 | Self::RIGHT.0);
    pub const VERTICAL: Self = Self(Self::TOP.0 | Self::BOTTOM.0);
    pub const ALL: Self = Self(Self::HORIZONTAL.0 | Self::VERTICAL.0);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, edge: Self) -> bool {
        self.0 & edge.0 == edge.0
    }
}

impl Default for SafeAreaEdges {
    fn default() -> Self {
        Self::ALL
    }
}

/// Insets one retained child by the current window's safe area.
///
/// Insets enter through `DpiInfo`, so a platform adapter can update them with
/// `WindowEvent::SafeAreaChanged` without rebuilding the widget tree.
pub struct SafeArea {
    edges: SafeAreaEdges,
    minimum: SafeAreaInsets,
    child: SingleChild,
}

impl SafeArea {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            edges: SafeAreaEdges::ALL,
            minimum: SafeAreaInsets::ZERO,
            child: SingleChild::new(child),
        }
    }

    pub fn edges(mut self, edges: SafeAreaEdges) -> Self {
        self.edges = edges;
        self
    }

    /// Guarantee at least these logical insets, even when the platform reports
    /// smaller values or no safe area.
    pub fn minimum(mut self, minimum: SafeAreaInsets) -> Self {
        self.minimum = minimum.normalized();
        self
    }

    fn resolved_insets(&self, platform: SafeAreaInsets) -> Padding {
        let platform = platform.normalized();
        Padding {
            left: if self.edges.contains(SafeAreaEdges::LEFT) {
                platform.left.max(self.minimum.left)
            } else {
                0.0
            },
            top: if self.edges.contains(SafeAreaEdges::TOP) {
                platform.top.max(self.minimum.top)
            } else {
                0.0
            },
            right: if self.edges.contains(SafeAreaEdges::RIGHT) {
                platform.right.max(self.minimum.right)
            } else {
                0.0
            },
            bottom: if self.edges.contains(SafeAreaEdges::BOTTOM) {
                platform.bottom.max(self.minimum.bottom)
            } else {
                0.0
            },
        }
    }
}

impl Widget for SafeArea {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let insets = self.resolved_insets(ctx.dpi().safe_area);
        let child_size = self.child.measure(
            ctx,
            Constraints::new(insets.inset(constraints.min), insets.inset(constraints.max)),
        );
        constraints.clamp(Size::new(
            child_size.width + insets.left + insets.right,
            child_size.height + insets.top + insets.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let insets = self.resolved_insets(ctx.dpi().safe_area);
        self.child.arrange(
            ctx,
            Rect::from_origin_size(
                bounds.origin + insets.offset().to_vector(),
                insets.inset(bounds.size),
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

/// Animates a retained subtree from its previous arranged origin to its new
/// origin while layout itself immediately settles at the destination.
///
/// Because the child pod is retained, focus, scroll position, editor state,
/// and keyed reconciliation identity survive the structural layout update.
pub struct LayoutTransition {
    child: SingleChild,
    duration: f64,
    easing: Easing,
    destination: Option<Point>,
    translation: Vector,
    transition: Option<Transition<Vector>>,
}

impl LayoutTransition {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
            duration: 0.18,
            easing: Easing::EaseOut,
            destination: None,
            translation: Vector::ZERO,
            transition: None,
        }
    }

    pub fn duration(mut self, duration: f64) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    fn sample(&self, time: f64) -> Vector {
        self.transition
            .map(|transition| transition.sample(time))
            .unwrap_or(self.translation)
    }
}

impl Widget for LayoutTransition {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Wake(WakeEvent::AnimationFrame { time, .. }) = event else {
            return;
        };
        let Some(transition) = self.transition else {
            return;
        };

        self.translation = transition.sample(*time);
        if transition.is_complete(*time) {
            self.translation = Vector::ZERO;
            self.transition = None;
        } else {
            ctx.request_animation_frame();
        }
        ctx.request_transform();
        ctx.set_handled();
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if let Some(previous_destination) = self.destination
            && previous_destination != bounds.origin
        {
            let visual_origin = previous_destination + self.sample(ctx.current_time());
            self.translation = visual_origin - bounds.origin;
            if self.duration <= f64::EPSILON || self.translation == Vector::ZERO {
                self.translation = Vector::ZERO;
                self.transition = None;
            } else {
                self.transition = Some(Transition::new(
                    self.translation,
                    Vector::ZERO,
                    ctx.current_time(),
                    self.duration,
                    self.easing,
                ));
                ctx.request_animation_frame();
                ctx.request_transform();
            }
        }
        self.destination = Some(bounds.origin);
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default().with_translation(self.translation)
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

fn aligned_start(alignment: Alignment, start: f32, available: f32, child: f32) -> f32 {
    match alignment {
        Alignment::Start | Alignment::Stretch => start,
        Alignment::Center => start + (available - child) * 0.5,
        Alignment::End => start + available - child,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::{Color, DpiInfo, WindowEvent, WindowId};
    use sui_reactive::Signal;
    use sui_runtime::{Application, WidgetPod, WindowBuilder};

    struct Fixed(Size);

    impl Widget for Fixed {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.0)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::WHITE);
        }

        fn intrinsic_size(
            &mut self,
            _ctx: &mut MeasureCtx,
            axis: Axis,
            _available_cross: f32,
        ) -> sui_layout::IntrinsicSize {
            let natural = match axis {
                Axis::Horizontal => self.0.width,
                Axis::Vertical => self.0.height,
            };
            sui_layout::IntrinsicSize::new(0.0, natural)
        }
    }

    #[test]
    fn grid_wraps_children_against_resolved_fraction_tracks() {
        let root = Grid::new([GridTrack::Fixed(80.0), GridTrack::Fraction(1.0)])
            .gap(8.0)
            .with_child(Fixed(Size::new(80.0, 20.0)))
            .with_child(Fixed(Size::new(220.0, 30.0)));
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(root))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime
            .handle_event(
                window,
                sui_core::Event::Window(sui_core::WindowEvent::Resized(Size::new(240.0, 80.0))),
            )
            .unwrap();
        let output = runtime.render(window).unwrap();
        let graph = runtime.widget_graph(window).unwrap();
        assert_eq!(output.frame.viewport, Size::new(240.0, 80.0));
        assert_eq!(graph.nodes[1].bounds.width(), 80.0);
        assert_eq!(graph.nodes[2].bounds.x(), 88.0);
        assert_eq!(graph.nodes[2].bounds.width(), 152.0);
    }

    #[test]
    fn aspect_ratio_contains_child_inside_tight_bounds() {
        let mut pod = WidgetPod::new(AspectRatio::new(16.0 / 9.0, Fixed(Size::new(10.0, 10.0))));
        let mut measure = sui_runtime::MeasureCtx::with_layout(
            WindowId::new(1),
            pod.id(),
            Rect::ZERO,
            sui_layout::LayoutContext::new(
                DpiInfo::default(),
                std::sync::Arc::new(sui_text::TextSystem::new()),
                std::sync::Arc::new(sui_text::FontRegistry::new()),
                std::sync::Arc::new(sui_scene::ImageRegistry::new()),
            ),
        );
        assert_eq!(
            pod.measure(&mut measure, Constraints::tight(Size::new(160.0, 120.0))),
            Size::new(160.0, 120.0)
        );
    }

    #[test]
    fn safe_area_tracks_window_insets_and_selected_edges() {
        let root = SafeArea::new(Fixed(Size::new(20.0, 20.0)))
            .edges(SafeAreaEdges::LEFT.union(SafeAreaEdges::TOP));
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(root))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime
            .handle_event(
                window,
                Event::Window(WindowEvent::SafeAreaChanged(SafeAreaInsets::new(
                    10.0, 12.0, 14.0, 16.0,
                ))),
            )
            .unwrap();
        runtime
            .handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(100.0, 80.0))),
            )
            .unwrap();
        runtime.render(window).unwrap();

        let graph = runtime.widget_graph(window).unwrap();
        assert_eq!(graph.nodes[1].bounds, Rect::new(10.0, 12.0, 90.0, 68.0));
    }

    struct MovingHost {
        moved: Signal<bool>,
        child: SingleChild,
    }

    impl Widget for MovingHost {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let _ = self
                .child
                .measure(ctx, Constraints::tight(Size::new(20.0, 20.0)));
            constraints.clamp(Size::new(100.0, 20.0))
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
            let x = if ctx.observe(&self.moved) { 60.0 } else { 0.0 };
            self.child
                .arrange(ctx, Rect::new(bounds.x() + x, bounds.y(), 20.0, 20.0));
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.child.paint(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.child.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            self.child.visit_children_mut(visitor);
        }
    }

    #[test]
    fn layout_transition_schedules_compositor_animation_after_rearrange() {
        let moved = Signal::new(false);
        let root = MovingHost {
            moved: moved.clone(),
            child: SingleChild::new(LayoutTransition::new(Fixed(Size::new(20.0, 20.0)))),
        };
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(root))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime
            .handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(100.0, 20.0))),
            )
            .unwrap();
        runtime.render(window).unwrap();

        moved.set(true);
        runtime.render(window).unwrap();
        let graph = runtime.widget_graph(window).unwrap();
        assert_eq!(graph.nodes[1].bounds.x(), 60.0);
        assert_eq!(runtime.next_wakeup_time(window).unwrap(), Some(0.0));

        runtime.tick(0.2);
        for (ready_window, event) in runtime.drain_ready_events() {
            runtime.handle_event(ready_window, event).unwrap();
        }
        runtime.render(window).unwrap();
        assert_eq!(runtime.next_wakeup_time(window).unwrap(), None);
    }
}
