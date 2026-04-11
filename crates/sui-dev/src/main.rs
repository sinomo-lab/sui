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

fn build_dev_application_with_widget_book_bounds(widget_book_bounds: Rect) -> Application {
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
        FloatingViewConfig::new(SETTINGS_TAB_LABEL, Rect::new(420.0, 440.0, 420.0, 320.0))
            .min_size(Size::new(300.0, 240.0)),
        build_render_settings_tab(),
    );

    let root = SplitView::horizontal(ViewSidebar::new(workspace.clone()), views)
        .name("Development workspace split")
        .ratio(0.24)
        .min_first(236.0)
        .min_second(420.0)
        .divider_thickness(12.0);

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

fn build_dev_application() -> Application {
    build_dev_application_with_widget_book_bounds(Rect::new(24.0, 24.0, 680.0, 760.0))
}

fn main() -> sui::Result<()> {
    build_dev_application().run()
}

#[cfg(test)]
mod tests {
    use super::*;

    use sui::{
        Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect,
        Result, SceneStatisticsDetailMode, SemanticsNode, SemanticsRole,
        WindowPerformanceSnapshot, Vector, set_window_scene_statistics_detail_mode,
    };
    use sui_testing::{Screenshot, TestApp, TestWindow, WindowSnapshot};

    #[test]
    fn widget_book_scroll_does_not_repaint_pixels_outside_shrunken_floating_view() -> Result<()> {
        let initial_bounds = Rect::new(320.0, 28.0, 560.0, 520.0);
        let app = TestApp::new(move || {
            build_dev_application_with_widget_book_bounds(initial_bounds).build()
        })?;
        let window = app.main_window()?;

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(&initial_snapshot, SemanticsRole::Window, WIDGET_BOOK_TAB_LABEL);
        let resize_start = Point::new(initial_view.bounds.max_x() - 8.0, initial_view.bounds.max_y() - 8.0);
        let resize_end = Point::new(initial_view.bounds.x() + 420.0, initial_view.bounds.y() + 328.0);
        drag_pointer(&window, resize_start, resize_end)?;

        let before_snapshot = window.snapshot()?;
        let view = find_named_node(&before_snapshot, SemanticsRole::Window, WIDGET_BOOK_TAB_LABEL);
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
                diff_count,
                0,
                "scrolling inside the shrunken widget book view changed pixels outside the view bounds in probe {:?}",
                probe,
            );
        }

        Ok(())
    }

    #[test]
    fn widget_book_image_and_swatch_stories_do_not_leak_outside_shrunken_floating_view() -> Result<()> {
        let initial_bounds = Rect::new(320.0, 28.0, 560.0, 520.0);
        let app = TestApp::new(move || {
            build_dev_application_with_widget_book_bounds(initial_bounds).build()
        })?;
        let window = app.main_window()?;

        let initial_snapshot = window.snapshot()?;
        let initial_view = find_named_node(&initial_snapshot, SemanticsRole::Window, WIDGET_BOOK_TAB_LABEL);
        let resize_start = Point::new(initial_view.bounds.max_x() - 8.0, initial_view.bounds.max_y() - 8.0);
        let resize_end = Point::new(initial_view.bounds.x() + 420.0, initial_view.bounds.y() + 328.0);
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

    fn drag_pointer(window: &TestWindow, from: Point, to: Point) -> Result<()> {
        drag_pointer_with_samples(window, from, to, 1).map(|_| ())
    }

    fn drag_pointer_with_samples(
        window: &TestWindow,
        from: Point,
        to: Point,
        steps: usize,
    ) -> Result<Vec<WindowPerformanceSnapshot>> {
        assert!(steps > 0, "drag steps must be greater than zero");

        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(PointerEventKind::Move, from)))?;

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
        root.dispatch_event(Event::Pointer(up))
            .map(|_| samples)
    }

    fn find_named_node(snapshot: &WindowSnapshot, role: SemanticsRole, name: &str) -> SemanticsNode {
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
        let initial_view =
            find_named_node(&initial_snapshot, SemanticsRole::Window, BUTTON_GRID_TAB_LABEL);
        let resize_start =
            Point::new(initial_view.bounds.max_x() - 8.0, initial_view.bounds.max_y() - 8.0);
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
        let resized_view = find_named_node(&after_snapshot, SemanticsRole::Window, BUTTON_GRID_TAB_LABEL);
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
        let avg_visible_tiles = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_tile_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_regenerated_tiles = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.regenerated_tile_count as f64)
            .sum::<f64>()
            / valid_count as f64;
        let avg_tile_generation_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.tile_generation_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_build_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;
        let avg_surface_acquire_ms = measured_samples
            .iter()
            .map(|sample| sample.renderer_submission.surface_acquire_time_us as f64 / 1000.0)
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Dev 64-Button Resize Benchmark ===");
        println!("frames measured:  {valid_count}");
        println!("avg frame time:   {avg_ms:.3} ms ({:.0} fps)", 1000.0 / avg_ms);
        println!("min frame time:   {min_ms:.3} ms");
        println!("max frame time:   {max_ms:.3} ms");
        println!("p95 frame time:   {p95_ms:.3} ms ({:.0} fps)", 1000.0 / p95_ms);
        println!("avg visible tiles:{avg_visible_tiles:.2}");
        println!("avg regen tiles:  {avg_regenerated_tiles:.2}");
        println!("avg tile gen:     {avg_tile_generation_ms:.3} ms");
        println!("avg packet build: {avg_packet_build_ms:.3} ms");
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

    fn viewport_bounds(snapshot: &WindowSnapshot) -> Rect {
        if let Some(scene) = &snapshot.scene_summary {
            return Rect::new(0.0, 0.0, scene.viewport.width, scene.viewport.height);
        }

        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == SemanticsRole::Window && node.name.as_deref() == Some(WINDOW_TITLE))
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
        let view = find_named_node(&before_snapshot, SemanticsRole::Window, WIDGET_BOOK_TAB_LABEL);
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
                diff_count,
                0,
                "scrolling story {:?} named {:?} fully outside the widget book viewport changed pixels outside the floating view in probe {:?}",
                role,
                name,
                probe,
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
}
