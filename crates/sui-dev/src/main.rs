use std::{cell::RefCell, rc::Rc};

use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, PointerButton, PointerEventKind,
    SemanticsNode, SemanticsRole, TextCoveragePolicy, WgpuRenderer, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowEvent, WindowTextRenderPolicy, prelude::*,
};
use sui_widget_book::{
    LivePerformanceRoot, build_button_grid_benchmark, build_widget_book_gallery,
    default_widget_book_state, register_widget_book_images,
};

const WINDOW_TITLE: &str = "SUI Dev";
const WINDOW_DESCRIPTION: &str =
    "Floating development workspace for the widget book and focused performance demos.";
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

const SIDEBAR_TITLE: &str = "Available views";

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarActionKind {
    Visibility,
    Maximize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SidebarActionTarget {
    view_id: u64,
    kind: SidebarActionKind,
}

struct SidebarRowLayout {
    view_id: u64,
    row_bounds: Rect,
    visibility_bounds: Rect,
    maximize_bounds: Rect,
}

struct ViewSidebar {
    theme: DefaultTheme,
    workspace: FloatingWorkspaceState,
    rows: Vec<SidebarRowLayout>,
    hovered: Option<SidebarActionTarget>,
    pressed: Option<SidebarActionTarget>,
    pointer_id: Option<u64>,
}

impl ViewSidebar {
    fn new(workspace: FloatingWorkspaceState) -> Self {
        Self {
            theme: DefaultTheme::default(),
            workspace,
            rows: Vec::new(),
            hovered: None,
            pressed: None,
            pointer_id: None,
        }
    }

    fn action_at(&self, position: Point) -> Option<SidebarActionTarget> {
        self.rows.iter().find_map(|row| {
            if row.visibility_bounds.contains(position) {
                return Some(SidebarActionTarget {
                    view_id: row.view_id,
                    kind: SidebarActionKind::Visibility,
                });
            }
            if row.maximize_bounds.contains(position) {
                return Some(SidebarActionTarget {
                    view_id: row.view_id,
                    kind: SidebarActionKind::Maximize,
                });
            }
            None
        })
    }

    fn button_label(&self, target: SidebarActionTarget) -> String {
        let Some(view) = self.workspace.snapshot(target.view_id) else {
            return String::new();
        };
        match target.kind {
            SidebarActionKind::Visibility => {
                if view.visible { "Hide" } else { "Show" }.to_string()
            }
            SidebarActionKind::Maximize => {
                if view.maximized { "Restore" } else { "Maximize" }.to_string()
            }
        }
    }

    fn apply_action(&mut self, target: SidebarActionTarget) -> bool {
        match target.kind {
            SidebarActionKind::Visibility => self.workspace.toggle_view_visible(target.view_id).is_some(),
            SidebarActionKind::Maximize => {
                let Some(view) = self.workspace.snapshot(target.view_id) else {
                    return false;
                };
                self.workspace.set_view_maximized(target.view_id, !view.maximized)
            }
        }
    }

    fn request_workspace_refresh(&self, ctx: &mut EventCtx, include_ordering: bool) {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::Measure,
        ));
        if include_ordering {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Window(ctx.window_id()),
                InvalidationKind::Ordering,
            ));
        }
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::Paint,
        ));
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::HitTest,
        ));
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::Semantics,
        ));
    }
}

impl Widget for ViewSidebar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.action_at(pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(target) = self.action_at(pointer.position) {
                    self.hovered = Some(target);
                    self.pressed = Some(target);
                    self.pointer_id = Some(pointer.pointer_id);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pointer_id == Some(pointer.pointer_id) =>
            {
                let hovered = self.action_at(pointer.position);
                let triggered = self.pressed.filter(|pressed| Some(*pressed) == hovered);
                self.pointer_id = None;
                self.pressed = None;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                if let Some(target) = triggered {
                    if self.apply_action(target) {
                        self.request_workspace_refresh(ctx, true);
                    }
                    ctx.set_handled();
                } else {
                    ctx.request_paint();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.pointer_id == Some(pointer.pointer_id) =>
            {
                self.pointer_id = None;
                self.pressed = None;
                self.hovered = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.pointer_id.is_none() && self.hovered.take().is_some() {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let row_height = 54.0;
        let height = 96.0 + (self.workspace.snapshots().len() as f32 * row_height);
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                272.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                height
            },
        ))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        self.rows.clear();
        let mut y = bounds.y() + 72.0;
        for view in self.workspace.snapshots() {
            let row_bounds = Rect::new(bounds.x() + 16.0, y, (bounds.width() - 32.0).max(0.0), 44.0);
            let maximize_width = 74.0;
            let visibility_width = 60.0;
            let gap = 8.0;
            let maximize_bounds = Rect::new(
                row_bounds.max_x() - maximize_width,
                row_bounds.y() + 7.0,
                maximize_width,
                30.0,
            );
            let visibility_bounds = Rect::new(
                maximize_bounds.x() - gap - visibility_width,
                row_bounds.y() + 7.0,
                visibility_width,
                30.0,
            );
            self.rows.push(SidebarRowLayout {
                view_id: view.id,
                row_bounds,
                visibility_bounds,
                maximize_bounds,
            });
            y += 54.0;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let border = StrokeStyle::new(metrics.border_width.max(1.0));

        ctx.fill_bounds(Color::rgba(0.965, 0.972, 0.985, 1.0));
        ctx.stroke_rect(
            Rect::new(ctx.bounds().max_x() - 1.0, ctx.bounds().y(), 1.0, ctx.bounds().height()),
            palette.border,
            border.clone(),
        );
        ctx.draw_text(
            Rect::new(ctx.bounds().x() + 16.0, ctx.bounds().y() + 16.0, ctx.bounds().width() - 32.0, 22.0),
            SIDEBAR_TITLE,
            TextStyle {
                font_size: 18.0,
                line_height: 22.0,
                color: Color::rgba(0.13, 0.17, 0.22, 1.0),
                ..TextStyle::default()
            },
        );
        ctx.draw_text(
            Rect::new(ctx.bounds().x() + 16.0, ctx.bounds().y() + 40.0, ctx.bounds().width() - 32.0, 20.0),
            "Show, hide, or maximize each floating tool view.",
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: Color::rgba(0.42, 0.48, 0.56, 1.0),
                ..TextStyle::default()
            },
        );

        for row in &self.rows {
            let Some(view) = self.workspace.snapshot(row.view_id) else {
                continue;
            };
            let hovered_row = self.hovered.is_some_and(|target| target.view_id == row.view_id);
            let row_fill = if hovered_row {
                Color::rgba(0.90, 0.93, 0.98, 1.0)
            } else {
                palette.surface.with_alpha(0.72)
            };
            ctx.fill_rect(row.row_bounds, row_fill);
            ctx.stroke_rect(row.row_bounds, palette.border.with_alpha(0.7), border.clone());
            ctx.draw_text(
                Rect::new(row.row_bounds.x() + 12.0, row.row_bounds.y() + 11.0, (row.row_bounds.width() - 166.0).max(0.0), 20.0),
                view.title,
                TextStyle {
                    font_size: 13.0,
                    line_height: 18.0,
                    color: if view.visible {
                        Color::rgba(0.12, 0.16, 0.22, 1.0)
                    } else {
                        Color::rgba(0.49, 0.54, 0.62, 1.0)
                    },
                    ..TextStyle::default()
                },
            );

            for target in [
                SidebarActionTarget {
                    view_id: row.view_id,
                    kind: SidebarActionKind::Visibility,
                },
                SidebarActionTarget {
                    view_id: row.view_id,
                    kind: SidebarActionKind::Maximize,
                },
            ] {
                let bounds = match target.kind {
                    SidebarActionKind::Visibility => row.visibility_bounds,
                    SidebarActionKind::Maximize => row.maximize_bounds,
                };
                let hovered = self.hovered == Some(target);
                let pressed = self.pressed == Some(target);
                let is_primary = matches!(target.kind, SidebarActionKind::Maximize) && view.maximized;
                let fill = if pressed {
                    Color::rgba(0.80, 0.85, 0.93, 1.0)
                } else if hovered {
                    Color::rgba(0.86, 0.90, 0.96, 1.0)
                } else if is_primary {
                    palette.accent.with_alpha(0.18)
                } else {
                    Color::rgba(0.94, 0.95, 0.98, 1.0)
                };
                let stroke_color = if is_primary {
                    palette.accent
                } else {
                    palette.border
                };
                ctx.fill_rect(bounds, fill);
                ctx.stroke_rect(bounds, stroke_color, border.clone());
                ctx.draw_text(
                    Rect::new(bounds.x() + 10.0, bounds.y() + 7.0, bounds.width() - 20.0, bounds.height() - 14.0),
                    self.button_label(target),
                    TextStyle {
                        font_size: 11.0,
                        line_height: 14.0,
                        color: if is_primary {
                            palette.accent
                        } else {
                            Color::rgba(0.22, 0.27, 0.34, 1.0)
                        },
                        ..TextStyle::default()
                    },
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(SIDEBAR_TITLE.to_string());
        node.description = Some("Floating workspace view controls".to_string());
        ctx.push(node);
    }
}

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

fn build_dev_workspace() -> impl Widget {
    let widget_book_state = default_widget_book_state();
    let workspace = FloatingWorkspaceState::new();

    let views = FloatingWorkspace::new(workspace.clone())
        .name("Development workspace")
        .with_view(
            FloatingViewConfig::new(WIDGET_BOOK_TAB_LABEL, Rect::new(24.0, 24.0, 680.0, 760.0))
                .min_size(Size::new(420.0, 320.0)),
            build_widget_book_gallery(widget_book_state),
        )
        .with_view(
            FloatingViewConfig::new(BUTTON_GRID_TAB_LABEL, Rect::new(560.0, 72.0, 420.0, 340.0))
                .min_size(Size::new(280.0, 220.0)),
            build_button_grid_benchmark(),
        )
        .with_view(
            FloatingViewConfig::new(SETTINGS_TAB_LABEL, Rect::new(420.0, 440.0, 420.0, 320.0))
                .min_size(Size::new(300.0, 240.0)),
            build_render_settings_tab(),
        );

    SplitView::horizontal(ViewSidebar::new(workspace), views)
        .name("Development workspace split")
        .ratio(0.24)
        .min_first(236.0)
        .min_second(420.0)
        .divider_thickness(12.0)
}

fn build_dev_application() -> Application {
    let mut app = Application::new();
    register_widget_book_images(&mut app);
    app.window(
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(LivePerformanceRoot::new(
                WINDOW_TITLE,
                WINDOW_DESCRIPTION,
                build_dev_workspace(),
            )),
    )
}

fn main() -> sui::Result<()> {
    build_dev_application().run()
}
