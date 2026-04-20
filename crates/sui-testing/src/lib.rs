#![forbid(unsafe_code)]

mod app;
mod diagnostics;
mod expect;
mod harness;
mod locator;
mod screenshot;
mod selector;
mod snapshot;
mod window;

pub use app::{IntoTestRuntime, TestApp};
pub use expect::Expectation;
pub use locator::Locator;
pub use screenshot::{
    ArtifactBundle, Screenshot, hdr_clip_mask, hdr_headroom_heatmap, hdr_luminance_heatmap,
    write_hdr_avif, write_hdr_exr,
};
pub use selector::Selector;
pub use snapshot::{SceneSummary, WindowSnapshot};
pub use window::TestWindow;

pub mod prelude {
    pub use crate::{
        ArtifactBundle, Expectation, IntoTestRuntime, Locator, SceneSummary, Screenshot, Selector,
        TestApp, TestWindow, hdr_clip_mask, hdr_headroom_heatmap, hdr_luminance_heatmap,
        write_hdr_avif, write_hdr_exr,
    };
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        fs,
        path::PathBuf,
        rc::Rc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::TestApp;
    use sui_core::{
        Color, Event, ImeEvent, KeyState, PointerEventKind, Result, SemanticsAction, SemanticsNode,
        SemanticsRole, SemanticsValue, Size, TimerToken, Vector, WakeEvent,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild, Widget,
        WidgetChildren, WidgetPodVisitor, WindowBuilder,
    };
    use sui_scene::StrokeStyle;

    #[derive(Debug, Default)]
    struct AppState {
        button_hovered: bool,
        button_clicks: usize,
        save_timer: Option<TimerToken>,
        status: String,
        input_value: String,
    }

    struct HarnessRoot {
        children: WidgetChildren,
    }

    impl HarnessRoot {
        fn new(state: Rc<RefCell<AppState>>) -> Self {
            let mut children = WidgetChildren::new();
            children.push(StatusButton::new(Rc::clone(&state)));
            children.push(TestInput::new(state));
            Self { children }
        }
    }

    impl Widget for HarnessRoot {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let viewport = constraints.clamp(Size::new(480.0, 240.0));
            let child_constraints = Constraints::tight(Size::new(180.0, 44.0));
            self.children.measure_child(0, ctx, child_constraints);
            self.children.measure_child(1, ctx, child_constraints);
            viewport
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui_core::Rect) {
            self.children.arrange_child(
                0,
                ctx,
                sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 24.0, 180.0, 44.0),
            );
            self.children.arrange_child(
                1,
                ctx,
                sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 92.0, 180.0, 44.0),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
            self.children.paint(ctx);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
            root.name = Some("Harness Window".to_string());
            ctx.push(root);
            self.children.semantics(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.children.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn sui_runtime::WidgetPodMutVisitor) {
            self.children.visit_children_mut(visitor);
        }
    }

    struct StatusButton {
        state: Rc<RefCell<AppState>>,
    }

    impl StatusButton {
        fn new(state: Rc<RefCell<AppState>>) -> Self {
            Self { state }
        }
    }

    impl Widget for StatusButton {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                    let hovered = ctx.bounds().contains(pointer.position);
                    let mut state = self.state.borrow_mut();
                    if state.button_hovered != hovered {
                        state.button_hovered = hovered;
                        ctx.request_paint();
                        ctx.request_semantics();
                    }
                }
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                    let mut state = self.state.borrow_mut();
                    state.button_clicks += 1;
                    state.status = "Saving".to_string();
                    state.save_timer = Some(ctx.schedule_timer_after(2.0));
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                Event::Keyboard(key)
                    if key.state == KeyState::Pressed
                        && ctx.is_focused()
                        && matches!(key.key.as_str(), "Enter" | " ") =>
                {
                    let mut state = self.state.borrow_mut();
                    state.status = "Activated".to_string();
                    state.save_timer = None;
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                Event::Wake(WakeEvent::Timer { token, .. }) => {
                    let mut state = self.state.borrow_mut();
                    if state.save_timer == Some(*token) {
                        state.status = "Saved".to_string();
                        state.save_timer = None;
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                }
                _ => {}
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 44.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let state = self.state.borrow();
            let color = if ctx.is_focused() {
                Color::rgba(0.25, 0.52, 0.88, 1.0)
            } else if state.button_hovered {
                Color::rgba(0.21, 0.38, 0.66, 1.0)
            } else {
                Color::rgba(0.17, 0.24, 0.37, 1.0)
            };
            ctx.fill_bounds(color);
            ctx.label(
                ctx.bounds(),
                format!("Save ({})", state.button_clicks),
                Color::rgba(0.95, 0.96, 0.98, 1.0),
            );
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let state = self.state.borrow();
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
            node.name = Some("Save".to_string());
            node.description = Some(state.status.clone());
            node.state.hovered = state.button_hovered;
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
            ctx.push(node);

            let mut status = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
            status.parent = Some(ctx.root_widget_id());
            status.name = Some(state.status.clone());
            ctx.push(status);
        }

        fn accepts_focus(&self) -> bool {
            true
        }

        fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    struct TestInput {
        state: Rc<RefCell<AppState>>,
    }

    impl TestInput {
        fn new(state: Rc<RefCell<AppState>>) -> Self {
            Self { state }
        }
    }

    impl Widget for TestInput {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                    self.state.borrow_mut().input_value = text.clone();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                _ => {}
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 44.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let border = if ctx.is_focused() {
                Color::rgba(0.66, 0.74, 0.93, 1.0)
            } else {
                Color::rgba(0.38, 0.43, 0.52, 1.0)
            };
            ctx.fill_bounds(Color::rgba(0.13, 0.15, 0.20, 1.0));
            ctx.stroke_rect(ctx.bounds(), border, StrokeStyle::new(1.0));
            ctx.label(
                ctx.bounds(),
                self.state.borrow().input_value.clone(),
                Color::rgba(0.95, 0.96, 0.98, 1.0),
            );
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
            node.name = Some("Name".to_string());
            node.value = Some(SemanticsValue::Text(
                self.state.borrow().input_value.clone(),
            ));
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
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

    fn build_app() -> Result<TestApp> {
        TestApp::new(|| {
            let state = Rc::new(RefCell::new(AppState::default()));
            Application::new().window(
                WindowBuilder::new()
                    .title("Harness")
                    .root(HarnessRoot::new(state)),
            )
        })
    }

    #[derive(Debug, Default)]
    struct ListState {
        selected: String,
    }

    struct ListRoot {
        state: Rc<RefCell<ListState>>,
        children: WidgetChildren,
    }

    impl ListRoot {
        fn new(state: Rc<RefCell<ListState>>) -> Self {
            let mut children = WidgetChildren::new();
            children.push(StatusLabel::new(Rc::clone(&state)));
            children.push(ContactRow::new(
                "Ada",
                sui_core::Point::new(320.0, 6.0),
                Rc::clone(&state),
            ));
            children.push(ContactRow::new(
                "Grace",
                sui_core::Point::new(320.0, 6.0),
                Rc::clone(&state),
            ));
            Self { state, children }
        }
    }

    impl Widget for ListRoot {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let viewport = constraints.clamp(Size::new(480.0, 260.0));
            let width = viewport.width - 48.0;

            self.children
                .measure_child(0, ctx, Constraints::tight(Size::new(width, 32.0)));
            self.children
                .measure_child(1, ctx, Constraints::tight(Size::new(width, 44.0)));
            self.children
                .measure_child(2, ctx, Constraints::tight(Size::new(width, 44.0)));

            viewport
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui_core::Rect) {
            let width = bounds.width() - 48.0;
            self.children.arrange_child(
                0,
                ctx,
                sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 20.0, width, 32.0),
            );
            self.children.arrange_child(
                1,
                ctx,
                sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 72.0, width, 44.0),
            );
            self.children.arrange_child(
                2,
                ctx,
                sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 128.0, width, 44.0),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let _ = &self.state;
            ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
            self.children.paint(ctx);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
            root.name = Some("List Harness Window".to_string());
            ctx.push(root);
            self.children.semantics(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.children.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn sui_runtime::WidgetPodMutVisitor) {
            self.children.visit_children_mut(visitor);
        }
    }

    struct StatusLabel {
        state: Rc<RefCell<ListState>>,
    }

    impl StatusLabel {
        fn new(state: Rc<RefCell<ListState>>) -> Self {
            Self { state }
        }
    }

    impl Widget for StatusLabel {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(320.0, 32.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.label(
                ctx.bounds(),
                format!("Selected: {}", self.state.borrow().selected),
                Color::rgba(0.95, 0.96, 0.98, 1.0),
            );
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
            node.name = Some(format!("Selected: {}", self.state.borrow().selected));
            ctx.push(node);
        }
    }

    struct ContactRow {
        name: String,
        button_origin: sui_core::Point,
        children: WidgetChildren,
    }

    impl ContactRow {
        fn new(name: &str, button_origin: sui_core::Point, state: Rc<RefCell<ListState>>) -> Self {
            let mut children = WidgetChildren::new();
            children.push(RowButton::new(name.to_string(), state));
            Self {
                name: name.to_string(),
                button_origin,
                children,
            }
        }
    }

    impl Widget for ContactRow {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(360.0, 44.0));
            self.children
                .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 32.0)));
            size
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui_core::Rect) {
            self.children.arrange_child(
                0,
                ctx,
                sui_core::Rect::new(
                    bounds.x() + self.button_origin.x,
                    bounds.y() + self.button_origin.y,
                    120.0,
                    32.0,
                ),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.13, 0.15, 0.20, 1.0));
            ctx.label(
                sui_core::Rect::new(ctx.bounds().x() + 12.0, ctx.bounds().y() + 8.0, 140.0, 24.0),
                &self.name,
                Color::rgba(0.95, 0.96, 0.98, 1.0),
            );
            self.children.paint(ctx);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.description = Some(self.name.clone());
            ctx.push(node);
            self.children.semantics(ctx);
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            self.children.visit_children(visitor);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn sui_runtime::WidgetPodMutVisitor) {
            self.children.visit_children_mut(visitor);
        }
    }

    struct RowButton {
        label: String,
        state: Rc<RefCell<ListState>>,
    }

    impl RowButton {
        fn new(label: String, state: Rc<RefCell<ListState>>) -> Self {
            Self { label, state }
        }
    }

    impl Widget for RowButton {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                    self.state.borrow_mut().selected = self.label.clone();
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                _ => {}
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 32.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let color = if ctx.is_focused() {
                Color::rgba(0.25, 0.52, 0.88, 1.0)
            } else {
                Color::rgba(0.17, 0.24, 0.37, 1.0)
            };
            ctx.fill_bounds(color);
            ctx.label(ctx.bounds(), "Select", Color::rgba(0.95, 0.96, 0.98, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
            node.name = Some("Select".to_string());
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
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

    fn build_list_app() -> Result<TestApp> {
        TestApp::new(|| {
            let state = Rc::new(RefCell::new(ListState::default()));
            Application::new().window(
                WindowBuilder::new()
                    .title("List Harness")
                    .root(ListRoot::new(state)),
            )
        })
    }

    struct ScrollHarness {
        offset_y: f32,
        child: SingleChild,
    }

    impl ScrollHarness {
        fn new() -> Self {
            Self {
                offset_y: 0.0,
                child: SingleChild::new(ScrollContent),
            }
        }
    }

    impl Widget for ScrollHarness {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Pointer(pointer)
                    if pointer.kind == PointerEventKind::Scroll
                        && ctx.bounds().contains(pointer.position) =>
                {
                    let delta = pointer
                        .scroll_delta
                        .map(|delta| match delta {
                            sui_core::ScrollDelta::Lines(delta) => {
                                Vector::new(delta.x * 40.0, delta.y * 40.0)
                            }
                            sui_core::ScrollDelta::Pixels(delta) => delta,
                        })
                        .unwrap_or(pointer.delta);
                    let next = (self.offset_y - delta.y).clamp(0.0, 120.0);
                    if (next - self.offset_y).abs() > f32::EPSILON {
                        self.offset_y = next;
                        ctx.request_arrange();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                }
                Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                    ctx.request_focus();
                    ctx.request_semantics();
                }
                _ => {}
            }
        }

        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let viewport = constraints.clamp(Size::new(160.0, 80.0));
            self.child
                .measure(ctx, Constraints::tight(Size::new(160.0, 200.0)));
            viewport
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui_core::Rect) {
            self.child.arrange(
                ctx,
                sui_core::Rect::new(bounds.x(), bounds.y() - self.offset_y, 160.0, 200.0),
            );
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.clear(Color::rgba(0.05, 0.06, 0.08, 1.0));
            ctx.push_clip_rect(ctx.bounds());
            self.child.paint(ctx);
            ctx.pop_clip();
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
            node.name = Some("Scroll Harness".to_string());
            node.actions = vec![SemanticsAction::Focus];
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

        fn visit_children_mut(&mut self, visitor: &mut dyn sui_runtime::WidgetPodMutVisitor) {
            self.child.visit_children_mut(visitor);
        }
    }

    struct ScrollContent;

    impl Widget for ScrollContent {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(160.0, 200.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_rect(
                sui_core::Rect::new(ctx.bounds().x(), ctx.bounds().y(), 160.0, 100.0),
                Color::rgba(0.18, 0.32, 0.68, 1.0),
            );
            ctx.fill_rect(
                sui_core::Rect::new(ctx.bounds().x(), ctx.bounds().y() + 100.0, 160.0, 100.0),
                Color::rgba(0.78, 0.32, 0.18, 1.0),
            );
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some("Scroll Content".to_string());
            ctx.push(node);
        }
    }

    fn build_scroll_app() -> Result<TestApp> {
        TestApp::new(|| {
            Application::new().window(
                WindowBuilder::new()
                    .title("Scroll Harness")
                    .root(ScrollHarness::new()),
            )
        })
    }

    #[test]
    fn locators_actions_and_focus_work_end_to_end() -> Result<()> {
        let app = build_app()?;
        let window = app.main_window()?;
        let save = window.get_by_role(SemanticsRole::Button).with_name("Save");

        save.expect().to_be_visible()?;
        save.hover()?;
        save.click()?;
        save.expect().to_be_focused()?;
        save.press("Enter")?;

        let focused = window.focused().with_name("Save");
        focused.expect().to_have_count(1)?;
        window.get_by_text("Activated").expect().to_be_visible()?;

        Ok(())
    }

    #[test]
    fn fill_and_virtual_time_waiting_work() -> Result<()> {
        let app = build_app()?;
        let window = app.main_window()?;

        window
            .get_by_role(SemanticsRole::TextInput)
            .with_name("Name")
            .fill("Ada")?;
        window
            .get_by_role(SemanticsRole::TextInput)
            .with_name("Name")
            .expect()
            .to_have_value("Ada")?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name("Save")
            .click()?;
        window.get_by_text("Saved").expect().to_be_visible()?;

        Ok(())
    }

    #[test]
    fn locator_scroll_updates_child_layout_and_screenshot() -> Result<()> {
        let app = build_scroll_app()?;
        let window = app.main_window()?;
        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name("Scroll Harness");

        let before = scroll.capture_screenshot()?;
        let before_snapshot = window.snapshot()?;
        let before_child = before_snapshot
            .widget_graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 160.0 && node.bounds.height() == 200.0)
            .expect("scroll content node present");

        scroll.scroll_pixels(Vector::new(0.0, -80.0))?;

        let after = scroll.capture_screenshot()?;
        let after_snapshot = window.snapshot()?;
        let after_child = after_snapshot
            .widget_graph
            .nodes
            .iter()
            .find(|node| node.bounds.width() == 160.0 && node.bounds.height() == 200.0)
            .expect("scroll content node present after scroll");

        assert_ne!(before, after);
        assert!(after_child.bounds.y() < before_child.bounds.y());

        Ok(())
    }

    #[test]
    fn failures_include_semantics_and_graph_diagnostics() {
        let app = build_app().unwrap();
        let window = app.main_window().unwrap();
        let error = window
            .get_by_text("missing")
            .expect()
            .with_timeout(0.0)
            .to_be_visible()
            .unwrap_err();

        assert!(error.message().contains("selector: text=\"missing\""));
        assert!(error.message().contains("Semantics snapshot:"));
        assert!(error.message().contains("Widget graph:"));
    }

    #[test]
    fn scoped_descendant_locators_target_repeated_widgets() -> Result<()> {
        let app = build_list_app()?;
        let window = app.main_window()?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name("Select")
            .expect()
            .to_have_count(2)?;

        let grace_row = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_description("Grace");

        grace_row
            .get_by_role(SemanticsRole::Button)
            .with_name("Select")
            .click()?;

        window
            .get_by_text("Selected: Grace")
            .expect()
            .to_be_visible()?;

        Ok(())
    }

    #[test]
    fn screenshot_capture_and_artifact_bundles_work() -> Result<()> {
        let app = build_app()?;
        let window = app.main_window()?;

        let screenshot = window.capture_screenshot()?;
        assert!(screenshot.width() > 0);
        assert!(screenshot.height() > 0);

        let artifacts = window.capture_artifacts()?;
        assert!(artifacts.screenshot.is_some());
        assert!(artifacts.semantics_overlay.is_some());
        assert!(artifacts.widget_overlay.is_some());
        assert!(artifacts.snapshot.scene_summary.is_some());

        let dir = unique_temp_path("artifacts");
        artifacts.write_to_dir(&dir)?;

        assert!(dir.join("summary.txt").exists());
        assert!(dir.join("semantics.txt").exists());
        assert!(dir.join("widget-graph.txt").exists());
        assert!(dir.join("scene.txt").exists());
        assert!(dir.join("screenshot.png").exists());
        assert!(dir.join("semantics-overlay.png").exists());
        assert!(dir.join("widget-overlay.png").exists());

        Ok(())
    }

    #[test]
    fn screenshot_expectations_compare_against_png_baselines() -> Result<()> {
        let app = build_app()?;
        let window = app.main_window()?;
        let save = window.get_by_role(SemanticsRole::Button).with_name("Save");
        let baseline = unique_temp_path("save-button.png");

        save.capture_screenshot()?.write_png(&baseline)?;
        save.expect().to_match_screenshot(&baseline)?;

        save.click()?;

        let error = save
            .expect()
            .with_timeout(0.0)
            .to_match_screenshot(&baseline)
            .unwrap_err();
        let actual = baseline.with_file_name("save-button.actual.png");
        let diff = baseline.with_file_name("save-button.diff.png");

        assert!(actual.exists());
        assert!(diff.exists());
        assert!(error.message().contains("screenshot assertion failed"));

        Ok(())
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after unix epoch")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("sui-testing-{}-{}", std::process::id(), nonce));
        fs::create_dir_all(&dir).expect("temporary screenshot directory created");
        dir.join(name)
    }
}
