use std::{cell::RefCell, rc::Rc};

use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, TextCoveragePolicy, WgpuRenderer,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent, WindowTextRenderPolicy, prelude::*,
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
const GLYPH_PIXEL_ALIGNMENT_TOGGLE_LABEL: &str = "Snap atlas glyphs to physical pixels";
const TEXT_RENDER_POLICY_NAME: &str = "Text render policy";
const TEXT_RENDER_GAMMA_NAME: &str = "Gamma exponent";
const TEXT_RENDER_POLICY_OPTIONS: [&str; 4] =
    ["Automatic", "Linear", "Gamma", "TwoCoverageMinusCoverageSq"];

fn window_text_render_policy_from_renderer(policy: TextCoveragePolicy) -> WindowTextRenderPolicy {
    match policy.normalized() {
        TextCoveragePolicy::AutomaticByTextLuminance => {
            WindowTextRenderPolicy::AutomaticByTextLuminance
        }
        TextCoveragePolicy::Linear => WindowTextRenderPolicy::Linear,
        TextCoveragePolicy::Gamma(gamma) => WindowTextRenderPolicy::Gamma(gamma),
        TextCoveragePolicy::TwoCoverageMinusCoverageSq => {
            WindowTextRenderPolicy::TwoCoverageMinusCoverageSq
        }
    }
}

fn text_render_policy_selected_index(policy: WindowTextRenderPolicy) -> usize {
    match policy.normalized() {
        WindowTextRenderPolicy::AutomaticByTextLuminance => 0,
        WindowTextRenderPolicy::Linear => 1,
        WindowTextRenderPolicy::Gamma(_) => 2,
        WindowTextRenderPolicy::TwoCoverageMinusCoverageSq => 3,
    }
}

fn update_text_render_policy_selection(state: &mut WindowRenderOptions, index: usize) {
    state.text_render_policy = match index {
        0 => WindowTextRenderPolicy::AutomaticByTextLuminance,
        1 => WindowTextRenderPolicy::Linear,
        2 => match state.text_render_policy.normalized() {
            WindowTextRenderPolicy::Gamma(gamma) => WindowTextRenderPolicy::Gamma(gamma),
            _ => WindowTextRenderPolicy::Gamma(1.4),
        },
        3 => WindowTextRenderPolicy::TwoCoverageMinusCoverageSq,
        _ => state.text_render_policy,
    };
}

struct RenderSettingsTab {
    content: SingleChild,
    state: Rc<RefCell<WindowRenderOptions>>,
    applied: Option<WindowRenderOptions>,
}

impl RenderSettingsTab {
    fn new() -> Self {
        let renderer = WgpuRenderer::new();
        let initial =
            WindowRenderOptions::new(renderer.feathering_enabled(), renderer.feather_width())
                .with_glyph_pixel_alignment_enabled(renderer.glyph_pixel_alignment_enabled())
                .with_text_render_policy(window_text_render_policy_from_renderer(
                    renderer.text_coverage_policy(),
                ));
        let state = Rc::new(RefCell::new(initial));
        let toggle_state = Rc::clone(&state);
        let width_state = Rc::clone(&state);
        let text_centering_state = Rc::clone(&state);
        let glyph_alignment_state = Rc::clone(&state);
        let text_policy_state = Rc::clone(&state);
        let gamma_state = Rc::clone(&state);

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
                        "These controls update the active window's runtime presentation, atlas glyph alignment, and grayscale text coverage policy on the next redraw.",
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
                    Checkbox::new(GLYPH_PIXEL_ALIGNMENT_TOGGLE_LABEL)
                        .checked(initial.glyph_pixel_alignment_enabled)
                        .on_toggle(move |checked| {
                            glyph_alignment_state.borrow_mut().glyph_pixel_alignment_enabled =
                                checked;
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
                    SizedBox::new().width(280.0).with_child(
                        Select::new(TEXT_RENDER_POLICY_NAME)
                            .options(TEXT_RENDER_POLICY_OPTIONS)
                            .selected(text_render_policy_selected_index(initial.text_render_policy))
                            .on_change(move |index, _| {
                                let mut state = text_policy_state.borrow_mut();
                                update_text_render_policy_selection(&mut state, index);
                            }),
                    ),
                )
                .with_child(
                    SizedBox::new().width(220.0).with_child(
                        NumberInput::new(TEXT_RENDER_GAMMA_NAME)
                            .range(0.1, 4.0)
                            .step(0.05)
                            .precision(2)
                            .value(match initial.text_render_policy.normalized() {
                                WindowTextRenderPolicy::Gamma(gamma) => gamma as f64,
                                _ => 1.4,
                            })
                            .on_change(move |value| {
                                let gamma = value.clamp(0.1, 4.0) as f32;
                                gamma_state.borrow_mut().text_render_policy =
                                    WindowTextRenderPolicy::Gamma(gamma);
                            }),
                    ),
                )
                .with_child(
                    Label::new(
                        "Optical centering uses cap height when available and a softened descent bias for Latin UI labels. Glyph pixel alignment only affects the atlas path for axis-aligned text. The render policy applies to both atlas and fallback glyph coverage; the gamma input is only used when the Gamma policy is selected.",
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
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(LivePerformanceRoot::new(
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
