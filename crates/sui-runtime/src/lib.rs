#![forbid(unsafe_code)]

use sui_core::{
    Color, DirtyRegion, Error, Event, InvalidationKind, Rect, Result, SemanticsNode,
    Size, WidgetId, WindowId,
};
use sui_layout::Constraints;
use sui_scene::{Brush, Scene, SceneCommand, SceneFrame};

pub trait Widget {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.max
    }

    fn paint(&self, _ctx: &mut PaintCtx) {}

    fn semantics(&self, _ctx: &mut SemanticsCtx) {}
}

pub struct WindowBuilder {
    title: String,
    root: Option<Box<dyn Widget>>,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self {
            title: "SUI Window".to_string(),
            root: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn root<W>(mut self, root: W) -> Self
    where
        W: Widget + 'static,
    {
        self.root = Some(Box::new(root));
        self
    }

    fn build(self, window_id: WindowId, root_widget_id: WidgetId) -> Result<WindowState> {
        let root = self
            .root
            .ok_or_else(|| Error::new("window root widget must be set before building"))?;

        Ok(WindowState {
            id: window_id,
            title: self.title,
            root,
            root_widget_id,
            last_semantics: Vec::new(),
        })
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct Application {
    windows: Vec<WindowBuilder>,
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.windows.push(window);
        self
    }

    pub fn build(self) -> Result<Runtime> {
        let mut runtime = Runtime::new();

        for window in self.windows {
            runtime.add_window(window)?;
        }

        Ok(runtime)
    }

    pub fn run(self) -> Result<()> {
        let mut runtime = self.build()?;
        runtime.tick(0.0);

        for window_id in runtime.window_ids() {
            let _ = runtime.render(window_id)?;
        }

        Ok(())
    }
}

pub struct Runtime {
    next_window_id: u64,
    next_widget_id: u64,
    windows: Vec<WindowState>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            next_window_id: 1,
            next_widget_id: 1,
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, builder: WindowBuilder) -> Result<WindowId> {
        let window_id = self.alloc_window_id();
        let root_widget_id = self.alloc_widget_id();
        let window = builder.build(window_id, root_widget_id)?;
        self.windows.push(window);
        Ok(window_id)
    }

    pub fn handle_event(&mut self, window_id: WindowId, event: Event) -> Result<()> {
        let window = self.window_mut(window_id)?;
        let mut ctx = EventCtx::new(window_id);
        window.root.event(&mut ctx, &event);
        Ok(())
    }

    pub fn tick(&mut self, _frame_time: f64) {}

    pub fn render(&mut self, window_id: WindowId) -> Result<RenderOutput> {
        let window = self.window_mut(window_id)?;

        let mut layout_ctx = LayoutCtx::new(window_id);
        let viewport = window.root.layout(&mut layout_ctx, Constraints::UNBOUNDED);

        let mut paint_ctx = PaintCtx::new(window_id);
        window.root.paint(&mut paint_ctx);

        let mut semantics_ctx = SemanticsCtx::new(window_id, window.root_widget_id);
        window.root.semantics(&mut semantics_ctx);
        window.last_semantics = semantics_ctx.into_nodes();

        let frame = SceneFrame {
            window_id,
            viewport,
            dirty_regions: vec![DirtyRegion {
                area: Rect::new(0.0, 0.0, viewport.width, viewport.height),
                kind: InvalidationKind::Paint,
            }],
            scene: paint_ctx.into_scene(),
        };

        Ok(RenderOutput {
            title: window.title.clone(),
            frame,
            semantics: window.last_semantics.clone(),
        })
    }

    pub fn semantics(&self, window_id: WindowId) -> Result<&[SemanticsNode]> {
        let window = self.window(window_id)?;
        Ok(&window.last_semantics)
    }

    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    fn alloc_window_id(&mut self) -> WindowId {
        let id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    fn alloc_widget_id(&mut self) -> WidgetId {
        let id = WidgetId::new(self.next_widget_id);
        self.next_widget_id += 1;
        id
    }

    fn window(&self, window_id: WindowId) -> Result<&WindowState> {
        self.windows
            .iter()
            .find(|window| window.id == window_id)
            .ok_or_else(|| Error::new(format!("window {} does not exist", window_id.get())))
    }

    fn window_mut(&mut self, window_id: WindowId) -> Result<&mut WindowState> {
        self.windows
            .iter_mut()
            .find(|window| window.id == window_id)
            .ok_or_else(|| Error::new(format!("window {} does not exist", window_id.get())))
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

struct WindowState {
    id: WindowId,
    title: String,
    root: Box<dyn Widget>,
    root_widget_id: WidgetId,
    last_semantics: Vec<SemanticsNode>,
}

#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub title: String,
    pub frame: SceneFrame,
    pub semantics: Vec<SemanticsNode>,
}

#[derive(Debug, Clone)]
pub struct EventCtx {
    pub window_id: WindowId,
    invalidations: Vec<InvalidationKind>,
}

impl EventCtx {
    pub fn new(window_id: WindowId) -> Self {
        Self {
            window_id,
            invalidations: Vec::new(),
        }
    }

    pub fn request(&mut self, kind: InvalidationKind) {
        self.invalidations.push(kind);
    }

    pub fn request_layout(&mut self) {
        self.request(InvalidationKind::Layout);
    }

    pub fn request_paint(&mut self) {
        self.request(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request(InvalidationKind::Semantics);
    }

    pub fn invalidations(&self) -> &[InvalidationKind] {
        &self.invalidations
    }
}

#[derive(Debug, Clone)]
pub struct LayoutCtx {
    pub window_id: WindowId,
    invalidations: Vec<InvalidationKind>,
}

impl LayoutCtx {
    pub fn new(window_id: WindowId) -> Self {
        Self {
            window_id,
            invalidations: Vec::new(),
        }
    }

    pub fn request_layout(&mut self) {
        self.invalidations.push(InvalidationKind::Layout);
    }

    pub fn invalidations(&self) -> &[InvalidationKind] {
        &self.invalidations
    }
}

#[derive(Debug, Clone)]
pub struct PaintCtx {
    pub window_id: WindowId,
    scene: Scene,
}

impl PaintCtx {
    pub fn new(window_id: WindowId) -> Self {
        Self {
            window_id,
            scene: Scene::new(),
        }
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

    pub fn label(&mut self, rect: Rect, text: impl Into<String>, color: Color) {
        self.scene.push(SceneCommand::Label {
            rect,
            text: text.into(),
            color,
        });
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn into_scene(self) -> Scene {
        self.scene
    }
}

#[derive(Debug, Clone)]
pub struct SemanticsCtx {
    pub window_id: WindowId,
    root_widget_id: WidgetId,
    nodes: Vec<SemanticsNode>,
}

impl SemanticsCtx {
    pub fn new(window_id: WindowId, root_widget_id: WidgetId) -> Self {
        Self {
            window_id,
            root_widget_id,
            nodes: Vec::new(),
        }
    }

    pub fn root_widget_id(&self) -> WidgetId {
        self.root_widget_id
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[SemanticsNode] {
        &self.nodes
    }

    pub fn into_nodes(self) -> Vec<SemanticsNode> {
        self.nodes
    }
}