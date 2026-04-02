use std::{cell::RefCell, rc::Rc};

use sui_core::{
    Color, Event, ImeEvent, KeyState, PointerEventKind, Result, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsValue, Size, TimerToken, WakeEvent,
};
use sui_layout::Constraints;
use sui_runtime::{
    Application, EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, Widget, WidgetChildren,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
};
use sui_testing::prelude::*;

#[test]
fn saves_form_in_a_unit_test() -> Result<()> {
    let state = Rc::new(RefCell::new(AppState::default()));
    let app = TestApp::new(|| {
        Application::new().window(
            WindowBuilder::new()
                .title("Unit Test Example")
                .root(FormRoot::new(state)),
        )
    })?;

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

#[derive(Debug, Default)]
struct AppState {
    name: String,
    status: String,
    save_timer: Option<TimerToken>,
}

struct FormRoot {
    children: WidgetChildren,
}

impl FormRoot {
    fn new(state: Rc<RefCell<AppState>>) -> Self {
        let mut children = WidgetChildren::new();
        children.push(NameInput::new(Rc::clone(&state)));
        children.push(SaveButton::new(state));
        Self { children }
    }
}

impl Widget for FormRoot {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(420.0, 220.0));
        let control = Constraints::tight(Size::new(200.0, 44.0));

        self.children.as_mut_slice()[0].layout_at(ctx, control, sui_core::Point::new(24.0, 24.0));
        self.children.as_mut_slice()[1].layout_at(ctx, control, sui_core::Point::new(24.0, 92.0));

        viewport
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some("Unit Test Example".to_string());
        ctx.push(root);
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

struct NameInput {
    state: Rc<RefCell<AppState>>,
}

impl NameInput {
    fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Widget for NameInput {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                self.state.borrow_mut().name = text.clone();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(200.0, 44.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let background = if ctx.is_focused() {
            Color::rgba(0.18, 0.23, 0.32, 1.0)
        } else {
            Color::rgba(0.13, 0.15, 0.20, 1.0)
        };
        ctx.fill_bounds(background);
        ctx.label(
            ctx.bounds(),
            format!("Name: {}", self.state.borrow().name),
            Color::rgba(0.95, 0.96, 0.98, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        node.name = Some("Name".to_string());
        node.value = Some(SemanticsValue::Text(self.state.borrow().name.clone()));
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

struct SaveButton {
    state: Rc<RefCell<AppState>>,
}

impl SaveButton {
    fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Widget for SaveButton {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                start_save(&self.state, ctx);
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                start_save(&self.state, ctx);
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
        constraints.clamp(Size::new(200.0, 44.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        let background = if ctx.is_focused() {
            Color::rgba(0.25, 0.52, 0.88, 1.0)
        } else {
            Color::rgba(0.17, 0.24, 0.37, 1.0)
        };
        ctx.fill_bounds(background);
        ctx.label(
            ctx.bounds(),
            format!("Save {}", state.status),
            Color::rgba(0.95, 0.96, 0.98, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();

        let mut button = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        button.name = Some("Save".to_string());
        button.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(button);

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

fn start_save(state: &Rc<RefCell<AppState>>, ctx: &mut EventCtx) {
    let mut state = state.borrow_mut();
    state.status = "Saving".to_string();
    state.save_timer = Some(ctx.schedule_timer_after(1.0));
    ctx.request_focus();
    ctx.request_paint();
    ctx.request_semantics();
    ctx.set_handled();
}
