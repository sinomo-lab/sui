use std::{cell::RefCell, rc::Rc};

use sui::{
    HdrThemeMode, InvalidationKind, InvalidationRequest, InvalidationTarget, PointerButton,
    PointerEventKind, SemanticsNode, SemanticsRole, TextCoveragePolicy, TextHinting, WgpuRenderer,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowColorManagementMode, WindowDynamicRangeMode,
    WindowEvent, WindowId, WindowOutputColorPrimaries, WindowRenderOptions, WindowStemDarkening,
    WindowTextHinting, WindowTextRenderPolicy, WindowToneMappingMode, prelude::*,
    window_output_diagnostics,
};
use sui_widget_book::{
    LivePerformanceRoot, build_button_grid_benchmark, build_color_validation_surface,
    build_retained_text_benchmark, build_text_editing_benchmark,
    build_text_rendering_comparison_surface, build_text_validation_surface,
    build_widget_book_gallery, default_widget_book_state, register_widget_book_images,
    set_widget_book_hdr_theme_mode, widget_book_hdr_theme_mode,
};

const WINDOW_TITLE: &str = "SUI Dev";
const WINDOW_DESCRIPTION: &str =
    "Floating development workspace for the widget book and focused performance demos.";
const WIDGET_BOOK_TAB_LABEL: &str = "Widget book";
const BUTTON_GRID_TAB_LABEL: &str = "64 buttons";
const RETAINED_TEXT_TAB_LABEL: &str = "Retained text";
const TEXT_RENDERING_COMPARISON_TAB_LABEL: &str = "Text comparison";
const TEXT_VALIDATION_TAB_LABEL: &str = "Text validation";
const TEXT_EDITING_TAB_LABEL: &str = "Text editing";
const HDR_VALIDATION_TAB_LABEL: &str = "HDR validation";
const SETTINGS_TAB_LABEL: &str = "Settings";
const FEATHERING_TOGGLE_LABEL: &str = "Enable renderer feathering";
const FEATHER_WIDTH_NAME: &str = "Feather width";
const OPTICAL_TEXT_CENTERING_TOGGLE_LABEL: &str = "Enable optical vertical text centering";
const GLYPH_PIXEL_ALIGNMENT_TOGGLE_LABEL: &str = "Snap atlas glyphs to physical pixels";
const TEXT_RENDER_POLICY_NAME: &str = "Text render policy";
const TEXT_RENDER_GAMMA_NAME: &str = "Gamma exponent";
const TEXT_HINTING_TOGGLE_LABEL: &str = "Enable slight small-text hinting";
const TEXT_HINTING_MAX_PPEM_NAME: &str = "Hinting max ppem";
const STEM_DARKENING_TOGGLE_LABEL: &str = "Enable small-text stem darkening";
const STEM_DARKENING_AMOUNT_NAME: &str = "Stem darkening amount";
const STEM_DARKENING_MAX_PPEM_NAME: &str = "Stem darkening max ppem";
const COLOR_MANAGEMENT_MODE_NAME: &str = "Color management";
const OUTPUT_PRIMARIES_NAME: &str = "Output primaries";
const DYNAMIC_RANGE_MODE_NAME: &str = "Dynamic range";
const TONE_MAPPING_MODE_NAME: &str = "Tone mapping";
const SDR_CONTENT_BRIGHTNESS_NAME: &str = "SDR content brightness";
const HDR_THEME_MODE_NAME: &str = "HDR theme mode";
const OUTPUT_DIAGNOSTICS_TITLE: &str = "Output diagnostics";
const HDR_THEME_INSPECTION_TITLE: &str = "HDR theme mode inspection";
const SETTINGS_SCROLL_NAME: &str = "Settings controls";
const TEXT_RENDER_POLICY_OPTIONS: [&str; 4] =
    ["Automatic", "Linear", "Gamma", "TwoCoverageMinusCoverageSq"];
const COLOR_MANAGEMENT_MODE_OPTIONS: [&str; 4] =
    ["Automatic", "Force SDR", "Prefer wide gamut", "Prefer HDR"];
const OUTPUT_PRIMARIES_OPTIONS: [&str; 3] = ["Automatic", "sRGB", "Display P3"];
const DYNAMIC_RANGE_MODE_OPTIONS: [&str; 3] = ["Automatic", "SDR", "HDR"];
const TONE_MAPPING_MODE_OPTIONS: [&str; 3] = ["Automatic", "Clamp", "Reinhard"];
const HDR_THEME_MODE_OPTIONS: [&str; 4] = [
    "Disabled (SDR baseline)",
    "Wide-gamut only",
    "Constrained HDR",
    "Full HDR",
];

const SIDEBAR_TITLE: &str = "Available views";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopAutomationMode {
    ButtonGridResize,
}

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

fn request_window_refresh(ctx: &mut EventCtx, include_ordering: bool) {
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

struct DesktopAutomationRoot {
    workspace: FloatingWorkspaceState,
    target_view_id: u64,
    initial_bounds: Rect,
    content: SingleChild,
    mode: Option<DesktopAutomationMode>,
    timer: Option<TimerToken>,
    started_at: Option<f64>,
    last_report_at: Option<f64>,
    last_report_frame_index: u64,
}

impl DesktopAutomationRoot {
    const STEP_INTERVAL_S: f64 = 1.0 / 120.0;
    const BENCH_DURATION_S: f64 = 3.0;
    const REPORT_INTERVAL_S: f64 = 0.5;

    fn new<T: Widget + 'static>(
        workspace: FloatingWorkspaceState,
        target_view_id: u64,
        initial_bounds: Rect,
        mode: Option<DesktopAutomationMode>,
        content: T,
    ) -> Self {
        Self {
            workspace,
            target_view_id,
            initial_bounds,
            content: SingleChild::new(content),
            mode,
            timer: None,
            started_at: None,
            last_report_at: None,
            last_report_frame_index: 0,
        }
    }

    fn ensure_started(&mut self, ctx: &mut EventCtx) {
        if self.timer.is_some() {
            return;
        }

        let now = ctx.current_time();
        self.started_at = Some(now);
        self.last_report_at = Some(now);
        self.last_report_frame_index = sui::window_performance_snapshot(ctx.window_id())
            .map(|snapshot| snapshot.frame_index)
            .unwrap_or(0);
        self.timer = Some(ctx.schedule_timer_after(Self::STEP_INTERVAL_S));
        println!(
            "[sui-dev automation] started {:?} on view {}",
            self.mode, self.target_view_id
        );
    }

    fn target_bounds_for_elapsed(&self, elapsed: f64) -> Rect {
        match self
            .mode
            .expect("automation mode should be active when ticking")
        {
            DesktopAutomationMode::ButtonGridResize => {
                let phase = ((elapsed / Self::BENCH_DURATION_S) * 2.0).fract() as f32;
                let triangle = if phase <= 0.5 {
                    phase * 2.0
                } else {
                    (1.0 - phase) * 2.0
                };
                Rect::new(
                    self.initial_bounds.x(),
                    self.initial_bounds.y(),
                    self.initial_bounds.width() + 420.0 * triangle,
                    self.initial_bounds.height() + 280.0 * triangle,
                )
            }
        }
    }

    fn report_progress(&mut self, ctx: &EventCtx, force: bool) {
        let now = ctx.current_time();
        let Some(last_report_at) = self.last_report_at else {
            self.last_report_at = Some(now);
            return;
        };
        if !force && now - last_report_at < Self::REPORT_INTERVAL_S {
            return;
        }

        let Some(snapshot) = sui::window_performance_snapshot(ctx.window_id()) else {
            self.last_report_at = Some(now);
            return;
        };

        let frame_delta = snapshot
            .frame_index
            .saturating_sub(self.last_report_frame_index);
        let elapsed = (now - last_report_at).max(f64::EPSILON);
        let fps = frame_delta as f64 / elapsed;
        println!(
            "[sui-dev automation] t={now:.3}s fps={fps:.1} frame={} total={:.3}ms acq={:.3}ms pres={:.3}ms build={:.3}ms state={:.3}ms",
            snapshot.frame_index,
            snapshot.total_time_ms,
            snapshot.renderer_submission.surface_acquire_time_us as f64 / 1000.0,
            snapshot.renderer_submission.surface_present_time_us as f64 / 1000.0,
            snapshot.renderer_submission.retained_packet_build_time_us as f64 / 1000.0,
            snapshot.renderer_submission.retained_state_update_time_us as f64 / 1000.0,
        );
        self.last_report_at = Some(now);
        self.last_report_frame_index = snapshot.frame_index;
    }

    fn tick(&mut self, ctx: &mut EventCtx) {
        if self.mode.is_none() {
            return;
        }
        self.ensure_started(ctx);
        let started_at = self.started_at.unwrap_or_else(|| ctx.current_time());
        let elapsed = (ctx.current_time() - started_at).max(0.0);
        let next_bounds = self.target_bounds_for_elapsed(elapsed);
        if self
            .workspace
            .set_view_bounds(self.target_view_id, next_bounds)
        {
            request_window_refresh(ctx, true);
        }
        self.report_progress(ctx, false);

        if elapsed < Self::BENCH_DURATION_S {
            self.timer = Some(ctx.schedule_timer_after(Self::STEP_INTERVAL_S));
        } else {
            self.timer = None;
            self.report_progress(ctx, true);
            println!("[sui-dev automation] completed {:?}", self.mode);
        }
    }
}

impl Widget for DesktopAutomationRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Window(
                WindowEvent::RedrawRequested
                | WindowEvent::Resized(_)
                | WindowEvent::Focused(_)
                | WindowEvent::ScaleFactorChanged { .. },
            ) if self.mode.is_some() && self.timer.is_none() => {
                self.ensure_started(ctx);
            }
            Event::Wake(WakeEvent::Timer { token, .. })
                if self.mode.is_some() && self.timer == Some(*token) =>
            {
                self.tick(ctx);
                ctx.set_handled();
            }
            _ => {}
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
            SidebarActionKind::Visibility => if view.visible { "Hide" } else { "Show" }.to_string(),
            SidebarActionKind::Maximize => if view.maximized {
                "Restore"
            } else {
                "Maximize"
            }
            .to_string(),
        }
    }

    fn apply_action(&mut self, target: SidebarActionTarget) -> bool {
        match target.kind {
            SidebarActionKind::Visibility => {
                self.workspace.toggle_view_visible(target.view_id).is_some()
            }
            SidebarActionKind::Maximize => {
                let Some(view) = self.workspace.snapshot(target.view_id) else {
                    return false;
                };
                self.workspace
                    .set_view_maximized(target.view_id, !view.maximized)
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
            let row_bounds =
                Rect::new(bounds.x() + 16.0, y, (bounds.width() - 32.0).max(0.0), 44.0);
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
            Rect::new(
                ctx.bounds().max_x() - 1.0,
                ctx.bounds().y(),
                1.0,
                ctx.bounds().height(),
            ),
            palette.border,
            border.clone(),
        );
        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 16.0,
                ctx.bounds().y() + 16.0,
                ctx.bounds().width() - 32.0,
                22.0,
            ),
            SIDEBAR_TITLE,
            TextStyle {
                font_size: 18.0,
                line_height: 22.0,
                color: Color::rgba(0.13, 0.17, 0.22, 1.0),
                ..TextStyle::default()
            },
        );
        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 16.0,
                ctx.bounds().y() + 40.0,
                ctx.bounds().width() - 32.0,
                20.0,
            ),
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
            let hovered_row = self
                .hovered
                .is_some_and(|target| target.view_id == row.view_id);
            let row_fill = if hovered_row {
                Color::rgba(0.90, 0.93, 0.98, 1.0)
            } else {
                palette.surface.with_alpha(0.72)
            };
            ctx.fill_rect(row.row_bounds, row_fill);
            ctx.stroke_rect(
                row.row_bounds,
                palette.border.with_alpha(0.7),
                border.clone(),
            );
            ctx.draw_text(
                Rect::new(
                    row.row_bounds.x() + 12.0,
                    row.row_bounds.y() + 11.0,
                    (row.row_bounds.width() - 166.0).max(0.0),
                    20.0,
                ),
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
                let is_primary =
                    matches!(target.kind, SidebarActionKind::Maximize) && view.maximized;
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
                    Rect::new(
                        bounds.x() + 10.0,
                        bounds.y() + 7.0,
                        bounds.width() - 20.0,
                        bounds.height() - 14.0,
                    ),
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

fn window_text_hinting_from_renderer(hinting: TextHinting) -> WindowTextHinting {
    match hinting.normalized() {
        TextHinting::None => WindowTextHinting::None,
        TextHinting::Slight { max_ppem } => WindowTextHinting::Slight { max_ppem },
    }
}

fn window_stem_darkening_from_renderer(darkening: sui::StemDarkening) -> WindowStemDarkening {
    match darkening.normalized() {
        sui::StemDarkening::None => WindowStemDarkening::None,
        sui::StemDarkening::Enabled { max_ppem, amount } => {
            WindowStemDarkening::Enabled { max_ppem, amount }
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

fn color_management_mode_selected_index(mode: WindowColorManagementMode) -> usize {
    match mode {
        WindowColorManagementMode::Automatic => 0,
        WindowColorManagementMode::ForceSdr => 1,
        WindowColorManagementMode::PreferWideGamut => 2,
        WindowColorManagementMode::PreferHdr => 3,
    }
}

fn update_color_management_mode_selection(state: &mut WindowRenderOptions, index: usize) {
    state.color_management_mode = match index {
        0 => WindowColorManagementMode::Automatic,
        1 => WindowColorManagementMode::ForceSdr,
        2 => WindowColorManagementMode::PreferWideGamut,
        3 => WindowColorManagementMode::PreferHdr,
        _ => state.color_management_mode,
    };
}

fn output_primaries_selected_index(primaries: WindowOutputColorPrimaries) -> usize {
    match primaries {
        WindowOutputColorPrimaries::Automatic => 0,
        WindowOutputColorPrimaries::Srgb => 1,
        WindowOutputColorPrimaries::DisplayP3 => 2,
    }
}

fn update_output_primaries_selection(state: &mut WindowRenderOptions, index: usize) {
    state.output_color_primaries = match index {
        0 => WindowOutputColorPrimaries::Automatic,
        1 => WindowOutputColorPrimaries::Srgb,
        2 => WindowOutputColorPrimaries::DisplayP3,
        _ => state.output_color_primaries,
    };
}

fn dynamic_range_mode_selected_index(mode: WindowDynamicRangeMode) -> usize {
    match mode {
        WindowDynamicRangeMode::Automatic => 0,
        WindowDynamicRangeMode::StandardDynamicRange => 1,
        WindowDynamicRangeMode::HighDynamicRange => 2,
    }
}

fn update_dynamic_range_mode_selection(state: &mut WindowRenderOptions, index: usize) {
    state.dynamic_range_mode = match index {
        0 => WindowDynamicRangeMode::Automatic,
        1 => WindowDynamicRangeMode::StandardDynamicRange,
        2 => WindowDynamicRangeMode::HighDynamicRange,
        _ => state.dynamic_range_mode,
    };
}

fn tone_mapping_mode_selected_index(mode: WindowToneMappingMode) -> usize {
    match mode {
        WindowToneMappingMode::Automatic => 0,
        WindowToneMappingMode::Clamp => 1,
        WindowToneMappingMode::Reinhard => 2,
    }
}

fn update_tone_mapping_mode_selection(state: &mut WindowRenderOptions, index: usize) {
    state.tone_mapping_mode = match index {
        0 => WindowToneMappingMode::Automatic,
        1 => WindowToneMappingMode::Clamp,
        2 => WindowToneMappingMode::Reinhard,
        _ => state.tone_mapping_mode,
    };
}

fn hdr_theme_mode_label(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "Disabled (SDR baseline)",
        HdrThemeMode::WideGamutOnly => "Wide-gamut only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

fn hdr_theme_mode_selected_index(mode: HdrThemeMode) -> usize {
    match mode {
        HdrThemeMode::Disabled => 0,
        HdrThemeMode::WideGamutOnly => 1,
        HdrThemeMode::ConstrainedHdr => 2,
        HdrThemeMode::FullHdr => 3,
    }
}

fn hdr_theme_mode_from_index(index: usize) -> HdrThemeMode {
    match index {
        1 => HdrThemeMode::WideGamutOnly,
        2 => HdrThemeMode::ConstrainedHdr,
        3 => HdrThemeMode::FullHdr,
        _ => HdrThemeMode::Disabled,
    }
}

fn output_policy_label(strategy_debug: &str) -> &'static str {
    if strategy_debug.starts_with("Hdr") {
        "HDR"
    } else if strategy_debug.starts_with("WideGamut") {
        "Wide gamut"
    } else {
        "SDR"
    }
}

fn hdr_theme_inspection_lines(window_id: WindowId) -> Vec<String> {
    let current_mode = widget_book_hdr_theme_mode();
    let mut lines = vec![format!(
        "Current theme mode: {}",
        hdr_theme_mode_label(current_mode)
    )];

    if let Some(diagnostics) = window_output_diagnostics(window_id) {
        let strategy_debug = format!("{:?}", diagnostics.active_output_strategy);
        lines.push(format!(
            "Window output policy: {}",
            output_policy_label(&strategy_debug)
        ));
        lines.push(format!(
            "Requested presentation: {:?} / {:?}",
            diagnostics.requested_color_management_mode, diagnostics.requested_dynamic_range_mode
        ));
        lines.push(format!(
            "Requested SDR content brightness: {:.0} nits",
            diagnostics.requested_sdr_content_brightness_nits
        ));
        lines.push(format!("Active strategy: {strategy_debug}"));
    } else {
        lines.push("Window output policy: waiting for first presented frame".to_string());
        lines.push("Requested presentation: waiting for output diagnostics".to_string());
    }

    lines
}

fn output_diagnostics_lines(window_id: WindowId) -> Vec<String> {
    let Some(diagnostics) = window_output_diagnostics(window_id) else {
        return vec!["Waiting for first presented frame…".to_string()];
    };

    vec![
        format!(
            "Requested mode: {:?}",
            diagnostics.requested_color_management_mode
        ),
        format!(
            "Requested primaries: {:?}",
            diagnostics.requested_output_primaries
        ),
        format!(
            "Requested dynamic range: {:?}",
            diagnostics.requested_dynamic_range_mode
        ),
        format!(
            "Requested tone mapping: {:?}",
            diagnostics.requested_tone_mapping_mode
        ),
        format!(
            "Requested SDR content brightness: {:.0} nits",
            diagnostics.requested_sdr_content_brightness_nits
        ),
        format!(
            "Detected primaries: {:?}",
            diagnostics.display_capabilities.preferred_primaries
        ),
        format!(
            "Detected dynamic range: {:?}",
            diagnostics.display_capabilities.preferred_dynamic_range
        ),
        format!(
            "Wide gamut: {} | HDR: {} | Native HDR: {}",
            diagnostics.display_capabilities.supports_wide_gamut,
            diagnostics.display_capabilities.supports_hdr,
            diagnostics
                .display_capabilities
                .native_hdr_presentation_supported,
        ),
        format!("Active strategy: {:?}", diagnostics.active_output_strategy),
        diagnostics.display_capabilities.notes,
    ]
}

fn labeled_settings_control<W>(label: &'static str, width: f32, control: W) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new(label)
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.20, 0.27, 0.35, 1.0)),
        )
        .with_child(SizedBox::new().width(width).with_child(control))
}

struct HdrThemeInspectionPanel;

impl Widget for HdrThemeInspectionPanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(constraints.max.width.min(640.0), 112.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = DefaultTheme::default().palette;
        let border = StrokeStyle::default();
        ctx.fill_rect(ctx.bounds(), palette.surface.with_alpha(0.35));
        ctx.stroke_rect(ctx.bounds(), palette.border.with_alpha(0.85), border);

        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 12.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            HDR_THEME_INSPECTION_TITLE,
            TextStyle {
                font_size: 14.0,
                line_height: 18.0,
                color: palette.text,
                ..TextStyle::default()
            },
        );

        for (index, line) in hdr_theme_inspection_lines(ctx.window_id())
            .iter()
            .enumerate()
        {
            ctx.draw_text(
                Rect::new(
                    ctx.bounds().x() + 14.0,
                    ctx.bounds().y() + 40.0 + index as f32 * 18.0,
                    ctx.bounds().width() - 28.0,
                    18.0,
                ),
                line,
                TextStyle {
                    font_size: 11.0,
                    line_height: 15.0,
                    color: palette.text.with_alpha(0.9),
                    ..TextStyle::default()
                },
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(HDR_THEME_INSPECTION_TITLE.to_string());
        node.description = Some(hdr_theme_inspection_lines(ctx.window_id()).join("\n"));
        ctx.push(node);
    }
}

struct OutputDiagnosticsPanel;

impl Widget for OutputDiagnosticsPanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            constraints.max.width.min(640.0),
            if constraints.max.height.is_finite() {
                200.0
            } else {
                200.0
            },
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = DefaultTheme::default().palette;
        let border = StrokeStyle::default();
        ctx.fill_rect(ctx.bounds(), palette.surface.with_alpha(0.35));
        ctx.stroke_rect(ctx.bounds(), palette.border.with_alpha(0.85), border);

        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 12.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            OUTPUT_DIAGNOSTICS_TITLE,
            TextStyle {
                font_size: 14.0,
                line_height: 18.0,
                color: palette.text,
                ..TextStyle::default()
            },
        );

        let lines = output_diagnostics_lines(ctx.window_id());
        for (index, line) in lines.iter().enumerate() {
            ctx.draw_text(
                Rect::new(
                    ctx.bounds().x() + 14.0,
                    ctx.bounds().y() + 40.0 + index as f32 * 18.0,
                    ctx.bounds().width() - 28.0,
                    18.0,
                ),
                line,
                TextStyle {
                    font_size: 11.0,
                    line_height: 15.0,
                    color: palette.text.with_alpha(0.9),
                    ..TextStyle::default()
                },
            );
        }
    }
}

struct RenderSettingsScrollPane {
    spacing: f32,
    content: SingleChild,
    scroll_bar: SingleChild,
}

impl RenderSettingsScrollPane {
    fn new<W, S>(content: W, scroll_bar: S) -> Self
    where
        W: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            spacing: 10.0,
            content: SingleChild::new(content),
            scroll_bar: SingleChild::new(scroll_bar),
        }
    }
}

impl Widget for RenderSettingsScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let scroll_bar_size = self.scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let content_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.min.height,
            ),
            Size::new(
                (constraints.max.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.max.height,
            ),
        );
        let content_size = self.content.measure(ctx, content_constraints);
        constraints.clamp(Size::new(
            content_size.width + scroll_bar_size.width + self.spacing,
            content_size.height.max(scroll_bar_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let scroll_bar_size = self.scroll_bar.child().measured_size();
        let content_width = (bounds.width() - scroll_bar_size.width - self.spacing).max(0.0);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), content_width, bounds.height()),
        );
        self.scroll_bar.arrange(
            ctx,
            Rect::new(
                bounds.max_x() - scroll_bar_size.width,
                bounds.y(),
                scroll_bar_size.width,
                bounds.height(),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        self.scroll_bar.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        self.scroll_bar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.scroll_bar.visit_children_mut(visitor);
    }
}

struct RenderSettingsTab {
    content: SingleChild,
    state: Rc<RefCell<WindowRenderOptions>>,
    applied: Option<WindowRenderOptions>,
    last_hdr_theme_mode: HdrThemeMode,
}

impl RenderSettingsTab {
    fn new() -> Self {
        let renderer = WgpuRenderer::new();
        let initial =
            WindowRenderOptions::new(renderer.feathering_enabled(), renderer.feather_width())
                .with_glyph_pixel_alignment_enabled(renderer.glyph_pixel_alignment_enabled())
                .with_text_render_policy(window_text_render_policy_from_renderer(
                    renderer.text_coverage_policy(),
                ))
                .with_text_hinting(window_text_hinting_from_renderer(renderer.text_hinting()))
                .with_stem_darkening(window_stem_darkening_from_renderer(
                    renderer.stem_darkening(),
                ));
        let state = Rc::new(RefCell::new(initial));
        let toggle_state = Rc::clone(&state);
        let width_state = Rc::clone(&state);
        let text_centering_state = Rc::clone(&state);
        let glyph_alignment_state = Rc::clone(&state);
        let text_policy_state = Rc::clone(&state);
        let gamma_state = Rc::clone(&state);
        let hinting_toggle_state = Rc::clone(&state);
        let hinting_max_ppem_state = Rc::clone(&state);
        let stem_darkening_toggle_state = Rc::clone(&state);
        let stem_darkening_amount_state = Rc::clone(&state);
        let stem_darkening_max_ppem_state = Rc::clone(&state);
        let color_management_state = Rc::clone(&state);
        let output_primaries_state = Rc::clone(&state);
        let dynamic_range_state = Rc::clone(&state);
        let tone_mapping_state = Rc::clone(&state);
        let sdr_content_brightness_state = Rc::clone(&state);
        let current_hdr_theme_mode = widget_book_hdr_theme_mode();
        let scroll_state = ScrollState::new();

        let content = RenderSettingsScrollPane::new(
            ScrollView::vertical(Padding::all(
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
                    .with_child(labeled_settings_control(
                        TEXT_RENDER_POLICY_NAME,
                        280.0,
                        Select::new(TEXT_RENDER_POLICY_NAME)
                            .options(TEXT_RENDER_POLICY_OPTIONS)
                            .selected(text_render_policy_selected_index(initial.text_render_policy))
                            .on_change(move |index, _| {
                                let mut state = text_policy_state.borrow_mut();
                                update_text_render_policy_selection(&mut state, index);
                            }),
                    ))
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
                        Checkbox::new(TEXT_HINTING_TOGGLE_LABEL)
                            .checked(!matches!(initial.text_hinting, WindowTextHinting::None))
                            .on_toggle(move |checked| {
                                let mut state = hinting_toggle_state.borrow_mut();
                                state.text_hinting = if checked {
                                    match state.text_hinting.normalized() {
                                        WindowTextHinting::Slight { max_ppem } => {
                                            WindowTextHinting::Slight { max_ppem }
                                        }
                                        WindowTextHinting::None => {
                                            WindowTextHinting::Slight { max_ppem: 18.0 }
                                        }
                                    }
                                } else {
                                    WindowTextHinting::None
                                };
                            }),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            NumberInput::new(TEXT_HINTING_MAX_PPEM_NAME)
                                .range(1.0, 64.0)
                                .step(0.5)
                                .precision(1)
                                .value(match initial.text_hinting.normalized() {
                                    WindowTextHinting::Slight { max_ppem } => max_ppem as f64,
                                    WindowTextHinting::None => 18.0,
                                })
                                .on_change(move |value| {
                                    let max_ppem = value.clamp(1.0, 64.0) as f32;
                                    hinting_max_ppem_state.borrow_mut().text_hinting =
                                        WindowTextHinting::Slight { max_ppem };
                                }),
                        ),
                    )
                    .with_child(
                        Checkbox::new(STEM_DARKENING_TOGGLE_LABEL)
                            .checked(!matches!(initial.stem_darkening, WindowStemDarkening::None))
                            .on_toggle(move |checked| {
                                let mut state = stem_darkening_toggle_state.borrow_mut();
                                state.stem_darkening = if checked {
                                    match state.stem_darkening.normalized() {
                                        WindowStemDarkening::Enabled { max_ppem, amount } => {
                                            WindowStemDarkening::Enabled { max_ppem, amount }
                                        }
                                        WindowStemDarkening::None => {
                                            WindowStemDarkening::Enabled {
                                                max_ppem: 18.0,
                                                amount: 0.08,
                                            }
                                        }
                                    }
                                } else {
                                    WindowStemDarkening::None
                                };
                            }),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            NumberInput::new(STEM_DARKENING_AMOUNT_NAME)
                                .range(0.0, 1.0)
                                .step(0.01)
                                .precision(2)
                                .value(match initial.stem_darkening.normalized() {
                                    WindowStemDarkening::Enabled { amount, .. } => amount as f64,
                                    WindowStemDarkening::None => 0.08,
                                })
                                .on_change(move |value| {
                                    let amount = value.clamp(0.0, 1.0) as f32;
                                    let max_ppem = match stem_darkening_amount_state
                                        .borrow()
                                        .stem_darkening
                                        .normalized()
                                    {
                                        WindowStemDarkening::Enabled { max_ppem, .. } => max_ppem,
                                        WindowStemDarkening::None => 18.0,
                                    };
                                    stem_darkening_amount_state.borrow_mut().stem_darkening =
                                        WindowStemDarkening::Enabled { max_ppem, amount };
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            NumberInput::new(STEM_DARKENING_MAX_PPEM_NAME)
                                .range(1.0, 64.0)
                                .step(0.5)
                                .precision(1)
                                .value(match initial.stem_darkening.normalized() {
                                    WindowStemDarkening::Enabled { max_ppem, .. } => max_ppem as f64,
                                    WindowStemDarkening::None => 18.0,
                                })
                                .on_change(move |value| {
                                    let max_ppem = value.clamp(1.0, 64.0) as f32;
                                    let amount = match stem_darkening_max_ppem_state
                                        .borrow()
                                        .stem_darkening
                                        .normalized()
                                    {
                                        WindowStemDarkening::Enabled { amount, .. } => amount,
                                        WindowStemDarkening::None => 0.08,
                                    };
                                    stem_darkening_max_ppem_state.borrow_mut().stem_darkening =
                                        WindowStemDarkening::Enabled { max_ppem, amount };
                                }),
                        ),
                    )
                    .with_child(labeled_settings_control(
                        COLOR_MANAGEMENT_MODE_NAME,
                        280.0,
                        Select::new(COLOR_MANAGEMENT_MODE_NAME)
                            .options(COLOR_MANAGEMENT_MODE_OPTIONS)
                            .selected(color_management_mode_selected_index(initial.color_management_mode))
                            .on_change(move |index, _| {
                                let mut state = color_management_state.borrow_mut();
                                update_color_management_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        OUTPUT_PRIMARIES_NAME,
                        240.0,
                        Select::new(OUTPUT_PRIMARIES_NAME)
                            .options(OUTPUT_PRIMARIES_OPTIONS)
                            .selected(output_primaries_selected_index(initial.output_color_primaries))
                            .on_change(move |index, _| {
                                let mut state = output_primaries_state.borrow_mut();
                                update_output_primaries_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        DYNAMIC_RANGE_MODE_NAME,
                        240.0,
                        Select::new(DYNAMIC_RANGE_MODE_NAME)
                            .options(DYNAMIC_RANGE_MODE_OPTIONS)
                            .selected(dynamic_range_mode_selected_index(initial.dynamic_range_mode))
                            .on_change(move |index, _| {
                                let mut state = dynamic_range_state.borrow_mut();
                                update_dynamic_range_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        TONE_MAPPING_MODE_NAME,
                        240.0,
                        Select::new(TONE_MAPPING_MODE_NAME)
                            .options(TONE_MAPPING_MODE_OPTIONS)
                            .selected(tone_mapping_mode_selected_index(initial.tone_mapping_mode))
                            .on_change(move |index, _| {
                                let mut state = tone_mapping_state.borrow_mut();
                                update_tone_mapping_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        SDR_CONTENT_BRIGHTNESS_NAME,
                        220.0,
                        NumberInput::new(SDR_CONTENT_BRIGHTNESS_NAME)
                            .range(48.0, 1000.0)
                            .step(1.0)
                            .precision(0)
                            .value(initial.sdr_content_brightness_nits as f64)
                            .on_change(move |value| {
                                sdr_content_brightness_state
                                    .borrow_mut()
                                    .sdr_content_brightness_nits = value.clamp(48.0, 1000.0)
                                    as f32;
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        HDR_THEME_MODE_NAME,
                        280.0,
                        Select::new(HDR_THEME_MODE_NAME)
                            .options(HDR_THEME_MODE_OPTIONS)
                            .selected(hdr_theme_mode_selected_index(current_hdr_theme_mode))
                            .on_change(move |index, _| {
                                set_widget_book_hdr_theme_mode(hdr_theme_mode_from_index(index));
                            }),
                    ))
                    .with_child(HdrThemeInspectionPanel)
                    .with_child(OutputDiagnosticsPanel)
                    .with_child(
                        Label::new(
                            "Optical centering uses cap height when available and a softened descent bias for Latin UI labels. Glyph pixel alignment only affects the atlas path for axis-aligned text. The render policy applies to both atlas and fallback glyph coverage; the gamma input is only used when the Gamma policy is selected. Slight hinting biases small-text rasterization below the configured ppem threshold. Stem darkening slightly boosts thin small-text coverage below its threshold. Phase 2 controls choose the preferred color-management policy, the HDR theme selector drives the shared widget-book preview mode, and the inspection panels show the detected monitor/output path after each redraw.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(Color::rgba(0.45, 0.52, 0.60, 1.0)),
                    ),
            ))
            .state(scroll_state.clone())
            .name(SETTINGS_SCROLL_NAME),
            ScrollBar::vertical(scroll_state).name("Settings scroll bar"),
        );

        Self {
            content: SingleChild::new(content),
            state,
            applied: None,
            last_hdr_theme_mode: current_hdr_theme_mode,
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
        let current_hdr_theme_mode = widget_book_hdr_theme_mode();
        if current_hdr_theme_mode != self.last_hdr_theme_mode {
            self.last_hdr_theme_mode = current_hdr_theme_mode;
            ctx.request_paint();
            ctx.request_semantics();
        }

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

pub fn build_dev_workspace_with_widget_book_bounds(
    widget_book_bounds: Rect,
) -> (FloatingWorkspaceState, FloatingWorkspace) {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);
    let widget_book_state = default_widget_book_state();
    let workspace = FloatingWorkspaceState::new();

    let mut views = FloatingWorkspace::new(workspace.clone()).name("Development workspace");
    views.push_view(
        FloatingViewConfig::new(WIDGET_BOOK_TAB_LABEL, widget_book_bounds)
            .min_size(Size::new(420.0, 320.0)),
        build_widget_book_gallery(widget_book_state),
    );
    views.push_view(
        FloatingViewConfig::new(BUTTON_GRID_TAB_LABEL, Rect::new(560.0, 72.0, 420.0, 340.0))
            .min_size(Size::new(280.0, 220.0)),
        build_button_grid_benchmark(),
    );
    views.push_view(
        FloatingViewConfig::new(
            RETAINED_TEXT_TAB_LABEL,
            Rect::new(860.0, 420.0, 360.0, 260.0),
        )
        .min_size(Size::new(320.0, 260.0))
        .visible(false),
        build_retained_text_benchmark(),
    );
    views.push_view(
        FloatingViewConfig::new(
            TEXT_RENDERING_COMPARISON_TAB_LABEL,
            Rect::new(980.0, 72.0, 520.0, 420.0),
        )
        .min_size(Size::new(420.0, 320.0))
        .visible(false),
        build_text_rendering_comparison_surface(),
    );
    views.push_view(
        FloatingViewConfig::new(
            TEXT_VALIDATION_TAB_LABEL,
            Rect::new(720.0, 72.0, 460.0, 380.0),
        )
        .min_size(Size::new(360.0, 280.0))
        .visible(false),
        build_text_validation_surface(),
    );
    views.push_view(
        FloatingViewConfig::new(
            TEXT_EDITING_TAB_LABEL,
            Rect::new(720.0, 470.0, 520.0, 360.0),
        )
        .min_size(Size::new(420.0, 300.0))
        .visible(false),
        build_text_editing_benchmark(),
    );
    views.push_view(
        FloatingViewConfig::new(
            HDR_VALIDATION_TAB_LABEL,
            Rect::new(980.0, 120.0, 620.0, 520.0),
        )
        .min_size(Size::new(460.0, 320.0)),
        build_color_validation_surface(),
    );
    views.push_view(
        FloatingViewConfig::new(SETTINGS_TAB_LABEL, Rect::new(420.0, 440.0, 420.0, 320.0))
            .min_size(Size::new(300.0, 240.0)),
        build_render_settings_tab(),
    );

    (workspace, views)
}

pub fn build_dev_application_with_widget_book_bounds_and_automation(
    widget_book_bounds: Rect,
    automation: Option<DesktopAutomationMode>,
) -> Application {
    let (workspace, views) = build_dev_workspace_with_widget_book_bounds(widget_book_bounds);
    let button_grid_view_id = workspace
        .snapshots()
        .into_iter()
        .find(|view| view.title == BUTTON_GRID_TAB_LABEL)
        .map(|view| (view.id, view.bounds))
        .expect("expected button grid view in dev workspace");

    let root = SplitView::horizontal(ViewSidebar::new(workspace.clone()), views)
        .name("Development workspace split")
        .ratio(0.24)
        .min_first(236.0)
        .min_second(420.0)
        .divider_thickness(12.0);

    let root = DesktopAutomationRoot::new(
        workspace.clone(),
        button_grid_view_id.0,
        button_grid_view_id.1,
        automation,
        root,
    );

    let mut app = Application::new();
    register_widget_book_images(&mut app);
    let app = app.window(
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(LivePerformanceRoot::new(
                WINDOW_TITLE,
                WINDOW_DESCRIPTION,
                root,
            )),
    );

    app
}

pub fn build_dev_application_with_widget_book_bounds(widget_book_bounds: Rect) -> Application {
    build_dev_application_with_widget_book_bounds_and_automation(widget_book_bounds, None)
}

pub fn build_dev_application() -> Application {
    build_dev_application_with_widget_book_bounds(Rect::new(24.0, 24.0, 680.0, 760.0))
}

pub fn build_dev_application_with_automation(
    automation: Option<DesktopAutomationMode>,
) -> Application {
    build_dev_application_with_widget_book_bounds_and_automation(
        Rect::new(24.0, 24.0, 680.0, 760.0),
        automation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use sui::{
        Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect, Result,
        SceneStatisticsDetailMode, SemanticsNode, SemanticsRole, StackOrderPolicy, Vector,
        WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent, WindowOutputColorPrimaries,
        WindowPerformanceSnapshot, WindowRenderOptions, WindowToneMappingMode,
        set_window_scene_statistics_detail_mode, window_performance_snapshot,
        window_scene_statistics_detail_mode,
    };
    use sui_render_wgpu::{
        DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
        DebugSdrVisualization,
    };
    use sui_testing::{
        Screenshot, TestApp, TestWindow, WindowSnapshot, hdr_clip_mask, hdr_headroom_heatmap,
        hdr_luminance_heatmap, write_hdr_avif, write_hdr_exr,
    };

    const FRONTING_TEST_TITLE: &str = "Fronting test";

    fn unique_debug_artifact_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "sui-hdr-debug-{}-{}-{}",
            std::process::id(),
            nonce,
            name
        ));
        std::fs::create_dir_all(&dir).expect("temporary HDR debug directory created");
        dir
    }

    struct SolidFill {
        color: Color,
    }

    impl SolidFill {
        fn new(color: Color) -> Self {
            Self { color }
        }
    }

    impl Widget for SolidFill {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.max
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(self.color);
        }
    }

    fn build_fronting_test_application() -> Application {
        let workspace = FloatingWorkspaceState::new();
        let mut views = FloatingWorkspace::new(workspace.clone()).name(FRONTING_TEST_TITLE);
        views.push_view(
            FloatingViewConfig::new("First", Rect::new(24.0, 48.0, 320.0, 240.0))
                .min_size(Size::new(220.0, 160.0)),
            SolidFill::new(Color::rgba(0.86, 0.22, 0.18, 1.0)),
        );
        views.push_view(
            FloatingViewConfig::new("Second", Rect::new(420.0, 88.0, 320.0, 240.0))
                .min_size(Size::new(220.0, 160.0)),
            SolidFill::new(Color::rgba(0.16, 0.62, 0.28, 1.0)),
        );

        let root = SplitView::horizontal(ViewSidebar::new(workspace), views)
            .name("Fronting test split")
            .ratio(0.24)
            .min_first(236.0)
            .min_second(420.0)
            .divider_thickness(12.0);

        Application::new().window(WindowBuilder::new().title(FRONTING_TEST_TITLE).root(
            LivePerformanceRoot::new(
                FRONTING_TEST_TITLE,
                "Floating workspace fronting regression.",
                root,
            ),
        ))
    }

    #[test]
    fn widget_book_scroll_does_not_repaint_pixels_outside_shrunken_floating_view() -> Result<()> {
        let initial_bounds = Rect::new(320.0, 28.0, 560.0, 520.0);
        let app = TestApp::new(move || {
            build_dev_application_with_widget_book_bounds(initial_bounds).build()
        })?;
        let window = app.main_window()?;

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            WIDGET_BOOK_TAB_LABEL,
        );
        let resize_start = Point::new(
            initial_view.bounds.max_x() - 8.0,
            initial_view.bounds.max_y() - 8.0,
        );
        let resize_end = Point::new(
            initial_view.bounds.x() + 420.0,
            initial_view.bounds.y() + 328.0,
        );
        drag_pointer(&window, resize_start, resize_end)?;

        let before_snapshot = window.snapshot()?;
        let view = find_named_node(
            &before_snapshot,
            SemanticsRole::Window,
            WIDGET_BOOK_TAB_LABEL,
        );
        assert!(
            view.bounds.width() <= 440.0,
            "expected the widget book floating view to shrink horizontally for the regression, before={:?} after={:?}",
            initial_view.bounds,
            view.bounds,
        );
        assert!(
            view.bounds.height() <= 360.0,
            "expected the widget book floating view to shrink vertically for the regression, before={:?} after={:?}",
            initial_view.bounds,
            view.bounds,
        );

        let viewport = viewport_bounds(&before_snapshot);
        let probes = leak_probe_regions(view.bounds, viewport);
        assert!(
            !probes.is_empty(),
            "expected at least one valid probe region outside the widget book view, view={:?}, viewport={:?}",
            view.bounds,
            viewport,
        );

        let gallery = window
            .get_by_role(SemanticsRole::Window)
            .with_name(WIDGET_BOOK_TAB_LABEL)
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(sui_widget_book::GALLERY_SCROLL_NAME);

        let before_frame = window.capture_screenshot()?;
        for _ in 0..6 {
            gallery.scroll_pixels(Vector::new(0.0, -120.0))?;
        }
        let after_snapshot = window.snapshot()?;
        let after_frame = window.capture_screenshot()?;

        for probe in probes {
            let before_crop = before_frame.crop(scale_bounds_for_screenshot(
                probe,
                &before_snapshot,
                &before_frame,
            ))?;
            let after_crop = after_frame.crop(scale_bounds_for_screenshot(
                probe,
                &after_snapshot,
                &after_frame,
            ))?;
            let diff_count = pixel_diff_count(&before_crop, &after_crop);
            assert_eq!(
                diff_count, 0,
                "scrolling inside the shrunken widget book view changed pixels outside the view bounds in probe {:?}",
                probe,
            );
        }

        Ok(())
    }

    #[test]
    fn dev_shell_clicking_floating_title_bar_reorders_frontmost_pixels() -> Result<()> {
        let app = TestApp::new(move || build_fronting_test_application().build())?;
        let window = app.main_window()?;

        let before_snapshot = window.snapshot()?;
        let first_view = find_named_node(&before_snapshot, SemanticsRole::Window, "First");
        let host = before_snapshot
            .widget_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should be present");
        assert_eq!(host.surfaces.len(), 2);
        let first_surface = host.surfaces[0];
        assert_eq!(host.surfaces[0], first_surface);

        let second_view = find_named_node(&before_snapshot, SemanticsRole::Window, "Second");
        let overlap_probe = overlap_probe(first_view.bounds, second_view.bounds);
        let before_frame = window.capture_screenshot()?;
        let before_pixel = sample_pixel(&before_frame, overlap_probe, &before_snapshot)?;

        let click_point = Point::new(first_view.bounds.x() + 32.0, first_view.bounds.y() + 18.0);
        click_pointer(&window, click_point)?;

        let after_snapshot = window.snapshot()?;
        let host = after_snapshot
            .widget_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        assert_eq!(host.surfaces.len(), 2);
        assert_eq!(host.surfaces[1], first_surface);

        let after_frame = window.capture_screenshot()?;
        let after_pixel = sample_pixel(&after_frame, overlap_probe, &after_snapshot)?;

        assert_ne!(
            before_pixel, after_pixel,
            "expected overlap pixel to change after fronting"
        );
        assert!(
            after_pixel[0] > after_pixel[1],
            "expected first view color to be frontmost after click, pixel={after_pixel:?}"
        );
        Ok(())
    }

    #[test]
    fn dev_shell_dragging_floating_title_bar_keeps_dragged_view_frontmost() -> Result<()> {
        let app = TestApp::new(move || build_fronting_test_application().build())?;
        let window = app.main_window()?;
        let root = window.root();

        let before_snapshot = window.snapshot()?;
        let first_view = find_named_node(&before_snapshot, SemanticsRole::Window, "First");
        let first_surface = before_snapshot
            .widget_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .and_then(|host| host.surfaces.first().copied())
            .expect("first surface should be present before drag");
        let second_view = find_named_node(&before_snapshot, SemanticsRole::Window, "Second");
        let overlap_probe = overlap_probe(first_view.bounds, second_view.bounds);
        let before_frame = window.capture_screenshot()?;
        let before_pixel = sample_pixel(&before_frame, overlap_probe, &before_snapshot)?;

        let drag_start = Point::new(first_view.bounds.x() + 32.0, first_view.bounds.y() + 18.0);
        let drag_end = Point::new(drag_start.x + 24.0, drag_start.y + 8.0);
        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            drag_start,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, drag_start);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let mut moved = PointerEvent::new(PointerEventKind::Move, drag_end);
        moved.buttons = PointerButtons::new(1);
        moved.delta = drag_end - drag_start;
        root.dispatch_event(Event::Pointer(moved))?;

        let during_drag_snapshot = window.snapshot()?;
        let during_drag_host = during_drag_snapshot
            .widget_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should be present during drag");
        assert_eq!(
            during_drag_host.surfaces.last().copied(),
            Some(first_surface)
        );
        let during_drag_frame = window.capture_screenshot()?;
        let during_drag_pixel =
            sample_pixel(&during_drag_frame, overlap_probe, &during_drag_snapshot)?;

        let mut up = PointerEvent::new(PointerEventKind::Up, drag_end);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up))?;

        let after_snapshot = window.snapshot()?;
        let moved_first_view = find_named_node(&after_snapshot, SemanticsRole::Window, "First");
        assert!(moved_first_view.bounds.x() > first_view.bounds.x());
        assert!(moved_first_view.bounds.y() > first_view.bounds.y());
        let host = after_snapshot
            .widget_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present after drag");
        assert_eq!(host.surfaces.last().copied(), Some(first_surface));
        let after_frame = window.capture_screenshot()?;
        let after_pixel = sample_pixel(&after_frame, overlap_probe, &after_snapshot)?;

        assert_ne!(
            before_pixel, during_drag_pixel,
            "expected overlap pixel to change while dragging a fronted view"
        );
        assert!(
            during_drag_pixel[0] > during_drag_pixel[1],
            "expected dragged first view color to be frontmost during drag, pixel={during_drag_pixel:?}"
        );
        assert_ne!(
            before_pixel, after_pixel,
            "expected overlap pixel to stay changed after dragging a fronted view"
        );
        assert!(
            after_pixel[0] > after_pixel[1],
            "expected dragged first view color to remain frontmost, pixel={after_pixel:?}"
        );
        Ok(())
    }

    #[test]
    fn widget_book_image_and_swatch_stories_do_not_leak_outside_shrunken_floating_view()
    -> Result<()> {
        let initial_bounds = Rect::new(320.0, 28.0, 560.0, 520.0);
        let app = TestApp::new(move || {
            build_dev_application_with_widget_book_bounds(initial_bounds).build()
        })?;
        let window = app.main_window()?;

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            WIDGET_BOOK_TAB_LABEL,
        );
        let resize_start = Point::new(
            initial_view.bounds.max_x() - 8.0,
            initial_view.bounds.max_y() - 8.0,
        );
        let resize_end = Point::new(
            initial_view.bounds.x() + 420.0,
            initial_view.bounds.y() + 328.0,
        );
        drag_pointer(&window, resize_start, resize_end)?;

        assert_story_exit_does_not_repaint_outside_view(
            &window,
            SemanticsRole::Image,
            sui_widget_book::DEMO_IMAGE_LABEL,
        )?;
        assert_story_exit_does_not_repaint_outside_view(
            &window,
            SemanticsRole::ColorSwatch,
            sui_widget_book::COLOR_SWATCH_NAME,
        )?;

        Ok(())
    }

    #[test]
    fn settings_view_scrolls_without_repainting_outside_its_floating_bounds() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;

        let before_snapshot = window.snapshot()?;
        let settings_view =
            find_named_node(&before_snapshot, SemanticsRole::Window, SETTINGS_TAB_LABEL);
        let settings_scroll = find_named_node(
            &before_snapshot,
            SemanticsRole::ScrollView,
            SETTINGS_SCROLL_NAME,
        );
        let viewport = viewport_bounds(&before_snapshot);
        let probes = leak_probe_regions(settings_view.bounds, viewport);
        assert!(
            !probes.is_empty(),
            "expected probe regions around the settings floating view, view={:?}, viewport={:?}",
            settings_view.bounds,
            viewport,
        );

        let interior_probe = Rect::new(
            settings_scroll.bounds.x() + 16.0,
            settings_scroll.bounds.y() + 16.0,
            (settings_scroll.bounds.width() - 32.0).max(24.0),
            (settings_scroll.bounds.height() - 32.0).max(24.0),
        );
        let scroll = window
            .get_by_role(SemanticsRole::Window)
            .with_name(SETTINGS_TAB_LABEL)
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(SETTINGS_SCROLL_NAME);

        let before_frame = window.capture_screenshot()?;
        for _ in 0..4 {
            scroll.scroll_pixels(Vector::new(0.0, -160.0))?;
        }

        let after_snapshot = window.snapshot()?;
        let after_frame = window.capture_screenshot()?;

        let before_interior = before_frame.crop(scale_bounds_for_screenshot(
            interior_probe,
            &before_snapshot,
            &before_frame,
        ))?;
        let after_interior = after_frame.crop(scale_bounds_for_screenshot(
            interior_probe,
            &after_snapshot,
            &after_frame,
        ))?;
        assert!(
            pixel_diff_count(&before_interior, &after_interior) > 0,
            "expected scrolling the settings scroll view to change pixels inside the scroll viewport",
        );

        for probe in probes {
            let before_crop = before_frame.crop(scale_bounds_for_screenshot(
                probe,
                &before_snapshot,
                &before_frame,
            ))?;
            let after_crop = after_frame.crop(scale_bounds_for_screenshot(
                probe,
                &after_snapshot,
                &after_frame,
            ))?;
            let diff_count = pixel_diff_count(&before_crop, &after_crop);
            assert_eq!(
                diff_count, 0,
                "scrolling the settings view changed pixels outside the floating bounds in probe {:?}",
                probe,
            );
        }

        Ok(())
    }

    #[test]
    fn hdr_validation_view_is_present_in_dev_workspace() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Window)
            .with_name(HDR_VALIDATION_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        Ok(())
    }

    #[test]
    fn hdr_validation_surface_debug_capture_exports_intermediate_artifacts() -> Result<()> {
        let options = WindowRenderOptions::new(true, 1.0)
            .with_color_management_mode(WindowColorManagementMode::PreferHdr)
            .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
            .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
            .with_tone_mapping_mode(WindowToneMappingMode::Automatic);
        let app = TestApp::new_visible_no_vsync(move || {
            sui_widget_book::build_color_validation_application()
                .with_window_render_options(options)
        })?;
        let window = app.main_window()?;
        let artifact = window.capture_debug_frame(DebugCaptureRequest {
            stage: DebugCaptureStage::HdrIntermediate,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        })?;
        let DebugCaptureArtifact::HdrLinearRgbaF32(image) = artifact else {
            panic!("expected HDR debug capture artifact");
        };

        let max_channel = image
            .pixels()
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let artifact_dir = unique_debug_artifact_dir("color-validation");
        write_hdr_exr(&image, artifact_dir.join("hdr-intermediate.exr"))?;
        write_hdr_avif(&image, artifact_dir.join("hdr-intermediate.avif"), 1.0)?;
        hdr_luminance_heatmap(&image)?.write_png(artifact_dir.join("luminance-map.png"))?;
        hdr_headroom_heatmap(&image, 1.0)?.write_png(artifact_dir.join("headroom-map.png"))?;
        hdr_clip_mask(&image, 1.0)?.write_png(artifact_dir.join("clip-mask.png"))?;

        let diagnostics = window_output_diagnostics(window.id())
            .expect("output diagnostics should be published for visible HDR debug capture");
        std::fs::write(
            artifact_dir.join("output-diagnostics.txt"),
            format!(
                "supports_hdr={}
native_hdr_presentation_supported={}
preferred_dynamic_range={:?}
requested_color_management_mode={:?}
requested_output_primaries={:?}
requested_dynamic_range_mode={:?}
requested_tone_mapping_mode={:?}
requested_sdr_content_brightness_nits={:.0}
active_output_strategy={:?}
notes={}
",
                diagnostics.display_capabilities.supports_hdr,
                diagnostics
                    .display_capabilities
                    .native_hdr_presentation_supported,
                diagnostics.display_capabilities.preferred_dynamic_range,
                diagnostics.requested_color_management_mode,
                diagnostics.requested_output_primaries,
                diagnostics.requested_dynamic_range_mode,
                diagnostics.requested_tone_mapping_mode,
                diagnostics.requested_sdr_content_brightness_nits,
                diagnostics.active_output_strategy,
                diagnostics.display_capabilities.notes,
            ),
        )
        .expect("write output diagnostics artifact");

        let final_artifact = window.capture_debug_frame(DebugCaptureRequest {
            stage: DebugCaptureStage::FinalComposed,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        })?;
        let intermediate_max_channel = max_channel;
        let intermediate_max_luminance = image
            .pixels()
            .chunks_exact(4)
            .map(|rgba| rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722)
            .fold(f32::NEG_INFINITY, f32::max);
        let (final_max_channel, final_max_luminance, final_artifact_kind) = match final_artifact {
            DebugCaptureArtifact::HdrLinearRgbaF32(final_image) => {
                write_hdr_exr(&final_image, artifact_dir.join("final-composed.exr"))?;
                write_hdr_avif(&final_image, artifact_dir.join("final-composed.avif"), 1.0)?;
                hdr_luminance_heatmap(&final_image)?
                    .write_png(artifact_dir.join("final-luminance-map.png"))?;
                let max_channel = final_image
                    .pixels()
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max);
                let max_luminance = final_image
                    .pixels()
                    .chunks_exact(4)
                    .map(|rgba| rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722)
                    .fold(f32::NEG_INFINITY, f32::max);
                (max_channel, max_luminance, "hdr")
            }
            DebugCaptureArtifact::SdrRgba8(final_image) => {
                Screenshot::new(
                    final_image.width(),
                    final_image.height(),
                    final_image.into_pixels(),
                )?
                .write_png(artifact_dir.join("final-composed.png"))?;
                (1.0, 1.0, "sdr")
            }
        };
        std::fs::write(
            artifact_dir.join("capture-metrics.txt"),
            format!(
                "intermediate_max_channel={intermediate_max_channel}
intermediate_max_luminance={intermediate_max_luminance}
final_artifact_kind={final_artifact_kind}
final_max_channel={final_max_channel}
final_max_luminance={final_max_luminance}
"
            ),
        )
        .expect("write capture metrics artifact");

        assert!(artifact_dir.join("hdr-intermediate.exr").exists());
        assert!(artifact_dir.join("hdr-intermediate.avif").exists());
        assert!(artifact_dir.join("luminance-map.png").exists());
        assert!(artifact_dir.join("headroom-map.png").exists());
        assert!(artifact_dir.join("clip-mask.png").exists());
        assert!(artifact_dir.join("output-diagnostics.txt").exists());
        assert!(artifact_dir.join("capture-metrics.txt").exists());
        assert!(
            artifact_dir.join("final-composed.exr").exists()
                || artifact_dir.join("final-composed.png").exists()
                || artifact_dir.join("final-composed.avif").exists()
        );
        assert!(
            max_channel > 1.0,
            "expected HDR validation surface to emit >1.0 scene-linear values, got max_channel={max_channel}; artifacts at {}",
            artifact_dir.display()
        );
        assert!(
            final_max_channel >= 1.0,
            "expected final composed capture to remain valid, got final_max_channel={final_max_channel}; artifacts at {}",
            artifact_dir.display()
        );

        Ok(())
    }

    #[test]
    fn hdr_validation_surface_scroll_reveals_overbright_hdr_probes() -> Result<()> {
        let options = WindowRenderOptions::new(true, 1.0)
            .with_color_management_mode(WindowColorManagementMode::PreferHdr)
            .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
            .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
            .with_tone_mapping_mode(WindowToneMappingMode::Automatic);
        let app = TestApp::new_visible_no_vsync(move || {
            sui_widget_book::build_color_validation_application()
                .with_window_render_options(options)
        })?;
        let window = app.main_window()?;
        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(sui_widget_book::COLOR_VALIDATION_SCROLL_NAME);
        scroll.scroll_pixels(Vector::new(0.0, -900.0))?;

        let artifact = window.capture_debug_frame(DebugCaptureRequest {
            stage: DebugCaptureStage::HdrIntermediate,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        })?;
        let DebugCaptureArtifact::HdrLinearRgbaF32(image) = artifact else {
            panic!("expected HDR debug capture artifact after scrolling");
        };

        let max_luminance = image
            .pixels()
            .chunks_exact(4)
            .map(|rgba| rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            max_luminance > 1.0,
            "expected scrolled HDR validation surface to expose overbright probes, got max_luminance={max_luminance}"
        );
        Ok(())
    }

    #[test]
    fn settings_view_exposes_visible_labels_for_render_selectors() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("dev application should render for settings semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");

        for label in [
            TEXT_RENDER_POLICY_NAME,
            COLOR_MANAGEMENT_MODE_NAME,
            OUTPUT_PRIMARIES_NAME,
            DYNAMIC_RANGE_MODE_NAME,
            TONE_MAPPING_MODE_NAME,
            SDR_CONTENT_BRIGHTNESS_NAME,
            HDR_THEME_MODE_NAME,
        ] {
            assert!(
                semantics
                    .iter()
                    .any(|node| node.name.as_deref() == Some(label)),
                "expected semantics tree to expose settings control {label:?}"
            );
        }
    }

    #[test]
    fn settings_view_exposes_hdr_theme_mode_controls() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("dev application should render for settings semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");

        assert!(
            semantics
                .iter()
                .any(|node| { node.name.as_deref() == Some(HDR_THEME_MODE_NAME) })
        );

        let inspection = semantics
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(HDR_THEME_INSPECTION_TITLE)
            })
            .expect("HDR theme inspection semantics node should be present");
        let description = inspection
            .description
            .as_deref()
            .expect("HDR theme inspection semantics description should be present");
        assert!(description.contains("Current theme mode: Disabled (SDR baseline)"));
        assert!(description.contains("Window output policy:"));
    }

    fn drag_pointer(window: &TestWindow, from: Point, to: Point) -> Result<()> {
        drag_pointer_with_samples(window, from, to, 1).map(|_| ())
    }

    fn wait_for_frame_advance(
        window: &TestWindow,
        previous_frame_index: u64,
        timeout: Duration,
    ) -> Result<Option<WindowPerformanceSnapshot>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            window.run_until_idle()?;
            let performance = window.performance_snapshot()?;
            if performance.frame_index > previous_frame_index {
                return Ok(Some(performance));
            }

            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }

            thread::sleep(Duration::from_millis(16));
        }
    }

    fn latest_published_frame(window: &TestWindow) -> Result<WindowPerformanceSnapshot> {
        window_performance_snapshot(window.id()).ok_or_else(|| {
            sui::Error::new(format!(
                "window {} does not have a published performance snapshot yet",
                window.id().get()
            ))
        })
    }

    fn live_performance_toggle_point(snapshot: &WindowSnapshot) -> Point {
        let overlay = find_named_node(
            snapshot,
            SemanticsRole::GenericContainer,
            "Live performance overlay",
        );
        Point::new(overlay.bounds.max_x() - 50.0, overlay.bounds.y() + 18.0)
    }

    fn ensure_live_overlay_detail_mode(window: &TestWindow) -> Result<()> {
        if window_scene_statistics_detail_mode(window.id()) == SceneStatisticsDetailMode::Detailed {
            return Ok(());
        }

        let snapshot = window.snapshot()?;
        click_pointer(window, live_performance_toggle_point(&snapshot))?;
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "expected live performance overlay toggle to enable detailed mode"
        );
        Ok(())
    }

    fn click_pointer(window: &TestWindow, position: Point) -> Result<()> {
        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            position,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up)).map(|_| ())
    }

    fn scroll_pointer(window: &TestWindow, position: Point, delta: Vector) -> Result<()> {
        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            position,
        )))?;

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, position);
        scroll.scroll_delta = Some(sui::ScrollDelta::Pixels(delta));
        root.dispatch_event(Event::Pointer(scroll)).map(|_| ())
    }

    fn drag_pointer_with_samples(
        window: &TestWindow,
        from: Point,
        to: Point,
        steps: usize,
    ) -> Result<Vec<WindowPerformanceSnapshot>> {
        assert!(steps > 0, "drag steps must be greater than zero");

        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            from,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, from);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let mut samples = Vec::with_capacity(steps);
        let total_delta = to - from;
        let mut previous = from;
        for step in 1..=steps {
            let progress = step as f32 / steps as f32;
            let position = Point::new(
                from.x + (total_delta.x * progress),
                from.y + (total_delta.y * progress),
            );
            let mut moved = PointerEvent::new(PointerEventKind::Move, position);
            moved.buttons = PointerButtons::new(1);
            moved.delta = position - previous;
            root.dispatch_event(Event::Pointer(moved))?;
            samples.push(window.performance_snapshot()?);
            previous = position;
        }

        let mut up = PointerEvent::new(PointerEventKind::Up, to);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up)).map(|_| samples)
    }

    fn find_named_node(
        snapshot: &WindowSnapshot,
        role: SemanticsRole,
        name: &str,
    ) -> SemanticsNode {
        let matches = snapshot
            .accessibility
            .nodes
            .iter()
            .filter(|node| node.role == role && node.name.as_deref() == Some(name))
            .cloned()
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [node] => node.clone(),
            [] => panic!("missing semantics node {:?} named {:?}", role, name),
            _ => panic!(
                "expected exactly one semantics node {:?} named {:?}, found {}",
                role,
                name,
                matches.len()
            ),
        }
    }

    #[test]
    fn button_grid_resize_stays_at_stable_60_fps_in_dev_workspace() -> Result<()> {
        const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
        const DRAG_STEPS: usize = 28;
        const WARMUP_SAMPLES: usize = 4;

        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            BUTTON_GRID_TAB_LABEL,
        );
        let resize_start = Point::new(
            initial_view.bounds.max_x() - 8.0,
            initial_view.bounds.max_y() - 8.0,
        );
        let resize_end = Point::new(
            initial_view.bounds.x() + 760.0,
            initial_view.bounds.y() + 560.0,
        );

        let frame_samples =
            drag_pointer_with_samples(&window, resize_start, resize_end, DRAG_STEPS)?;
        let measured_samples = frame_samples
            .into_iter()
            .skip(WARMUP_SAMPLES)
            .collect::<Vec<_>>();
        assert!(
            !measured_samples.is_empty(),
            "expected resize benchmark to record measured frame samples"
        );

        let after_snapshot = window.snapshot()?;
        let resized_view = find_named_node(
            &after_snapshot,
            SemanticsRole::Window,
            BUTTON_GRID_TAB_LABEL,
        );
        assert!(
            resized_view.bounds.width() > initial_view.bounds.width(),
            "expected the 64-button view to grow during the resize benchmark, before={:?} after={:?}",
            initial_view.bounds,
            resized_view.bounds,
        );
        assert!(
            resized_view.bounds.height() > initial_view.bounds.height(),
            "expected the 64-button view height to grow during the resize benchmark, before={:?} after={:?}",
            initial_view.bounds,
            resized_view.bounds,
        );

        let frame_times_ms = measured_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let avg_visible_layers = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_widget_count = measured_samples
            .iter()
            .map(|sample| sample.scene.total_widget_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_repaint_boundary_count = measured_samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_stack_surface_count = measured_samples
            .iter()
            .map(|sample| sample.scene.stack_surface_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_overlay_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.overlay_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_direct_packets = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.direct_packet_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_count = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_new = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_rebuild_new_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_signature = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_signature_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_acquire_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_draws = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.draw_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_uploaded_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.uploaded_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_instances = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_glyph_instance_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_atlas_miss_count = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_atlas_miss_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_cache_hits = measured_samples
            .iter()
            .map(|sample| sample.text_cache_deltas.renderer_glyph.hits as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_cache_misses = measured_samples
            .iter()
            .map(|sample| sample.text_cache_deltas.renderer_glyph.misses as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_cache_entries = measured_samples
            .iter()
            .map(|sample| sample.text_cache_deltas.renderer_glyph.entries_delta as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_path_cache_hits = measured_samples
            .iter()
            .map(|sample| sample.text_cache_deltas.renderer_path.hits as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_path_cache_misses = measured_samples
            .iter()
            .map(|sample| sample.text_cache_deltas.renderer_path.misses as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.command_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.text_command_count as f64)
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev 64-Button Resize Benchmark ===");
        println!("frames measured:  {valid_count}");
        println!(
            "avg frame time:   {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:   {min_ms:.3} ms");
        println!("max frame time:   {max_ms:.3} ms");
        println!(
            "p95 frame time:   {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg scene cmds:   {avg_scene_commands:.2} ({avg_text_commands:.2} text)");
        println!("avg draws:        {avg_draws:.2}");
        println!("avg vertex bytes: {:.0}", avg_uploaded_vertex_bytes);
        println!(
            "avg text bytes:   {:.0} ({avg_glyph_instances:.2} glyphs)",
            avg_text_vertex_bytes
        );
        println!("avg widgets:      {avg_widget_count:.2}");
        println!(
            "avg repaint(now): {avg_repaint_boundary_count:.2} ({avg_scene_layer_count:.2} scene | {avg_stack_surface_count:.2} stack | {avg_overlay_layer_count:.2} overlay)"
        );
        println!("avg visible layers:{avg_visible_layers:.2}");
        println!("avg direct packets:{avg_direct_packets:.2}");
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!(
            "avg packet build: {avg_packet_build_ms:.3} ms ({avg_packet_build_count:.2} packets)"
        );
        println!(
            "avg packet why:   new {avg_packet_rebuild_new:.2} | coord {avg_packet_rebuild_coordinate_space:.2} | sig {avg_packet_rebuild_signature:.2} | scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2}"
        );
        println!("avg atlas misses: {avg_text_atlas_miss_count:.2}");
        println!(
            "avg glyph cache Δ:{avg_glyph_cache_entries:.2} entries / {avg_glyph_cache_hits:.2} hits / {avg_glyph_cache_misses:.2} misses"
        );
        println!(
            "avg path cache Δ: {avg_path_cache_hits:.2} hits / {avg_path_cache_misses:.2} misses"
        );
        println!("avg surface acq:  {avg_surface_acquire_ms:.3} ms");
        println!("=========================================\n");

        assert!(
            avg_ms < FRAME_BUDGET_MS,
            "average resize frame time {avg_ms:.3} ms exceeds the 16.67 ms budget for 60 fps",
        );
        assert!(
            p95_ms < FRAME_BUDGET_MS,
            "p95 resize frame time {p95_ms:.3} ms exceeds the 16.67 ms budget for stable 60 fps",
        );

        Ok(())
    }

    #[test]
    fn dev_workspace_exposes_retained_text_benchmark_view() -> Result<()> {
        let (workspace, _views) =
            build_dev_workspace_with_widget_book_bounds(Rect::new(24.0, 24.0, 680.0, 760.0));

        let retained_text_view = workspace
            .snapshots()
            .into_iter()
            .find(|view| view.title == RETAINED_TEXT_TAB_LABEL)
            .expect(
                "expected retained text benchmark view to be registered in the sui-dev workspace",
            );
        assert_eq!(retained_text_view.min_size, Size::new(320.0, 260.0));
        assert!(
            !retained_text_view.visible,
            "expected retained text benchmark view to be available from the sidebar without changing the default sui-dev layout",
        );

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for no-vsync full-workspace resize cost on the live path"]
    fn dev_workspace_button_grid_resize_live_no_vsync_benchmark() -> Result<()> {
        const DRAG_STEPS: usize = 36;
        const WARMUP_SAMPLES: usize = 6;

        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            BUTTON_GRID_TAB_LABEL,
        );
        let resize_start = Point::new(
            initial_view.bounds.max_x() - 8.0,
            initial_view.bounds.max_y() - 8.0,
        );
        let resize_end = Point::new(
            initial_view.bounds.x() + 820.0,
            initial_view.bounds.y() + 620.0,
        );

        let frame_samples =
            drag_pointer_with_samples(&window, resize_start, resize_end, DRAG_STEPS)?;
        let measured_samples = frame_samples
            .into_iter()
            .skip(WARMUP_SAMPLES)
            .collect::<Vec<_>>();
        assert!(
            !measured_samples.is_empty(),
            "expected no-vsync resize benchmark to record measured frame samples"
        );

        let frame_times_ms = measured_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let avg_visible_layers = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_widget_count = measured_samples
            .iter()
            .map(|sample| sample.scene.total_widget_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_repaint_boundary_count = measured_samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_stack_surface_count = measured_samples
            .iter()
            .map(|sample| sample.scene.stack_surface_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_overlay_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.overlay_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_direct_packets = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.direct_packet_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_count = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_acquire_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_present_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_present_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_draws = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.draw_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_uploaded_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.uploaded_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_instances = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_glyph_instance_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.command_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.text_command_count as f64)
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev Visible No-Vsync 64-Button Resize Benchmark ===");
        println!("frames measured:   {valid_count}");
        println!(
            "avg frame time:    {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:    {min_ms:.3} ms");
        println!("max frame time:    {max_ms:.3} ms");
        println!(
            "p95 frame time:    {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg scene cmds:    {avg_scene_commands:.2} ({avg_text_commands:.2} text)");
        println!("avg draws:         {avg_draws:.2}");
        println!("avg vertex bytes:  {:.0}", avg_uploaded_vertex_bytes);
        println!(
            "avg text bytes:    {:.0} ({avg_glyph_instances:.2} glyphs)",
            avg_text_vertex_bytes
        );
        println!("avg widgets:      {avg_widget_count:.2}");
        println!(
            "avg repaint(now): {avg_repaint_boundary_count:.2} ({avg_scene_layer_count:.2} scene | {avg_stack_surface_count:.2} stack | {avg_overlay_layer_count:.2} overlay)"
        );
        println!("avg visible layers:{avg_visible_layers:.2}");
        println!("avg direct packets:{avg_direct_packets:.2}");
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!(
            "avg packet build:  {avg_packet_build_ms:.3} ms ({avg_packet_build_count:.2} packets)"
        );
        println!(
            "avg packet why:    scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2} | coord {avg_packet_rebuild_coordinate_space:.2}"
        );
        println!(
            "avg surface:       acq {avg_surface_acquire_ms:.3} ms | pres {avg_surface_present_ms:.3} ms"
        );
        println!("==============================================\n");

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for dragging the large widget-book floating view in the full dev workspace"]
    fn dev_workspace_widget_book_drag_live_no_vsync_benchmark() -> Result<()> {
        const DRAG_STEPS: usize = 42;
        const WARMUP_SAMPLES: usize = 6;

        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);

        let initial_snapshot = window.snapshot()?;
        let widget_book_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            WIDGET_BOOK_TAB_LABEL,
        );
        let drag_start = Point::new(
            widget_book_view.bounds.x() + 64.0,
            widget_book_view.bounds.y() + 18.0,
        );
        let drag_end = Point::new(drag_start.x + 280.0, drag_start.y + 140.0);

        let frame_samples = drag_pointer_with_samples(&window, drag_start, drag_end, DRAG_STEPS)?;
        let measured_samples = frame_samples
            .into_iter()
            .skip(WARMUP_SAMPLES)
            .collect::<Vec<_>>();
        assert!(
            !measured_samples.is_empty(),
            "expected widget-book drag benchmark to record measured frame samples"
        );

        let frame_times_ms = measured_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let avg_visible_layers = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_widget_count = measured_samples
            .iter()
            .map(|sample| sample.scene.total_widget_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_repaint_boundary_count = measured_samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_stack_surface_count = measured_samples
            .iter()
            .map(|sample| sample.scene.stack_surface_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_overlay_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.overlay_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_direct_packets = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.direct_packet_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_count = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_acquire_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_present_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_present_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_draws = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.draw_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_uploaded_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.uploaded_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_instances = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_glyph_instance_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.command_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.text_command_count as f64)
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev Visible No-Vsync Widget-Book Drag Benchmark ===");
        println!("frames measured:   {valid_count}");
        println!(
            "avg frame time:    {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:    {min_ms:.3} ms");
        println!("max frame time:    {max_ms:.3} ms");
        println!(
            "p95 frame time:    {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg scene cmds:    {avg_scene_commands:.2} ({avg_text_commands:.2} text)");
        println!("avg draws:         {avg_draws:.2}");
        println!("avg vertex bytes:  {:.0}", avg_uploaded_vertex_bytes);
        println!(
            "avg text bytes:    {:.0} ({avg_glyph_instances:.2} glyphs)",
            avg_text_vertex_bytes
        );
        println!("avg widgets:      {avg_widget_count:.2}");
        println!(
            "avg repaint(now): {avg_repaint_boundary_count:.2} ({avg_scene_layer_count:.2} scene | {avg_stack_surface_count:.2} stack | {avg_overlay_layer_count:.2} overlay)"
        );
        println!("avg visible layers:{avg_visible_layers:.2}");
        println!("avg direct packets:{avg_direct_packets:.2}");
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!(
            "avg packet build:  {avg_packet_build_ms:.3} ms ({avg_packet_build_count:.2} packets)"
        );
        println!(
            "avg packet why:    scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2} | coord {avg_packet_rebuild_coordinate_space:.2}"
        );
        println!(
            "avg surface:       acq {avg_surface_acquire_ms:.3} ms | pres {avg_surface_present_ms:.3} ms"
        );
        println!("===============================================\n");

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for steady-state visible no-vsync redraw rate in the full dev workspace"]
    fn dev_workspace_idle_visible_no_vsync_benchmark() -> Result<()> {
        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);

        let initial = window.performance_snapshot()?;
        let sample_duration = Duration::from_millis(1500);
        thread::sleep(sample_duration);
        let final_snapshot = window.performance_snapshot()?;
        let elapsed_s = sample_duration.as_secs_f64();
        let frame_delta = final_snapshot
            .frame_index
            .saturating_sub(initial.frame_index);
        let fps = frame_delta as f64 / elapsed_s;

        println!("\n=== SUI Dev Visible No-Vsync Idle Benchmark ===");
        println!("sample duration:   {:.3} s", elapsed_s);
        println!("frame delta:       {frame_delta}");
        println!("observed fps:      {fps:.1}");
        println!("last frame time:   {:.3} ms", final_snapshot.total_time_ms);
        println!(
            "last surface:      acq {:.3} ms | pres {:.3} ms",
            final_snapshot.renderer_submission.surface_acquire_time_us as f64 / 1000.0,
            final_snapshot.renderer_submission.surface_present_time_us as f64 / 1000.0,
        );
        println!(
            "last packet build: {:.3} ms (scene {} | state {} | coord {})",
            final_snapshot
                .renderer_submission
                .retained_packet_build_time_us as f64
                / 1000.0,
            final_snapshot
                .renderer_submission
                .retained_packet_rebuild_scene_count,
            final_snapshot
                .renderer_submission
                .retained_packet_rebuild_state_count,
            final_snapshot
                .renderer_submission
                .retained_packet_rebuild_coordinate_space_count,
        );
        println!("==============================================\n");

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for visible no-vsync widget-book gallery scrolling in the full dev workspace"]
    fn dev_workspace_widget_book_scroll_live_no_vsync_benchmark() -> Result<()> {
        const TARGET_FRAMES: usize = 24;
        const MAX_SCROLL_INPUTS: usize = 96;
        const WARMUP_SAMPLES: usize = 4;
        const SCROLL_DELTA_Y: f32 = -80.0;

        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);

        let gallery = window
            .get_by_role(SemanticsRole::Window)
            .with_name(WIDGET_BOOK_TAB_LABEL)
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(sui_widget_book::GALLERY_SCROLL_NAME);

        let mut previous_frame_index = window.performance_snapshot()?.frame_index;
        let mut frame_samples = Vec::with_capacity(TARGET_FRAMES);
        let mut scroll_inputs = 0usize;
        while frame_samples.len() < TARGET_FRAMES && scroll_inputs < MAX_SCROLL_INPUTS {
            gallery.scroll_pixels(Vector::new(0.0, SCROLL_DELTA_Y))?;
            let _snapshot = window.snapshot()?;
            if let Some(performance) =
                wait_for_frame_advance(&window, previous_frame_index, Duration::from_millis(150))?
            {
                previous_frame_index = performance.frame_index;
                frame_samples.push(performance);
            }
            scroll_inputs += 1;
        }
        if frame_samples.len() < TARGET_FRAMES {
            println!("\n=== SUI Dev Visible No-Vsync Widget-Book Scroll Benchmark ===");
            println!("scroll inputs:     {scroll_inputs}");
            println!("frames captured:   {}", frame_samples.len());
            println!(
                "note: live TestApp scroll input did not publish enough post-scroll frames for a reliable benchmark; a desktop-host harness is likely required for this path"
            );
            println!("===============================================\n");
            return Ok(());
        }

        let measured_samples = frame_samples
            .into_iter()
            .skip(WARMUP_SAMPLES)
            .collect::<Vec<_>>();
        assert!(
            !measured_samples.is_empty(),
            "expected no-vsync widget-book scroll benchmark to record measured frame samples"
        );

        let frame_times_ms = measured_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let avg_visible_layers = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_widget_count = measured_samples
            .iter()
            .map(|sample| sample.scene.total_widget_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_repaint_boundary_count = measured_samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_stack_surface_count = measured_samples
            .iter()
            .map(|sample| sample.scene.stack_surface_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_overlay_layer_count = measured_samples
            .iter()
            .map(|sample| sample.scene.overlay_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_direct_packets = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.direct_packet_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_count = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_acquire_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_present_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_present_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_draws = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.draw_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_uploaded_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.uploaded_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_vertex_bytes = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_vertex_bytes as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_glyph_instances = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.text_glyph_instance_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.command_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_text_commands = measured_samples
            .iter()
            .map(|sample| sample.scene.text_command_count as f64)
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev Visible No-Vsync Widget-Book Scroll Benchmark ===");
        println!("frames measured:   {valid_count}");
        println!(
            "avg frame time:    {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:    {min_ms:.3} ms");
        println!("max frame time:    {max_ms:.3} ms");
        println!(
            "p95 frame time:    {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg scene cmds:    {avg_scene_commands:.2} ({avg_text_commands:.2} text)");
        println!("avg draws:         {avg_draws:.2}");
        println!("avg vertex bytes:  {:.0}", avg_uploaded_vertex_bytes);
        println!(
            "avg text bytes:    {:.0} ({avg_glyph_instances:.2} glyphs)",
            avg_text_vertex_bytes
        );
        println!("avg widgets:      {avg_widget_count:.2}");
        println!(
            "avg repaint(now): {avg_repaint_boundary_count:.2} ({avg_scene_layer_count:.2} scene | {avg_stack_surface_count:.2} stack | {avg_overlay_layer_count:.2} overlay)"
        );
        println!("avg visible layers:{avg_visible_layers:.2}");
        println!("avg direct packets:{avg_direct_packets:.2}");
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!(
            "avg packet build:  {avg_packet_build_ms:.3} ms ({avg_packet_build_count:.2} packets)"
        );
        println!(
            "avg packet why:    scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2} | coord {avg_packet_rebuild_coordinate_space:.2}"
        );
        println!(
            "avg surface:       acq {avg_surface_acquire_ms:.3} ms | pres {avg_surface_present_ms:.3} ms"
        );
        println!("===============================================\n");

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for real-time visible no-vsync resize pacing in the full dev workspace"]
    fn dev_workspace_button_grid_resize_realtime_visible_no_vsync_benchmark() -> Result<()> {
        const DRAG_STEPS: usize = 180;
        const INPUT_INTERVAL: Duration = Duration::from_millis(4);
        const TAIL_POLL_DURATION: Duration = Duration::from_millis(220);

        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
        ensure_live_overlay_detail_mode(&window)?;

        let root = window.root();
        root.dispatch_event(Event::Window(WindowEvent::Focused(true)))?;

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(
            &initial_snapshot,
            SemanticsRole::Window,
            BUTTON_GRID_TAB_LABEL,
        );
        let resize_start = Point::new(
            initial_view.bounds.max_x() - 8.0,
            initial_view.bounds.max_y() - 8.0,
        );
        let resize_end = Point::new(
            initial_view.bounds.x() + 820.0,
            initial_view.bounds.y() + 620.0,
        );
        let before_frame = window.capture_screenshot()?;

        let mut previous_frame_index = latest_published_frame(&window)?.frame_index;
        let mut frame_samples = Vec::new();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            resize_start,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, resize_start);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let benchmark_start = std::time::Instant::now();
        let total_delta = resize_end - resize_start;
        let mut previous_position = resize_start;
        for step in 1..=DRAG_STEPS {
            let progress = step as f32 / DRAG_STEPS as f32;
            let position = Point::new(
                resize_start.x + (total_delta.x * progress),
                resize_start.y + (total_delta.y * progress),
            );
            let mut moved = PointerEvent::new(PointerEventKind::Move, position);
            moved.buttons = PointerButtons::new(1);
            moved.delta = position - previous_position;
            root.dispatch_event(Event::Pointer(moved))?;
            previous_position = position;

            thread::sleep(INPUT_INTERVAL);
            if let Some(snapshot) = window_performance_snapshot(window.id())
                .filter(|snapshot| snapshot.frame_index > previous_frame_index)
            {
                previous_frame_index = snapshot.frame_index;
                frame_samples.push(snapshot);
            }
        }

        let mut up = PointerEvent::new(PointerEventKind::Up, resize_end);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up))?;

        let tail_deadline = std::time::Instant::now() + TAIL_POLL_DURATION;
        while std::time::Instant::now() < tail_deadline {
            thread::sleep(INPUT_INTERVAL);
            if let Some(snapshot) = window_performance_snapshot(window.id())
                .filter(|snapshot| snapshot.frame_index > previous_frame_index)
            {
                previous_frame_index = snapshot.frame_index;
                frame_samples.push(snapshot);
            }
        }

        let benchmark_elapsed_s = benchmark_start.elapsed().as_secs_f64();
        assert!(
            !frame_samples.is_empty(),
            "expected real-time resize benchmark to capture at least one published frame"
        );

        let after_snapshot = window.snapshot()?;
        let after_frame = window.capture_screenshot()?;
        assert!(
            pixel_diff_count(&before_frame, &after_frame) > 0,
            "expected real-time resize benchmark to change rendered pixels"
        );
        let resized_view = find_named_node(
            &after_snapshot,
            SemanticsRole::Window,
            BUTTON_GRID_TAB_LABEL,
        );
        assert!(
            resized_view.bounds.width() > initial_view.bounds.width() + 40.0,
            "expected button grid view to resize during the real-time benchmark, before={:?} after={:?}",
            initial_view.bounds,
            resized_view.bounds,
        );

        let frame_times_ms = frame_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let observed_fps = valid_count as f64 / benchmark_elapsed_s;
        let avg_surface_acquire_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_present_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_present_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev Realtime Visible No-Vsync 64-Button Resize Benchmark ===");
        println!("frames captured:   {valid_count}");
        println!("elapsed:           {:.3} s", benchmark_elapsed_s);
        println!("observed fps:      {observed_fps:.1}");
        println!(
            "avg frame time:    {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:    {min_ms:.3} ms");
        println!("max frame time:    {max_ms:.3} ms");
        println!(
            "p95 frame time:    {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!("avg packet build:  {avg_packet_build_ms:.3} ms");
        println!(
            "avg packet why:    scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2} | coord {avg_packet_rebuild_coordinate_space:.2}"
        );
        println!(
            "avg surface:       acq {avg_surface_acquire_ms:.3} ms | pres {avg_surface_present_ms:.3} ms"
        );
        println!("========================================================\n");

        Ok(())
    }

    #[test]
    #[ignore = "diagnostic benchmark for real-time visible no-vsync widget-book scrolling in the full dev workspace"]
    fn dev_workspace_widget_book_scroll_realtime_visible_no_vsync_benchmark() -> Result<()> {
        const SCROLL_EVENTS: usize = 160;
        const INPUT_INTERVAL: Duration = Duration::from_millis(8);
        const TAIL_POLL_DURATION: Duration = Duration::from_millis(220);
        const SCROLL_DELTA_Y: f32 = -48.0;

        let app = TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
        ensure_live_overlay_detail_mode(&window)?;

        let root = window.root();
        root.dispatch_event(Event::Window(WindowEvent::Focused(true)))?;

        let initial_snapshot = window.snapshot()?;
        let gallery = find_named_node(
            &initial_snapshot,
            SemanticsRole::ScrollView,
            sui_widget_book::GALLERY_SCROLL_NAME,
        );
        let scroll_point = Point::new(
            gallery.bounds.x() + gallery.bounds.width() * 0.5,
            gallery.bounds.y() + gallery.bounds.height() * 0.5,
        );
        let before_frame = window.capture_screenshot()?;

        let mut previous_frame_index = latest_published_frame(&window)?.frame_index;
        let mut frame_samples = Vec::new();

        let benchmark_start = std::time::Instant::now();
        for _ in 0..SCROLL_EVENTS {
            scroll_pointer(&window, scroll_point, Vector::new(0.0, SCROLL_DELTA_Y))?;

            thread::sleep(INPUT_INTERVAL);
            if let Some(snapshot) = window_performance_snapshot(window.id())
                .filter(|snapshot| snapshot.frame_index > previous_frame_index)
            {
                previous_frame_index = snapshot.frame_index;
                frame_samples.push(snapshot);
            }
        }

        let tail_deadline = std::time::Instant::now() + TAIL_POLL_DURATION;
        while std::time::Instant::now() < tail_deadline {
            thread::sleep(INPUT_INTERVAL);
            if let Some(snapshot) = window_performance_snapshot(window.id())
                .filter(|snapshot| snapshot.frame_index > previous_frame_index)
            {
                previous_frame_index = snapshot.frame_index;
                frame_samples.push(snapshot);
            }
        }

        let benchmark_elapsed_s = benchmark_start.elapsed().as_secs_f64();
        assert!(
            !frame_samples.is_empty(),
            "expected real-time widget-book scroll benchmark to capture at least one published frame"
        );

        let after_snapshot = window.snapshot()?;
        let after_frame = window.capture_screenshot()?;
        assert!(
            pixel_diff_count(&before_frame, &after_frame) > 0,
            "expected real-time widget-book scroll benchmark to change rendered pixels"
        );
        let after_gallery = find_named_node(
            &after_snapshot,
            SemanticsRole::ScrollView,
            sui_widget_book::GALLERY_SCROLL_NAME,
        );
        assert_eq!(
            after_gallery.bounds, gallery.bounds,
            "expected scrolling to keep the gallery viewport stable while its contents move"
        );

        let frame_times_ms = frame_samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        let valid_count = frame_times_ms.len();
        let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
        let avg_ms = total_frame_time_ms / valid_count as f64;
        let min_ms = frame_times_ms
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let max_ms = frame_times_ms
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);
        let mut sorted = frame_times_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
        let p95_ms = sorted[p95_index];
        let observed_fps = valid_count as f64 / benchmark_elapsed_s;
        let avg_surface_acquire_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_present_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_present_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_state_update_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_visible_layers = frame_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_widget_count = frame_samples
            .iter()
            .map(|sample| sample.scene.total_widget_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_repaint_boundary_count = frame_samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_scene_layer_count = frame_samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_stack_surface_count = frame_samples
            .iter()
            .map(|sample| sample.scene.stack_surface_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_overlay_layer_count = frame_samples
            .iter()
            .map(|sample| sample.scene.overlay_layer_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_scene = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuild_coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev Realtime Visible No-Vsync Widget-Book Scroll Benchmark ===");
        println!("frames captured:   {valid_count}");
        println!("elapsed:           {:.3} s", benchmark_elapsed_s);
        println!("observed fps:      {observed_fps:.1}");
        println!(
            "avg frame time:    {avg_ms:.3} ms ({:.0} fps)",
            1000.0 / avg_ms
        );
        println!("min frame time:    {min_ms:.3} ms");
        println!("max frame time:    {max_ms:.3} ms");
        println!(
            "p95 frame time:    {p95_ms:.3} ms ({:.0} fps)",
            1000.0 / p95_ms
        );
        println!("avg widgets:      {avg_widget_count:.2}");
        println!(
            "avg repaint(now): {avg_repaint_boundary_count:.2} ({avg_scene_layer_count:.2} scene | {avg_stack_surface_count:.2} stack | {avg_overlay_layer_count:.2} overlay)"
        );
        println!("avg visible layers:{avg_visible_layers:.2}");
        println!("avg state update:  {avg_state_update_ms:.3} ms");
        println!("avg packet build:  {avg_packet_build_ms:.3} ms");
        println!(
            "avg packet why:    scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2} | coord {avg_packet_rebuild_coordinate_space:.2}"
        );
        println!(
            "avg surface:       acq {avg_surface_acquire_ms:.3} ms | pres {avg_surface_present_ms:.3} ms"
        );
        println!("============================================================\n");

        Ok(())
    }

    fn viewport_bounds(snapshot: &WindowSnapshot) -> Rect {
        if let Some(scene) = &snapshot.scene_summary {
            return Rect::new(0.0, 0.0, scene.viewport.width, scene.viewport.height);
        }

        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Window && node.name.as_deref() == Some(WINDOW_TITLE)
            })
            .map(|node| node.bounds)
            .unwrap_or(Rect::new(0.0, 0.0, 1280.0, 720.0))
    }

    fn assert_story_exit_does_not_repaint_outside_view(
        window: &TestWindow,
        role: SemanticsRole,
        name: &str,
    ) -> Result<()> {
        let gallery = window
            .get_by_role(SemanticsRole::Window)
            .with_name(WIDGET_BOOK_TAB_LABEL)
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(sui_widget_book::GALLERY_SCROLL_NAME);

        scroll_story_until_visible(window, &gallery, role.clone(), name, 80)?;
        let before_snapshot = window.snapshot()?;
        let viewport = viewport_bounds(&before_snapshot);
        let view = find_named_node(
            &before_snapshot,
            SemanticsRole::Window,
            WIDGET_BOOK_TAB_LABEL,
        );
        let probes = leak_probe_regions(view.bounds, viewport);
        assert!(
            !probes.is_empty(),
            "expected probe regions around the shrunken widget book view, view={:?}, viewport={:?}",
            view.bounds,
            viewport,
        );

        let before_frame = window.capture_screenshot()?;

        scroll_story_until_hidden(window, &gallery, role.clone(), name, 120)?;

        let after_snapshot = window.snapshot()?;
        let after_frame = window.capture_screenshot()?;

        for probe in probes {
            let before_crop = before_frame.crop(scale_bounds_for_screenshot(
                probe,
                &before_snapshot,
                &before_frame,
            ))?;
            let after_crop = after_frame.crop(scale_bounds_for_screenshot(
                probe,
                &after_snapshot,
                &after_frame,
            ))?;
            let diff_count = pixel_diff_count(&before_crop, &after_crop);
            assert_eq!(
                diff_count, 0,
                "scrolling story {:?} named {:?} fully outside the widget book viewport changed pixels outside the floating view in probe {:?}",
                role, name, probe,
            );
        }

        Ok(())
    }

    fn scroll_story_until_visible(
        window: &TestWindow,
        gallery: &sui_testing::Locator,
        role: SemanticsRole,
        name: &str,
        max_steps: usize,
    ) -> Result<()> {
        for _ in 0..max_steps {
            let snapshot = window.snapshot()?;
            if let Some(story) = find_named_node_optional(&snapshot, role.clone(), name) {
                let gallery_bounds = find_named_node(
                    &snapshot,
                    SemanticsRole::ScrollView,
                    sui_widget_book::GALLERY_SCROLL_NAME,
                )
                .bounds;
                if visible_area_ratio(story.bounds, gallery_bounds) > 0.0 {
                    return Ok(());
                }
            }

            gallery.scroll_pixels(Vector::new(0.0, -120.0))?;
        }

        Err(sui::Error::new(format!(
            "failed to scroll story {:?} named {:?} into the widget book viewport",
            role, name,
        )))
    }

    fn scroll_story_until_hidden(
        window: &TestWindow,
        gallery: &sui_testing::Locator,
        role: SemanticsRole,
        name: &str,
        max_steps: usize,
    ) -> Result<()> {
        let mut last_observation = None;
        for _ in 0..max_steps {
            gallery.scroll_pixels(Vector::new(0.0, 24.0))?;

            let snapshot = window.snapshot()?;
            let Some(story) = find_named_node_optional(&snapshot, role.clone(), name) else {
                return Ok(());
            };
            let gallery_bounds = find_named_node(
                &snapshot,
                SemanticsRole::ScrollView,
                sui_widget_book::GALLERY_SCROLL_NAME,
            )
            .bounds;
            last_observation = Some((story.bounds, gallery_bounds));
            if visible_area_ratio(story.bounds, gallery_bounds) == 0.0 {
                return Ok(());
            }
        }

        let detail = last_observation
            .map(|(story_bounds, gallery_bounds)| {
                format!(
                    ", last story bounds={:?}, gallery bounds={:?}",
                    story_bounds, gallery_bounds
                )
            })
            .unwrap_or_default();

        Err(sui::Error::new(format!(
            "failed to scroll story {:?} named {:?} completely outside the widget book viewport{}",
            role, name, detail,
        )))
    }

    fn find_named_node_optional(
        snapshot: &WindowSnapshot,
        role: SemanticsRole,
        name: &str,
    ) -> Option<SemanticsNode> {
        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .cloned()
    }

    fn visible_area_ratio(bounds: Rect, viewport: Rect) -> f32 {
        let Some(visible) = bounds.intersection(viewport) else {
            return 0.0;
        };
        let bounds_area = bounds.width() * bounds.height();
        if bounds_area <= 0.0 {
            return 0.0;
        }
        (visible.width() * visible.height()) / bounds_area
    }

    fn leak_probe_regions(view_bounds: Rect, viewport: Rect) -> Vec<Rect> {
        let margin = 8.0;
        let thickness = 48.0;
        let mut probes = Vec::new();

        let left_probe = Rect::new(
            view_bounds.x() - margin - thickness,
            view_bounds.y() + 16.0,
            thickness,
            (view_bounds.height() - 32.0).max(24.0),
        );
        if let Some(probe) = left_probe.intersection(viewport) {
            if probe.width() >= 24.0 && probe.height() >= 24.0 {
                probes.push(probe);
            }
        }

        let top_probe = Rect::new(
            view_bounds.x() + 16.0,
            view_bounds.y() - margin - thickness,
            (view_bounds.width() - 32.0).max(24.0),
            thickness,
        );
        if let Some(probe) = top_probe.intersection(viewport) {
            if probe.width() >= 24.0 && probe.height() >= 24.0 {
                probes.push(probe);
            }
        }

        let right_probe = Rect::new(
            view_bounds.max_x() + margin,
            view_bounds.y() + 16.0,
            thickness,
            (view_bounds.height() - 32.0).max(24.0),
        );
        if let Some(probe) = right_probe.intersection(viewport) {
            if probe.width() >= 24.0 && probe.height() >= 24.0 {
                probes.push(probe);
            }
        }

        let bottom_probe = Rect::new(
            view_bounds.x() + 16.0,
            view_bounds.max_y() + margin,
            (view_bounds.width() - 32.0).max(24.0),
            thickness,
        );
        if let Some(probe) = bottom_probe.intersection(viewport) {
            if probe.width() >= 24.0 && probe.height() >= 24.0 {
                probes.push(probe);
            }
        }

        probes
    }

    fn scale_bounds_for_screenshot(
        bounds: Rect,
        snapshot: &WindowSnapshot,
        screenshot: &Screenshot,
    ) -> Rect {
        let Some(scene) = &snapshot.scene_summary else {
            return bounds;
        };
        let viewport = scene.viewport;
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return bounds;
        }

        let scale_x = screenshot.width() as f32 / viewport.width;
        let scale_y = screenshot.height() as f32 / viewport.height;
        Rect::new(
            bounds.x() * scale_x,
            bounds.y() * scale_y,
            bounds.width() * scale_x,
            bounds.height() * scale_y,
        )
    }

    fn pixel_diff_count(left: &Screenshot, right: &Screenshot) -> usize {
        assert_eq!(left.width(), right.width(), "screenshot widths differ");
        assert_eq!(left.height(), right.height(), "screenshot heights differ");

        left.pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .filter(|(left_pixel, right_pixel)| left_pixel != right_pixel)
            .count()
    }

    fn overlap_probe(first: Rect, second: Rect) -> Rect {
        let overlap = first
            .intersection(second)
            .expect("floating views should overlap for the probe");
        Rect::new(overlap.x() + 24.0, overlap.y() + 48.0, 1.0, 1.0)
    }

    fn sample_pixel(
        screenshot: &Screenshot,
        bounds: Rect,
        snapshot: &WindowSnapshot,
    ) -> Result<[u8; 4]> {
        let pixel = screenshot.crop(scale_bounds_for_screenshot(bounds, snapshot, screenshot))?;
        let rgba = pixel.pixels();
        Ok([rgba[0], rgba[1], rgba[2], rgba[3]])
    }
}
