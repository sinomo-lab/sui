#![forbid(unsafe_code)]

mod widget;

use sui_core::{
    DirtyRegion, Error, Event, InvalidationKind, InvalidationRequest, Point, Rect, Result,
    SemanticsNode, Size, WidgetId, WindowId,
};
use sui_layout::Constraints;
use sui_scene::SceneFrame;

pub use widget::{EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, Widget};

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
            pending_invalidations: Vec::new(),
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
        let mut ctx = EventCtx::new(window_id, window.root_widget_id);
        window.root.event(&mut ctx, &event);
        window.pending_invalidations.extend(ctx.take_invalidations());
        Ok(())
    }

    pub fn tick(&mut self, _frame_time: f64) {}

    pub fn render(&mut self, window_id: WindowId) -> Result<RenderOutput> {
        let window = self.window_mut(window_id)?;

        let mut layout_ctx = LayoutCtx::new(window_id, window.root_widget_id);
        let viewport = window.root.layout(&mut layout_ctx, Constraints::UNBOUNDED);
        let bounds = Rect::from_origin_size(Point::ZERO, viewport);

        let mut paint_ctx = PaintCtx::new(window_id, window.root_widget_id, bounds);
        window.root.paint(&mut paint_ctx);

        let mut semantics_ctx = SemanticsCtx::new(window_id, window.root_widget_id, bounds);
        window.root.semantics(&mut semantics_ctx);
        window.last_semantics = semantics_ctx.into_nodes();

        let mut invalidations = std::mem::take(&mut window.pending_invalidations);
        invalidations.extend(layout_ctx.take_invalidations());
        let (scene, paint_invalidations) = paint_ctx.into_parts();
        invalidations.extend(paint_invalidations);
        let dirty_regions = collect_dirty_regions(viewport, &invalidations);

        let frame = SceneFrame {
            window_id,
            viewport,
            dirty_regions,
            scene,
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
    pending_invalidations: Vec<InvalidationRequest>,
}

fn collect_dirty_regions(viewport: Size, invalidations: &[InvalidationRequest]) -> Vec<DirtyRegion> {
    let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);

    if invalidations.is_empty() {
        return vec![DirtyRegion::new(viewport_rect, InvalidationKind::Paint)];
    }

    let mut dirty_regions: Vec<_> = invalidations
        .iter()
        .map(|request| DirtyRegion::new(request.region.unwrap_or(viewport_rect), request.kind))
        .collect();

    if dirty_regions.iter().all(|region| region.kind != InvalidationKind::Paint) {
        dirty_regions.push(DirtyRegion::new(viewport_rect, InvalidationKind::Paint));
    }

    dirty_regions
}

#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub title: String,
    pub frame: SceneFrame,
    pub semantics: Vec<SemanticsNode>,
}
