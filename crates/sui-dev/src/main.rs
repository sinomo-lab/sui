use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Event, KeyState, PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole};

fn main() -> Result<()> {
    run_desktop_app()
}

fn run_desktop_app() -> Result<()> {
    let state = Rc::new(RefCell::new(AppState::default()));

    Application::new()
        .window(
            WindowBuilder::new()
                .title("SUI Playground")
                .root(AppRoot::new(state)),
        )
        .run()
}

#[derive(Debug, Clone, Default)]
struct AppState {
    pointer_downs: usize,
    key_activations: usize,
    focused: bool,
}

struct AppRoot {
    state: Rc<RefCell<AppState>>,
}

impl AppRoot {
    fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Widget for AppRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                self.state.borrow_mut().pointer_downs += 1;
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
                self.state.borrow_mut().key_activations += 1;
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(1280.0, 720.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        let panel = Rect::new(64.0, 64.0, 420.0, 180.0);
        let accent = if state.focused {
            Color::rgba(0.36, 0.63, 0.96, 1.0)
        } else {
            Color::rgba(0.24, 0.30, 0.38, 1.0)
        };

        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        ctx.fill_rect(panel, Color::rgba(0.16, 0.19, 0.25, 1.0));
        ctx.fill_rect(Rect::new(64.0, 64.0, 420.0, 8.0), accent);
        ctx.label(
            Rect::new(88.0, 98.0, 360.0, 32.0),
            "Phase 1 vertical slice",
            Color::rgba(0.95, 0.96, 0.98, 1.0),
        );
        ctx.label(
            Rect::new(88.0, 144.0, 360.0, 24.0),
            format!(
                "pointer={} key={} focused={}",
                state.pointer_downs, state.key_activations, state.focused
            ),
            Color::rgba(0.77, 0.82, 0.89, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("Phase 1 vertical slice".to_string());
        node.description = Some(format!(
            "Pointer downs: {}, keyboard activations: {}",
            state.pointer_downs, state.key_activations
        ));
        node.state.focused = state.focused;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.state.borrow_mut().focused = focused;
        ctx.request_paint();
        ctx.request_semantics();
    }
}
