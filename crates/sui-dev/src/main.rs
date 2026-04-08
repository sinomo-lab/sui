use std::{cell::RefCell, rc::Rc};

use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, WidgetPodMutVisitor,
    WidgetPodVisitor, WgpuRenderer, WindowEvent, prelude::*,
};
use sui_widget_book::{
    LivePerformanceRoot, build_button_grid_benchmark, build_widget_book_gallery,
    default_widget_book_state, register_widget_book_images,
};

const WINDOW_TITLE: &str = "SUI Dev";
const WINDOW_DESCRIPTION: &str =
    "Tabbed development host for the widget book and focused performance demos.";
const DEV_TABS_NAME: &str = "SUI Dev tabs";
const WIDGET_BOOK_TAB_LABEL: &str = "Widget book";
const BUTTON_GRID_TAB_LABEL: &str = "64 buttons";
const SETTINGS_TAB_LABEL: &str = "Settings";
const FEATHERING_TOGGLE_LABEL: &str = "Enable renderer feathering";
const FEATHER_WIDTH_NAME: &str = "Feather width";
const OPTICAL_TEXT_CENTERING_TOGGLE_LABEL: &str = "Enable optical vertical text centering";

struct RenderSettingsTab {
    content: SingleChild,
    state: Rc<RefCell<WindowRenderOptions>>,
    applied: Option<WindowRenderOptions>,
}

impl RenderSettingsTab {
    fn new() -> Self {
        let renderer = WgpuRenderer::new();
        let initial = WindowRenderOptions::new(
            renderer.feathering_enabled(),
            renderer.feather_width(),
        );
        let state = Rc::new(RefCell::new(initial));
        let toggle_state = Rc::clone(&state);
        let width_state = Rc::clone(&state);
        let text_centering_state = Rc::clone(&state);

        let content = Padding::all(
            28.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Renderer settings")
                        .font_size(24.0)
                        .line_height(30.0)
                        .color(Color::rgba(0.14, 0.18, 0.24, 1.0)),
                )
                .with_child(
                    Label::new(
                        "These controls update the active window's runtime presentation and text-alignment options on the next redraw. The optical centering toggle lets you compare the visual baseline shift against geometric centering without rebuilding.",
                    )
                    .font_size(14.0)
                    .line_height(20.0)
                    .color(Color::rgba(0.40, 0.47, 0.56, 1.0)),
                )
                .with_child(
                    Checkbox::new(FEATHERING_TOGGLE_LABEL)
                        .checked(initial.feathering_enabled)
                        .on_toggle(move |checked| {
                            toggle_state.borrow_mut().feathering_enabled = checked;
                        }),
                )
                .with_child(
                    Checkbox::new(OPTICAL_TEXT_CENTERING_TOGGLE_LABEL)
                        .checked(initial.optical_vertical_text_alignment_enabled)
                        .on_toggle(move |checked| {
                            text_centering_state
                                .borrow_mut()
                                .optical_vertical_text_alignment_enabled = checked;
                        }),
                )
                .with_child(
                    SizedBox::new().width(220.0).with_child(
                        NumberInput::new(FEATHER_WIDTH_NAME)
                            .range(0.0, 8.0)
                            .step(0.05)
                            .precision(2)
                            .value(initial.feather_width as f64)
                            .on_change(move |value| {
                                width_state.borrow_mut().feather_width = value.max(0.0) as f32;
                            }),
                    ),
                )
                .with_child(
                    Label::new(
                        "Optical centering uses cap height when available and a softened descent bias for Latin UI labels. A feather width around 1.0 matches the current renderer default; larger values exaggerate edge behavior for visual checks.",
                    )
                    .font_size(13.0)
                    .line_height(18.0)
                    .color(Color::rgba(0.45, 0.52, 0.60, 1.0)),
                ),
        );

        Self {
            content: SingleChild::new(content),
            state,
            applied: None,
        }
    }

    fn sync_render_options(&mut self, ctx: &mut EventCtx, rerender: bool) {
        let options = self.state.borrow().clamped();
        if self.applied == Some(options) {
            return;
        }

        set_window_render_options(ctx.window_id(), options);
        self.applied = Some(options);

        if rerender {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Window(ctx.window_id()),
                InvalidationKind::Paint,
            ));
        }
    }
}

impl Widget for RenderSettingsTab {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let rerender = !matches!(event, Event::Window(WindowEvent::RedrawRequested))
            && ctx.phase() != sui::EventPhase::Capture;

        if rerender || matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            self.sync_render_options(ctx, rerender);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.content.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

fn build_render_settings_tab() -> impl Widget {
    RenderSettingsTab::new()
}

fn build_dev_application() -> Application {
    let widget_book_state = default_widget_book_state();

    let mut app = Application::new();
    register_widget_book_images(&mut app);
    app.window(
        WindowBuilder::new().title(WINDOW_TITLE).root(LivePerformanceRoot::new(
            WINDOW_TITLE,
            WINDOW_DESCRIPTION,
            Tabs::new(DEV_TABS_NAME)
                .selected(0)
                .tab(
                    WIDGET_BOOK_TAB_LABEL,
                    build_widget_book_gallery(widget_book_state),
                )
                .tab(BUTTON_GRID_TAB_LABEL, build_button_grid_benchmark())
                .tab(SETTINGS_TAB_LABEL, build_render_settings_tab()),
        )),
    )
}

fn main() -> sui::Result<()> {
    build_dev_application().run()
}
