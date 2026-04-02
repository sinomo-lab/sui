#![forbid(unsafe_code)]

mod app;
mod diagnostics;
mod expect;
mod harness;
mod locator;
mod selector;
mod snapshot;
mod window;

pub use app::{IntoTestRuntime, TestApp};
pub use expect::Expectation;
pub use locator::Locator;
pub use selector::Selector;
pub use snapshot::WindowSnapshot;
pub use window::TestWindow;

pub mod prelude {
    pub use crate::{Expectation, IntoTestRuntime, Locator, Selector, TestApp, TestWindow};
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::TestApp;
    use sui_core::{
        Color, Event, ImeEvent, KeyState, PointerEventKind, Result, SemanticsAction,
        SemanticsNode, SemanticsRole, SemanticsValue, Size, TimerToken, WakeEvent,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, Widget, WidgetChildren,
        WidgetPodVisitor, WindowBuilder,
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
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let viewport = constraints.clamp(Size::new(480.0, 240.0));
            let child_constraints = Constraints::tight(Size::new(180.0, 44.0));
            self.children.as_mut_slice()[0].layout_at(
                ctx,
                child_constraints,
                sui_core::Point::new(24.0, 24.0),
            );
            self.children.as_mut_slice()[1].layout_at(
                ctx,
                child_constraints,
                sui_core::Point::new(24.0, 92.0),
            );
            viewport
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
            node.name = Some("Name".to_string());
            node.value = Some(SemanticsValue::Text(self.state.borrow().input_value.clone()));
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
        let state = Rc::new(RefCell::new(AppState::default()));
        TestApp::new(|| {
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
                sui_core::Point::new(320.0, 78.0),
                Rc::clone(&state),
            ));
            children.push(ContactRow::new(
                "Grace",
                sui_core::Point::new(320.0, 134.0),
                Rc::clone(&state),
            ));
            Self { state, children }
        }
    }

    impl Widget for ListRoot {
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let viewport = constraints.clamp(Size::new(480.0, 260.0));
            let width = viewport.width - 48.0;

            self.children.as_mut_slice()[0].layout_at(
                ctx,
                Constraints::tight(Size::new(width, 32.0)),
                sui_core::Point::new(24.0, 20.0),
            );
            self.children.as_mut_slice()[1].layout_at(
                ctx,
                Constraints::tight(Size::new(width, 44.0)),
                sui_core::Point::new(24.0, 72.0),
            );
            self.children.as_mut_slice()[2].layout_at(
                ctx,
                Constraints::tight(Size::new(width, 44.0)),
                sui_core::Point::new(24.0, 128.0),
            );

            viewport
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
        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(360.0, 44.0));
            self.children.as_mut_slice()[0].layout_at(
                ctx,
                Constraints::tight(Size::new(120.0, 32.0)),
                self.button_origin,
            );
            size
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 32.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let color = if ctx.is_focused() {
                Color::rgba(0.25, 0.52, 0.88, 1.0)
            } else {
                Color::rgba(0.17, 0.24, 0.37, 1.0)
            };
            ctx.fill_bounds(color);
            ctx.label(
                ctx.bounds(),
                "Select",
                Color::rgba(0.95, 0.96, 0.98, 1.0),
            );
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
        let state = Rc::new(RefCell::new(ListState::default()));
        TestApp::new(|| {
            Application::new().window(
                WindowBuilder::new()
                    .title("List Harness")
                    .root(ListRoot::new(state)),
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
}
