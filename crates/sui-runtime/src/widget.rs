use sui_core::{
    Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Rect,
    SemanticsNode, Size, WidgetId, WindowId,
};
use sui_layout::Constraints;
use sui_scene::{Brush, Scene, SceneCommand};

pub trait Widget {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.max
    }

    fn paint(&self, _ctx: &mut PaintCtx) {}

    fn semantics(&self, _ctx: &mut SemanticsCtx) {}
}

#[derive(Debug, Clone)]
pub struct EventCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    handled: bool,
    invalidations: Vec<InvalidationRequest>,
}

impl EventCtx {
    pub(crate) fn new(window_id: WindowId, widget_id: WidgetId) -> Self {
        Self {
            window_id,
            widget_id,
            handled: false,
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn is_handled(&self) -> bool {
        self.handled
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_layout(&mut self) {
        self.request_widget(InvalidationKind::Layout);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn request_hit_test(&mut self) {
        self.request_widget(InvalidationKind::HitTest);
    }

    pub fn request_text(&mut self) {
        self.request_widget(InvalidationKind::Text);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn request_resources(&mut self) {
        self.request_widget(InvalidationKind::Resources);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct LayoutCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    invalidations: Vec<InvalidationRequest>,
}

impl LayoutCtx {
    pub(crate) fn new(window_id: WindowId, widget_id: WidgetId) -> Self {
        Self {
            window_id,
            widget_id,
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_layout(&mut self) {
        self.request_widget(InvalidationKind::Layout);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct PaintCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    bounds: Rect,
    scene: Scene,
    invalidations: Vec<InvalidationRequest>,
}

impl PaintCtx {
    pub(crate) fn new(window_id: WindowId, widget_id: WidgetId, bounds: Rect) -> Self {
        Self {
            window_id,
            widget_id,
            bounds,
            scene: Scene::new(),
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn clear(&mut self, color: Color) {
        self.scene.push(SceneCommand::Clear(color));
    }

    pub fn fill_rect(&mut self, rect: Rect, brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillRect {
            rect,
            brush: brush.into(),
        });
    }

    pub fn fill_bounds(&mut self, brush: impl Into<Brush>) {
        self.fill_rect(self.bounds, brush);
    }

    pub fn label(&mut self, rect: Rect, text: impl Into<String>, color: Color) {
        self.scene.push(SceneCommand::Label {
            rect,
            text: text.into(),
            color,
        });
    }

    pub fn push(&mut self, command: SceneCommand) {
        self.scene.push(command);
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn into_parts(self) -> (Scene, Vec<InvalidationRequest>) {
        (self.scene, self.invalidations)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct SemanticsCtx {
    window_id: WindowId,
    root_widget_id: WidgetId,
    bounds: Rect,
    nodes: Vec<SemanticsNode>,
}

impl SemanticsCtx {
    pub(crate) fn new(window_id: WindowId, root_widget_id: WidgetId, bounds: Rect) -> Self {
        Self {
            window_id,
            root_widget_id,
            bounds,
            nodes: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.root_widget_id
    }

    pub const fn root_widget_id(&self) -> WidgetId {
        self.root_widget_id
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[SemanticsNode] {
        &self.nodes
    }

    pub(crate) fn into_nodes(self) -> Vec<SemanticsNode> {
        self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::{EventCtx, LayoutCtx, PaintCtx};
    use sui_core::{InvalidationKind, Rect, WidgetId, WindowId};

    #[test]
    fn event_ctx_tracks_widget_scoped_invalidations() {
        let mut ctx = EventCtx::new(WindowId::new(1), WidgetId::new(2));

        ctx.request_layout();
        ctx.request_paint_rect(Rect::new(8.0, 12.0, 24.0, 36.0));
        ctx.set_handled();

        assert!(ctx.is_handled());
        assert_eq!(ctx.invalidations().len(), 2);
        assert_eq!(ctx.invalidations()[0].kind, InvalidationKind::Layout);
        assert_eq!(ctx.invalidations()[1].region, Some(Rect::new(8.0, 12.0, 24.0, 36.0)));
    }

    #[test]
    fn layout_and_paint_ctx_expose_widget_metadata() {
        let mut layout = LayoutCtx::new(WindowId::new(3), WidgetId::new(4));
        layout.request_paint();

        let mut paint = PaintCtx::new(
            WindowId::new(3),
            WidgetId::new(4),
            Rect::new(0.0, 0.0, 120.0, 60.0),
        );
        paint.fill_bounds(sui_core::Color::rgba(0.2, 0.3, 0.4, 1.0));

        assert_eq!(layout.window_id(), WindowId::new(3));
        assert_eq!(paint.widget_id(), WidgetId::new(4));
        assert_eq!(paint.bounds(), Rect::new(0.0, 0.0, 120.0, 60.0));
        assert_eq!(paint.scene().commands().len(), 1);
    }
}