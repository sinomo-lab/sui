use sui::prelude::*;
use sui::{Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState};

fn main() -> Result<()> {
    Application::new()
        .window(WindowBuilder::new().title("SUI Playground").root(AppRoot))
        .run()
}

struct AppRoot;

impl Widget for AppRoot {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(1280.0, 720.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        ctx.fill_rect(Rect::new(48.0, 48.0, 320.0, 120.0), Color::rgba(0.16, 0.19, 0.25, 1.0));
        ctx.label(
            Rect::new(64.0, 76.0, 280.0, 36.0),
            "SUI facade scaffold",
            Color::rgba(0.95, 0.96, 0.98, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.root_widget_id(),
            SemanticsRole::Window,
            ctx.bounds(),
        );
        node.name = Some("SUI Playground".to_string());
        node.description = Some("Phase-1 scaffold application".to_string());
        node.state = SemanticsState::default();
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
    }
}
