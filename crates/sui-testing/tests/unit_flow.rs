use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use sui_core::{
    Color, Event, ImageHandle, ImeEvent, KeyState, PointerEventKind, PointerKind, Result,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, TimerToken, WakeEvent,
    WindowEvent,
};
use sui_layout::Constraints;
use sui_render_wgpu::WgpuExternalTextureRegistry;
use sui_runtime::{
    Application, ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetChildren,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
};
use sui_scene::{ImageSource, RegisteredExternalImage};
use sui_testing::prelude::*;

#[test]
fn saves_form_in_a_unit_test() -> Result<()> {
    let app = TestApp::new(|| {
        let state = Rc::new(RefCell::new(AppState::default()));
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

#[test]
fn touch_tap_delivers_touch_pointer_events() -> Result<()> {
    let state = Arc::new(Mutex::new(String::new()));
    let runtime = {
        let state = Arc::clone(&state);
        Application::new()
            .window(
                WindowBuilder::new()
                    .title("Touch Test")
                    .root(TouchProbe::new(state)),
            )
            .build()?
    };
    let app = TestApp::from_runtime(runtime)?;

    app.main_window()?
        .get_by_role(SemanticsRole::Button)
        .with_name("Touch target")
        .touch_tap()?;

    assert_eq!(state.lock().expect("touch state").as_str(), "touch-down:1");
    Ok(())
}

#[test]
fn registry_aware_headless_app_renders_an_external_texture() -> Result<()> {
    let handle = ImageHandle::new(9_501);
    let ready = Arc::new(AtomicBool::new(false));
    let registry = WgpuExternalTextureRegistry::new();
    let runtime =
        Application::new()
            .window(WindowBuilder::new().title("External texture test").root(
                ExternalTextureProbe {
                    handle,
                    ready: Arc::clone(&ready),
                },
            ))
            .build()?;
    let app = TestApp::from_runtime_with_external_texture_registry(runtime, registry.clone())?;
    let window = app.main_window()?;

    registry.upload_rgba8(
        handle,
        2,
        2,
        Arc::<[u8]>::from(
            [0_u8, 255, 0, 255]
                .into_iter()
                .cycle()
                .take(2 * 2 * 4)
                .collect::<Vec<_>>(),
        ),
        1,
    )?;
    ready.store(true, Ordering::Release);
    window.dispatch_event_now(Event::Window(WindowEvent::Resized(Size::new(32.0, 32.0))))?;
    window.run_until_idle()?;

    window
        .get_by_role(SemanticsRole::Image)
        .with_name("External texture probe")
        .expect()
        .to_be_visible()?;
    let screenshot = window.capture_screenshot()?;
    assert!(
        screenshot
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[1] > 200 && pixel[0] < 20 && pixel[2] < 20),
        "the headless renderer must sample the uploaded green texture"
    );
    Ok(())
}

#[derive(Debug, Default)]
struct AppState {
    name: String,
    status: String,
    save_timer: Option<TimerToken>,
}

struct TouchProbe {
    state: Arc<Mutex<String>>,
}

struct ExternalTextureProbe {
    handle: ImageHandle,
    ready: Arc<AtomicBool>,
}

impl Widget for ExternalTextureProbe {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(32.0, 32.0))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: sui_core::Rect) {}

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.ready.load(Ordering::Acquire) {
            ctx.register_external_image(
                self.handle,
                RegisteredExternalImage::new(2, 2).expect("valid external test image extent"),
            );
            ctx.draw_image_source(ctx.bounds(), ImageSource::new(self.handle));
        } else {
            ctx.fill_bounds(Color::BLACK);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Image, ctx.bounds());
        node.name = Some("External texture probe".to_owned());
        ctx.push(node);
    }
}

impl TouchProbe {
    fn new(state: Arc<Mutex<String>>) -> Self {
        Self { state }
    }
}

impl Widget for TouchProbe {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Down
            && pointer.pointer_kind == PointerKind::Touch
        {
            *self.state.lock().expect("touch state") = format!("touch-down:{}", pointer.pointer_id);
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(160.0, 64.0))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: sui_core::Rect) {}

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.18, 0.25, 0.34, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("Touch target".to_string());
        node.actions = vec![SemanticsAction::Activate];
        ctx.push(node);
    }
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
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(420.0, 220.0));
        let control = Constraints::tight(Size::new(200.0, 44.0));

        self.children.measure_child(0, ctx, control);
        self.children.measure_child(1, ctx, control);

        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui_core::Rect) {
        self.children.arrange_child(
            0,
            ctx,
            sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 24.0, 200.0, 44.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            sui_core::Rect::new(bounds.x() + 24.0, bounds.y() + 92.0, 200.0, 44.0),
        );
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

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
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
