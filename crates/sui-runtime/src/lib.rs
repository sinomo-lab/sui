#![forbid(unsafe_code)]

mod widget;

use std::collections::HashMap;

use sui_core::{
    DirtyRegion, Error, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point,
    PointerEventKind, Rect, Result, SemanticsNode, Size, WidgetId, WindowEvent, WindowId,
};
use sui_layout::Constraints;
use sui_scene::SceneFrame;

pub use widget::{
    EventCtx, EventPhase, LayoutCtx, PaintCtx, SemanticsCtx, Widget, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use widget::FocusRequest;

pub struct WindowBuilder {
    title: String,
    root: Option<WidgetPod>,
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
        self.root = Some(WidgetPod::new(root));
        self
    }

    fn build(self, window_id: WindowId) -> Result<WindowState> {
        let root = self
            .root
            .ok_or_else(|| Error::new("window root widget must be set before building"))?;

        Ok(WindowState::new(window_id, self.title, root))
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
            if runtime.needs_render(window_id)? {
                let _ = runtime.render(window_id)?;
            }
        }

        Ok(())
    }
}

pub struct Runtime {
    next_window_id: u64,
    windows: Vec<WindowState>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            next_window_id: 1,
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, builder: WindowBuilder) -> Result<WindowId> {
        let window_id = self.alloc_window_id();
        let window = builder.build(window_id)?;
        self.windows.push(window);
        Ok(window_id)
    }

    pub fn handle_event(&mut self, window_id: WindowId, event: Event) -> Result<()> {
        let window = self.window_mut(window_id)?;
        window.handle_event(event);
        Ok(())
    }

    pub fn tick(&mut self, frame_time: f64) {
        for window in &mut self.windows {
            window.last_tick_time = frame_time;
        }
    }

    pub fn render(&mut self, window_id: WindowId) -> Result<RenderOutput> {
        let window = self.window_mut(window_id)?;
        Ok(window.render())
    }

    pub fn semantics(&self, window_id: WindowId) -> Result<&[SemanticsNode]> {
        let window = self.window(window_id)?;
        Ok(&window.last_semantics)
    }

    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    pub fn needs_render(&self, window_id: WindowId) -> Result<bool> {
        let window = self.window(window_id)?;
        Ok(window.needs_render())
    }

    pub fn schedule(&self, window_id: WindowId) -> Result<FrameSchedule> {
        let window = self.window(window_id)?;
        Ok(window.schedule)
    }

    pub fn focus_state(&self, window_id: WindowId) -> Result<FocusState> {
        let window = self.window(window_id)?;
        Ok(window.focus)
    }

    pub fn focused_widget(&self, window_id: WindowId) -> Result<Option<WidgetId>> {
        Ok(self.focus_state(window_id)?.focused_widget)
    }

    pub fn widget_graph(&self, window_id: WindowId) -> Result<WidgetGraphSnapshot> {
        let window = self.window(window_id)?;
        Ok(window.graph.snapshot())
    }

    fn alloc_window_id(&mut self) -> WindowId {
        let id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FocusState {
    pub focused_widget: Option<WidgetId>,
    pub window_focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameSchedule {
    pub layout: bool,
    pub paint: bool,
    pub semantics: bool,
    pub hit_test: bool,
    pub text: bool,
    pub resources: bool,
}

impl FrameSchedule {
    fn bootstrap() -> Self {
        Self {
            layout: true,
            paint: true,
            semantics: true,
            hit_test: true,
            text: false,
            resources: false,
        }
    }

    pub const fn any(self) -> bool {
        self.layout || self.paint || self.semantics || self.hit_test || self.text || self.resources
    }

    pub const fn needs_render(self) -> bool {
        self.layout || self.paint || self.semantics || self.text || self.resources
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn mark(&mut self, kind: InvalidationKind) {
        match kind {
            InvalidationKind::Layout => {
                self.layout = true;
                self.paint = true;
                self.hit_test = true;
                self.semantics = true;
            }
            InvalidationKind::Paint => {
                self.paint = true;
            }
            InvalidationKind::HitTest => {
                self.hit_test = true;
            }
            InvalidationKind::Text => {
                self.text = true;
                self.layout = true;
                self.paint = true;
                self.semantics = true;
            }
            InvalidationKind::Semantics => {
                self.semantics = true;
            }
            InvalidationKind::Resources => {
                self.resources = true;
                self.paint = true;
            }
        }
    }

    fn extend(&mut self, invalidations: &[InvalidationRequest]) {
        for request in invalidations {
            self.mark(request.kind);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetNodeSnapshot {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub bounds: Rect,
    pub accepts_focus: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetGraphSnapshot {
    pub root: WidgetId,
    pub nodes: Vec<WidgetNodeSnapshot>,
}

struct WindowState {
    id: WindowId,
    title: String,
    root: WidgetPod,
    graph: WidgetGraph,
    focus: FocusState,
    schedule: FrameSchedule,
    viewport_hint: Option<Size>,
    viewport: Option<Size>,
    last_frame: Option<SceneFrame>,
    last_semantics: Vec<SemanticsNode>,
    pending_invalidations: Vec<InvalidationRequest>,
    last_tick_time: f64,
}

impl WindowState {
    fn new(id: WindowId, title: String, root: WidgetPod) -> Self {
        let focus = FocusState {
            focused_widget: None,
            window_focused: true,
        };

        Self {
            id,
            title,
            graph: WidgetGraph::empty(root.id()),
            root,
            focus,
            schedule: FrameSchedule::bootstrap(),
            viewport_hint: None,
            viewport: None,
            last_frame: None,
            last_semantics: Vec::new(),
            pending_invalidations: Vec::new(),
            last_tick_time: 0.0,
        }
    }

    fn needs_render(&self) -> bool {
        self.last_frame.is_none() || self.schedule.needs_render()
    }

    fn handle_event(&mut self, event: Event) {
        self.preprocess_window_event(&event);
        self.ensure_graph_for_event(&event);

        let hit_target = match &event {
            Event::Pointer(pointer) => self.graph.hit_test(pointer.position),
            _ => None,
        };

        let target = match &event {
            Event::Pointer(_) => hit_target.unwrap_or(self.root.id()),
            Event::Keyboard(_) | Event::Ime(_) => self.focus.focused_widget.unwrap_or(self.root.id()),
            _ => self.root.id(),
        };

        let route = self.route_event(target, &event);
        let mut invalidations = route.invalidations;

        let focus_request = route.focus_request.or_else(|| {
            self.default_focus_request(&event, hit_target, &route.path)
        });

        if let Some(request) = focus_request {
            invalidations.extend(self.apply_focus_request(request));
        }

        self.schedule.extend(&invalidations);
        self.pending_invalidations.extend(invalidations);
    }

    fn preprocess_window_event(&mut self, event: &Event) {
        let Event::Window(window_event) = event else {
            return;
        };

        match window_event {
            WindowEvent::Resized(size) => {
                self.viewport_hint = Some(*size);
                self.schedule.mark(InvalidationKind::Layout);
            }
            WindowEvent::ScaleFactorChanged { suggested_size, .. } => {
                if let Some(size) = suggested_size {
                    self.viewport_hint = Some(*size);
                    self.schedule.mark(InvalidationKind::Layout);
                }
            }
            WindowEvent::Focused(focused) => {
                self.focus.window_focused = *focused;
            }
            WindowEvent::RedrawRequested => {
                self.schedule.mark(InvalidationKind::Paint);
            }
            WindowEvent::CloseRequested | WindowEvent::Occluded(_) => {}
        }
    }

    fn ensure_graph_for_event(&mut self, event: &Event) {
        if self.schedule.layout || self.viewport.is_none() {
            self.run_layout_pass();
            return;
        }

        if self.schedule.hit_test || self.graph.is_empty() {
            self.refresh_graph();
        }

        if matches!(event, Event::Keyboard(_) | Event::Ime(_))
            && self.focus.focused_widget.is_some()
            && !self.graph.contains(self.focus.focused_widget.unwrap_or_default())
        {
            self.focus.focused_widget = None;
        }
    }

    fn route_event(&mut self, target: WidgetId, event: &Event) -> EventRouteResult {
        let path = self
            .graph
            .path_to(target)
            .unwrap_or_else(|| vec![self.root.id()]);
        let mut invalidations = Vec::new();
        let mut handled = false;
        let mut focus_request = None;

        if path.len() > 1 {
            for &widget_id in &path[..path.len() - 1] {
                let dispatch = self
                    .root
                    .dispatch_event_for(
                        widget_id,
                        self.id,
                        EventPhase::Capture,
                        self.focus.focused_widget,
                        event,
                    )
                    .unwrap_or_else(|| empty_dispatch());
                invalidations.extend(dispatch.invalidations);
                if dispatch.focus_request.is_some() {
                    focus_request = dispatch.focus_request;
                }
                if dispatch.handled {
                    handled = true;
                    break;
                }
            }
        }

        if !handled {
            let target_id = *path.last().unwrap_or(&self.root.id());
            let dispatch = self
                .root
                .dispatch_event_for(
                    target_id,
                    self.id,
                    EventPhase::Target,
                    self.focus.focused_widget,
                    event,
                )
                .unwrap_or_else(|| empty_dispatch());
            invalidations.extend(dispatch.invalidations);
            if dispatch.focus_request.is_some() {
                focus_request = dispatch.focus_request;
            }
            handled = dispatch.handled;
        }

        if !handled && path.len() > 1 {
            for &widget_id in path[..path.len() - 1].iter().rev() {
                let dispatch = self
                    .root
                    .dispatch_event_for(
                        widget_id,
                        self.id,
                        EventPhase::Bubble,
                        self.focus.focused_widget,
                        event,
                    )
                    .unwrap_or_else(|| empty_dispatch());
                invalidations.extend(dispatch.invalidations);
                if dispatch.focus_request.is_some() {
                    focus_request = dispatch.focus_request;
                }
                if dispatch.handled {
                    break;
                }
            }
        }

        EventRouteResult {
            path,
            invalidations,
            focus_request,
        }
    }

    fn default_focus_request(
        &self,
        event: &Event,
        hit_target: Option<WidgetId>,
        path: &[WidgetId],
    ) -> Option<FocusRequest> {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let Some(hit_target) = hit_target else {
                    return Some(FocusRequest::Clear);
                };

                path.iter()
                    .rev()
                    .copied()
                    .find(|widget_id| self.graph.node(*widget_id).is_some_and(|node| node.accepts_focus))
                    .map(FocusRequest::Focus)
                    .or_else(|| Some(FocusRequest::Clear))
                    .filter(|request| matches!(request, FocusRequest::Focus(id) if *id == hit_target) || matches!(request, FocusRequest::Clear))
            }
            Event::Window(WindowEvent::Focused(false)) => Some(FocusRequest::Clear),
            _ => None,
        }
    }

    fn apply_focus_request(&mut self, request: FocusRequest) -> Vec<InvalidationRequest> {
        let next_focus = match request {
            FocusRequest::Focus(widget_id) => Some(widget_id),
            FocusRequest::Clear => None,
        };

        if self.focus.focused_widget == next_focus {
            return Vec::new();
        }

        let previous_focus = self.focus.focused_widget;
        self.focus.focused_widget = next_focus;

        let mut invalidations = Vec::new();

        if let Some(widget_id) = previous_focus {
            invalidations.extend(self.focus_transition_invalidations(widget_id));
            if let Some(extra) = self
                .root
                .notify_focus_change_for(widget_id, self.id, self.focus.focused_widget, false)
            {
                invalidations.extend(extra);
            }
        }

        if let Some(widget_id) = next_focus {
            invalidations.extend(self.focus_transition_invalidations(widget_id));
            if let Some(extra) = self
                .root
                .notify_focus_change_for(widget_id, self.id, self.focus.focused_widget, true)
            {
                invalidations.extend(extra);
            }
        }

        self.schedule.mark(InvalidationKind::Paint);
        self.schedule.mark(InvalidationKind::Semantics);
        self.schedule.mark(InvalidationKind::HitTest);
        self.refresh_graph();

        invalidations
    }

    fn focus_transition_invalidations(&self, widget_id: WidgetId) -> Vec<InvalidationRequest> {
        let mut invalidations = vec![InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Semantics,
        )];

        if let Some(node) = self.graph.node(widget_id) {
            invalidations.push(
                InvalidationRequest::new(
                    InvalidationTarget::Widget(widget_id),
                    InvalidationKind::Paint,
                )
                .with_region(node.bounds),
            );
        }

        invalidations
    }

    fn render(&mut self) -> RenderOutput {
        let mut invalidations = std::mem::take(&mut self.pending_invalidations);
        let mut repainted = false;

        if self.last_frame.is_none() {
            self.schedule = FrameSchedule::bootstrap();
        }

        if self.schedule.layout || self.viewport.is_none() {
            invalidations.extend(self.run_layout_pass());
        } else if self.schedule.hit_test || self.graph.is_empty() {
            self.refresh_graph();
        }

        let viewport = self.viewport.unwrap_or(Size::ZERO);

        if self.schedule.paint || self.last_frame.is_none() {
            repainted = true;

            let mut paint_ctx = PaintCtx::new(
                self.id,
                self.root.id(),
                self.root.bounds(),
                self.focus.focused_widget,
            );
            self.root.paint(&mut paint_ctx);
            let (scene, paint_invalidations) = paint_ctx.into_parts();
            invalidations.extend(paint_invalidations);
            self.last_frame = Some(SceneFrame {
                window_id: self.id,
                viewport,
                dirty_regions: Vec::new(),
                scene,
            });
        }

        if self.schedule.semantics || self.last_semantics.is_empty() {
            let mut semantics_ctx = SemanticsCtx::new(
                self.id,
                self.root.id(),
                self.root.id(),
                self.root.bounds(),
                self.focus.focused_widget,
            );
            self.root.semantics(&mut semantics_ctx);
            self.last_semantics = semantics_ctx
                .into_nodes()
                .into_iter()
                .map(|mut node| {
                    node.state.focused = Some(node.id) == self.focus.focused_widget;
                    node
                })
                .collect();
        }

        let dirty_regions = collect_dirty_regions(viewport, &invalidations, repainted);
        let mut frame = self
            .last_frame
            .clone()
            .unwrap_or_else(|| SceneFrame::new(self.id, viewport));
        frame.dirty_regions = dirty_regions;

        self.schedule.clear();

        RenderOutput {
            title: self.title.clone(),
            frame,
            semantics: self.last_semantics.clone(),
        }
    }

    fn run_layout_pass(&mut self) -> Vec<InvalidationRequest> {
        let mut layout_ctx = LayoutCtx::new(self.id, self.root.id());
        let viewport = self.root.layout(&mut layout_ctx, self.layout_constraints());
        self.root
            .set_bounds(Rect::from_origin_size(Point::ZERO, viewport));
        self.viewport = Some(viewport);
        self.schedule.layout = false;
        self.schedule.hit_test = false;
        self.refresh_graph();
        layout_ctx.take_invalidations()
    }

    fn refresh_graph(&mut self) {
        self.graph = WidgetGraph::rebuild(&self.root, self.focus.focused_widget);
        self.schedule.hit_test = false;
    }

    fn layout_constraints(&self) -> Constraints {
        self.viewport_hint
            .map(Constraints::tight)
            .unwrap_or(Constraints::UNBOUNDED)
    }
}

#[derive(Default)]
struct WidgetGraph {
    root: WidgetId,
    nodes: HashMap<WidgetId, WidgetNodeSnapshot>,
    order: Vec<WidgetId>,
}

impl WidgetGraph {
    fn empty(root: WidgetId) -> Self {
        Self {
            root,
            nodes: HashMap::new(),
            order: Vec::new(),
        }
    }

    fn rebuild(root: &WidgetPod, focused_widget: Option<WidgetId>) -> Self {
        let mut graph = Self::empty(root.id());
        graph.collect(root, None, focused_widget);
        graph
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn contains(&self, widget_id: WidgetId) -> bool {
        self.nodes.contains_key(&widget_id)
    }

    fn node(&self, widget_id: WidgetId) -> Option<&WidgetNodeSnapshot> {
        self.nodes.get(&widget_id)
    }

    fn snapshot(&self) -> WidgetGraphSnapshot {
        WidgetGraphSnapshot {
            root: self.root,
            nodes: self
                .order
                .iter()
                .filter_map(|widget_id| self.nodes.get(widget_id).cloned())
                .collect(),
        }
    }

    fn hit_test(&self, point: Point) -> Option<WidgetId> {
        self.hit_test_node(self.root, point)
    }

    fn path_to(&self, target: WidgetId) -> Option<Vec<WidgetId>> {
        let mut path = Vec::new();
        let mut current = Some(target);

        while let Some(widget_id) = current {
            let node = self.node(widget_id)?;
            path.push(widget_id);
            current = node.parent;
        }

        path.reverse();
        Some(path)
    }

    fn collect(&mut self, pod: &WidgetPod, parent: Option<WidgetId>, focused_widget: Option<WidgetId>) {
        let id = pod.id();
        self.order.push(id);

        let children = {
            let mut visitor = CollectChildrenVisitor {
                graph: self,
                parent: id,
                focused_widget,
                children: Vec::new(),
            };
            pod.visit_children(&mut visitor);
            visitor.children
        };

        self.nodes.insert(
            id,
            WidgetNodeSnapshot {
                id,
                parent,
                children,
                bounds: pod.bounds(),
                accepts_focus: pod.accepts_focus(),
                focused: Some(id) == focused_widget,
            },
        );
    }

    fn hit_test_node(&self, widget_id: WidgetId, point: Point) -> Option<WidgetId> {
        let node = self.node(widget_id)?;
        if !node.bounds.contains(point) {
            return None;
        }

        for child_id in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child_id, point) {
                return Some(hit);
            }
        }

        Some(widget_id)
    }
}

struct CollectChildrenVisitor<'a> {
    graph: &'a mut WidgetGraph,
    parent: WidgetId,
    focused_widget: Option<WidgetId>,
    children: Vec<WidgetId>,
}

impl WidgetPodVisitor for CollectChildrenVisitor<'_> {
    fn visit(&mut self, child: &WidgetPod) {
        self.children.push(child.id());
        self.graph.collect(child, Some(self.parent), self.focused_widget);
    }
}

struct EventRouteResult {
    path: Vec<WidgetId>,
    invalidations: Vec<InvalidationRequest>,
    focus_request: Option<FocusRequest>,
}

fn empty_dispatch() -> widget::EventDispatch {
    widget::EventDispatch {
        handled: false,
        invalidations: Vec::new(),
        focus_request: None,
    }
}

fn collect_dirty_regions(
    viewport: Size,
    invalidations: &[InvalidationRequest],
    repainted: bool,
) -> Vec<DirtyRegion> {
    let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);

    if invalidations.is_empty() {
        return if repainted {
            vec![DirtyRegion::new(viewport_rect, InvalidationKind::Paint)]
        } else {
            Vec::new()
        };
    }

    let mut dirty_regions: Vec<_> = invalidations
        .iter()
        .map(|request| DirtyRegion::new(request.region.unwrap_or(viewport_rect), request.kind))
        .collect();

    if repainted && dirty_regions.iter().all(|region| region.kind != InvalidationKind::Paint) {
        dirty_regions.push(DirtyRegion::new(viewport_rect, InvalidationKind::Paint));
    }

    dirty_regions
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderOutput {
    pub title: String,
    pub frame: SceneFrame,
    pub semantics: Vec<SemanticsNode>,
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Application, EventCtx, FocusState, FrameSchedule, LayoutCtx, PaintCtx,
        Runtime, SemanticsCtx, Widget, WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPod,
        WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
    };
    use sui_core::{
        Color, CustomEvent, Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons,
        PointerEvent, PointerEventKind, Rect, SemanticsNode, SemanticsRole, Size,
    };
    use sui_layout::Constraints;

    #[derive(Default)]
    struct Counters {
        paint: usize,
        semantics: usize,
        keyboard: usize,
        focus_changes: usize,
    }

    struct FocusLeaf {
        counters: Rc<RefCell<Counters>>,
    }

    impl Widget for FocusLeaf {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Keyboard(_) = event {
                self.counters.borrow_mut().keyboard += 1;
                ctx.set_handled();
            }
        }

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 40.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counters.borrow_mut().paint += 1;
            ctx.fill_bounds(Color::rgba(0.22, 0.31, 0.42, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            self.counters.borrow_mut().semantics += 1;
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
            node.name = Some("focus-leaf".to_string());
            ctx.push(node);
        }

        fn accepts_focus(&self) -> bool {
            true
        }

        fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
            self.counters.borrow_mut().focus_changes += 1;
            ctx.request_paint_rect(ctx.bounds());
            ctx.request_semantics();
        }
    }

    struct TestRoot {
        counters: Rc<RefCell<Counters>>,
        child: WidgetPod,
    }

    impl Widget for TestRoot {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Custom(custom) = event && custom.kind == "semantics-only" {
                ctx.request_semantics();
            }
        }

        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(320.0, 180.0));
            let child_size = self.child.layout(ctx, Constraints::tight(Size::new(120.0, 40.0)));
            self.child
                .set_bounds(Rect::from_origin_size(Point::new(32.0, 24.0), child_size));
            size
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counters.borrow_mut().paint += 1;
            ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
            self.child.paint(ctx);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            self.counters.borrow_mut().semantics += 1;
            ctx.push(SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Window,
                ctx.bounds(),
            ));
            self.child.semantics(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            visitor.visit(&self.child);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            visitor.visit(&mut self.child);
        }
    }

    fn build_runtime() -> (Runtime, sui_core::WindowId, Rc<RefCell<Counters>>, Rc<RefCell<Counters>>) {
        let root_counters = Rc::new(RefCell::new(Counters::default()));
        let leaf_counters = Rc::new(RefCell::new(Counters::default()));

        let runtime = Application::new()
            .window(
                WindowBuilder::new().title("Test").root(TestRoot {
                    counters: Rc::clone(&root_counters),
                    child: WidgetPod::new(FocusLeaf {
                        counters: Rc::clone(&leaf_counters),
                    }),
                }),
            )
            .build()
            .unwrap();

        let window_id = runtime.window_ids()[0];
        (runtime, window_id, root_counters, leaf_counters)
    }

    fn graph_child(graph: &WidgetGraphSnapshot) -> &WidgetNodeSnapshot {
        &graph.nodes[1]
    }

    #[test]
    fn runtime_exposes_retained_widget_graph() {
        let (mut runtime, window_id, _, _) = build_runtime();

        let output = runtime.render(window_id).unwrap();
        let graph = runtime.widget_graph(window_id).unwrap();

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes[0].id, graph.root);
        assert_eq!(graph_child(&graph).parent, Some(graph.root));
        assert!(graph_child(&graph).accepts_focus);
        assert_eq!(output.frame.viewport, Size::new(320.0, 180.0));
    }

    #[test]
    fn semantics_only_invalidation_skips_repaint() {
        let (mut runtime, window_id, root_counters, leaf_counters) = build_runtime();

        let _ = runtime.render(window_id).unwrap();
        let root_paint_before = root_counters.borrow().paint;
        let leaf_paint_before = leaf_counters.borrow().paint;

        runtime
            .handle_event(
                window_id,
                Event::Custom(CustomEvent::new("semantics-only")),
            )
            .unwrap();

        assert_eq!(
            runtime.schedule(window_id).unwrap(),
            FrameSchedule {
                semantics: true,
                ..FrameSchedule::default()
            }
        );

        let _ = runtime.render(window_id).unwrap();

        assert_eq!(root_counters.borrow().paint, root_paint_before);
        assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
        assert!(root_counters.borrow().semantics >= 2);
    }

    #[test]
    fn pointer_focus_routes_keyboard_to_focused_widget() {
        let (mut runtime, window_id, _, leaf_counters) = build_runtime();

        let _ = runtime.render(window_id).unwrap();
        let child_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

        let mut pointer = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
        pointer.button = Some(PointerButton::Primary);
        pointer.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(pointer))
            .unwrap();

        assert_eq!(
            runtime.focus_state(window_id).unwrap(),
            FocusState {
                focused_widget: Some(child_id),
                window_focused: true,
            }
        );

        let output = runtime.render(window_id).unwrap();
        let focused_node = output
            .semantics
            .iter()
            .find(|node| node.id == child_id)
            .unwrap();
        assert!(focused_node.state.focused);

        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
            )
            .unwrap();

        assert_eq!(leaf_counters.borrow().keyboard, 1);
        assert!(leaf_counters.borrow().focus_changes >= 1);
    }

    #[test]
    fn initial_runtime_needs_render() {
        let (runtime, window_id, _, _) = build_runtime();

        assert!(runtime.needs_render(window_id).unwrap());
    }
}