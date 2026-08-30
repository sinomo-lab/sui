use sui_core::{
    Color, Event, InvalidationKind, Path, Point, PointerButton, PointerEventKind, Rect,
    ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector,
};
use sui_layout::Constraints;
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::StrokeStyle;
use sui_widgets::DefaultTheme;

use crate::{GraphSnapshot, NodeGraphState, Viewport};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NodeMiniMapAppearance {
    pub background: Option<Color>,
    pub border: Option<Color>,
    pub node: Option<Color>,
    pub node_selected: Option<Color>,
    pub mask: Option<Color>,
    pub viewport_border: Option<Color>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedAppearance {
    background: Color,
    border: Color,
    node: Color,
    node_selected: Color,
    mask: Color,
    viewport_border: Color,
}

impl NodeMiniMapAppearance {
    fn resolve(self, theme: &DefaultTheme) -> ResolvedAppearance {
        ResolvedAppearance {
            background: self.background.unwrap_or(theme.palette.surface_raised),
            border: self.border.unwrap_or(theme.palette.border),
            node: self
                .node
                .unwrap_or(theme.palette.text_muted.with_alpha(0.62)),
            node_selected: self.node_selected.unwrap_or(theme.palette.accent),
            mask: self.mask.unwrap_or(theme.palette.surface.with_alpha(0.58)),
            viewport_border: self.viewport_border.unwrap_or(theme.palette.focus_ring),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MiniMapTransform {
    content: Rect,
    scale: f32,
    offset: Vector,
}

impl MiniMapTransform {
    fn new(content: Rect, bounds: Rect, padding: f32) -> Option<Self> {
        if content.is_empty() || bounds.is_empty() {
            return None;
        }
        let available_width = (bounds.width() - padding * 2.0).max(1.0);
        let available_height = (bounds.height() - padding * 2.0).max(1.0);
        let scale = (available_width / content.width())
            .min(available_height / content.height())
            .max(0.0001);
        let drawn_width = content.width() * scale;
        let drawn_height = content.height() * scale;
        Some(Self {
            content,
            scale,
            offset: Vector::new(
                bounds.x() + (bounds.width() - drawn_width) * 0.5,
                bounds.y() + (bounds.height() - drawn_height) * 0.5,
            ),
        })
    }

    fn flow_to_screen(self, point: Point) -> Point {
        Point::new(
            self.offset.x + ((point.x - self.content.x()) * self.scale),
            self.offset.y + ((point.y - self.content.y()) * self.scale),
        )
    }

    fn screen_to_flow(self, point: Point) -> Point {
        Point::new(
            self.content.x() + ((point.x - self.offset.x) / self.scale),
            self.content.y() + ((point.y - self.offset.y) / self.scale),
        )
    }

    fn rect(self, rect: Rect) -> Rect {
        Rect::from_origin_size(
            self.flow_to_screen(rect.origin),
            Size::new(rect.width() * self.scale, rect.height() * self.scale),
        )
    }
}

pub struct NodeMiniMap<N = (), E = ()> {
    name: String,
    state: NodeGraphState<N, E>,
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    appearance: NodeMiniMapAppearance,
    desired_size: Size,
    padding: f32,
    pannable: bool,
    zoomable: bool,
    min_zoom: f32,
    max_zoom: f32,
    zoom_speed: f32,
    drag_pointer: Option<u64>,
}

impl<N, E> NodeMiniMap<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    pub fn new(name: impl Into<String>, state: NodeGraphState<N, E>) -> Self {
        Self {
            name: name.into(),
            state,
            theme: DefaultTheme::default(),
            theme_reader: None,
            appearance: NodeMiniMapAppearance::default(),
            desired_size: Size::new(200.0, 140.0),
            padding: 10.0,
            pannable: false,
            zoomable: false,
            min_zoom: 0.1,
            max_zoom: 4.0,
            zoom_speed: 0.002,
            drag_pointer: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
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

    pub fn appearance(mut self, appearance: NodeMiniMapAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn desired_size(mut self, size: Size) -> Self {
        self.desired_size = Size::new(size.width.max(48.0), size.height.max(48.0));
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    pub fn pannable(mut self, pannable: bool) -> Self {
        self.pannable = pannable;
        self
    }

    pub fn zoomable(mut self, zoomable: bool) -> Self {
        self.zoomable = zoomable;
        self
    }

    pub fn zoom_range(mut self, min_zoom: f32, max_zoom: f32) -> Self {
        self.min_zoom = min_zoom.max(0.001);
        self.max_zoom = max_zoom.max(self.min_zoom);
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.theme)
    }

    fn visible_flow_rect(snapshot: &GraphSnapshot<N, E>) -> Option<Rect> {
        if snapshot.viewport_size.is_empty() {
            return None;
        }
        Some(
            snapshot
                .viewport
                .visible_flow_rect(Rect::from_origin_size(Point::ZERO, snapshot.viewport_size)),
        )
    }

    fn content_bounds(snapshot: &GraphSnapshot<N, E>) -> Option<Rect> {
        match (snapshot.graph.bounds(), Self::visible_flow_rect(snapshot)) {
            (Some(graph), Some(viewport)) => Some(graph.union(viewport)),
            (Some(graph), None) => Some(graph),
            (None, Some(viewport)) => Some(viewport),
            (None, None) => None,
        }
    }

    fn transform(&self, snapshot: &GraphSnapshot<N, E>, bounds: Rect) -> Option<MiniMapTransform> {
        MiniMapTransform::new(Self::content_bounds(snapshot)?, bounds, self.padding)
    }

    fn pan_to(&self, snapshot: &GraphSnapshot<N, E>, transform: MiniMapTransform, point: Point) {
        if snapshot.viewport_size.is_empty() {
            return;
        }
        let flow_point = transform.screen_to_flow(point);
        let viewport = Viewport::centered_on(
            flow_point,
            snapshot.viewport_size,
            snapshot.viewport.zoom,
            self.min_zoom,
            self.max_zoom,
        );
        self.state.set_viewport(viewport);
    }
}

impl<N, E> Widget for NodeMiniMap<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let snapshot = ctx.observe(&self.state.signal, InvalidationKind::Paint);
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && self.zoomable
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = scroll_offset(pointer.scroll_delta, pointer.delta);
                let amount = if delta.y.abs() >= delta.x.abs() {
                    delta.y
                } else {
                    delta.x
                };
                self.state.zoom_by(
                    (amount * self.zoom_speed).exp(),
                    self.min_zoom,
                    self.max_zoom,
                );
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pannable
                    && ctx.bounds().contains(pointer.position) =>
            {
                if let Some(transform) = self.transform(&snapshot, ctx.bounds()) {
                    self.pan_to(&snapshot, transform, pointer.position);
                    self.drag_pointer = Some(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                if let Some(transform) = self.transform(&snapshot, ctx.bounds()) {
                    self.pan_to(&snapshot, transform, pointer.position);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                self.drag_pointer = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.desired_size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let theme = self.resolved_theme();
        let appearance = self.appearance.resolve(&theme);
        let bounds = ctx.bounds();
        let path = Path::rounded_rect(bounds, theme.metrics.corner_radius);
        ctx.fill(path.clone(), appearance.background);
        ctx.stroke(
            path.clone(),
            appearance.border,
            StrokeStyle::new(theme.metrics.border_width),
        );
        let Some(transform) = self.transform(&snapshot, bounds) else {
            return;
        };
        ctx.push_clip(path);
        for node in &snapshot.graph.nodes {
            if node.hidden {
                continue;
            }
            let rect = transform.rect(
                snapshot
                    .graph
                    .node_bounds(node)
                    .unwrap_or_else(|| node.bounds()),
            );
            ctx.fill(
                Path::rounded_rect(rect, 2.0),
                if node.selected {
                    appearance.node_selected
                } else {
                    appearance.node
                },
            );
        }
        if let Some(visible) = Self::visible_flow_rect(&snapshot) {
            let viewport = transform
                .rect(visible)
                .intersection(bounds)
                .unwrap_or(bounds);
            paint_outside_mask(ctx, bounds, viewport, appearance.mask);
            ctx.stroke_rect(viewport, appearance.viewport_border, StrokeStyle::new(1.5));
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some("Overview of the node graph and current viewport".to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "{} nodes, zoom {:.0}%",
            snapshot.graph.nodes.len(),
            snapshot.viewport.zoom * 100.0
        )));
        node.state.focused = ctx.is_focused();
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Custom("Pan".into()),
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.pannable || self.zoomable
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn paint_outside_mask(ctx: &mut PaintCtx, bounds: Rect, viewport: Rect, color: Color) {
    let top_height = (viewport.y() - bounds.y()).max(0.0);
    let bottom_height = (bounds.max_y() - viewport.max_y()).max(0.0);
    let left_width = (viewport.x() - bounds.x()).max(0.0);
    let right_width = (bounds.max_x() - viewport.max_x()).max(0.0);
    ctx.fill_rect(
        Rect::new(bounds.x(), bounds.y(), bounds.width(), top_height),
        color,
    );
    ctx.fill_rect(
        Rect::new(bounds.x(), viewport.max_y(), bounds.width(), bottom_height),
        color,
    );
    ctx.fill_rect(
        Rect::new(bounds.x(), viewport.y(), left_width, viewport.height()),
        color,
    );
    ctx.fill_rect(
        Rect::new(
            viewport.max_x(),
            viewport.y(),
            right_width,
            viewport.height(),
        ),
        color,
    );
}

fn scroll_offset(delta: Option<ScrollDelta>, fallback: Vector) -> Vector {
    match delta {
        Some(ScrollDelta::Lines(delta)) => Vector::new(delta.x * 48.0, delta.y * 48.0),
        Some(ScrollDelta::Pixels(delta)) => delta,
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use sui_core::Point;

    use super::*;
    use crate::Node;

    #[test]
    fn minimap_transform_round_trips_flow_coordinates() {
        let transform = MiniMapTransform::new(
            Rect::new(-100.0, 50.0, 500.0, 250.0),
            Rect::new(20.0, 30.0, 200.0, 120.0),
            10.0,
        )
        .unwrap();
        let flow = Point::new(125.0, 175.0);

        let round_trip = transform.screen_to_flow(transform.flow_to_screen(flow));

        assert!((round_trip.x - flow.x).abs() < 0.001);
        assert!((round_trip.y - flow.y).abs() < 0.001);
    }

    #[test]
    fn minimap_content_includes_nodes_and_visible_viewport() {
        let state = NodeGraphState::<(), ()>::new(
            vec![Node::new("node", Point::new(600.0, 400.0), ())],
            Vec::new(),
        )
        .unwrap();
        state.set_viewport_size(Size::new(400.0, 300.0));

        let content = NodeMiniMap::<(), ()>::content_bounds(&state.snapshot()).unwrap();

        assert!(content.contains(Point::ZERO));
        assert!(content.contains(Point::new(780.0, 472.0)));
    }
}
