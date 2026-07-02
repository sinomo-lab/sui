#![forbid(unsafe_code)]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{OnceLock, RwLock},
};

use sui::prelude::*;
use sui::{
    FramePhase, HdrLuminanceTokens, HdrThemeMode, HdrThemeTokens, InvalidationKind,
    InvalidationRequest, InvalidationTarget, PointerEventKind, Rect, SceneStatisticsDetailMode,
    SemanticColorToken, SemanticsNode, SemanticsRole, SemanticsValue, TextDirection, TextStyle,
    TextSurface, TextSurfaceOverlayKind, TextSurfaceStyleOverlay, TextSurfaceStyleSpan, TextWrap,
    ThemeColorScheme, ThemeDensity, Vector, WidgetColorRole, WidgetLuminanceRole,
    WidgetMaterialRole, WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent,
    WindowPerformanceSnapshot, resolve_semantic_color, resolve_widget_hdr_style,
    set_window_scene_statistics_detail_mode, window_performance_snapshot,
    window_scene_statistics_detail_mode,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::{LayerCompositionMode, LayerProperties};

#[cfg(feature = "artifacts")]
mod visual_artifacts;

#[cfg(feature = "artifacts")]
pub use visual_artifacts::write_visual_artifacts;

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
pub const RETAINED_TEXT_BENCHMARK_TITLE: &str = "SUI Retained Text Scroll Benchmark";
pub const ANIMATION_BENCHMARK_TITLE: &str = "SUI Animation Benchmark";
pub const TEXT_RENDERING_COMPARISON_TITLE: &str = "SUI Text Rendering Comparison";
pub const COLOR_VALIDATION_VIEW_TITLE: &str = "SUI HDR and Color Validation";
pub const TEXT_VALIDATION_VIEW_TITLE: &str = "SUI Text Validation";
pub const TEXT_EDITING_BENCHMARK_TITLE: &str = "SUI Text Editing Benchmark";
pub const NAME_INPUT_LABEL: &str = "Name";
pub const TEXT_AREA_LABEL: &str = "Notes";
pub const SUBSCRIBE_LABEL: &str = "Subscribe to product updates";
pub const PRIMARY_BUTTON_LABEL: &str = "Trigger action";
pub const TOOLBAR_SEPARATOR_NAME: &str = "Toolbar divider";
pub const ICON_LABEL: &str = "Search icon";
pub const ICON_BUTTON_LABEL: &str = "More actions";
pub const SWITCH_LABEL: &str = "Enable snapping";
pub const RADIO_BUTTON_LABEL: &str = "Standalone radio sample";
pub const RADIO_GROUP_NAME: &str = "Render quality";
pub const SLIDER_NAME: &str = "Opacity";
pub const NUMBER_INPUT_NAME: &str = "Brush size";
pub const SELECT_NAME: &str = "Blend mode";
pub const TAB_BAR_NAME: &str = "Workspace tabs";
pub const TABS_NAME: &str = "Inspector tabs";
pub const MENU_NAME: &str = "Command menu";
pub const CONTEXT_MENU_NAME: &str = "Layer context menu";
pub const TOOLTIP_TRIGGER_LABEL: &str = "Hover for shortcuts";
pub const TOOLTIP_TEXT: &str = "Quick access to common commands";
pub const POPOVER_NAME: &str = "Inline inspector";
pub const POPOVER_TRIGGER_LABEL: &str = "Open inspector";
pub const WIDGET_STATES_GALLERY_NAME: &str = "Widget States";
pub const WIDGET_STATES_BUTTON_LABEL: &str = "States button";
pub const WIDGET_STATES_ICON_BUTTON_LABEL: &str = "States icon action";
pub const WIDGET_STATES_TEXT_INPUT_LABEL: &str = "States text input";
pub const WIDGET_STATES_TEXT_AREA_LABEL: &str = "States text area";
pub const WIDGET_STATES_SELECT_NAME: &str = "States select";
pub const WIDGET_STATES_CHECKBOX_LABEL: &str = "States checkbox";
pub const WIDGET_STATES_SWITCH_LABEL: &str = "States switch";
pub const WIDGET_STATES_SLIDER_NAME: &str = "States slider";
pub const WIDGET_STATES_TABS_NAME: &str = "States tabs";
pub const WIDGET_STATES_MENU_NAME: &str = "States menu";
pub const WIDGET_STATES_POPOVER_NAME: &str = "States popover";
const WIDGET_STATE_ROW_WIDTH: f32 = 952.0;
const WIDGET_STATE_COLUMN_WIDTH: f32 = 447.0;
pub const SIZE_PRESETS_GALLERY_NAME: &str = "Size presets";
pub const SIZE_PRESET_COMPACT_ACTION_LABEL: &str = "Compact preset action";
pub const SIZE_PRESET_COMFORTABLE_ACTION_LABEL: &str = "Comfortable preset action";
pub const SIZE_PRESET_TOUCH_ACTION_LABEL: &str = "Touch preset action";
pub const SIZE_PRESET_COMPACT_INPUT_LABEL: &str = "Compact preset input";
pub const SIZE_PRESET_COMFORTABLE_INPUT_LABEL: &str = "Comfortable preset input";
pub const SIZE_PRESET_TOUCH_INPUT_LABEL: &str = "Touch preset input";
pub const DIALOG_TITLE: &str = "Project settings";
pub const DIALOG_TRIGGER_LABEL: &str = "Toggle project settings";
pub const PROGRESS_NAME: &str = "Export progress";
pub const SPINNER_NAME: &str = "Background work";
pub const SUMMARY_NAME: &str = "Widget book summary";
pub const GALLERY_SCROLL_NAME: &str = "Widget book gallery";
pub const GALLERY_SCROLL_BAR_NAME: &str = "Widget book scroll bar";
const GALLERY_TEXT_MAX_WIDTH: f32 = 980.0;
const ROOT_GALLERY_PADDING: Insets = Insets {
    left: 24.0,
    top: 0.0,
    right: 24.0,
    bottom: 0.0,
};
pub const THEME_DEMO_TITLE: &str = "Themes";
pub const THEME_DEMO_DESCRIPTION: &str =
    "Dedicated theme previews for SDR, wide-gamut, and HDR UI styling.";
pub const THEME_DEMO_SCROLL_NAME: &str = "Theme demo gallery";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_NAME: &str = "Retained text benchmark scroll";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME: &str =
    "Retained text benchmark vertical scroll bar";
pub const TEXT_RENDERING_COMPARISON_SCROLL_NAME: &str = "Text rendering comparison scroll";
pub const TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME: &str =
    "Text rendering comparison vertical scroll bar";
pub const TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME: &str =
    "Text rendering comparison horizontal scroll bar";
pub const COLOR_VALIDATION_SCROLL_NAME: &str = "Color validation scroll";
pub const COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME: &str = "Color validation vertical scroll bar";
pub const COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME: &str =
    "Color validation horizontal scroll bar";
pub const TEXT_VALIDATION_SCROLL_NAME: &str = "Text validation scroll";
pub const TEXT_VALIDATION_EDITOR_NAME: &str = "Validation editor";
pub const TEXT_EDITING_BENCHMARK_SPLIT_NAME: &str = "Text editing benchmark split";
pub const TEXT_EDITING_BENCHMARK_EDITOR_NAME: &str = "Text editing benchmark editor";
pub const TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME: &str = "Text editing benchmark syntax preview";
pub const THEME_PREVIEW_NAME: &str = "Theme preview showcase";
pub const THEME_PREVIEW_TOGGLE_LABEL: &str = "Compare default themes";
pub const LIGHT_THEME_PREVIEW_CARD_NAME: &str = "Light theme preview card";
pub const DARK_THEME_PREVIEW_CARD_NAME: &str = "Dark theme preview card";
pub const HIGH_CONTRAST_THEME_PREVIEW_CARD_NAME: &str = "High contrast theme preview card";
pub const HDR_THEME_LAB_NAME: &str = "HDR theme mode lab";
pub const HDR_THEME_LAB_ACTIVE_PREVIEW_NAME: &str = "Current HDR theme mode preview";
pub const LIGHT_PREVIEW_ACTION_LABEL: &str = "Light preview action";
pub const DARK_PREVIEW_ACTION_LABEL: &str = "Dark preview action";
pub const HIGH_CONTRAST_PREVIEW_ACTION_LABEL: &str = "High contrast preview action";
pub const LIGHT_PREVIEW_INPUT_LABEL: &str = "Light preview query";
pub const DARK_PREVIEW_INPUT_LABEL: &str = "Dark preview query";
pub const HIGH_CONTRAST_PREVIEW_INPUT_LABEL: &str = "High contrast preview query";
pub const LIST_VIEW_NAME: &str = "Assets list";
pub const TREE_VIEW_NAME: &str = "Scene tree";
pub const TABLE_NAME: &str = "Material table";
pub const SPLIT_VIEW_NAME: &str = "Editor split";
pub const BREADCRUMB_NAME: &str = "Project path";
pub const COLOR_SWATCH_NAME: &str = "Primary swatch";
pub const COLOR_PICKER_NAME: &str = "Accent picker";
pub const DEMO_IMAGE_LABEL: &str = "Preview image";
pub const ANIMATION_DEMO_NAME: &str = "Animation demo";
pub const ANIMATION_DEMO_BUTTON_LABEL: &str = "Animation demo action";
pub const ANIMATION_DEMO_SWITCH_LABEL: &str = "Animation demo switch";
pub const ANIMATION_DEMO_TEXT_INPUT_LABEL: &str = "Animation demo query";
pub const ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL: &str = "Animation demo shortcuts";
pub const ANIMATION_DEMO_TOOLTIP_TEXT: &str =
    "Tooltip entry motion uses retained translation and opacity";
pub const ANIMATION_DEMO_POPOVER_NAME: &str = "Animation demo inspector";
pub const ANIMATION_DEMO_POPOVER_TRIGGER_LABEL: &str = "Open animation demo inspector";
pub const TIMELINE_ANIMATION_PREVIEW_NAME: &str = "Timeline animation preview";
pub const ANIMATION_EDITOR_SURFACE_NAME: &str = "Animation editor surface";
pub const ANIMATION_BENCHMARK_RETAINED_NAME: &str = "Animation benchmark retained lane";
pub const ANIMATION_BENCHMARK_REPAINT_NAME: &str = "Animation benchmark repaint lane";
pub const ANIMATION_BENCHMARK_SCALE_NAME: &str = "Animation benchmark scale grid";

const WIDGET_BOOK_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);
const TIMELINE_ANIMATION_PREVIEW_TARGET: &str = "timeline-preview";
const TIMELINE_ANIMATION_PREVIEW_RADIUS_PATH: &str = "paint.radius";
const ANIMATION_BENCHMARK_RETAINED_TARGET: &str = "animation-benchmark-retained";
const ANIMATION_BENCHMARK_REPAINT_TARGET: &str = "animation-benchmark-repaint";
const ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX: &str = "animation-benchmark-cell-";
const ANIMATION_BENCHMARK_RADIUS_PATH: &str = "paint.radius";
const ANIMATION_BENCHMARK_ALPHA_PATH: &str = "paint.alpha";
const ANIMATION_BENCHMARK_SCALE_CELLS: usize = 96;
const ANIMATION_BENCHMARK_SCALE_COLUMNS: usize = 12;

const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
const BLEND_MODE_OPTIONS: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];
const TAB_BAR_OPTIONS: [&str; 3] = ["Canvas", "Inspector", "Export"];
const TAB_PANEL_OPTIONS: [&str; 3] = ["Layout", "Data", "History"];
const TEXT_RENDERING_COMPARISON_MIN_WIDTH: f32 = 1094.0;
const TEXT_RENDERING_COMPARISON_CARD_WIDTH: f32 = 520.0;
const TEXT_RENDERING_SAMPLE_TILE_WIDTH: f32 = 232.0;
const TEXT_RENDERING_SAMPLE_TILE_HEIGHT: f32 = 108.0;
const TEXT_VALIDATION_CONTENT_WIDTH: f32 = 1040.0;
const TEXT_VALIDATION_PROBE_CARD_WIDTH: f32 = 320.0;

pub type WidgetBookThemeReader = Rc<dyn Fn() -> DefaultTheme>;

fn default_widget_book_theme_reader() -> WidgetBookThemeReader {
    Rc::new(DefaultTheme::default)
}

fn clone_widget_book_theme_reader(
    theme_reader: &WidgetBookThemeReader,
) -> impl Fn() -> DefaultTheme + 'static {
    let theme_reader = Rc::clone(theme_reader);
    move || theme_reader()
}

fn widget_book_density_theme_reader(
    theme_reader: &WidgetBookThemeReader,
    density: ThemeDensity,
) -> impl Fn() -> DefaultTheme + 'static {
    let theme_reader = Rc::clone(theme_reader);
    move || theme_reader().with_density(density)
}

fn widget_book_theme_color<F>(
    theme_reader: &WidgetBookThemeReader,
    color: F,
) -> impl Fn() -> Color + 'static
where
    F: Fn(DefaultTheme) -> Color + 'static,
{
    let theme_reader = Rc::clone(theme_reader);
    move || color(theme_reader())
}
const TEXT_RENDERING_MODE_DATA: [(&str, &str, &str); 6] = [
    (
        "Grayscale baseline",
        "Control sample for coverage-only text.",
        "Dark and light samples should match perceived weight without extra policy adjustments.",
    ),
    (
        "Grayscale + hinting",
        "Small UI text snapped to the pixel grid.",
        "Check that 11-14 px labels become steadier without shifting medium-size copy.",
    ),
    (
        "Grayscale + stem darkening",
        "Thin strokes receive a restrained weight boost.",
        "Look for stronger stems while avoiding bold-looking captions on bright surfaces.",
    ),
    (
        "LCD subpixel",
        "Axis-aligned text can use subpixel coverage.",
        "Inspect fine edge detail and keep color fringing under control on neutral text.",
    ),
    (
        "LCD subpixel + hinting",
        "Subpixel coverage with small-text grid fitting.",
        "Use this as the practical UI-label candidate for dense toolbars and status rows.",
    ),
    (
        "LCD subpixel + hinting + stem darkening",
        "Most assertive small-text rendering policy.",
        "Validate tiny labels first, then confirm body-size text still feels neutral.",
    ),
];

fn hdr_theme_lab_mode_store() -> &'static RwLock<HdrThemeMode> {
    static STORE: OnceLock<RwLock<HdrThemeMode>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HdrThemeMode::Disabled))
}

pub fn widget_book_hdr_theme_mode() -> HdrThemeMode {
    *hdr_theme_lab_mode_store()
        .read()
        .expect("widget-book HDR theme mode lock should not be poisoned")
}

pub fn set_widget_book_hdr_theme_mode(mode: HdrThemeMode) {
    *hdr_theme_lab_mode_store()
        .write()
        .expect("widget-book HDR theme mode lock should not be poisoned") = mode;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WidgetBookState {
    pub name: String,
    pub subscribed: bool,
    pub theme_preview_comparison: bool,
    pub button_presses: usize,
    pub icon_button_presses: usize,
    pub switch_on: bool,
    pub standalone_radio_selected: bool,
    pub radio_choice: String,
    pub slider_value: f64,
    pub number_value: f64,
    pub notes: String,
    pub mode: String,
    pub tab_bar_choice: String,
    pub tabs_choice: String,
    pub last_menu_action: String,
    pub last_context_action: String,
    pub dialog_apply_count: usize,
}

pub struct LivePerformanceRoot {
    content: SingleChild,
    performance_overlay: SingleChild,
    performance_display: Rc<RefCell<LivePerformanceDisplay>>,
    watched_state: Option<Rc<RefCell<WidgetBookState>>>,
    last_seen_state: Option<WidgetBookState>,
    window_title: String,
    window_description: String,
    overlay_enabled: bool,
    overlay_enabled_reader: Option<Rc<dyn Fn() -> bool>>,
    last_overlay_enabled: bool,
    owns_detail_mode: bool,
}

impl LivePerformanceRoot {
    const OVERLAY_MARGIN: Insets = Insets {
        left: 0.0,
        top: 18.0,
        right: 18.0,
        bottom: 0.0,
    };

    pub fn new<Content>(
        window_title: impl Into<String>,
        window_description: impl Into<String>,
        content: Content,
    ) -> Self
    where
        Content: Widget + 'static,
    {
        let performance_display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        Self {
            content: SingleChild::new(content),
            performance_overlay: SingleChild::new(LivePerformancePanel::with_display(Rc::clone(
                &performance_display,
            ))),
            performance_display,
            watched_state: None,
            last_seen_state: None,
            window_title: window_title.into(),
            window_description: window_description.into(),
            overlay_enabled: false,
            overlay_enabled_reader: None,
            last_overlay_enabled: false,
            owns_detail_mode: false,
        }
    }

    pub fn show_performance_overlay(mut self) -> Self {
        self.overlay_enabled = true;
        self
    }

    pub fn performance_overlay_enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.overlay_enabled_reader = Some(Rc::new(enabled));
        self
    }

    pub fn watch_widget_book_state(mut self, state: Rc<RefCell<WidgetBookState>>) -> Self {
        self.last_seen_state = Some(state.borrow().clone());
        self.watched_state = Some(state);
        self
    }

    fn overlay_enabled(&self) -> bool {
        self.overlay_enabled
            || self
                .overlay_enabled_reader
                .as_ref()
                .map_or(false, |enabled| enabled())
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSnapshot>,
        idle: bool,
    ) -> bool {
        let mut samples = self.performance_display.borrow().samples.clone();
        if let Some(snapshot) = &snapshot {
            if samples
                .last()
                .map_or(true, |sample| sample.frame_index != snapshot.frame_index)
            {
                samples.push(LivePerformanceFrameSample::from_snapshot(snapshot));
                if samples.len() > LIVE_PERFORMANCE_HISTORY_LIMIT {
                    let overflow = samples.len() - LIVE_PERFORMANCE_HISTORY_LIMIT;
                    samples.drain(0..overflow);
                }
            }
        } else {
            samples.clear();
        }

        let next = LivePerformanceDisplay {
            snapshot,
            idle,
            samples,
        };
        let mut display = self.performance_display.borrow_mut();
        if *display == next {
            return false;
        }

        *display = next;
        true
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct LivePerformanceDisplay {
    snapshot: Option<WindowPerformanceSnapshot>,
    idle: bool,
    samples: Vec<LivePerformanceFrameSample>,
}

const LIVE_PERFORMANCE_HISTORY_LIMIT: usize = 72;
const LIVE_PERFORMANCE_STAGE_COUNT: usize = 9;

#[derive(Debug, Clone, PartialEq)]
struct LivePerformanceFrameSample {
    frame_index: u64,
    total_time_ms: f32,
    stage_costs: [f32; LIVE_PERFORMANCE_STAGE_COUNT],
}

impl LivePerformanceFrameSample {
    fn from_snapshot(snapshot: &WindowPerformanceSnapshot) -> Self {
        let mut stage_costs = [0.0; LIVE_PERFORMANCE_STAGE_COUNT];
        for timing in &snapshot.phase_timings {
            stage_costs[frame_phase_index(timing.phase)] += timing.duration_ms.max(0.0) as f32;
        }

        if snapshot.phase_timings.is_empty() {
            stage_costs[frame_phase_index(FramePhase::Renderer)] = snapshot.total_time_ms as f32;
        }

        Self {
            frame_index: snapshot.frame_index,
            total_time_ms: snapshot.total_time_ms.max(0.0) as f32,
            stage_costs,
        }
    }
}

const fn frame_phase_index(phase: FramePhase) -> usize {
    match phase {
        FramePhase::Event => 0,
        FramePhase::Redraw => 1,
        FramePhase::MeasureArrange => 2,
        FramePhase::HitTest => 3,
        FramePhase::Paint => 4,
        FramePhase::Semantics => 5,
        FramePhase::Renderer => 6,
        FramePhase::SurfaceWait => 7,
        FramePhase::Diagnostics => 8,
    }
}

pub fn default_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Ada".to_string(),
        subscribed: true,
        theme_preview_comparison: true,
        button_presses: 0,
        icon_button_presses: 0,
        switch_on: true,
        standalone_radio_selected: false,
        radio_choice: RADIO_OPTIONS[0].to_string(),
        slider_value: 72.0,
        number_value: 12.0,
        notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
        mode: BLEND_MODE_OPTIONS[0].to_string(),
        tab_bar_choice: TAB_BAR_OPTIONS[0].to_string(),
        tabs_choice: TAB_PANEL_OPTIONS[0].to_string(),
        last_menu_action: "New tab".to_string(),
        last_context_action: "Rename".to_string(),
        dialog_apply_count: 0,
    }))
}

/// Register the images used by [`build_widget_book_gallery`] onto the given
/// application. Call this while configuring app resources when you are
/// assembling the application yourself rather than using
/// [`build_widget_book_application`].
pub fn register_widget_book_images(resources: &mut ResourceRegistry<'_>) {
    resources
        .image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");
}

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);

    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            Window::new(WINDOW_TITLE).root(
                LivePerformanceRoot::new(
                    WINDOW_TITLE,
                    WINDOW_DESCRIPTION,
                    build_widget_book_gallery(Rc::clone(&state)),
                )
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

pub fn build_theme_demo_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);

    App::new()
        .window(
            Window::new(THEME_DEMO_TITLE).root(
                LivePerformanceRoot::new(
                    THEME_DEMO_TITLE,
                    THEME_DEMO_DESCRIPTION,
                    build_theme_demo_surface(Rc::clone(&state)),
                )
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

#[cfg(feature = "native")]
pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for LivePerformanceRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            let overlay_enabled = self.overlay_enabled();
            if overlay_enabled != self.last_overlay_enabled {
                self.last_overlay_enabled = overlay_enabled;
                ctx.request_measure();
                ctx.request_semantics();
                ctx.request_paint();

                if !overlay_enabled {
                    self.set_performance_display(None, true);
                    if self.owns_detail_mode {
                        set_window_scene_statistics_detail_mode(
                            ctx.window_id(),
                            SceneStatisticsDetailMode::Lightweight,
                        );
                        self.owns_detail_mode = false;
                    }
                }
            }

            if let Some(state) = &self.watched_state {
                let next_state = state.borrow().clone();
                if self.last_seen_state.as_ref() != Some(&next_state) {
                    self.last_seen_state = Some(next_state);
                    let content_id = self.content.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Semantics,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                }
            }

            if overlay_enabled {
                if !window_scene_statistics_detail_mode(ctx.window_id()).is_detailed() {
                    set_window_scene_statistics_detail_mode(
                        ctx.window_id(),
                        SceneStatisticsDetailMode::Detailed,
                    );
                    self.owns_detail_mode = true;
                }

                if let Some(snapshot) = window_performance_snapshot(ctx.window_id()) {
                    if self.set_performance_display(Some(snapshot), false) {
                        let overlay_id = self.performance_overlay.child().id();
                        ctx.request(
                            InvalidationRequest::new(
                                InvalidationTarget::Widget(overlay_id),
                                InvalidationKind::Paint,
                            )
                            .with_region(self.performance_overlay.child().bounds()),
                        );
                    }
                }
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1280.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                720.0
            },
        ));
        self.content.measure(ctx, Constraints::tight(viewport));
        if self.overlay_enabled() {
            self.performance_overlay
                .measure(ctx, Constraints::new(Size::ZERO, viewport));
        }
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, Rect::from_origin_size(bounds.origin, bounds.size));

        if self.overlay_enabled() {
            let overlay_size = self.performance_overlay.child().measured_size();
            let overlay_x = (bounds.max_x() - overlay_size.width - Self::OVERLAY_MARGIN.right)
                .max(bounds.x() + Self::OVERLAY_MARGIN.left);
            let overlay_y = bounds.y() + Self::OVERLAY_MARGIN.top;
            self.performance_overlay.arrange(
                ctx,
                Rect::from_origin_size(Point::new(overlay_x, overlay_y), overlay_size),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.95, 0.968, 0.985, 1.0));
        self.content.paint(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(self.window_title.clone());
        root.description = Some(self.window_description.clone());
        ctx.push(root);
        self.content.semantics(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children_mut(visitor);
        }
    }
}

struct ProjectSettingsPreview {
    trigger: SingleChild,
    dialog: SingleChild,
    dialog_open: bool,
    trigger_pressed: bool,
}

struct WidgetBookGalleryScrollPane {
    spacing: f32,
    content: SingleChild,
    scroll_bar: SingleChild,
}

impl WidgetBookGalleryScrollPane {
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

impl Widget for WidgetBookGalleryScrollPane {
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

struct MinimumWidth {
    min_width: f32,
    child: SingleChild,
}

impl MinimumWidth {
    fn new<W>(min_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            min_width: min_width.max(0.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MinimumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = constraints.max.width.max(self.min_width);
        let child_constraints = Constraints::new(
            Size::new(
                constraints.min.width.max(self.min_width).min(max_width),
                constraints.min.height,
            ),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        Size::new(child_size.width.max(self.min_width), child_size.height)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct MaximumWidth {
    max_width: f32,
    child: SingleChild,
}

impl MaximumWidth {
    fn new<W>(max_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            max_width: max_width.max(1.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MaximumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.min(self.max_width)
        } else {
            self.max_width
        };
        let child_constraints = Constraints::new(
            Size::new(constraints.min.width.min(max_width), constraints.min.height),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);

        Size::new(
            child_size
                .width
                .min(max_width)
                .max(constraints.min.width.min(max_width)),
            child_size
                .height
                .clamp(constraints.min.height, constraints.max.height),
        )
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct VerticalScrollPane {
    spacing: f32,
    content: SingleChild,
    scroll_bar: SingleChild,
}

impl VerticalScrollPane {
    fn new<W, S>(content: W, scroll_bar: S) -> Self
    where
        W: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            content: SingleChild::new(content),
            scroll_bar: SingleChild::new(scroll_bar),
        }
    }
}

impl Widget for VerticalScrollPane {
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

struct TwoAxisScrollPane {
    spacing: f32,
    state: ScrollState,
    show_vertical_scroll_bar: bool,
    show_horizontal_scroll_bar: bool,
    content: SingleChild,
    vertical_scroll_bar: SingleChild,
    horizontal_scroll_bar: SingleChild,
}

impl TwoAxisScrollPane {
    fn new<W, V, H>(
        state: ScrollState,
        content: W,
        vertical_scroll_bar: V,
        horizontal_scroll_bar: H,
    ) -> Self
    where
        W: Widget + 'static,
        V: Widget + 'static,
        H: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            state,
            show_vertical_scroll_bar: true,
            show_horizontal_scroll_bar: true,
            content: SingleChild::new(content),
            vertical_scroll_bar: SingleChild::new(vertical_scroll_bar),
            horizontal_scroll_bar: SingleChild::new(horizontal_scroll_bar),
        }
    }

    fn viewport_size(&self, bounds: Size) -> Size {
        let vertical_size = self.vertical_scroll_bar.child().measured_size();
        let horizontal_size = self.horizontal_scroll_bar.child().measured_size();
        let vertical_extent = if self.show_vertical_scroll_bar {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if self.show_horizontal_scroll_bar {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        Size::new(
            (bounds.width - vertical_extent).max(0.0),
            (bounds.height - horizontal_extent).max(0.0),
        )
    }

    fn content_constraints(
        constraints: Constraints,
        vertical_size: Size,
        horizontal_size: Size,
        show_vertical_scroll_bar: bool,
        show_horizontal_scroll_bar: bool,
        spacing: f32,
    ) -> Constraints {
        let vertical_extent = if show_vertical_scroll_bar {
            vertical_size.width + spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal_scroll_bar {
            horizontal_size.height + spacing
        } else {
            0.0
        };
        Constraints::new(
            Size::new(
                (constraints.min.width - vertical_extent).max(0.0),
                (constraints.min.height - horizontal_extent).max(0.0),
            ),
            Size::new(
                (constraints.max.width - vertical_extent).max(0.0),
                (constraints.max.height - horizontal_extent).max(0.0),
            ),
        )
    }

    fn scroll_bar_visibility(&self) -> (bool, bool) {
        let viewport = self.state.viewport_size();
        let content = self.state.content_size();
        (
            content.width > viewport.width + 0.001,
            content.height > viewport.height + 0.001,
        )
    }
}

impl Widget for TwoAxisScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let vertical_size = self.vertical_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let horizontal_size = self.horizontal_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY)),
        );
        let mut show_vertical = false;
        let mut show_horizontal = false;
        let mut content_size = self.content.measure(
            ctx,
            Self::content_constraints(
                constraints,
                vertical_size,
                horizontal_size,
                show_vertical,
                show_horizontal,
                self.spacing,
            ),
        );
        for _ in 0..3 {
            let (next_horizontal, next_vertical) = self.scroll_bar_visibility();
            if next_vertical == show_vertical && next_horizontal == show_horizontal {
                break;
            }
            show_vertical = next_vertical;
            show_horizontal = next_horizontal;
            content_size = self.content.measure(
                ctx,
                Self::content_constraints(
                    constraints,
                    vertical_size,
                    horizontal_size,
                    show_vertical,
                    show_horizontal,
                    self.spacing,
                ),
            );
        }

        self.show_vertical_scroll_bar = show_vertical;
        self.show_horizontal_scroll_bar = show_horizontal;
        let vertical_extent = if show_vertical {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            content_size.width + vertical_extent,
            content_size.height + horizontal_extent,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport = self.viewport_size(bounds.size);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), viewport.width, viewport.height),
        );
        self.vertical_scroll_bar.arrange(
            ctx,
            if self.show_vertical_scroll_bar {
                Rect::new(
                    bounds.x() + viewport.width + self.spacing,
                    bounds.y(),
                    self.vertical_scroll_bar.child().measured_size().width,
                    viewport.height,
                )
            } else {
                Rect::new(bounds.max_x(), bounds.y(), 0.0, 0.0)
            },
        );
        self.horizontal_scroll_bar.arrange(
            ctx,
            if self.show_horizontal_scroll_bar {
                Rect::new(
                    bounds.x(),
                    bounds.y() + viewport.height + self.spacing,
                    viewport.width,
                    self.horizontal_scroll_bar.child().measured_size().height,
                )
            } else {
                Rect::new(bounds.x(), bounds.max_y(), 0.0, 0.0)
            },
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.paint(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.semantics(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.vertical_scroll_bar.visit_children(visitor);
        self.horizontal_scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.vertical_scroll_bar.visit_children_mut(visitor);
        self.horizontal_scroll_bar.visit_children_mut(visitor);
    }
}

impl ProjectSettingsPreview {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self {
            trigger: SingleChild::new(Button::new(DIALOG_TRIGGER_LABEL).min_width(220.0)),
            dialog: SingleChild::new(
                Dialog::new(
                    DIALOG_TITLE,
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new("Autosave every 90 seconds")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        )
                        .with_child(
                            Label::new("Export color profile: Display P3")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        )
                        .with_child(
                            Label::new("Scratch disk: fast-local-ssd")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        ),
                )
                .description(
                    "Compact dialog framing for confirmations, settings, and import/export flows.",
                )
                .modal(false)
                .secondary_action("Cancel", || {})
                .primary_action("Apply", move || {
                    state.borrow_mut().dialog_apply_count += 1;
                }),
            ),
            dialog_open: false,
            trigger_pressed: false,
        }
    }

    fn trigger_bounds(&self) -> Rect {
        self.trigger.child().bounds()
    }
}

impl Widget for ProjectSettingsPreview {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Capture {
            return;
        }

        match event {
            Event::Window(WindowEvent::RedrawRequested) => {}
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Down
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                self.trigger_pressed = self.trigger_bounds().contains(pointer.position);
            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Up
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                let activate =
                    self.trigger_pressed && self.trigger_bounds().contains(pointer.position);
                self.trigger_pressed = false;
                if activate {
                    self.dialog_open = !self.dialog_open;
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Cancel => {
                self.trigger_pressed = false;
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());

        if !self.dialog_open {
            return constraints.clamp(trigger_size);
        }

        let dialog_size = self
            .dialog
            .measure(ctx, Constraints::tight(Size::new(560.0, 320.0)));

        constraints.clamp(Size::new(
            trigger_size.width.max(dialog_size.width),
            trigger_size.height + 12.0 + dialog_size.height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        self.trigger
            .arrange(ctx, Rect::from_origin_size(bounds.origin, trigger_size));
        if self.dialog_open {
            let dialog_size = self.dialog.child().measured_size();
            self.dialog.arrange(
                ctx,
                Rect::new(
                    bounds.x(),
                    bounds.y() + trigger_size.height + 12.0,
                    dialog_size.width,
                    dialog_size.height,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.dialog_open {
            self.dialog.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.trigger.semantics(ctx);
        if self.dialog_open {
            self.dialog.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.dialog_open {
            self.dialog.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.dialog_open {
            self.dialog.visit_children_mut(visitor);
        }
    }
}

struct ThemePreviewShowcase {
    state: Rc<RefCell<WidgetBookState>>,
    toggle: SingleChild,
    light_card: SingleChild,
    dark_card: SingleChild,
    high_contrast_card: SingleChild,
}

impl ThemePreviewShowcase {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        let comparison_enabled = state.borrow().theme_preview_comparison;
        let toggle_state = Rc::clone(&state);

        Self {
            state,
            toggle: SingleChild::new(
                Switch::new(THEME_PREVIEW_TOGGLE_LABEL)
                    .on(comparison_enabled)
                    .on_toggle(move |checked| {
                        toggle_state.borrow_mut().theme_preview_comparison = checked;
                    }),
            ),
            light_card: SingleChild::new(NamedSection::new(
                LIGHT_THEME_PREVIEW_CARD_NAME,
                theme_preview_card(
                    DefaultTheme::light(),
                    "Light",
                    LIGHT_PREVIEW_ACTION_LABEL,
                    LIGHT_PREVIEW_INPUT_LABEL,
                ),
            )),
            dark_card: SingleChild::new(NamedSection::new(
                DARK_THEME_PREVIEW_CARD_NAME,
                theme_preview_card(
                    DefaultTheme::dark(),
                    "Dark",
                    DARK_PREVIEW_ACTION_LABEL,
                    DARK_PREVIEW_INPUT_LABEL,
                ),
            )),
            high_contrast_card: SingleChild::new(NamedSection::new(
                HIGH_CONTRAST_THEME_PREVIEW_CARD_NAME,
                theme_preview_card(
                    DefaultTheme::high_contrast(),
                    "High contrast",
                    HIGH_CONTRAST_PREVIEW_ACTION_LABEL,
                    HIGH_CONTRAST_PREVIEW_INPUT_LABEL,
                ),
            )),
        }
    }

    fn card_height() -> f32 {
        320.0
    }

    fn comparison_enabled(&self) -> bool {
        self.state.borrow().theme_preview_comparison
    }
}

impl Widget for ThemePreviewShowcase {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Capture {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    sui::PointerEventKind::Down | sui::PointerEventKind::Up
                ) && self.toggle.child().bounds().contains(pointer.position) =>
            {
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let comparison_enabled = self.comparison_enabled();
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(320.0)
        } else {
            1080.0
        };
        let toggle_size = self.toggle.measure(ctx, constraints.loosen());
        let top = toggle_size.height + 16.0;
        let gap = 16.0;
        let card_height = Self::card_height();

        if comparison_enabled {
            let stacked = max_width < 1020.0;
            if stacked {
                let light_size = self
                    .light_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));
                let dark_size = self
                    .dark_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));
                let high_contrast_size = self
                    .high_contrast_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));

                return constraints.clamp(Size::new(
                    max_width,
                    top + light_size.height
                        + gap
                        + dark_size.height
                        + gap
                        + high_contrast_size.height,
                ));
            }

            let card_width = ((max_width - (gap * 2.0)) / 3.0).max(280.0);
            let light_size = self
                .light_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));
            let dark_size = self
                .dark_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));
            let high_contrast_size = self
                .high_contrast_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));

            return constraints.clamp(Size::new(
                light_size.width + gap + dark_size.width + gap + high_contrast_size.width,
                top + light_size
                    .height
                    .max(dark_size.height)
                    .max(high_contrast_size.height),
            ));
        }

        let light_width = max_width.min(420.0);
        let light_size = self
            .light_card
            .measure(ctx, Constraints::tight(Size::new(light_width, card_height)));

        constraints.clamp(Size::new(light_size.width, top + light_size.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let comparison_enabled = self.comparison_enabled();
        let toggle_size = self.toggle.child().measured_size();
        let snapped_origin = Point::new(bounds.x().round(), bounds.y().round());
        self.toggle
            .arrange(ctx, Rect::from_origin_size(snapped_origin, toggle_size));

        let top = (bounds.y() + toggle_size.height + 16.0).round();
        let gap = 16.0;
        if comparison_enabled {
            if bounds.width() < 1020.0 {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
                let high_contrast_size = self.high_contrast_card.child().measured_size();
                self.light_card.arrange(
                    ctx,
                    Rect::new(bounds.x().round(), top, light_size.width, light_size.height),
                );
                self.dark_card.arrange(
                    ctx,
                    Rect::new(
                        bounds.x().round(),
                        top + light_size.height + gap,
                        dark_size.width,
                        dark_size.height,
                    ),
                );
                self.high_contrast_card.arrange(
                    ctx,
                    Rect::new(
                        bounds.x().round(),
                        top + light_size.height + gap + dark_size.height + gap,
                        high_contrast_size.width,
                        high_contrast_size.height,
                    ),
                );
            } else {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
                let high_contrast_size = self.high_contrast_card.child().measured_size();
                self.light_card.arrange(
                    ctx,
                    Rect::new(bounds.x().round(), top, light_size.width, light_size.height),
                );
                self.dark_card.arrange(
                    ctx,
                    Rect::new(
                        (bounds.x() + light_size.width + gap).round(),
                        top,
                        dark_size.width,
                        dark_size.height,
                    ),
                );
                self.high_contrast_card.arrange(
                    ctx,
                    Rect::new(
                        (bounds.x() + light_size.width + gap + dark_size.width + gap).round(),
                        top,
                        high_contrast_size.width,
                        high_contrast_size.height,
                    ),
                );
            }
        } else {
            let light_size = self.light_card.child().measured_size();
            self.light_card.arrange(
                ctx,
                Rect::new(bounds.x().round(), top, light_size.width, light_size.height),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.paint(ctx);
        self.light_card.paint(ctx);
        if comparison_enabled {
            self.dark_card.paint(ctx);
            self.high_contrast_card.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let comparison_enabled = self.comparison_enabled();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(THEME_PREVIEW_NAME.to_string());
        node.description = Some(if comparison_enabled {
            "Light, dark, and high contrast preview cards are visible.".to_string()
        } else {
            "Only the light preview card is visible.".to_string()
        });
        ctx.push(node);
        self.toggle.semantics(ctx);
        self.light_card.semantics(ctx);
        if comparison_enabled {
            self.dark_card.semantics(ctx);
            self.high_contrast_card.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children(visitor);
        self.light_card.visit_children(visitor);
        if comparison_enabled {
            self.dark_card.visit_children(visitor);
            self.high_contrast_card.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children_mut(visitor);
        self.light_card.visit_children_mut(visitor);
        if comparison_enabled {
            self.dark_card.visit_children_mut(visitor);
            self.high_contrast_card.visit_children_mut(visitor);
        }
    }
}

fn hdr_theme_mode_title(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

fn hdr_theme_mode_explanation(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => {
            "Uses the SDR fallback path only. Wide-gamut and HDR token branches stay available in the theme, but built-in widgets resolve to the existing SDR palette and luminance ceilings."
        }
        HdrThemeMode::WideGamutOnly => {
            "Prefers richer gamut variants while keeping luminance pinned to reference white. This validates color-volume differences without introducing above-white UI chrome."
        }
        HdrThemeMode::ConstrainedHdr => {
            "Allows a modest lift for accents, focused states, and emissive indicators while still treating reference white as the visual anchor."
        }
        HdrThemeMode::FullHdr => {
            "Allows the same semantic tokens to push farther into HDR headroom so popup arrivals and indicator energy can separate more clearly from the constrained path."
        }
    }
}

fn hdr_theme_lab_section_name(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline comparison",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only comparison",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR comparison",
        HdrThemeMode::FullHdr => "Full HDR comparison",
    }
}

fn hdr_theme_lab_theme(mode: HdrThemeMode) -> DefaultTheme {
    let mut theme = DefaultTheme::dark();
    theme.hdr = HdrThemeTokens::from_default_theme(theme);
    theme.hdr.mode = mode;
    theme.hdr.color_roles.surface = SemanticColorToken::from_sdr(theme.colors.base_100)
        .with_wide_gamut(Color::display_p3(0.13, 0.16, 0.23, 1.0))
        .with_hdr(Color::linear_display_p3(0.18, 0.21, 0.30, 1.0));
    theme.hdr.color_roles.surface_elevated = SemanticColorToken::from_sdr(theme.colors.base_200)
        .with_wide_gamut(Color::display_p3(0.16, 0.19, 0.28, 1.0))
        .with_hdr(Color::linear_display_p3(0.24, 0.27, 0.38, 1.0));
    theme.hdr.color_roles.surface_outline = SemanticColorToken::from_sdr(theme.colors.base_300)
        .with_wide_gamut(Color::display_p3(0.33, 0.39, 0.50, 1.0))
        .with_hdr(Color::linear_display_p3(0.42, 0.48, 0.62, 1.0));
    theme.hdr.color_roles.text = SemanticColorToken::from_sdr(theme.colors.base_content)
        .with_wide_gamut(Color::display_p3(0.92, 0.95, 0.99, 1.0))
        .with_hdr(Color::linear_display_p3(1.02, 1.04, 1.10, 1.0));
    theme.hdr.color_roles.text_muted =
        SemanticColorToken::from_sdr(theme.colors.base_content.with_alpha(0.74))
            .with_wide_gamut(Color::display_p3(0.75, 0.80, 0.89, 1.0))
            .with_hdr(Color::linear_display_p3(0.86, 0.90, 0.98, 1.0));
    theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.colors.primary)
        .with_wide_gamut(Color::display_p3(0.18, 0.74, 0.96, 1.0))
        .with_hdr(Color::linear_display_p3(0.78, 2.40, 3.20, 1.0));
    theme.hdr.color_roles.accent_text = SemanticColorToken::from_sdr(theme.colors.primary_content)
        .with_wide_gamut(Color::display_p3(0.03, 0.08, 0.12, 1.0))
        .with_hdr(Color::linear_display_p3(0.10, 0.14, 0.20, 1.0));
    theme.hdr.color_roles.secondary = SemanticColorToken::from_sdr(theme.colors.secondary)
        .with_wide_gamut(Color::display_p3(0.43, 0.66, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.96, 1.72, 2.42, 1.0));
    theme.hdr.color_roles.warning = SemanticColorToken::from_sdr(theme.colors.warning)
        .with_wide_gamut(Color::display_p3(0.98, 0.68, 0.18, 1.0))
        .with_hdr(Color::linear_display_p3(3.00, 1.50, 0.30, 1.0));
    theme.hdr.color_roles.info = SemanticColorToken::from_sdr(theme.colors.info)
        .with_wide_gamut(Color::display_p3(0.40, 0.78, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.88, 2.00, 2.90, 1.0));
    theme.hdr.luminance = HdrLuminanceTokens::constrained_defaults();
    theme.hdr.policy.max_large_area_lift = 1.18;
    theme.hdr.policy.max_constrained_lift = 1.32;
    theme.hdr.policy.max_emissive_lift = 1.75;
    theme.hdr.effects.pulse.speed = 1.1;
    theme.hdr.effects.pulse.color = Some(resolve_semantic_color(
        theme.hdr.color_roles.warning,
        HdrThemeMode::FullHdr,
    ));

    match mode {
        HdrThemeMode::Disabled | HdrThemeMode::WideGamutOnly => {}
        HdrThemeMode::ConstrainedHdr => {
            theme.hdr.luminance.focused = 1.08;
            theme.hdr.luminance.semantic_accent = 1.16;
            theme.hdr.luminance.emissive_indicator = 1.55;
            theme.hdr.luminance.alert_pulse = 1.42;
        }
        HdrThemeMode::FullHdr => {
            theme.hdr.luminance.focused = 1.18;
            theme.hdr.luminance.semantic_accent = 1.34;
            theme.hdr.luminance.emissive_indicator = 2.40;
            theme.hdr.luminance.alert_pulse = 2.05;
            theme.hdr.policy.max_large_area_lift = 1.36;
            theme.hdr.policy.max_emissive_lift = 2.60;
            theme.hdr.materials.raised.specular_strength = 0.18;
            theme.hdr.materials.raised.rim_light_strength = 0.14;
            theme.hdr.effects.glow.intensity = 0.32;
            theme.hdr.effects.pulse.intensity = 0.54;
        }
    }

    theme
}

fn hdr_theme_lab_card(
    section_name: impl Into<String>,
    mode: HdrThemeMode,
    prefix: impl Into<String>,
    lead_text: impl Into<String>,
) -> impl Widget {
    let section_name = section_name.into();
    let prefix = prefix.into();
    let lead_text = lead_text.into();
    let theme = hdr_theme_lab_theme(mode);
    let indicator_style = resolve_widget_hdr_style(
        &theme.hdr,
        WidgetColorRole::Accent,
        WidgetLuminanceRole::EmissiveIndicator,
        WidgetMaterialRole::Flat,
        None,
    );
    let indicator_color = Color::new(
        indicator_style.color.space,
        indicator_style
            .color
            .red
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .green
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .blue
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style.color.alpha,
    );
    let button_label = format!("{prefix} sample action");
    let switch_label = format!("{prefix} sample live indicator");
    let popover_name = format!("{prefix} attention popover");
    let popover_trigger_label = format!("{prefix} attention trigger");
    let swatch_name = format!("{prefix} emissive indicator");

    NamedSection::new(
        section_name,
        ThemePreviewCardFrame::new(
            theme,
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Start)
                .with_child(
                    Label::new(hdr_theme_mode_title(mode))
                        .font_size(18.0)
                        .line_height(22.0)
                        .color(theme.palette.text),
                )
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(lead_text)
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(theme.palette.placeholder),
                ))
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(format!(
                        "Token mode: {} · accent peak {:.2}× · indicator peak {:.2}× · alert peak {:.2}×",
                        hdr_theme_mode_title(mode),
                        theme.hdr.luminance.semantic_accent,
                        theme.hdr.luminance.emissive_indicator,
                        theme.hdr.luminance.alert_pulse,
                    ))
                    .font_size(12.0)
                    .line_height(17.0)
                    .color(theme.palette.placeholder),
                ))
                .with_child(
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Center)
                        .with_child(
                            SizedBox::new().width(300.0).with_child(
                                Button::new(button_label).min_width(280.0).theme(theme),
                            ),
                        )
                        .with_child(
                            ColorSwatch::new(swatch_name, indicator_color)
                                .size(Size::new(64.0, 28.0)),
                        )
                        .with_child(MaximumWidth::new(
                            520.0,
                            Label::new(
                                "The swatch mirrors the accent token resolved for the current gamut/HDR mode.",
                            )
                            .font_size(12.0)
                            .line_height(17.0)
                            .color(theme.palette.placeholder),
                        )),
                )
                .with_child(
                    SizedBox::new().width(520.0).with_child(
                        Switch::new(switch_label)
                            .on(!matches!(mode, HdrThemeMode::Disabled))
                            .theme(theme),
                    ),
                )
                .with_child(
                    SizedBox::new().width(430.0).with_child(
                        Popover::new(
                            popover_name,
                            Button::new(popover_trigger_label)
                                .min_width(400.0)
                                .theme(theme),
                            Stack::vertical()
                                .spacing(8.0)
                                .alignment(Alignment::Stretch)
                                .with_child(
                                    Label::new(
                                        "Small popup surfaces are where constrained vs full HDR arrival cues become easiest to validate.",
                                    )
                                    .font_size(13.0)
                                    .line_height(18.0)
                                    .color(theme.palette.text),
                                )
                                .with_child(
                                    Label::new(
                                        "Use this trigger to compare popup chrome, border lift, and arrival emphasis against the matching button and switch.",
                                    )
                                    .font_size(12.0)
                                    .line_height(17.0)
                                    .color(theme.palette.placeholder),
                                ),
                        )
                        .open(true)
                        .theme(theme),
                    ),
                ),
        ),
    )
}

struct HdrThemeLabShowcase {
    active_mode: HdrThemeMode,
    active_preview: SingleChild,
    sdr_card: SingleChild,
    wide_gamut_card: SingleChild,
    constrained_card: SingleChild,
    full_hdr_card: SingleChild,
}

impl HdrThemeLabShowcase {
    const SECTION_GAP: f32 = 14.0;

    fn new() -> Self {
        let active_mode = widget_book_hdr_theme_mode();
        Self {
            active_mode,
            active_preview: SingleChild::new(Self::build_active_preview(active_mode)),
            sdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::Disabled),
                HdrThemeMode::Disabled,
                hdr_theme_mode_title(HdrThemeMode::Disabled),
                hdr_theme_mode_explanation(HdrThemeMode::Disabled),
            )),
            wide_gamut_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::WideGamutOnly),
                HdrThemeMode::WideGamutOnly,
                hdr_theme_mode_title(HdrThemeMode::WideGamutOnly),
                hdr_theme_mode_explanation(HdrThemeMode::WideGamutOnly),
            )),
            constrained_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::ConstrainedHdr),
                HdrThemeMode::ConstrainedHdr,
                hdr_theme_mode_title(HdrThemeMode::ConstrainedHdr),
                hdr_theme_mode_explanation(HdrThemeMode::ConstrainedHdr),
            )),
            full_hdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::FullHdr),
                HdrThemeMode::FullHdr,
                hdr_theme_mode_title(HdrThemeMode::FullHdr),
                hdr_theme_mode_explanation(HdrThemeMode::FullHdr),
            )),
        }
    }

    fn build_active_preview(mode: HdrThemeMode) -> impl Widget {
        hdr_theme_lab_card(
            HDR_THEME_LAB_ACTIVE_PREVIEW_NAME,
            mode,
            format!("Current {} preview", hdr_theme_mode_title(mode)),
            format!(
                "This preview follows the shared HDR theme mode currently selected by the dev host: {}. Use it to compare the active styling path against the four fixed comparison cards below.",
                hdr_theme_mode_title(mode),
            ),
        )
    }

    fn sync_active_preview(&mut self) -> bool {
        let next_mode = widget_book_hdr_theme_mode();
        if next_mode == self.active_mode {
            return false;
        }

        self.active_mode = next_mode;
        self.active_preview = SingleChild::new(Self::build_active_preview(next_mode));
        true
    }
}

impl Widget for HdrThemeLabShowcase {
    fn event(&mut self, ctx: &mut EventCtx, _event: &Event) {
        if self.sync_active_preview() {
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(320.0)
        } else {
            760.0
        };
        let child_constraints = Constraints::new(Size::ZERO, Size::new(max_width, f32::INFINITY));
        let mut height = 0.0;
        let mut width: f32 = 0.0;

        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.measure(ctx, child_constraints);
            width = width.max(size.width);
            height += size.height;
        }

        height += Self::SECTION_GAP * 4.0;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let mut y = bounds.y();
        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.child().measured_size();
            child.arrange(
                ctx,
                Rect::new(bounds.x(), y, bounds.width().min(size.width), size.height),
            );
            y += size.height + Self::SECTION_GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.active_preview.paint(ctx);
        self.sdr_card.paint(ctx);
        self.wide_gamut_card.paint(ctx);
        self.constrained_card.paint(ctx);
        self.full_hdr_card.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(HDR_THEME_LAB_NAME.to_string());
        node.description = Some(format!(
            "Compares the same button, switch, emissive indicator, and popup trigger across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The shared preview currently uses {}.",
            hdr_theme_mode_title(self.active_mode),
        ));
        ctx.push(node);
        self.active_preview.semantics(ctx);
        self.sdr_card.semantics(ctx);
        self.wide_gamut_card.semantics(ctx);
        self.constrained_card.semantics(ctx);
        self.full_hdr_card.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.active_preview.visit_children(visitor);
        self.sdr_card.visit_children(visitor);
        self.wide_gamut_card.visit_children(visitor);
        self.constrained_card.visit_children(visitor);
        self.full_hdr_card.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.active_preview.visit_children_mut(visitor);
        self.sdr_card.visit_children_mut(visitor);
        self.wide_gamut_card.visit_children_mut(visitor);
        self.constrained_card.visit_children_mut(visitor);
        self.full_hdr_card.visit_children_mut(visitor);
    }
}

#[cfg(test)]
fn build_animation_demo_panel() -> impl Widget {
    build_animation_demo_panel_with_theme(default_widget_book_theme_reader())
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TimelinePreviewPresentation {
    opacity: f32,
    translation: Vector,
    fill: Color,
    radius: f32,
}

impl Default for TimelinePreviewPresentation {
    fn default() -> Self {
        Self {
            opacity: 0.4,
            translation: Vector::new(-18.0, 0.0),
            fill: Color::rgba(0.20, 0.45, 0.95, 1.0),
            radius: 12.0,
        }
    }
}

impl TimelineBindingSink for TimelinePreviewPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != TIMELINE_ANIMATION_PREVIEW_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.0, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == TIMELINE_ANIMATION_PREVIEW_RADIUS_PATH =>
            {
                let value = value.max(4.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            _ => false,
        }
    }
}

struct TimelineAnimationPreview {
    player: TimelinePlayer,
    presentation: TimelinePreviewPresentation,
}

impl TimelineAnimationPreview {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(timeline_animation_preview_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        Self {
            player,
            presentation: TimelinePreviewPresentation::default(),
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for TimelineAnimationPreview {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Enter | PointerEventKind::Move | PointerEventKind::Down
                ) =>
            {
                if ctx.bounds().contains(pointer.position) {
                    self.start(ctx);
                    ctx.request_paint();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(320.0, 118.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.08, 0.10, 0.14, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 28.0,
            bounds.y() + (bounds.height() * 0.5) - 3.0,
            bounds.width() - 56.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.32, 0.36, 0.43, 0.55),
        );

        let center = Point::new(bounds.x() + bounds.width() * 0.5, bounds.y() + 58.0);
        ctx.fill(
            Path::circle(center, self.presentation.radius),
            self.presentation.fill,
        );
        ctx.stroke(
            Path::circle(center, self.presentation.radius + 4.0),
            Color::rgba(1.0, 1.0, 1.0, 0.42),
            StrokeStyle::new(1.5),
        );

        let meter = Rect::new(
            bounds.x() + 28.0,
            bounds.max_y() - 24.0,
            (bounds.width() - 56.0) * self.presentation.opacity,
            5.0,
        );
        ctx.fill(
            Path::rounded_rect(meter, 2.5),
            Color::rgba(0.72, 0.86, 1.0, 0.88),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(TIMELINE_ANIMATION_PREVIEW_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, radius {:.1}",
            self.presentation.opacity, self.presentation.radius
        )));
        ctx.push(node);
    }
}

fn timeline_animation_preview_timeline() -> Timeline {
    let target = AnimationTargetId::new(TIMELINE_ANIMATION_PREVIEW_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.6).with_clip(
        Clip::new("timeline-preview-loop", 0.0, 1.6)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.4)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.8, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.6, AnimationValue::Scalar(0.4)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-18.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.8, AnimationValue::Vector(Vector::new(18.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.6, AnimationValue::Vector(Vector::new(-18.0, 0.0))),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.8,
                        AnimationValue::Color(Color::rgba(0.10, 0.76, 0.52, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.6,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(TIMELINE_ANIMATION_PREVIEW_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(12.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.8, AnimationValue::Scalar(24.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.6, AnimationValue::Scalar(12.0)),
                ]),
            ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkRetainedPresentation {
    opacity: f32,
    translation: Vector,
}

impl Default for AnimationBenchmarkRetainedPresentation {
    fn default() -> Self {
        Self {
            opacity: 0.72,
            translation: Vector::new(-24.0, 0.0),
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkRetainedPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_RETAINED_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkRetainedLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkRetainedPresentation,
}

impl AnimationBenchmarkRetainedLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_retained_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkRetainedPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRetainedLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 112.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.10, 0.12, 0.14, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 38.0,
            bounds.y() + bounds.height() * 0.5 - 3.0,
            bounds.width() - 76.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.42, 0.47, 0.56, 0.40),
        );

        let marker = Rect::new(
            bounds.x() + bounds.width() * 0.5 - 36.0,
            bounds.y() + 28.0,
            72.0,
            44.0,
        );
        ctx.fill(
            Path::rounded_rect(marker, 7.0),
            Color::rgba(0.34, 0.72, 0.88, 0.88),
        );
        ctx.stroke_rect(
            marker,
            Color::rgba(0.86, 0.96, 1.0, 0.78),
            StrokeStyle::new(1.0),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_RETAINED_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, x {:.1}",
            self.presentation.opacity, self.presentation.translation.x
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkPaintPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkPaintPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.82, 0.33, 0.24, 1.0),
            radius: 18.0,
            alpha: 0.76,
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkPaintPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_REPAINT_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(3.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.alpha - value).abs() > 0.001;
                self.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkRepaintLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkPaintPresentation,
}

impl AnimationBenchmarkRepaintLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_repaint_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkPaintPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRepaintLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 136.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.13, 0.12, 0.11, 1.0),
        );

        let lanes = 11;
        for lane in 0..lanes {
            let t = lane as f32 / (lanes - 1) as f32;
            let x = bounds.x() + 38.0 + (bounds.width() - 76.0) * t;
            let y = bounds.y() + bounds.height() * 0.5;
            let radius = self.presentation.radius * (0.56 + 0.045 * lane as f32);
            let alpha = (self.presentation.alpha * (1.0 - t * 0.35)).clamp(0.15, 1.0);
            let color = Color::rgba(
                (self.presentation.fill.red + t * 0.10).min(1.0),
                self.presentation.fill.green,
                (self.presentation.fill.blue + (1.0 - t) * 0.10).min(1.0),
                alpha,
            );
            ctx.fill(Path::circle(Point::new(x, y), radius), color);
            ctx.stroke(
                Path::circle(Point::new(x, y), radius + 3.5),
                Color::rgba(1.0, 1.0, 1.0, 0.20 * alpha),
                StrokeStyle::new(1.0),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_REPAINT_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "radius {:.1}, alpha {:.2}",
            self.presentation.radius, self.presentation.alpha
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBenchmarkCellPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkCellPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.20, 0.48, 0.86, 1.0),
            radius: 7.0,
            alpha: 0.7,
        }
    }
}

struct AnimationBenchmarkScalePresentation {
    cells: Vec<AnimationBenchmarkCellPresentation>,
}

impl Default for AnimationBenchmarkScalePresentation {
    fn default() -> Self {
        Self {
            cells: vec![
                AnimationBenchmarkCellPresentation::default();
                ANIMATION_BENCHMARK_SCALE_CELLS
            ],
        }
    }
}

impl AnimationBenchmarkScalePresentation {
    fn cell_index(&self, binding: &AnimationBinding) -> Option<usize> {
        let index = binding
            .target
            .as_str()
            .strip_prefix(ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX)?
            .parse::<usize>()
            .ok()?;
        (index < self.cells.len()).then_some(index)
    }
}

impl TimelineBindingSink for AnimationBenchmarkScalePresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        let Some(index) = self.cell_index(binding) else {
            return false;
        };
        let cell = &mut self.cells[index];

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = cell.fill != value;
                cell.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(2.0);
                let changed = (cell.radius - value).abs() > 0.001;
                cell.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.18, 1.0);
                let changed = (cell.alpha - value).abs() > 0.001;
                cell.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

struct AnimationBenchmarkScaleGrid {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkScalePresentation,
}

impl AnimationBenchmarkScaleGrid {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_scale_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkScalePresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkScaleGrid {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 296.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.085, 0.095, 0.11, 1.0),
        );

        let rows = ANIMATION_BENCHMARK_SCALE_CELLS / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let grid = Rect::new(
            bounds.x() + 20.0,
            bounds.y() + 18.0,
            bounds.width() - 40.0,
            bounds.height() - 36.0,
        );
        let cell_width = grid.width() / ANIMATION_BENCHMARK_SCALE_COLUMNS as f32;
        let cell_height = grid.height() / rows as f32;

        for (index, cell) in self.presentation.cells.iter().enumerate() {
            let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let center = Point::new(
                grid.x() + cell_width * (column as f32 + 0.5),
                grid.y() + cell_height * (row as f32 + 0.5),
            );
            let bounds = Rect::new(
                center.x - cell_width * 0.34,
                center.y - cell_height * 0.30,
                cell_width * 0.68,
                cell_height * 0.60,
            );
            ctx.fill(
                Path::rounded_rect(bounds, 5.0),
                Color::rgba(0.14, 0.16, 0.19, 0.92),
            );
            ctx.fill(
                Path::circle(center, cell.radius),
                cell.fill.with_alpha(cell.alpha),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_SCALE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "{} animated cells",
            self.presentation.cells.len()
        )));
        ctx.push(node);
    }
}

fn animation_benchmark_retained_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_RETAINED_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.4).with_clip(
        Clip::new("retained-lane", 0.0, 1.4)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.44)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Scalar(0.44)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Vector(Vector::new(32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Vector(Vector::new(-32.0, 0.0))),
                ]),
            ),
    )
}

fn animation_benchmark_repaint_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_REPAINT_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.2).with_clip(
        Clip::new("repaint-lane", 0.0, 1.2)
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.6,
                        AnimationValue::Color(Color::rgba(0.22, 0.66, 0.82, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.2,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(14.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(26.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(14.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_ALPHA_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.52)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(0.52)),
                ]),
            ),
    )
}

fn animation_benchmark_scale_timeline() -> Timeline {
    let mut clip = Clip::new("scale-grid", 0.0, 1.8);
    for index in 0..ANIMATION_BENCHMARK_SCALE_CELLS {
        let target =
            AnimationTargetId::new(format!("{ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX}{index}"));
        let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let phase = ((column + row) % 6) as f32 / 6.0;
        let low_radius = 4.0 + (index % 5) as f32 * 0.35;
        let high_radius = 9.0 + (index % 7) as f32 * 0.45;
        let cool = Color::rgba(0.16 + phase * 0.16, 0.42 + phase * 0.16, 0.84, 1.0);
        let warm = Color::rgba(0.84, 0.36 + phase * 0.18, 0.20 + phase * 0.18, 1.0);

        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_RADIUS_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(low_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(high_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(low_radius)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_ALPHA_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(0.38 + phase * 0.24))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(0.82 + phase * 0.14))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(0.38 + phase * 0.24)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(target, AnimationProperty::FillColor)).with_keyframes(
                [
                    Keyframe::new(0.0, AnimationValue::Color(cool)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Color(warm)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Color(cool)),
                ],
            ),
        );
    }

    Timeline::new(1.8).with_clip(clip)
}

#[derive(Debug, Clone, Copy)]
struct AnimationEditorLayout {
    play_button: Rect,
    undo_button: Rect,
    redo_button: Rect,
    timeline: Rect,
    inspector: Rect,
    curve: Rect,
    preview: Rect,
}

struct AnimationEditorSurface {
    editor: AnimationEditorState,
    player: TimelinePlayer,
    presentation: TimelinePreviewPresentation,
}

impl AnimationEditorSurface {
    fn new() -> Self {
        let timeline = timeline_animation_preview_timeline();
        let mut editor = AnimationEditorState::new(AnimationDocument::new(
            "Widget-book animation editor",
            timeline.clone(),
        ));
        editor.apply_command(AnimationEditorCommand::SelectKeyframe(KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 1,
        }));

        let mut player = TimelinePlayer::new(timeline);
        player.playback_mut().loop_mode = LoopMode::Repeat;

        let mut presentation = TimelinePreviewPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }

        Self {
            editor,
            player,
            presentation,
        }
    }

    fn sync_player_timeline(&mut self) {
        self.player
            .set_timeline(self.editor.document.timeline.clone());
    }

    fn toggle_playback(&mut self, ctx: &mut EventCtx) {
        if self.player.playback().playing {
            self.player.pause();
            self.editor.playback.pause();
        } else {
            self.player.play();
            self.editor.playback.play();
            ctx.request_animation_frame();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn seek_to_position(
        &mut self,
        position: Point,
        layout: AnimationEditorLayout,
        ctx: &mut EventCtx,
    ) {
        let t = ((position.x - layout.timeline.x()) / layout.timeline.width()).clamp(0.0, 1.0);
        let time = self.editor.document.timeline.duration * t as f64;
        self.player.seek(time);
        self.editor
            .apply_command(AnimationEditorCommand::SetPlayhead(time));
        for sample in self.player.sample_reusing_scratch() {
            self.presentation
                .apply_animation_value(&sample.binding, sample.value);
        }
        ctx.request_transform();
        ctx.request_effect();
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn select_keyframe_at(
        &mut self,
        position: Point,
        layout: AnimationEditorLayout,
        ctx: &mut EventCtx,
    ) -> bool {
        for (selection, rect) in animation_editor_keyframe_hits(&self.editor, layout) {
            if rect.contains(position) {
                self.editor
                    .apply_command(AnimationEditorCommand::SelectKeyframe(selection));
                ctx.request_paint();
                ctx.request_semantics();
                return true;
            }
        }
        false
    }

    fn cycle_selected_easing(&mut self, ctx: &mut EventCtx) {
        let Some(selection) = self.editor.selection.keyframes.last().copied() else {
            return;
        };
        let Some(current) = self
            .editor
            .document
            .timeline
            .clips
            .get(selection.clip_index)
            .and_then(|clip| clip.tracks.get(selection.track_index))
            .and_then(|track| track.keyframes.get(selection.keyframe_index))
            .map(|keyframe| keyframe.easing)
        else {
            return;
        };
        let next = match current {
            Easing::Linear => Easing::EaseIn,
            Easing::EaseIn => Easing::EaseOut,
            Easing::EaseOut => Easing::EaseInOut,
            Easing::EaseInOut | Easing::CubicBezier { .. } => Easing::Linear,
        };

        if self
            .editor
            .apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection,
                easing: next,
            })
        {
            self.sync_player_timeline();
            for sample in self.player.sample_reusing_scratch() {
                self.presentation
                    .apply_animation_value(&sample.binding, sample.value);
            }
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn undo(&mut self, ctx: &mut EventCtx) {
        if self.editor.undo() {
            self.sync_player_timeline();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn redo(&mut self, ctx: &mut EventCtx) {
        if self.editor.redo() {
            self.sync_player_timeline();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for AnimationEditorSurface {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let layout = animation_editor_layout(ctx.bounds());
                if layout.play_button.contains(pointer.position) {
                    self.toggle_playback(ctx);
                    ctx.set_handled();
                } else if layout.undo_button.contains(pointer.position) {
                    self.undo(ctx);
                    ctx.set_handled();
                } else if layout.redo_button.contains(pointer.position) {
                    self.redo(ctx);
                    ctx.set_handled();
                } else if self.select_keyframe_at(pointer.position, layout, ctx) {
                    ctx.set_handled();
                } else if layout.timeline.contains(pointer.position) {
                    self.seek_to_position(pointer.position, layout, ctx);
                    ctx.set_handled();
                } else if layout.curve.contains(pointer.position) {
                    self.cycle_selected_easing(ctx);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                {
                    let tick = self.player.tick(*delta, &mut self.presentation);
                    tick.request_current_widget_invalidations(ctx);
                }
                self.editor.playback = self.player.playback();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(620.0, 320.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let layout = animation_editor_layout(bounds);

        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.075, 0.085, 0.105, 1.0),
        );
        draw_editor_label(
            ctx,
            Rect::new(bounds.x() + 14.0, bounds.y() + 10.0, 180.0, 20.0),
            "Animation editor",
            13.0,
            Color::rgba(0.90, 0.94, 1.0, 1.0),
        );

        paint_editor_button(
            ctx,
            layout.play_button,
            if self.player.playback().playing {
                "Pause"
            } else {
                "Play"
            },
            self.player.playback().playing,
        );
        paint_editor_button(ctx, layout.undo_button, "Undo", self.editor.can_undo());
        paint_editor_button(ctx, layout.redo_button, "Redo", self.editor.can_redo());

        self.paint_timeline(ctx, layout);
        self.paint_inspector(ctx, layout);
        self.paint_curve(ctx, layout);
        self.paint_live_preview(ctx, layout);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_EDITOR_SURFACE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "playhead {:.2}, selected keyframes {}",
            self.player.playback().playhead,
            self.editor.selection.keyframes.len()
        )));
        ctx.push(node);
    }
}

impl AnimationEditorSurface {
    fn paint_timeline(&self, ctx: &mut PaintCtx, layout: AnimationEditorLayout) {
        ctx.fill(
            Path::rounded_rect(layout.timeline, 6.0),
            Color::rgba(0.105, 0.12, 0.15, 1.0),
        );
        draw_editor_label(
            ctx,
            Rect::new(
                layout.timeline.x() + 10.0,
                layout.timeline.y() + 8.0,
                160.0,
                18.0,
            ),
            "Timeline",
            11.0,
            Color::rgba(0.72, 0.78, 0.88, 1.0),
        );

        let Some(clip) = self.editor.document.timeline.clips.first() else {
            return;
        };
        let lane_area = Rect::new(
            layout.timeline.x() + 14.0,
            layout.timeline.y() + 34.0,
            layout.timeline.width() - 28.0,
            layout.timeline.height() - 48.0,
        );
        let lane_height = 28.0;
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
            let lane = Rect::new(lane_area.x(), y, lane_area.width(), lane_height);
            ctx.fill(
                Path::rounded_rect(lane, 4.0),
                Color::rgba(0.14, 0.16, 0.20, 1.0),
            );
            draw_editor_label(
                ctx,
                Rect::new(lane.x() + 8.0, lane.y() + 5.0, 132.0, 16.0),
                track.binding.property.path(),
                10.0,
                Color::rgba(0.72, 0.76, 0.84, 1.0),
            );
        }

        for (selection, rect) in animation_editor_keyframe_hits(&self.editor, layout) {
            let selected = self.editor.selection.keyframes.contains(&selection);
            let center = Point::new(
                rect.x() + rect.width() * 0.5,
                rect.y() + rect.height() * 0.5,
            );
            ctx.fill(
                Path::circle(center, if selected { 6.5 } else { 4.8 }),
                if selected {
                    Color::rgba(0.90, 0.72, 0.28, 1.0)
                } else {
                    Color::rgba(0.46, 0.68, 1.0, 1.0)
                },
            );
        }

        let playhead_x = lane_area.x()
            + lane_area.width()
                * (self.player.playback().playhead / self.editor.document.timeline.duration)
                    .clamp(0.0, 1.0) as f32;
        ctx.fill_rect(
            Rect::new(playhead_x - 1.0, lane_area.y(), 2.0, lane_area.height()),
            Color::rgba(1.0, 0.36, 0.26, 0.92),
        );
    }

    fn paint_inspector(&self, ctx: &mut PaintCtx, layout: AnimationEditorLayout) {
        ctx.fill(
            Path::rounded_rect(layout.inspector, 6.0),
            Color::rgba(0.11, 0.13, 0.16, 1.0),
        );
        draw_editor_label(
            ctx,
            Rect::new(
                layout.inspector.x() + 10.0,
                layout.inspector.y() + 8.0,
                layout.inspector.width() - 20.0,
                18.0,
            ),
            "Keyframe inspector",
            11.0,
            Color::rgba(0.84, 0.88, 0.94, 1.0),
        );
        let detail = self
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe_detail(&self.editor, *selection))
            .unwrap_or_else(|| "No keyframe selected".to_string());
        draw_editor_label(
            ctx,
            Rect::new(
                layout.inspector.x() + 10.0,
                layout.inspector.y() + 32.0,
                layout.inspector.width() - 20.0,
                48.0,
            ),
            &detail,
            10.0,
            Color::rgba(0.70, 0.76, 0.86, 1.0),
        );
    }

    fn paint_curve(&self, ctx: &mut PaintCtx, layout: AnimationEditorLayout) {
        ctx.fill(
            Path::rounded_rect(layout.curve, 6.0),
            Color::rgba(0.11, 0.13, 0.16, 1.0),
        );
        draw_editor_label(
            ctx,
            Rect::new(
                layout.curve.x() + 10.0,
                layout.curve.y() + 8.0,
                layout.curve.width() - 20.0,
                18.0,
            ),
            "Curve",
            11.0,
            Color::rgba(0.84, 0.88, 0.94, 1.0),
        );

        let easing = self
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe(&self.editor, *selection))
            .map(|keyframe| keyframe.easing)
            .unwrap_or(Easing::Linear);
        let graph = Rect::new(
            layout.curve.x() + 14.0,
            layout.curve.y() + 34.0,
            layout.curve.width() - 28.0,
            layout.curve.height() - 48.0,
        );
        ctx.stroke_rect(
            graph,
            Color::rgba(0.26, 0.30, 0.38, 1.0),
            StrokeStyle::new(1.0),
        );
        let mut path = Path::builder();
        for step in 0..=24 {
            let t = step as f32 / 24.0;
            let point = Point::new(
                graph.x() + graph.width() * t,
                graph.max_y() - graph.height() * easing.sample(t),
            );
            if step == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        ctx.stroke(
            path.build(),
            Color::rgba(0.92, 0.70, 0.24, 1.0),
            StrokeStyle::new(2.0),
        );
    }

    fn paint_live_preview(&self, ctx: &mut PaintCtx, layout: AnimationEditorLayout) {
        ctx.fill(
            Path::rounded_rect(layout.preview, 6.0),
            Color::rgba(0.095, 0.105, 0.13, 1.0),
        );
        draw_editor_label(
            ctx,
            Rect::new(
                layout.preview.x() + 10.0,
                layout.preview.y() + 8.0,
                layout.preview.width() - 20.0,
                18.0,
            ),
            "Live preview",
            11.0,
            Color::rgba(0.84, 0.88, 0.94, 1.0),
        );
        let center = Point::new(
            layout.preview.x() + layout.preview.width() * 0.5 + self.presentation.translation.x,
            layout.preview.y() + layout.preview.height() * 0.60 + self.presentation.translation.y,
        );
        ctx.fill(
            Path::circle(center, self.presentation.radius),
            self.presentation.fill.with_alpha(self.presentation.opacity),
        );
        ctx.stroke(
            Path::circle(center, self.presentation.radius + 4.0),
            Color::rgba(1.0, 1.0, 1.0, 0.35 * self.presentation.opacity),
            StrokeStyle::new(1.5),
        );
    }
}

fn animation_editor_layout(bounds: Rect) -> AnimationEditorLayout {
    let x = bounds.x() + 12.0;
    let y = bounds.y() + 36.0;
    let width = (bounds.width() - 24.0).max(1.0);
    let content_bottom = bounds.max_y() - 12.0;
    let button_y = y;
    let timeline = Rect::new(
        x,
        y + 42.0,
        width * 0.62,
        (content_bottom - y - 42.0).max(1.0),
    );
    let side_x = timeline.max_x() + 12.0;
    let side_width = (bounds.max_x() - side_x - 12.0).max(1.0);
    let preview_height = 104.0;
    let inspector_height = 86.0;

    AnimationEditorLayout {
        play_button: Rect::new(x, button_y, 64.0, 28.0),
        undo_button: Rect::new(x + 74.0, button_y, 64.0, 28.0),
        redo_button: Rect::new(x + 148.0, button_y, 64.0, 28.0),
        timeline,
        inspector: Rect::new(side_x, y, side_width, inspector_height),
        curve: Rect::new(
            side_x,
            y + inspector_height + 10.0,
            side_width,
            (content_bottom - y - inspector_height - preview_height - 20.0).max(78.0),
        ),
        preview: Rect::new(
            side_x,
            content_bottom - preview_height,
            side_width,
            preview_height,
        ),
    }
}

fn animation_editor_keyframe_hits(
    editor: &AnimationEditorState,
    layout: AnimationEditorLayout,
) -> Vec<(KeyframeSelection, Rect)> {
    let Some(clip) = editor.document.timeline.clips.first() else {
        return Vec::new();
    };
    let lane_area = Rect::new(
        layout.timeline.x() + 14.0,
        layout.timeline.y() + 34.0,
        layout.timeline.width() - 28.0,
        layout.timeline.height() - 48.0,
    );
    let lane_height = 28.0;
    let mut hits = Vec::new();
    for (track_index, track) in clip.tracks.iter().enumerate() {
        let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
        for (keyframe_index, keyframe) in track.keyframes.iter().enumerate() {
            let x = lane_area.x()
                + lane_area.width()
                    * (keyframe.time / clip.duration.max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
            hits.push((
                KeyframeSelection {
                    clip_index: 0,
                    track_index,
                    keyframe_index,
                },
                Rect::new(x - 7.0, y + (lane_height * 0.5) - 7.0, 14.0, 14.0),
            ));
        }
    }
    hits
}

fn selected_keyframe(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<Keyframe<AnimationValue>> {
    editor
        .document
        .timeline
        .clips
        .get(selection.clip_index)
        .and_then(|clip| clip.tracks.get(selection.track_index))
        .and_then(|track| track.keyframes.get(selection.keyframe_index))
        .copied()
}

fn selected_keyframe_detail(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<String> {
    let clip = editor.document.timeline.clips.get(selection.clip_index)?;
    let track = clip.tracks.get(selection.track_index)?;
    let keyframe = track.keyframes.get(selection.keyframe_index)?;
    Some(format!(
        "{}\ntime {:.2}s, easing {:?}",
        track.binding.property.path(),
        keyframe.time,
        keyframe.easing
    ))
}

fn paint_editor_button(ctx: &mut PaintCtx, rect: Rect, label: &str, active: bool) {
    ctx.fill(
        Path::rounded_rect(rect, 5.0),
        if active {
            Color::rgba(0.25, 0.42, 0.74, 1.0)
        } else {
            Color::rgba(0.16, 0.18, 0.23, 1.0)
        },
    );
    ctx.stroke(
        Path::rounded_rect(rect, 5.0),
        Color::rgba(0.42, 0.48, 0.58, 0.9),
        StrokeStyle::new(1.0),
    );
    draw_editor_label(
        ctx,
        Rect::new(rect.x() + 8.0, rect.y() + 5.0, rect.width() - 16.0, 18.0),
        label,
        10.0,
        Color::rgba(0.92, 0.95, 1.0, 1.0),
    );
}

fn draw_editor_label(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: impl Into<String>,
    size: f32,
    color: Color,
) {
    ctx.draw_text(
        rect,
        text.into(),
        TextStyle {
            font_size: size,
            line_height: size + 4.0,
            color,
            ..TextStyle::default()
        },
    );
}

fn build_animation_demo_panel_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        ANIMATION_DEMO_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            "Animation demo",
            "Exercise built-in hover, focus, and retained overlay motion in one compact surface.",
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Button::new(ANIMATION_DEMO_BUTTON_LABEL)
                        .min_width(220.0)
                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                )
                .with_child(
                    Switch::new(ANIMATION_DEMO_SWITCH_LABEL)
                        .on(true)
                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                )
                .with_child(
                    SizedBox::new()
                        .width(360.0)
                        .height(128.0)
                        .with_child(TimelineAnimationPreview::new()),
                )
                .with_child(
                    SizedBox::new()
                        .width(640.0)
                        .height(336.0)
                        .with_child(AnimationEditorSurface::new()),
                )
                .with_child(
                    SizedBox::new().width(320.0).with_child(
                        TextInput::new(ANIMATION_DEMO_TEXT_INPUT_LABEL)
                            .value("Retained overlay motion")
                            .placeholder("Focus to inspect caret blink")
                            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(240.0).with_child(
                        Tooltip::new(
                            ANIMATION_DEMO_TOOLTIP_TEXT,
                            Button::new(ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL)
                                .min_width(200.0)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    ),
                )
                .with_child(
                    SizedBox::new().width(360.0).with_child(
                        Popover::new(
                            ANIMATION_DEMO_POPOVER_NAME,
                            Button::new(ANIMATION_DEMO_POPOVER_TRIGGER_LABEL)
                                .min_width(220.0)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            Stack::vertical()
                                .spacing(8.0)
                                .alignment(Alignment::Stretch)
                                .with_child(
                                    Label::new(
                                        "Open this popover to validate retained translation + opacity on explicit overlay layers.",
                                    )
                                    .font_size(13.0)
                                    .line_height(18.0)
                                    .color_when(widget_book_theme_color(
                                        &theme_reader,
                                        |theme| theme.palette.text,
                                    )),
                                )
                                .with_child(
                                    Label::new(
                                        "Keep the pointer over the trigger or switch focus to the text input to inspect animation continuation.",
                                    )
                                    .font_size(12.0)
                                    .line_height(17.0)
                                    .color_when(widget_book_theme_color(
                                        &theme_reader,
                                        |theme| theme.palette.text_muted,
                                    )),
                                ),
                        ),
                    ),
                )
                .with_child(
                    Label::new(
                        "Suggested check: hover the shortcuts button, focus the text input, and open the inspector popover while watching the performance overlay in the corner.",
                    )
                    .font_size(13.0)
                    .line_height(18.0)
                    .color_when(widget_book_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
                ),
        ),
    )
}

pub fn build_theme_demo_surface(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
    let scroll_state = ScrollState::new();

    VirtualScrollView::new()
        .name(THEME_DEMO_SCROLL_NAME)
        .state(scroll_state)
        .padding(ROOT_GALLERY_PADDING)
        .spacing(18.0)
        .with_child(
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(THEME_DEMO_TITLE)
                        .font_size(30.0)
                        .line_height(34.0)
                        .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                ))
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(THEME_DEMO_DESCRIPTION)
                        .font_size(15.0)
                        .line_height(20.0)
                        .color(Color::rgba(0.40, 0.48, 0.58, 1.0)),
                )),
        )
        .with_child(panel(
            "Theme preview",
            "Flip the compare toggle to inspect light, dark, and high contrast themes with the same control composition.",
            ThemePreviewShowcase::new(Rc::clone(&state)),
        ))
        .with_child(panel(
            "HDR theme lab",
            "Compare the same tokenized theme across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The first card follows the shared mode currently selected by the dev host.",
            HdrThemeLabShowcase::new(),
        ))
}

pub fn build_widget_book_gallery(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
    build_widget_book_gallery_with_theme(state, default_widget_book_theme_reader())
}

pub fn build_widget_book_gallery_with_theme(
    state: Rc<RefCell<WidgetBookState>>,
    theme_reader: WidgetBookThemeReader,
) -> impl Widget {
    let snapshot = state.borrow().clone();
    let initial_name = snapshot.name.clone();
    let initial_notes = snapshot.notes.clone();
    let initial_subscribed = snapshot.subscribed;
    let initial_switch_on = snapshot.switch_on;
    let initial_standalone_radio = snapshot.standalone_radio_selected;
    let initial_slider_value = snapshot.slider_value;
    let initial_number_value = snapshot.number_value;
    let initial_radio_choice = snapshot.radio_choice.clone();
    let initial_mode = snapshot.mode.clone();
    let initial_tab_bar_choice = snapshot.tab_bar_choice.clone();
    let initial_tabs_choice = snapshot.tabs_choice.clone();

    let name_state = Rc::clone(&state);
    let subscribed_state = Rc::clone(&state);
    let action_state = Rc::clone(&state);
    let icon_action_state = Rc::clone(&state);
    let switch_state = Rc::clone(&state);
    let radio_button_state = Rc::clone(&state);
    let radio_group_state = Rc::clone(&state);
    let slider_state = Rc::clone(&state);
    let number_state = Rc::clone(&state);
    let notes_state = Rc::clone(&state);
    let select_state = Rc::clone(&state);
    let tab_bar_state = Rc::clone(&state);
    let tabs_state = Rc::clone(&state);
    let menu_state = Rc::clone(&state);
    let context_menu_state = Rc::clone(&state);
    let dialog_state = Rc::clone(&state);
    let scroll_state = ScrollState::new();

    let gallery = VirtualScrollView::new()
        .name(GALLERY_SCROLL_NAME)
        .state(scroll_state.clone())
        .padding(ROOT_GALLERY_PADDING)
        .spacing(18.0)
        .with_child(
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(WINDOW_TITLE)
                        .font_size(30.0)
                        .line_height(34.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text
                        })),
                ))
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(
                        "A dedicated widget book for exercising built-in controls, generating inspection artifacts, and providing stable screenshot stories.",
                    )
                    .font_size(15.0)
                    .line_height(20.0)
                    .color_when(widget_book_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
                )),
        )
            .with_child(build_widget_states_gallery_with_theme(Rc::clone(
                &theme_reader,
            )))
            .with_child(build_size_presets_gallery_with_theme(Rc::clone(
                &theme_reader,
            )))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Common controls",
                "These defaults should feel contemporary and light, while still staying dense enough for inspectors, toolbars, and side panels.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        Stack::horizontal()
                            .spacing(14.0)
                            .alignment(Alignment::Start)
                            .with_child(control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Input state",
                                "Text entry and boolean opt-in controls keep their natural widths instead of stretching across the page.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Start)
                                    .with_child(
                                        SizedBox::new().width(300.0).with_child(
                                            TextInput::new(NAME_INPUT_LABEL)
                                                .value(initial_name)
                                                .placeholder("Type your name")
                                                .theme_when(clone_widget_book_theme_reader(
                                                    &theme_reader,
                                                ))
                                                .on_change(move |value| {
                                                    name_state.borrow_mut().name = value;
                                                }),
                                        ),
                                    )
                                    .with_child(
                                        Checkbox::new(SUBSCRIBE_LABEL)
                                            .checked(initial_subscribed)
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            ))
                                            .on_toggle(move |checked| {
                                                subscribed_state.borrow_mut().subscribed = checked;
                                            }),
                                    ),
                            ))
                            .with_child(control_story_with_theme(
                                Rc::clone(&theme_reader),
                                "Primary action",
                                "A dense action plus supporting copy demonstrates the button without turning the row into a full-width form.",
                                Stack::vertical()
                                    .spacing(12.0)
                                    .alignment(Alignment::Start)
                                    .with_child(
                                        SizedBox::new().width(180.0).with_child(
                                            Button::new(PRIMARY_BUTTON_LABEL).on_press(move || {
                                                action_state.borrow_mut().button_presses += 1;
                                            })
                                            .theme_when(clone_widget_book_theme_reader(
                                                &theme_reader,
                                            )),
                                        ),
                                    )
                                    .with_child(
                                        Label::new(
                                            "Related controls should feel like one composed workflow, not separate experiments.",
                                        )
                                        .font_size(13.0)
                                        .line_height(18.0)
                                        .color_when(widget_book_theme_color(
                                            &theme_reader,
                                            |theme| theme.palette.text_muted,
                                        )),
                                    ),
                            )),
                    )
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        Label::new(
                            "The widget book tests capture these controls directly so visual regressions can be reviewed manually or compared automatically.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                    )),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Toolbar pieces",
                "Compact controls, separators, and icons need to feel intentional before any themed application shell exists.",
                control_story_with_theme(
                    Rc::clone(&theme_reader),
                    "Toolbar cluster",
                    "Small controls stay aligned, scannable, and visually grouped before any app-specific toolbar exists.",
                    Stack::vertical()
                        .spacing(14.0)
                        .alignment(Alignment::Start)
                        .with_child(
                            Stack::horizontal()
                                .spacing(14.0)
                                .alignment(Alignment::Center)
                                .with_child(
                                    Icon::new(IconGlyph::Search).label(ICON_LABEL).size(24.0),
                                )
                                .with_child(
                                    IconButton::new(IconGlyph::MoreHorizontal, ICON_BUTTON_LABEL)
                                        .theme_when(clone_widget_book_theme_reader(
                                            &theme_reader,
                                        ))
                                        .on_press(move || {
                                            icon_action_state.borrow_mut().icon_button_presses += 1;
                                        }),
                                )
                                .with_child(
                                    Label::new(
                                        "Icons and icon buttons round out dense toolbar layouts.",
                                    )
                                    .font_size(14.0)
                                    .line_height(18.0)
                                    .color_when(widget_book_theme_color(
                                        &theme_reader,
                                        |theme| theme.palette.text_muted,
                                    )),
                                ),
                        )
                        .with_child(SizedBox::new().width(260.0).with_child(
                            Separator::horizontal()
                                .name(TOOLBAR_SEPARATOR_NAME)
                                .inset(12.0),
                        )),
                ),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Choices and ranges",
                "Desktop-style inspectors rely on switches, radio groups, sliders, numeric inputs, and selects more than oversized form controls.",
                Stack::horizontal()
                    .spacing(14.0)
                    .alignment(Alignment::Start)
                    .with_child(control_story_with_theme(
                        Rc::clone(&theme_reader),
                        "Boolean choices",
                        "Switches and radios are compact choices, so they should sit in a compact inspector-like block.",
                        Stack::vertical()
                            .spacing(12.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                Switch::new(SWITCH_LABEL)
                                    .on(initial_switch_on)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .on_toggle(move |checked| {
                                        switch_state.borrow_mut().switch_on = checked;
                                    }),
                            )
                            .with_child(
                                RadioButton::new(RADIO_BUTTON_LABEL)
                                    .selected(initial_standalone_radio)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .on_select(move || {
                                        radio_button_state.borrow_mut().standalone_radio_selected =
                                            true;
                                    }),
                            )
                            .with_child(
                                SizedBox::new().width(280.0).with_child(
                                    RadioGroup::new(RADIO_GROUP_NAME)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .options(RADIO_OPTIONS)
                                        .selected(
                                            option_index(&RADIO_OPTIONS, &initial_radio_choice)
                                                .unwrap_or(0),
                                        )
                                        .on_change(move |_, value| {
                                            radio_group_state.borrow_mut().radio_choice = value;
                                        }),
                                ),
                            ),
                    ))
                    .with_child(control_story_with_theme(
                        Rc::clone(&theme_reader),
                        "Numeric range",
                        "Slider, spinbox, and select examples now read like inspector controls instead of long page rows.",
                        Stack::vertical()
                            .spacing(12.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                SizedBox::new().width(320.0).with_child(
                                    Slider::new(SLIDER_NAME)
                                        .range(0.0, 100.0)
                                        .step(1.0)
                                        .value(initial_slider_value)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |value| {
                                            slider_state.borrow_mut().slider_value = value;
                                        }),
                                ),
                            )
                            .with_child(
                                SizedBox::new().width(220.0).with_child(
                                    NumberInput::new(NUMBER_INPUT_NAME)
                                        .range(1.0, 256.0)
                                        .step(1.0)
                                        .precision(0)
                                        .value(initial_number_value)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |value| {
                                            number_state.borrow_mut().number_value = value;
                                        }),
                                ),
                            )
                            .with_child(
                                SizedBox::new().width(260.0).with_child(
                                    Select::new(SELECT_NAME)
                                        .placeholder("Choose blend mode")
                                        .options(BLEND_MODE_OPTIONS)
                                        .selected(
                                            option_index(&BLEND_MODE_OPTIONS, &initial_mode)
                                                .unwrap_or(0),
                                        )
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                        .on_change(move |_, value| {
                                            select_state.borrow_mut().mode = value;
                                        }),
                                ),
                            ),
                    )),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Multiline and scroll",
                "The widget book itself now scrolls, and the multiline editor fills the long-form text entry gap for notes, JSON, and small scripting panes.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(420.0).with_child(
                            TextArea::new(TEXT_AREA_LABEL)
                                .min_height(150.0)
                                .value(initial_notes)
                                .placeholder("Write notes")
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .on_change(move |value| {
                                    notes_state.borrow_mut().notes = value;
                                }),
                        ),
                    )
                    .with_child(
                        Label::new(
                            "Use PageDown on the outer scroll view story to capture the lower panels and prove the gallery exceeds the viewport.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                    ),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Typography",
                "Static text is now a real widget too, so the dev host no longer needs to hand-paint every heading and caption.",
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("Section heading")
                            .font_size(22.0)
                            .line_height(26.0)
                            .color_when(widget_book_theme_color(&theme_reader, |theme| {
                                theme.palette.text
                            })),
                    )
                    .with_child(
                        Label::new("Body copy can use the same widget with different size and color settings.")
                            .font_size(15.0)
                            .line_height(20.0)
                            .color_when(widget_book_theme_color(&theme_reader, |theme| {
                                theme.palette.text
                            })),
                    )
                    .with_child(
                        Label::new("Secondary note")
                            .font_size(13.0)
                            .line_height(18.0)
                            .color_when(widget_book_theme_color(&theme_reader, |theme| {
                                theme.palette.text_muted
                            })),
                    ),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Navigation surfaces",
                "Tab bars and tab containers should work for editor chrome and docked inspectors without waiting for a custom application shell.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(520.0).with_child(
                            TabBar::new(TAB_BAR_NAME)
                                .tabs(TAB_BAR_OPTIONS)
                                .selected(option_index(&TAB_BAR_OPTIONS, &initial_tab_bar_choice).unwrap_or(0))
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .on_change(move |_, value| {
                                    tab_bar_state.borrow_mut().tab_bar_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(540.0).height(220.0).with_child(
                            Tabs::new(TABS_NAME)
                                .selected(option_index(&TAB_PANEL_OPTIONS, &initial_tabs_choice).unwrap_or(0))
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .tab(
                                    TAB_PANEL_OPTIONS[0],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Alignment, spacing, and surface geometry controls belong in a compact inspector tab.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text,
                                                    )),
                                            )
                                            .with_child(
                                                ProgressBar::new("Layout completion")
                                                    .range(0.0, 100.0)
                                                    .value(initial_slider_value)
                                                    .show_value(true)
                                                    .theme_when(clone_widget_book_theme_reader(
                                                        &theme_reader,
                                                    )),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[1],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Inline data summaries and editable metadata fit naturally in a reusable tabs widget.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text,
                                                    )),
                                            )
                                            .with_child(
                                                Label::new("Selection: 4 layers, 2 masks, 1 smart object")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text_muted,
                                                    )),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[2],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Undo groups, import checkpoints, and review markers are another common fit for tabbed panels.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text,
                                                    )),
                                            )
                                            .with_child(
                                                Label::new("Replaying history cache")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text_muted,
                                                    )),
                                            ),
                                    ),
                                )
                                .on_change(move |_, value| {
                                    tabs_state.borrow_mut().tabs_choice = value;
                                }),
                        ),
                    ),
            ))
            .with_child(build_animation_demo_panel_with_theme(Rc::clone(&theme_reader)))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Menus and overlays",
                "App menus, context menus, popovers, tooltips, and dialogs are the small but high-value surfaces that make desktop workflows feel complete.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(300.0).with_child(
                            Menu::new(MENU_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .item(MenuItem::new("New tab").shortcut("Ctrl+T"))
                                .item(MenuItem::new("Duplicate panel").shortcut("Ctrl+D"))
                                .item(
                                    MenuItem::new("Delete layer")
                                        .shortcut("Del")
                                        .separator_before()
                                        .destructive(),
                                )
                                .on_activate(move |_, item| {
                                    menu_state.borrow_mut().last_menu_action = item.label().to_string();
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ContextMenu::new(
                                CONTEXT_MENU_NAME,
                                Background::new(
                                    theme_reader().palette.control,
                                    Padding::all(
                                        14.0,
                                        Label::new("Right-click this explicit surface")
                                            .font_size(14.0)
                                            .line_height(18.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text,
                                            )),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.control,
                                )),
                            )
                            .theme_when(clone_widget_book_theme_reader(&theme_reader))
                            .item(MenuItem::new("Rename"))
                            .item(MenuItem::new("Duplicate"))
                            .item(MenuItem::new("Delete").separator_before().destructive())
                            .on_activate(move |_, item| {
                                context_menu_state.borrow_mut().last_context_action = item.label().to_string();
                            }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            Tooltip::new(
                                TOOLTIP_TEXT,
                                Button::new(TOOLTIP_TRIGGER_LABEL)
                                    .min_width(180.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(360.0).with_child(
                            Popover::new(
                                POPOVER_NAME,
                                Button::new(POPOVER_TRIGGER_LABEL)
                                    .min_width(190.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        Label::new("Inline inspector content can stay lightweight instead of forcing a full modal.")
                                            .font_size(14.0)
                                            .line_height(19.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text,
                                            )),
                                    )
                                    .with_child(
                                        Label::new("Blend preview: Screen @ 72%")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text_muted,
                                            )),
                                    ),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(560.0).with_child(
                            ProjectSettingsPreview::new(dialog_state),
                        ),
                    ),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Progress and busy",
                "Progress bars and busy indicators are simple, but they anchor long-running exports, caching, and background processing workflows.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ProgressBar::new(PROGRESS_NAME)
                                .range(0.0, 100.0)
                                .value(initial_slider_value)
                                .show_value(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            Spinner::new(SPINNER_NAME)
                                .label(SPINNER_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Live state",
                "This summary reads state produced by reusable controls so screenshot stories can cover both isolated widgets and composed UI.",
                WidgetBookSummary::new(state, Rc::clone(&theme_reader)),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Live performance overlay",
                "The stats card now floats over the gallery so frame timing stays visible while you inspect any part of the widget book.",
                Label::new(
                    "Use the compact panel pinned in the top-right corner while you scroll and interact with the rest of the gallery.",
                )
                .font_size(13.0)
                .line_height(18.0)
                .color_when(widget_book_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Debugging and inspection",
                "The sui-debug crate composes reusable diagnostics chrome with SUI-specific views over focus, semantics, widget graph, and scene summaries.",
                Label::new(
                    "Debug inspector available via sui-debug crate. Open the standalone debug view for full semantics, widget graph, and scene inspection."
                )
                .font_size(13.0)
                .line_height(18.0)
                .color_when(widget_book_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Collections and hierarchy",
                "Foundational editor widgets need to cover lists, trees, and structured tables without requiring app-specific shells first.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(360.0).height(220.0).with_child(
                            ListView::new(LIST_VIEW_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .items([
                                    ListItem::new("Hero texture").detail("2048 x 2048 RGBA").accent(Color::rgba(0.16, 0.54, 0.88, 1.0)),
                                    ListItem::new("Normals atlas").detail("Streaming mip chain"),
                                    ListItem::new("Glass material").detail("Referenced in 3 prefabs"),
                                    ListItem::new("UI icon sheet").detail("Tagged for export").accent(Color::rgba(0.78, 0.50, 0.17, 1.0)),
                                    ListItem::new("Archive cache").detail("Read only").disabled(),
                                ])
                                .selected(1),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(420.0).height(240.0).with_child(
                            TreeView::new(TREE_VIEW_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .items([
                                    TreeItem::new("Scene")
                                        .expanded(true)
                                        .with_child(
                                            TreeItem::new("Environment")
                                                .expanded(true)
                                                .with_child(TreeItem::new("Sky dome").detail("Visible"))
                                                .with_child(TreeItem::new("Fog volume").detail("Animated")),
                                        )
                                        .with_child(
                                            TreeItem::new("Characters")
                                                .expanded(true)
                                                .with_child(TreeItem::new("Pilot").detail("Selected"))
                                                .with_child(TreeItem::new("Companion drone")),
                                        )
                                        .with_child(TreeItem::new("FX").detail("Collapsed group")),
                                ]),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(250.0).with_child(
                            Table::new(TABLE_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .columns([
                                    TableColumn::new("Material"),
                                    TableColumn::new("Domain").width(120.0),
                                    TableColumn::new("Shader").width(180.0),
                                    TableColumn::new("Passes").width(90.0).alignment(TableColumnAlignment::End),
                                    TableColumn::new("Last edit").width(130.0),
                                ])
                                .rows([
                                    TableRow::new(["ClearCoat_Glass", "Surface", "pbr.clearcoat", "3", "2 min ago"]),
                                    TableRow::new(["Terrain_Master", "Surface", "terrain.layered", "5", "11 min ago"]),
                                    TableRow::new(["UI_Highlight", "Overlay", "ui.gradient", "1", "24 min ago"]),
                                    TableRow::new(["CloudShadow", "Decal", "fx.projected", "2", "1 hour ago"]),
                                    TableRow::new(["Water_Foam", "Surface", "water.foam", "4", "yesterday"]),
                                ])
                                .selected(2),
                        ),
                    ),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Layout and pathing",
                "Editor shells need split panes and breadcrumb-style navigation before the rest of the UI can settle into place.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(620.0).with_child(
                            Breadcrumb::new(BREADCRUMB_NAME)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                .items([
                                    BreadcrumbItem::new("Workspace"),
                                    BreadcrumbItem::new("Projects"),
                                    BreadcrumbItem::new("Starfall"),
                                    BreadcrumbItem::new("Materials"),
                                    BreadcrumbItem::new("Glass"),
                                ])
                                .current(4),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(240.0).with_child(
                            SplitView::horizontal(
                                Background::new(
                                    theme_reader().palette.control,
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Viewport")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text,
                                                    )),
                                            )
                                            .with_child(
                                                Label::new("Resizable panes let editor shells settle into familiar two-up and inspector layouts.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text_muted,
                                                    )),
                                            ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.control,
                                )),
                                Background::new(
                                    theme_reader().palette.surface_raised,
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Inspector")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text,
                                                    )),
                                            )
                                            .with_child(
                                                Label::new("Drag the divider to rebalance the viewport and detail pane without custom shell code.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color_when(widget_book_theme_color(
                                                        &theme_reader,
                                                        |theme| theme.palette.text_muted,
                                                    )),
                                            ),
                                    ),
                                )
                                .brush_when(widget_book_theme_color(
                                    &theme_reader,
                                    |theme| theme.palette.surface_raised,
                                )),
                            )
                            .name(SPLIT_VIEW_NAME)
                            .ratio(0.62),
                        ),
                    ),
            ))
            .with_child(build_color_and_imagery_story_with_theme(Rc::clone(
                &theme_reader,
            )));

    WidgetBookGalleryScrollPane::new(
        gallery,
        ScrollBar::vertical(scroll_state).name(GALLERY_SCROLL_BAR_NAME),
    )
}

fn build_size_presets_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        SIZE_PRESETS_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            SIZE_PRESETS_GALLERY_NAME,
            "Density presets resize the same supported widgets for compact inspectors, comfortable desktop controls, and touch-friendly surfaces.",
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Start)
                        .with_child(density_preset_column_with_theme(
                            Rc::clone(&theme_reader),
                            ThemeDensity::Compact,
                        ))
                        .with_child(density_preset_column_with_theme(
                            Rc::clone(&theme_reader),
                            ThemeDensity::Comfortable,
                        ))
                        .with_child(density_preset_column_with_theme(
                            Rc::clone(&theme_reader),
                            ThemeDensity::Touch,
                        )),
                )
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(
                        "These samples use each widget's theme-aware defaults instead of fixed demo slots, so height, padding, icons, overlays, tabs, and command rows can be compared directly.",
                    )
                    .font_size(13.0)
                    .line_height(18.0)
                    .color_when(widget_book_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
                )),
        ),
    )
}

fn density_preset_column_with_theme(
    theme_reader: WidgetBookThemeReader,
    density: ThemeDensity,
) -> impl Widget {
    let title = density_preset_title(density);
    let action_label = density_preset_action_label(density);
    let input_label = density_preset_input_label(density);
    let switch_label = format!("{title} preset switch");
    let checkbox_label = format!("{title} preset checkbox");
    let select_name = format!("{title} preset select");
    let slider_name = format!("{title} preset slider");
    let tab_name = format!("{title} preset tabs");
    let preset_name = format!("{title} preset strip");
    let toolbar_name = format!("{title} preset toolbar");

    SizedBox::new().width(300.0).with_child(
        StoryCard::new(
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Start)
                .with_child(
                    Label::new(title)
                        .font_size(15.0)
                        .line_height(19.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text
                        })),
                )
                .with_child(MaximumWidth::new(
                    250.0,
                    Label::new(density_preset_caption(density))
                        .font_size(12.0)
                        .line_height(16.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                ))
                .with_child(
                    Button::new(action_label)
                        .icon(IconGlyph::Check)
                        .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                )
                .with_child(
                    SizedBox::new().width(230.0).with_child(
                        TextInput::new(input_label)
                            .value("Layer name")
                            .leading_icon(IconGlyph::Search)
                            .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                    ),
                )
                .with_child(
                    Checkbox::new(checkbox_label)
                        .checked(true)
                        .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                )
                .with_child(
                    Switch::new(switch_label)
                        .on(true)
                        .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                )
                .with_child(
                    SizedBox::new().width(230.0).with_child(
                        Slider::new(slider_name)
                            .range(0.0, 100.0)
                            .value(64.0)
                            .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(230.0).with_child(
                        Select::new(select_name)
                            .options(BLEND_MODE_OPTIONS)
                            .selected(1)
                            .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(250.0).with_child(
                        TabBar::new(tab_name)
                            .tabs(["Canvas", "Inspect"])
                            .selected(1)
                            .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                    ),
                )
                .with_child(
                    PresetStrip::new(preset_name)
                        .presets(["8 px", "18 px", "36 px"])
                        .selected(1)
                        .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
                )
                .with_child(
                    Toolbar::horizontal()
                        .name(toolbar_name)
                        .theme_when(widget_book_density_theme_reader(&theme_reader, density))
                        .with_child(
                            IconButton::new(IconGlyph::Undo, format!("{title} preset undo"))
                                .theme_when(widget_book_density_theme_reader(
                                    &theme_reader,
                                    density,
                                )),
                        )
                        .with_child(
                            IconButton::new(IconGlyph::Redo, format!("{title} preset redo"))
                                .theme_when(widget_book_density_theme_reader(
                                    &theme_reader,
                                    density,
                                )),
                        )
                        .with_child(
                            Button::new(format!("{title} preset apply")).theme_when(
                                widget_book_density_theme_reader(&theme_reader, density),
                            ),
                        ),
                ),
        )
        .theme_when(widget_book_density_theme_reader(&theme_reader, density)),
    )
}

fn density_preset_title(density: ThemeDensity) -> &'static str {
    match density {
        ThemeDensity::Compact => "Compact",
        ThemeDensity::Comfortable => "Comfortable",
        ThemeDensity::Touch => "Touch",
    }
}

fn density_preset_caption(density: ThemeDensity) -> &'static str {
    match density {
        ThemeDensity::Compact => "Dense inspector and toolbar layouts.",
        ThemeDensity::Comfortable => "Default desktop application sizing.",
        ThemeDensity::Touch => "Larger targets for pointer and touch input.",
    }
}

fn density_preset_action_label(density: ThemeDensity) -> &'static str {
    match density {
        ThemeDensity::Compact => SIZE_PRESET_COMPACT_ACTION_LABEL,
        ThemeDensity::Comfortable => SIZE_PRESET_COMFORTABLE_ACTION_LABEL,
        ThemeDensity::Touch => SIZE_PRESET_TOUCH_ACTION_LABEL,
    }
}

fn density_preset_input_label(density: ThemeDensity) -> &'static str {
    match density {
        ThemeDensity::Compact => SIZE_PRESET_COMPACT_INPUT_LABEL,
        ThemeDensity::Comfortable => SIZE_PRESET_COMFORTABLE_INPUT_LABEL,
        ThemeDensity::Touch => SIZE_PRESET_TOUCH_INPUT_LABEL,
    }
}

fn build_widget_states_gallery_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    NamedSection::new(
        WIDGET_STATES_GALLERY_NAME,
        panel_with_theme(
            Rc::clone(&theme_reader),
            WIDGET_STATES_GALLERY_NAME,
            "Compact state matrix for the core controls. Each sample uses the same theme tokens so density, alignment, focus chrome, and overlay spacing can be reviewed together.",
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Stretch)
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Actions",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Default",
                            Button::new(WIDGET_STATES_BUTTON_LABEL)
                                .icon(IconGlyph::Check)
                                .min_width(170.0)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Selected",
                            IconButton::new(IconGlyph::Hand, WIDGET_STATES_ICON_BUTTON_LABEL)
                                .selected(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Disabled",
                            Button::new("Disabled action")
                                .icon(IconGlyph::Lock)
                                .min_width(170.0)
                                .enabled(false)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        )),
                    "Text entry",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Placeholder",
                            SizedBox::new().width(240.0).with_child(
                                TextInput::new(WIDGET_STATES_TEXT_INPUT_LABEL)
                                    .placeholder("Search layers")
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Value",
                            SizedBox::new().width(240.0).with_child(
                                TextInput::new("States text input value")
                                    .value("Layer 08 / mask")
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Multiline",
                            SizedBox::new().width(240.0).with_child(
                                TextArea::new(WIDGET_STATES_TEXT_AREA_LABEL)
                                    .value("Frame notes\nOpacity ramp is locked")
                                    .min_height(72.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )),
                ))
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Choices",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Unchecked",
                            Checkbox::new(WIDGET_STATES_CHECKBOX_LABEL)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Checked",
                            Checkbox::new("States checkbox checked")
                                .checked(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Switch on",
                            Switch::new(WIDGET_STATES_SWITCH_LABEL)
                                .on(true)
                                .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        )),
                    "Ranges and selects",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Low value",
                            SizedBox::new().width(240.0).with_child(
                                Slider::new(WIDGET_STATES_SLIDER_NAME)
                                    .range(0.0, 100.0)
                                    .value(28.0)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Selected",
                            SizedBox::new().width(240.0).with_child(
                                Select::new(WIDGET_STATES_SELECT_NAME)
                                    .placeholder("Blend mode")
                                    .options(BLEND_MODE_OPTIONS)
                                    .selected(2)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Expanded",
                            SizedBox::new().width(240.0).with_child(
                                Select::new("States select expanded")
                                    .options(BLEND_MODE_OPTIONS)
                                    .selected(1)
                                    .expanded(true)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                            ),
                        )),
                ))
                .with_child(widget_state_row_with_theme(
                    Rc::clone(&theme_reader),
                    "Navigation",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Tabs",
                            SizedBox::new().width(300.0).with_child(
                                Tabs::new(WIDGET_STATES_TABS_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .selected(1)
                                    .tab(
                                        "Canvas",
                                        Label::new("Viewport controls")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text_muted,
                                            )),
                                    )
                                    .tab(
                                        "Inspector",
                                        Label::new("Selected layer properties")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text_muted,
                                            )),
                                    )
                                    .tab(
                                        "Export",
                                        Label::new("Preset summary")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color_when(widget_book_theme_color(
                                                &theme_reader,
                                                |theme| theme.palette.text_muted,
                                            )),
                                    ),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Menu",
                            SizedBox::new().width(260.0).with_child(
                                Menu::new(WIDGET_STATES_MENU_NAME)
                                    .theme_when(clone_widget_book_theme_reader(&theme_reader))
                                    .highlighted(1)
                                    .items([
                                        MenuItem::new("Rename").shortcut("Enter"),
                                        MenuItem::new("Duplicate").shortcut("Ctrl+D"),
                                        MenuItem::new("Bake preview").disabled(),
                                        MenuItem::new("Delete").separator_before().destructive(),
                                    ]),
                            ),
                        )),
                    "Overlays",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Start)
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Closed popover",
                            SizedBox::new().width(260.0).with_child(
                                Popover::new(
                                    WIDGET_STATES_POPOVER_NAME,
                                    Button::new("Open details")
                                        .min_width(180.0)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                    Label::new("Hidden until opened"),
                                )
                                .theme(theme_reader()),
                            ),
                        ))
                        .with_child(state_sample_with_theme(
                            Rc::clone(&theme_reader),
                            "Open popover",
                            SizedBox::new().width(260.0).with_child(
                                Popover::new(
                                    "States popover open",
                                    Button::new("Details open")
                                        .min_width(180.0)
                                        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                                    Stack::vertical()
                                        .spacing(6.0)
                                        .alignment(Alignment::Start)
                                        .with_child(
                                            Label::new("Layer blend")
                                                .font_size(13.0)
                                                .line_height(18.0)
                                                .color_when(widget_book_theme_color(
                                                    &theme_reader,
                                                    |theme| theme.palette.text,
                                                )),
                                        )
                                        .with_child(
                                            Label::new("Screen, 72% opacity")
                                                .font_size(12.0)
                                                .line_height(16.0)
                                                .color_when(widget_book_theme_color(
                                                    &theme_reader,
                                                    |theme| theme.palette.text_muted,
                                                )),
                                        ),
                                )
                                .theme(theme_reader())
                                .open(true),
                            ),
                        )),
                )),
        ),
    )
}

fn widget_state_row_with_theme<L, R>(
    theme_reader: WidgetBookThemeReader,
    left_title: &'static str,
    left_body: L,
    right_title: &'static str,
    right_body: R,
) -> impl Widget
where
    L: Widget + 'static,
    R: Widget + 'static,
{
    let separator_theme = Rc::clone(&theme_reader);
    SizedBox::new().width(WIDGET_STATE_ROW_WIDTH).with_child(
        StoryCard::new(
            Stack::horizontal()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(widget_state_column_with_theme(
                    Rc::clone(&theme_reader),
                    left_title,
                    left_body,
                ))
                .with_child(
                    Separator::vertical()
                        .theme_when(move || separator_theme())
                        .inset(0.0),
                )
                .with_child(widget_state_column_with_theme(
                    Rc::clone(&theme_reader),
                    right_title,
                    right_body,
                )),
        )
        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
    )
}

fn widget_state_column_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    SizedBox::new().width(WIDGET_STATE_COLUMN_WIDTH).with_child(
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new(title)
                    .font_size(14.0)
                    .line_height(18.0)
                    .color_when(widget_book_theme_color(&theme_reader, |theme| {
                        theme.palette.text
                    })),
            )
            .with_child(body),
    )
}

fn state_sample_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    state: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(5.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new(state)
                .font_size(11.0)
                .line_height(14.0)
                .color_when(widget_book_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(body)
}

#[cfg(test)]
fn build_color_and_imagery_story() -> impl Widget {
    build_color_and_imagery_story_with_theme(default_widget_book_theme_reader())
}

fn build_color_and_imagery_story_with_theme(theme_reader: WidgetBookThemeReader) -> impl Widget {
    panel_with_theme(
        Rc::clone(&theme_reader),
        "Color and imagery",
        "SUI targets visual tooling, so swatches, a usable picker, and image previews need to exist as first-class widgets.",
        Stack::vertical()
            .spacing(16.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Stack::horizontal()
                    .spacing(12.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        ColorSwatch::new(COLOR_SWATCH_NAME, Color::rgba(0.12, 0.55, 0.88, 1.0))
                            .size(Size::new(64.0, 36.0)),
                    )
                    .with_child(
                        ColorSwatch::new("Shadow swatch", Color::rgba(0.08, 0.10, 0.14, 0.84))
                            .size(Size::new(64.0, 36.0)),
                    )
                    .with_child(
                        Label::new(
                            "Use swatches for palettes, material chips, and compact property rows.",
                        )
                        .font_size(14.0)
                        .line_height(18.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                    ),
            )
            .with_child(
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        SizedBox::new().width(434.0).height(448.0).with_child(
                            ColorPicker::from_color(
                                COLOR_PICKER_NAME,
                                Color::new(sui::ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
                            )
                            .theme_when(clone_widget_book_theme_reader(&theme_reader)),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).height(220.0).with_child(
                            Image::new(WIDGET_BOOK_IMAGE_HANDLE)
                                .label(DEMO_IMAGE_LABEL)
                                .fit(ImageFit::Contain)
                                .background_when(widget_book_theme_color(&theme_reader, |theme| {
                                    theme.palette.control
                                }))
                                .corner_radius(12.0),
                        ),
                    ),
            ),
    )
}

pub fn build_animation_benchmark() -> impl Widget {
    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(AnimationBenchmarkRetainedLane::new())
            .with_child(AnimationBenchmarkRepaintLane::new())
            .with_child(AnimationBenchmarkScaleGrid::new()),
    )
}

pub fn build_animation_benchmark_application() -> Application {
    App::new()
        .window(Window::new(ANIMATION_BENCHMARK_TITLE).root(build_animation_benchmark()))
        .into_application()
}

pub fn build_retained_text_benchmark() -> impl Widget {
    const SECTION_COUNT: usize = 72;
    const PARAGRAPHS_PER_SECTION: usize = 4;

    let scroll_state = ScrollState::new();
    let mut content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(panel(
        "Retained text wall",
        "Focused benchmark surface for measuring text-heavy cached scroll regeneration without the live overlay or mixed control chrome.",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "The outer scroll view stays retained, the visible content stays dominated by wrapped labels, and the benchmark scrolls through enough sections to keep retained packet rebuilds focused on atlas text payloads.",
                    )
                    .font_size(14.0)
                    .line_height(20.0)
                    .color(Color::rgba(0.38, 0.46, 0.56, 1.0)),
                ),
            )
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "Each section deliberately uses several long paragraphs so the per-frame upload delta is shaped by text submission rather than button chrome, icons, or image content.",
                    )
                    .font_size(14.0)
                    .line_height(20.0)
                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                ),
            ),
    ));

    for section_index in 0..SECTION_COUNT {
        let (title, subtitle) = retained_text_benchmark_section(section_index);
        let mut body = Stack::vertical().spacing(8.0).alignment(Alignment::Stretch);

        for paragraph_index in 0..PARAGRAPHS_PER_SECTION {
            body = body.with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(retained_text_benchmark_paragraph(
                        section_index,
                        paragraph_index,
                    ))
                    .font_size(14.0)
                    .line_height(20.0)
                    .color(Color::rgba(0.36, 0.44, 0.53, 1.0)),
                ),
            );
        }

        content = content.with_child(Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new(title)
                            .font_size(20.0)
                            .line_height(24.0)
                            .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                    )
                    .with_child(
                        Label::new(subtitle)
                            .font_size(14.0)
                            .line_height(19.0)
                            .color(Color::rgba(0.44, 0.51, 0.60, 1.0)),
                    )
                    .with_child(body),
            ),
        ));
    }

    VerticalScrollPane::new(
        ScrollView::vertical(Padding::all(
            24.0,
            SizedBox::new().width(948.0).with_child(content),
        ))
        .state(scroll_state.clone())
        .name(RETAINED_TEXT_BENCHMARK_SCROLL_NAME),
        ScrollBar::vertical(scroll_state).name(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME),
    )
}

pub fn build_retained_text_benchmark_application() -> Application {
    App::new()
        .window(Window::new(RETAINED_TEXT_BENCHMARK_TITLE).root(build_retained_text_benchmark()))
        .into_application()
}

pub fn build_text_rendering_comparison_surface() -> impl Widget {
    let scroll_state = ScrollState::new();
    let mut mode_grid = Stack::vertical()
        .spacing(14.0)
        .alignment(Alignment::Stretch);

    for row in TEXT_RENDERING_MODE_DATA.chunks(2) {
        let mut row_stack = Stack::horizontal()
            .spacing(14.0)
            .alignment(Alignment::Start);
        for &(title, subtitle, notes) in row {
            row_stack =
                row_stack.with_child(build_text_rendering_mode_card(title, subtitle, notes));
        }
        mode_grid = mode_grid.with_child(row_stack);
    }

    let content = MinimumWidth::new(
        TEXT_RENDERING_COMPARISON_MIN_WIDTH,
        Padding::all(
            20.0,
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(panel(
                    "Text rendering matrix",
                    "A compact visual QA surface for comparing small UI text policies across light and dark surfaces.",
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Center)
                        .with_child(build_text_rendering_summary_metric(
                            "Modes",
                            "6",
                            "coverage, hinting, LCD, darkening",
                        ))
                        .with_child(build_text_rendering_summary_metric(
                            "Pairs",
                            "2",
                            "light and dark contrast checks",
                        ))
                        .with_child(build_text_rendering_summary_metric(
                            "Stress",
                            "11-16 px",
                            "dense labels and status text",
                        )),
                ))
                .with_child(mode_grid),
        ),
    );

    TwoAxisScrollPane::new(
        scroll_state.clone(),
        ScrollView::both(content)
            .state(scroll_state.clone())
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Auto)
            .name(TEXT_RENDERING_COMPARISON_SCROLL_NAME),
        ScrollBar::vertical(scroll_state.clone())
            .name(TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME),
        ScrollBar::horizontal(scroll_state)
            .name(TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME),
    )
}

pub fn build_text_rendering_comparison_application() -> Application {
    App::new()
        .window(Window::new(TEXT_RENDERING_COMPARISON_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_RENDERING_COMPARISON_TITLE,
                "Side-by-side validation surface for grayscale, hinted, darkened, and LCD-oriented text rendering modes.",
                build_text_rendering_comparison_surface(),
            ),
        ))
        .into_application()
}

pub fn build_color_validation_surface() -> impl Widget {
    const COLOR_VALIDATION_MIN_CONTENT_WIDTH: f32 = 780.0;
    const COLOR_VALIDATION_SWATCH_MIN_WIDTH: f32 = 150.0;

    let scroll_state = ScrollState::new();
    let content = MinimumWidth::new(
        COLOR_VALIDATION_MIN_CONTENT_WIDTH,
        Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(panel(
                "HDR brightness and clipping probes",
                "Start here when checking HDR. These rows show whether values above SDR reference white stay visually distinct. On SDR or clamp-heavy paths, the brighter swatches may collapse together. On HDR-capable paths, higher steps should remain separable and retain highlight structure.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(build_color_validation_quad_row(
                        "HDR white ladder",
                        "Reference white is 1.0. Higher linear-light steps intentionally exceed SDR range. If 2.0, 4.0, and 8.0 all look identical, the path is clipping or tone mapping aggressively.",
                        [
                            ("Reference white 1.0", Color::linear_rgba(1.0, 1.0, 1.0, 1.0)),
                            ("Highlight white 2.0", Color::linear_rgba(2.0, 2.0, 2.0, 1.0)),
                            ("Highlight white 4.0", Color::linear_rgba(4.0, 4.0, 4.0, 1.0)),
                            ("Highlight white 8.0", Color::linear_rgba(8.0, 8.0, 8.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_quad_row(
                        "HDR color highlight ladder",
                        "Colored highlights help catch cases where luminance is preserved but saturation shifts unexpectedly. Compare how orange and cyan energy above 1.0 behaves relative to SDR-bright controls.",
                        [
                            ("Orange highlight 1.0", Color::linear_rgba(1.0, 0.55, 0.18, 1.0)),
                            ("Orange highlight 2.0", Color::linear_rgba(2.0, 1.1, 0.36, 1.0)),
                            ("Cyan highlight 1.0", Color::linear_rgba(0.20, 0.80, 1.0, 1.0)),
                            ("Cyan highlight 2.0", Color::linear_rgba(0.40, 1.60, 2.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row(
                        "SDR clipping reference",
                        "This pair makes SDR clipping easy to spot. If the boosted sample looks no brighter than the baseline, the path is still constrained to SDR output at this stage.",
                        [
                            ("SDR white baseline", Color::linear_rgba(1.0, 1.0, 1.0, 1.0)),
                            ("SDR clipped white 2.0", Color::linear_rgba(2.0, 2.0, 2.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    )),
            ))
            .with_child(panel(
                "Wide-gamut reference swatches",
                "Use these after the HDR ladder. This surface validates that sRGB and Display-P3 colors stay distinct in the renderer's linear working space before final display output.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(build_color_validation_row(
                        "Red primary",
                        "Display-P3 red should preserve its native primaries instead of being treated as an sRGB red with only transfer decoding.",
                        [
                            ("sRGB reference red", Color::rgba(1.0, 0.0, 0.0, 1.0)),
                            ("Display P3 reference red", Color::display_p3(1.0, 0.0, 0.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row(
                        "Green primary",
                        "The Display-P3 green sample intentionally lives outside the sRGB gamut. Compare it against the clipped sRGB control when checking wide-gamut correctness.",
                        [
                            ("sRGB clipped lime", Color::rgba(0.0, 1.0, 0.0, 1.0)),
                            ("Display P3 vivid lime", Color::display_p3(0.0, 1.0, 0.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row(
                        "Cyan accent mix",
                        "A mixed-color sample helps catch cases where Display-P3 is incorrectly reduced to transfer decoding only. The P3 version should retain a more vivid cyan accent on wide-gamut outputs.",
                        [
                            ("sRGB accent cyan", Color::rgba(0.0, 0.78, 1.0, 1.0)),
                            ("Display P3 accent cyan", Color::display_p3(0.0, 0.78, 1.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    )),
            )),
    ));

    TwoAxisScrollPane::new(
        scroll_state.clone(),
        ScrollView::both(content)
            .state(scroll_state.clone())
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Auto)
            .name(COLOR_VALIDATION_SCROLL_NAME),
        ScrollBar::vertical(scroll_state.clone()).name(COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME),
        ScrollBar::horizontal(scroll_state).name(COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME),
    )
}

pub fn build_color_validation_application() -> Application {
    App::new()
        .window(Window::new(COLOR_VALIDATION_VIEW_TITLE).root(
            LivePerformanceRoot::new(
                COLOR_VALIDATION_VIEW_TITLE,
                "Reference surface for validating wide-gamut color handling, HDR brightness separation, and SDR clipping behavior while native HDR support lands in phases.",
                build_color_validation_surface(),
            ),
        ))
        .into_application()
}

fn build_color_validation_row(
    title: &'static str,
    description: &'static str,
    swatches: [(&'static str, Color); 2],
    swatch_min_width: f32,
) -> impl Widget {
    NamedSection::new(
        title,
        Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new(title)
                            .font_size(18.0)
                            .line_height(22.0)
                            .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                    )
                    .with_child(
                        Label::new(description)
                            .font_size(14.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.40, 0.47, 0.56, 1.0)),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(18.0)
                            .alignment(Alignment::Center)
                            .with_child(build_color_validation_swatch(
                                swatches[0].0,
                                swatches[0].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch(
                                swatches[1].0,
                                swatches[1].1,
                                swatch_min_width,
                            )),
                    ),
            ),
        ),
    )
}

fn build_color_validation_quad_row(
    title: &'static str,
    description: &'static str,
    swatches: [(&'static str, Color); 4],
    swatch_min_width: f32,
) -> impl Widget {
    NamedSection::new(
        title,
        Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new(title)
                            .font_size(18.0)
                            .line_height(22.0)
                            .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                    )
                    .with_child(
                        Label::new(description)
                            .font_size(14.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.40, 0.47, 0.56, 1.0)),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(18.0)
                            .alignment(Alignment::Center)
                            .with_child(build_color_validation_swatch(
                                swatches[0].0,
                                swatches[0].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch(
                                swatches[1].0,
                                swatches[1].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch(
                                swatches[2].0,
                                swatches[2].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch(
                                swatches[3].0,
                                swatches[3].1,
                                swatch_min_width,
                            )),
                    ),
            ),
        ),
    )
}

fn build_color_validation_swatch(name: &'static str, color: Color, min_width: f32) -> impl Widget {
    MinimumWidth::new(
        min_width,
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Center)
            .with_child(ColorSwatch::new(name, color).size(Size::new(132.0, 56.0)))
            .with_child(
                Label::new(name)
                    .font_size(13.0)
                    .line_height(18.0)
                    .color(Color::rgba(0.16, 0.21, 0.28, 1.0)),
            ),
    )
}

fn build_text_rendering_mode_card(
    title: &'static str,
    subtitle: &'static str,
    notes: &'static str,
) -> impl Widget {
    NamedSection::new(
        title,
        SizedBox::new()
            .width(TEXT_RENDERING_COMPARISON_CARD_WIDTH)
            .with_child(Background::new(
                Color::rgba(0.985, 0.99, 1.0, 1.0),
                Padding::all(
                    14.0,
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new(title)
                                .font_size(17.0)
                                .line_height(21.0)
                                .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                        )
                        .with_child(MaximumWidth::new(
                            480.0,
                            Label::new(subtitle)
                                .font_size(12.0)
                                .line_height(16.0)
                                .color(Color::rgba(0.44, 0.51, 0.60, 1.0)),
                        ))
                        .with_child(
                            Stack::horizontal()
                                .spacing(10.0)
                                .alignment(Alignment::Start)
                                .with_child(build_text_rendering_sample_tile(
                                    format!("{title} light sample"),
                                    "Light",
                                    false,
                                ))
                                .with_child(build_text_rendering_sample_tile(
                                    format!("{title} dark sample"),
                                    "Dark",
                                    true,
                                )),
                        )
                        .with_child(MaximumWidth::new(
                            480.0,
                            Label::new(notes)
                                .font_size(12.0)
                                .line_height(16.0)
                                .color(Color::rgba(0.41, 0.48, 0.56, 1.0)),
                        )),
                ),
            )),
    )
}

fn build_text_rendering_summary_metric(
    label: &'static str,
    value: &'static str,
    caption: &'static str,
) -> impl Widget {
    SizedBox::new().width(210.0).with_child(StoryCard::new(
        Stack::vertical()
            .spacing(5.0)
            .alignment(Alignment::Start)
            .with_child(
                Label::new(label)
                    .font_size(11.0)
                    .line_height(14.0)
                    .color(Color::rgba(0.48, 0.55, 0.64, 1.0)),
            )
            .with_child(
                Label::new(value)
                    .font_size(18.0)
                    .line_height(21.0)
                    .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
            )
            .with_child(
                Label::new(caption)
                    .font_size(11.0)
                    .line_height(14.0)
                    .color(Color::rgba(0.38, 0.45, 0.54, 1.0)),
            ),
    ))
}

fn build_text_rendering_sample_tile(
    name: impl Into<String>,
    label: &'static str,
    dark: bool,
) -> impl Widget {
    let background = if dark {
        Color::rgba(0.12, 0.16, 0.22, 1.0)
    } else {
        Color::rgba(0.995, 0.998, 1.0, 1.0)
    };
    let label_color = if dark {
        Color::rgba(0.70, 0.78, 0.86, 1.0)
    } else {
        Color::rgba(0.42, 0.49, 0.57, 1.0)
    };
    let primary_color = if dark {
        Color::rgba(0.96, 0.98, 1.0, 1.0)
    } else {
        Color::rgba(0.10, 0.14, 0.20, 1.0)
    };
    let secondary_color = if dark {
        Color::rgba(0.82, 0.88, 0.95, 1.0)
    } else {
        Color::rgba(0.18, 0.24, 0.32, 1.0)
    };

    NamedSection::new(
        name,
        SizedBox::new()
            .width(TEXT_RENDERING_SAMPLE_TILE_WIDTH)
            .height(TEXT_RENDERING_SAMPLE_TILE_HEIGHT)
            .with_child(Background::new(
                background,
                Padding::all(
                    12.0,
                    Stack::vertical()
                        .spacing(7.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new(label)
                                .font_size(11.0)
                                .line_height(14.0)
                                .color(label_color),
                        )
                        .with_child(
                            Label::new("minimum ill scroll")
                                .font_size(12.0)
                                .line_height(15.0)
                                .color(primary_color),
                        )
                        .with_child(
                            Label::new("Toolbar 12 px glyph atlas")
                                .font_size(13.0)
                                .line_height(17.0)
                                .color(secondary_color),
                        )
                        .with_child(
                            Label::new("Status row 16 px")
                                .font_size(16.0)
                                .line_height(20.0)
                                .color(primary_color),
                        ),
                ),
            )),
    )
}

pub fn build_text_validation_surface() -> impl Widget {
    let content = Stack::vertical()
        .spacing(16.0)
        .alignment(Alignment::Stretch)
        .with_child(panel(
            "Text validation lab",
            "Focused smoke checks for shaping, wrapping, bidi boundaries, IME commits, and selection overlays.",
            Stack::horizontal()
                .spacing(14.0)
                .alignment(Alignment::Start)
                .with_child(build_text_validation_probe_card(
                    "Glyph coverage probe",
                    "Glyph coverage",
                    "Aa ill minimum | Cyrillic Привет",
                    "Checks Latin stems and one common fallback family without filling the page with missing-glyph blocks.",
                ))
                .with_child(build_text_validation_probe_card(
                    "Line wrapping probe",
                    "Line wrapping",
                    "wrap -> metrics -> caret -> overlay",
                    "Constrained text should reflow cleanly while selection geometry stays aligned to visible lines.",
                ))
                .with_child(build_text_validation_probe_card(
                    "Bidi caret probe",
                    "Bidi caret",
                    "abc 123 | RTL run | caret crosses",
                    "Use the editor below for live RTL input while this card keeps the visual checklist compact.",
                )),
        ))
        .with_child(panel(
            "Interactive editor target",
            "Manual target for caret movement, selection ranges, scrolling, IME preedit, and fallback text entry.",
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    MaximumWidth::new(
                        960.0,
                        Label::new("Focus the editor, type with IME or keyboard input, extend selection with Shift+Arrow, and wheel-scroll to inspect visible-line extraction.")
                            .font_size(13.0)
                            .line_height(19.0)
                            .color(Color::rgba(0.43, 0.50, 0.58, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new()
                        .width(980.0)
                        .height(300.0)
                        .with_child(
                            TextSurface::new(TEXT_VALIDATION_EDITOR_NAME)
                                .value(text_validation_editor_seed())
                                .wrap(TextWrap::Word)
                                .direction(TextDirection::Auto)
                                .min_width(980.0)
                                .min_height(300.0)
                                .text_style(TextStyle {
                                    font_size: 14.0,
                                    line_height: 20.0,
                                    color: Color::rgba(0.15, 0.19, 0.25, 1.0),
                                    ..TextStyle::default()
                                }),
                        ),
                ),
        ));

    ScrollView::vertical(Padding::all(
        24.0,
        SizedBox::new()
            .width(TEXT_VALIDATION_CONTENT_WIDTH)
            .with_child(content),
    ))
    .name(TEXT_VALIDATION_SCROLL_NAME)
}

fn build_text_validation_probe_card(
    name: &'static str,
    title: &'static str,
    sample: &'static str,
    caption: &'static str,
) -> impl Widget {
    NamedSection::new(
        name,
        SizedBox::new()
            .width(TEXT_VALIDATION_PROBE_CARD_WIDTH)
            .with_child(StoryCard::new(
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        Label::new(title)
                            .font_size(13.0)
                            .line_height(17.0)
                            .color(Color::rgba(0.45, 0.52, 0.61, 1.0)),
                    )
                    .with_child(
                        Label::new(sample)
                            .font_size(16.0)
                            .line_height(21.0)
                            .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                    )
                    .with_child(
                        Label::new(caption)
                            .font_size(12.0)
                            .line_height(16.0)
                            .color(Color::rgba(0.39, 0.47, 0.56, 1.0)),
                    ),
            )),
    )
}

pub fn build_text_editing_benchmark() -> impl Widget {
    let editor_document = text_editing_benchmark_document();
    let editor_style_spans = text_editing_benchmark_style_spans(&editor_document);
    let editor_style_overlays = text_editing_benchmark_style_overlays(&editor_document);
    let editor_panel = panel(
        "Editable styled code surface",
        "Benchmark typing, selection, IME preedit, wheel scrolling, and syntax-overlay churn against one long text surface.",
        SizedBox::new().width(560.0).height(700.0).with_child(
            TextSurface::new(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
                .value(editor_document)
                .direction(TextDirection::LeftToRight)
                .min_width(560.0)
                .min_height(700.0)
                .style_spans(editor_style_spans)
                .style_overlays(editor_style_overlays)
                .text_style(TextStyle {
                    font_size: 13.0,
                    line_height: 18.0,
                    color: Color::rgba(0.13, 0.17, 0.23, 1.0),
                    ..TextStyle::default()
                }),
        ),
    );
    let syntax_panel = panel(
        "Syntax-highlight preview",
        "A scrollable code-preview column keeps the benchmark honest about syntax-color churn instead of measuring only plain-text editing.",
        SizedBox::new()
            .width(520.0)
            .height(700.0)
            .with_child(build_text_editing_syntax_preview()),
    );

    Padding::all(
        24.0,
        SplitView::horizontal(editor_panel, syntax_panel)
            .name(TEXT_EDITING_BENCHMARK_SPLIT_NAME)
            .ratio(0.54)
            .min_first(420.0)
            .min_second(360.0),
    )
}

pub fn build_text_editing_benchmark_application() -> Application {
    App::new()
        .window(Window::new(TEXT_EDITING_BENCHMARK_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_EDITING_BENCHMARK_TITLE,
                "Focused benchmark surface for editor-style typing, selection, scrolling, and syntax-highlight preview cost.",
                build_text_editing_benchmark(),
            ),
        ))
        .into_application()
}

fn retained_text_benchmark_section(section_index: usize) -> (String, String) {
    const THEMES: [(&str, &str); 6] = [
        (
            "Atlas residency",
            "Repeated prose keeps the retained packet mix biased toward atlas glyph work.",
        ),
        (
            "Viewport churn",
            "Small scroll deltas expose new wrapped lines while leaving most state unchanged.",
        ),
        (
            "Packet rebuilds",
            "Retained packets should stay text-heavy instead of expanding glyph quads into generic geometry.",
        ),
        (
            "Glyph density",
            "Wide paragraphs keep each visible retained surface loaded with enough glyph instances to show byte deltas clearly.",
        ),
        (
            "Cache locality",
            "Stable content and repeated vocabulary encourage glyph atlas reuse after the initial warmup.",
        ),
        (
            "Scroll pacing",
            "No-vsync harness runs isolate renderer prep and upload cost from present back-pressure.",
        ),
    ];

    let (topic, subtitle) = THEMES[section_index % THEMES.len()];
    (
        format!("Section {:02} · {topic}", section_index + 1),
        subtitle.to_string(),
    )
}

fn retained_text_benchmark_paragraph(section_index: usize, paragraph_index: usize) -> String {
    const OPENERS: [&str; 6] = [
        "Atlas uploads should now track per-glyph instance payloads instead of six transient vertices per shaped glyph.",
        "This retained scroll surface keeps the scene composition simple so upload accounting is easier to read.",
        "Visible paragraphs change a little on each wheel tick, which keeps retained packet rebuilds centered on text.",
        "Repeated headings and body copy help stabilize glyph atlas misses after the initial scroll warmup.",
        "The benchmark is intentionally prose-heavy because text submission is the renderer path under inspection.",
        "Scroll delta size is fixed so frame samples stay comparable across runs and git revisions.",
    ];
    const DETAILS: [&str; 6] = [
        "Long wrapped lines are useful here because they raise glyph count without introducing extra widget complexity.",
        "Retained caches should still avoid rebuilding unrelated packets while the scroll layer reveals fresh text bands.",
        "Frame summaries can then compare upload bytes, glyph counts, and timing without guessing how much non-text work leaked into the sample.",
        "The same prose appears in varied combinations so the atlas can reuse cached glyph shapes while the instance buffer still changes per frame.",
        "This also mirrors the next line-window phase, where large text surfaces should only submit visible lines to the renderer.",
        "Running the harness with vsync disabled keeps the benchmark focused on renderer cost rather than swapchain pacing.",
    ];

    let opener = OPENERS[(section_index + paragraph_index) % OPENERS.len()];
    let detail = DETAILS[(section_index * 3 + paragraph_index) % DETAILS.len()];
    let cadence = 12 + ((section_index + paragraph_index) % 9);
    let packet_hint = 4 + ((section_index * 5 + paragraph_index) % 7);

    format!(
        "Section {:02}, paragraph {}. {} {} The visible cadence in this sample targets about {} wrapped lines per viewport slice, while adjacent retained packets typically contribute around {} neighboring text blocks before the next wheel event moves the window again.",
        section_index + 1,
        paragraph_index + 1,
        opener,
        detail,
        cadence,
        packet_hint,
    )
}

fn text_validation_editor_seed() -> String {
    [
        "Validation checklist",
        "- Shape: Latin stems and common fallback families should stay readable.",
        "- Wrapping: long diagnostics must reflow without selection gaps when the viewport narrows.",
        "- IME: composition commits should land near the caret instead of invalidating the whole surface.",
        "- Caret: moving across bidi boundaries should preserve stable layout handles and visible overlays.",
        "",
        "Fallback probes to paste, edit, or compare:",
        "Arabic: مرحبا | Hebrew: שלום | Hindi: नमस्ते | Han: 中文 | Emoji: 🙂",
        "",
        "Type here to confirm the runtime still exposes semantics-first text input for automated tests.",
    ]
    .join("\n")
}

fn text_editing_benchmark_document() -> String {
    let mut lines = Vec::new();
    lines.push("// Text editing benchmark: long code-like document with mixed comments and repeated glyph traffic".to_string());
    lines.push("mod editor_benchmark {".to_string());
    for index in 0..240 {
        let indent = if index % 6 == 0 { "        " } else { "    " };
        let keyword = ["let", "if", "match", "while", "for", "return"][index % 6];
        let symbol = [
            "shape_visible_window",
            "apply_incremental_edit",
            "measure_selection_overlay",
            "resolve_fallback_face",
            "update_syntax_cache",
            "record_scroll_sample",
        ][(index * 3) % 6];
        let comment = [
            "// atlas reuse should stay warm 🙂",
            "// bidi note: abc אבג 123 مرحبا",
            "// syntax colors keep changing across the preview pane",
            "// fallback sample includes Ж, 中, and नमस्ते in comments",
            "// selection overlays should repaint locally",
            "// retained packets should not rebuild unrelated code blocks",
        ][(index * 5) % 6];
        lines.push(format!(
            "{indent}{keyword} row_{index:03} = {symbol}(cursor + {delta}, viewport_height - {trim}); {comment}",
            delta = 3 + (index % 17),
            trim = 1 + (index % 7),
        ));
        if index % 8 == 7 {
            lines.push(format!(
                "        // folded section {:02}: syntax_color = accent::{:?}; ime = \"候補{}\";",
                (index / 8) + 1,
                ["Keyword", "Type", "Comment", "Number"][index % 4],
                index
            ));
        }
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn text_editing_benchmark_style_spans(document: &str) -> Vec<TextSurfaceStyleSpan> {
    let keyword_style = text_editing_benchmark_span_style(Color::rgba(0.78, 0.34, 0.16, 1.0));
    let symbol_style = text_editing_benchmark_span_style(Color::rgba(0.09, 0.43, 0.58, 1.0));
    let string_style = text_editing_benchmark_span_style(Color::rgba(0.42, 0.32, 0.74, 1.0));
    let comment_style = text_editing_benchmark_span_style(Color::rgba(0.36, 0.45, 0.25, 1.0));
    let number_style = text_editing_benchmark_span_style(Color::rgba(0.14, 0.49, 0.24, 1.0));
    let keywords = ["mod", "let", "if", "match", "while", "for", "return"];
    let symbols = [
        "editor_benchmark",
        "shape_visible_window",
        "apply_incremental_edit",
        "measure_selection_overlay",
        "resolve_fallback_face",
        "update_syntax_cache",
        "record_scroll_sample",
    ];
    let mut spans = Vec::new();
    let mut line_offset = 0usize;

    for line_with_break in document.split_inclusive('\n') {
        let line = line_with_break
            .strip_suffix('\n')
            .unwrap_or(line_with_break);
        let comment_start = line.find("//");
        let code_end = comment_start.unwrap_or(line.len());

        for keyword in keywords {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                keyword,
                keyword_style.clone(),
            );
        }
        for symbol in symbols {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                symbol,
                symbol_style.clone(),
            );
        }
        collect_text_editing_number_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            number_style.clone(),
        );
        collect_text_editing_string_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            string_style.clone(),
        );
        if let Some(comment_start) = comment_start {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + comment_start..line_offset + line.len(),
                comment_style.clone(),
            ));
        }

        line_offset += line_with_break.len();
    }

    spans
}

fn text_editing_benchmark_style_overlays(document: &str) -> Vec<TextSurfaceStyleOverlay> {
    let search_style = text_editing_benchmark_span_style(Color::rgba(0.08, 0.38, 0.72, 1.0));
    let diagnostic_style = text_editing_benchmark_span_style(Color::rgba(0.70, 0.14, 0.20, 1.0));
    let rich_preview_style = text_editing_benchmark_span_style(Color::rgba(0.46, 0.23, 0.66, 1.0));
    let mut overlays = Vec::new();

    collect_text_editing_overlays(
        &mut overlays,
        document,
        "shape_visible_window",
        TextSurfaceOverlayKind::SearchMatch,
        search_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "fallback",
        TextSurfaceOverlayKind::Diagnostic,
        diagnostic_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "🙂",
        TextSurfaceOverlayKind::RichTextPreview,
        rich_preview_style,
    );

    overlays
}

fn text_editing_benchmark_span_style(color: Color) -> TextStyle {
    TextStyle {
        font_size: 13.0,
        line_height: 18.0,
        color,
        ..TextStyle::default()
    }
}

fn collect_text_editing_word_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    word: &str,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = line[search_offset..].find(word) {
        let start = search_offset + relative_start;
        let end = start + word.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if text_editing_word_boundary(before) && text_editing_word_boundary(after) {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + end,
                style.clone(),
            ));
        }
        search_offset = end;
    }
}

fn collect_text_editing_number_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut span_start = None;
    for (index, ch) in line.char_indices() {
        let number_char = ch.is_ascii_digit() || ch == '.';
        match (span_start, number_char) {
            (None, true) => span_start = Some(index),
            (Some(start), false) => {
                spans.push(TextSurfaceStyleSpan::new(
                    line_offset + start..line_offset + index,
                    style.clone(),
                ));
                span_start = None;
            }
            _ => {}
        }
    }

    if let Some(start) = span_start {
        spans.push(TextSurfaceStyleSpan::new(
            line_offset + start..line_offset + line.len(),
            style,
        ));
    }
}

fn collect_text_editing_string_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut string_start = None;
    for (index, ch) in line.char_indices() {
        if ch != '"' {
            continue;
        }
        if let Some(start) = string_start.take() {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + index + ch.len_utf8(),
                style.clone(),
            ));
        } else {
            string_start = Some(index);
        }
    }
}

fn collect_text_editing_overlays(
    overlays: &mut Vec<TextSurfaceStyleOverlay>,
    document: &str,
    needle: &str,
    kind: TextSurfaceOverlayKind,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = document[search_offset..].find(needle) {
        let start = search_offset + relative_start;
        let end = start + needle.len();
        overlays.push(TextSurfaceStyleOverlay::new(
            start..end,
            style.clone(),
            kind.clone(),
        ));
        search_offset = end;
    }
}

fn text_editing_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(ch) => !ch.is_alphanumeric() && ch != '_',
        None => true,
    }
}

fn build_text_editing_syntax_preview() -> impl Widget {
    let mut content = Stack::vertical().spacing(2.0).alignment(Alignment::Stretch);

    for line_index in 0..220 {
        content = content.with_child(build_text_editing_syntax_line(line_index));
    }

    ScrollView::vertical(Padding::all(
        12.0,
        SizedBox::new().width(520.0).with_child(content),
    ))
    .name(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME)
}

fn build_text_editing_syntax_line(line_index: usize) -> impl Widget {
    let keyword = ["fn", "let", "match", "if", "while", "return"][line_index % 6];
    let type_name =
        ["Editor", "Glyphs", "Select", "Syntax", "Window", "Frame"][(line_index * 7) % 6];
    let method = ["shape", "cache", "cursor", "paint", "fallback", "commit"][(line_index * 11) % 6];
    let accent = ["keyword", "type", "comment", "number"][line_index % 4];
    let line = Stack::horizontal()
        .spacing(0.0)
        .alignment(Alignment::Start)
        .with_child(
            SizedBox::new().width(44.0).with_child(
                Label::new(format!("{:>3}", line_index + 1))
                    .font_size(12.0)
                    .line_height(18.0)
                    .color(Color::rgba(0.58, 0.64, 0.72, 1.0)),
            ),
        )
        .with_child(
            Label::new(format!("{keyword} "))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.78, 0.34, 0.16, 1.0)),
        )
        .with_child(
            Label::new(format!("sample_{line_index:03}"))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.15, 0.19, 0.26, 1.0)),
        )
        .with_child(
            Label::new(format!(": {type_name}"))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.09, 0.43, 0.58, 1.0)),
        )
        .with_child(
            Label::new(format!(" = {method}("))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.21, 0.27, 0.35, 1.0)),
        )
        .with_child(
            Label::new(format!("{:.2}", 0.5 + ((line_index % 17) as f32 * 0.125)))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.14, 0.49, 0.24, 1.0)),
        )
        .with_child(
            Label::new("); ")
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.21, 0.27, 0.35, 1.0)),
        )
        .with_child(
            Label::new(format!("// {accent} glyph {}", line_index % 9))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.36, 0.45, 0.25, 1.0)),
        );

    if line_index % 2 == 0 {
        Background::new(
            Color::rgba(0.978, 0.984, 0.994, 1.0),
            Padding::all(6.0, line),
        )
    } else {
        Background::new(
            Color::rgba(0.958, 0.968, 0.984, 1.0),
            Padding::all(6.0, line),
        )
    }
}

fn widget_book_demo_image_pixels() -> Vec<u8> {
    let width = 72usize;
    let height = 72usize;
    let mut pixels = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let mut red = if checker { 228 } else { 208 };
            let mut green = if checker { 236 } else { 216 };
            let mut blue = if checker { 248 } else { 228 };
            let alpha = 255u8;

            if x > 10 && x < 62 && y > 10 && y < 62 {
                red = 38 + ((x as f32 / width as f32) * 50.0) as u8;
                green = 108 + ((y as f32 / height as f32) * 60.0) as u8;
                blue = 190;
            }

            if (x > 18 && x < 54) && (y > 18 && y < 54) {
                red = 245;
                green = 248;
                blue = 252;
            }

            if (x > 28 && x < 44) && (y > 24 && y < 48) {
                red = 255;
                green = 168;
                blue = 60;
            }

            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = alpha;
        }
    }

    pixels
}

fn panel<W>(title: &str, subtitle: &str, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    panel_with_theme(default_widget_book_theme_reader(), title, subtitle, body)
}

fn panel_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &str,
    subtitle: &str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    Background::new(
        theme_reader().palette.surface,
        Padding::all(
            18.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(title)
                        .font_size(20.0)
                        .line_height(24.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text
                        })),
                ))
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    Label::new(subtitle)
                        .font_size(14.0)
                        .line_height(19.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                ))
                .with_child(body),
        ),
    )
    .brush_when(widget_book_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn control_story_with_theme<W>(
    theme_reader: WidgetBookThemeReader,
    title: &str,
    caption: &str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    SizedBox::new().width(430.0).with_child(
        StoryCard::new(
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Start)
                .with_child(
                    Label::new(title)
                        .font_size(14.0)
                        .line_height(18.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text
                        })),
                )
                .with_child(MaximumWidth::new(
                    380.0,
                    Label::new(caption)
                        .font_size(12.0)
                        .line_height(16.0)
                        .color_when(widget_book_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                ))
                .with_child(body),
        )
        .theme_when(clone_widget_book_theme_reader(&theme_reader)),
    )
}

struct StoryCard {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    padding: Insets,
    child: SingleChild,
}

impl StoryCard {
    fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            padding: Insets::all(14.0),
            child: SingleChild::new(child),
        }
    }

    fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for StoryCard {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.min.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
            Size::new(
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let child_bounds = Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right)
                .max(0.0)
                .min(measured.width),
            (bounds.height() - self.padding.top - self.padding.bottom)
                .max(0.0)
                .min(measured.height),
        );
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        ctx.fill(Path::rounded_rect(bounds, 8.0), palette.surface_raised);
        ctx.stroke(
            Path::rounded_rect(bounds, 8.0),
            palette.border,
            StrokeStyle::new(1.0),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct NamedSection {
    name: String,
    content: SingleChild,
}

impl NamedSection {
    fn new(name: impl Into<String>, content: impl Widget + 'static) -> Self {
        Self {
            name: name.into(),
            content: SingleChild::new(content),
        }
    }
}

impl Widget for NamedSection {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

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
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        ctx.push(node);
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

fn theme_preview_card(
    theme: DefaultTheme,
    title: &'static str,
    action_label: &'static str,
    input_label: &'static str,
) -> impl Widget {
    let body = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new(format!("{title} theme"))
                .font_size(18.0)
                .line_height(22.0)
                .color(theme.palette.text),
        )
        .with_child(MaximumWidth::new(
            520.0,
            Label::new(format!(
                "{} base surface with {} accent for primary actions.",
                theme.colors.name, theme.colors.name
            ))
            .font_size(13.0)
            .line_height(18.0)
            .color(theme.palette.placeholder),
        ))
        .with_child(
            SizedBox::new().width(220.0).with_child(
                TextInput::new(input_label)
                    .placeholder("Find layer, panel, or asset")
                    .theme(theme),
            ),
        )
        .with_child(
            Stack::horizontal()
                .spacing(12.0)
                .alignment(Alignment::Center)
                .with_child(Button::new(action_label).theme(theme))
                .with_child(MaximumWidth::new(
                    280.0,
                    Label::new(
                        "Reusable controls should stay coherent across all theme directions.",
                    )
                    .font_size(13.0)
                    .line_height(18.0)
                    .color(theme.palette.placeholder),
                )),
        )
        .with_child(
            SizedBox::new().width(310.0).with_child(
                Checkbox::new(format!("{title} preview snap to grid"))
                    .checked(true)
                    .theme(theme),
            ),
        )
        .with_child(
            SizedBox::new().width(310.0).with_child(
                Switch::new(format!("{title} preview live updates"))
                    .on(true)
                    .theme(theme),
            ),
        )
        .with_child(
            Stack::horizontal()
                .spacing(10.0)
                .alignment(Alignment::Center)
                .with_child(
                    ColorSwatch::new(format!("{title} base swatch"), theme.colors.base_200)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} primary swatch"), theme.colors.primary)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} secondary swatch"), theme.colors.secondary)
                        .size(Size::new(58.0, 28.0)),
                ),
        );

    ThemePreviewCardFrame::new(theme, body)
}

struct ThemePreviewCardFrame {
    theme: DefaultTheme,
    padding: Insets,
    child: SingleChild,
}

impl ThemePreviewCardFrame {
    fn new<W>(theme: DefaultTheme, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme,
            padding: Insets::all(18.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ThemePreviewCardFrame {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.min.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
            Size::new(
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let child_bounds = Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right)
                .max(0.0)
                .min(measured.width),
            (bounds.height() - self.padding.top - self.padding.bottom)
                .max(0.0)
                .min(measured.height),
        );
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let border = self.theme.palette.border.with_alpha(0.92);
        let background = if self.theme.colors.scheme == ThemeColorScheme::HighContrast {
            self.theme.palette.surface
        } else {
            self.theme.palette.surface_raised
        };
        ctx.fill(Path::rounded_rect(bounds, 10.0), background);
        ctx.stroke(
            Path::rounded_rect(bounds, 10.0),
            border,
            StrokeStyle::new(1.0),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct WidgetBookSummary {
    theme_reader: WidgetBookThemeReader,
    state: Rc<RefCell<WidgetBookState>>,
    last_seen_state: WidgetBookState,
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>, theme_reader: WidgetBookThemeReader) -> Self {
        let last_seen_state = state.borrow().clone();
        Self {
            theme_reader,
            state,
            last_seen_state,
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }
}

struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
}

impl LivePerformancePanel {
    const WIDTH: f32 = 340.0;
    const HEIGHT: f32 = 158.0;
    const PADDING_X: f32 = 12.0;
    const PADDING_Y: f32 = 10.0;
    const CORNER_RADIUS: f32 = 8.0;
    const HEADER_HEIGHT: f32 = 32.0;
    const GRAPH_HEIGHT: f32 = 72.0;
    const LEGEND_HEIGHT: f32 = 28.0;
    const BAR_GAP: f32 = 1.0;

    #[cfg(test)]
    fn new() -> Self {
        Self::with_display(Rc::new(RefCell::new(LivePerformanceDisplay::default())))
    }

    fn with_display(display: Rc<RefCell<LivePerformanceDisplay>>) -> Self {
        Self { display }
    }

    fn graph_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.y() + Self::PADDING_Y + Self::HEADER_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::GRAPH_HEIGHT,
        )
    }

    fn legend_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.max_y() - Self::PADDING_Y - Self::LEGEND_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::LEGEND_HEIGHT,
        )
    }

    fn frame_cost_scale(samples: &[LivePerformanceFrameSample]) -> f32 {
        samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .fold(16.67, f32::max)
            .clamp(16.67, 66.67)
    }

    fn stage_short_label(phase: FramePhase) -> &'static str {
        match phase {
            FramePhase::Event => "evt",
            FramePhase::Redraw => "redraw",
            FramePhase::MeasureArrange => "layout",
            FramePhase::HitTest => "hit",
            FramePhase::Paint => "paint",
            FramePhase::Semantics => "a11y",
            FramePhase::Renderer => "rend",
            FramePhase::SurfaceWait => "wait",
            FramePhase::Diagnostics => "diag",
        }
    }

    fn stage_color(phase: FramePhase, alpha: f32) -> Color {
        let alpha = alpha.clamp(0.0, 1.0);
        match phase {
            FramePhase::Event => Color::rgba(0.24, 0.78, 0.68, alpha),
            FramePhase::Redraw => Color::rgba(0.50, 0.68, 0.95, alpha),
            FramePhase::MeasureArrange => Color::rgba(0.96, 0.68, 0.30, alpha),
            FramePhase::HitTest => Color::rgba(0.68, 0.56, 0.92, alpha),
            FramePhase::Paint => Color::rgba(0.94, 0.42, 0.54, alpha),
            FramePhase::Semantics => Color::rgba(0.72, 0.82, 0.36, alpha),
            FramePhase::Renderer => Color::rgba(0.36, 0.78, 0.96, alpha),
            FramePhase::SurfaceWait => Color::rgba(0.68, 0.72, 0.78, alpha),
            FramePhase::Diagnostics => Color::rgba(0.78, 0.80, 0.86, alpha),
        }
    }

    fn paint_budget_line(ctx: &mut PaintCtx, graph: Rect, scale_ms: f32, budget_ms: f32) {
        if budget_ms > scale_ms {
            return;
        }

        let y = graph.max_y() - graph.height() * (budget_ms / scale_ms);
        let mut path = Path::builder();
        path.move_to(Point::new(graph.x(), y));
        path.line_to(Point::new(graph.max_x(), y));
        ctx.stroke(
            path.build(),
            Color::rgba(0.98, 1.0, 1.0, 0.28),
            StrokeStyle::new(1.0),
        );
    }

    fn paint_graph(&self, ctx: &mut PaintCtx, display: &LivePerformanceDisplay, graph: Rect) {
        ctx.fill_rect(graph, Color::rgba(0.0, 0.0, 0.0, 0.24));
        ctx.stroke_rect(
            graph,
            Color::rgba(0.98, 1.0, 1.0, 0.22),
            StrokeStyle::new(1.0),
        );

        let scale_ms = Self::frame_cost_scale(&display.samples);
        Self::paint_budget_line(ctx, graph, scale_ms, 16.67);
        Self::paint_budget_line(ctx, graph, scale_ms, 33.33);

        if display.samples.is_empty() {
            ctx.draw_text(
                graph,
                "waiting for frames",
                text_style(Color::rgba(0.92, 0.96, 1.0, 0.72), 11.0, 14.0),
            );
            return;
        }

        let slot_width = (graph.width() / LIVE_PERFORMANCE_HISTORY_LIMIT as f32).max(2.0);
        let bar_width = (slot_width - Self::BAR_GAP).max(1.0);
        let visible_count = ((graph.width() / slot_width).floor() as usize)
            .min(display.samples.len())
            .max(1);
        let samples = &display.samples[display.samples.len() - visible_count..];
        let start_x = graph.max_x() - slot_width * samples.len() as f32;

        ctx.push_clip_rect(graph);
        for (sample_index, sample) in samples.iter().enumerate() {
            let x = start_x + sample_index as f32 * slot_width;
            let mut y = graph.max_y();
            for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
                let duration = sample.stage_costs[frame_phase_index(phase)];
                if duration <= 0.0 {
                    continue;
                }

                let height = (graph.height() * (duration / scale_ms)).max(0.5);
                y = (y - height).max(graph.y());
                ctx.fill_rect(
                    Rect::new(x, y, bar_width, (graph.max_y() - y).min(height)),
                    Self::stage_color(phase, 0.88),
                );
            }
        }
        ctx.pop_clip();

        ctx.draw_text(
            Rect::new(graph.x() + 4.0, graph.y() + 2.0, 48.0, 14.0),
            format!("{scale_ms:.0} ms"),
            text_style(Color::rgba(0.92, 0.96, 1.0, 0.62), 10.0, 12.0),
        );
        ctx.draw_text(
            Rect::new(graph.x() + 4.0, graph.max_y() - 16.0, 56.0, 14.0),
            "16.7 ms",
            text_style(Color::rgba(0.92, 0.96, 1.0, 0.62), 10.0, 12.0),
        );
    }

    fn paint_legend(&self, ctx: &mut PaintCtx, bounds: Rect) {
        let mut x = bounds.x();
        let y = bounds.y() + 4.0;
        for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
            let label = Self::stage_short_label(phase);
            let label_width = match phase {
                FramePhase::MeasureArrange => 38.0,
                FramePhase::Redraw => 42.0,
                FramePhase::Renderer | FramePhase::SurfaceWait => 34.0,
                _ => 30.0,
            };
            if x + label_width > bounds.max_x() {
                break;
            }

            ctx.fill_rect(
                Rect::new(x, y + 4.0, 7.0, 7.0),
                Self::stage_color(phase, 0.95),
            );
            ctx.draw_text(
                Rect::new(x + 10.0, y, label_width - 10.0, 16.0),
                label,
                text_style(Color::rgba(0.94, 0.97, 1.0, 0.74), 10.0, 13.0),
            );
            x += label_width;
        }
    }

    fn snapshot_phase_duration(snapshot: &WindowPerformanceSnapshot, phase: FramePhase) -> f64 {
        snapshot
            .phase_timings
            .iter()
            .filter(|sample| sample.phase == phase)
            .map(|sample| sample.duration_ms)
            .sum()
    }
}

const LIVE_PERFORMANCE_GRAPH_PHASES: [FramePhase; LIVE_PERFORMANCE_STAGE_COUNT] = [
    FramePhase::Event,
    FramePhase::Redraw,
    FramePhase::MeasureArrange,
    FramePhase::HitTest,
    FramePhase::Paint,
    FramePhase::Semantics,
    FramePhase::Renderer,
    FramePhase::SurfaceWait,
    FramePhase::Diagnostics,
];

impl Widget for LivePerformancePanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(Self::WIDTH)
        } else {
            Self::WIDTH
        };
        constraints.clamp(Size::new(width, Self::HEIGHT))
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let display = self.display.borrow().clone();
        let frame = rounded_rect_path(ctx.bounds(), Self::CORNER_RADIUS);
        ctx.fill(frame.clone(), Color::rgba(0.015, 0.025, 0.035, 0.50));
        ctx.stroke(
            frame,
            Color::rgba(0.98, 1.0, 1.0, 0.18),
            StrokeStyle::new(1.0),
        );

        let header_y = ctx.bounds().y() + Self::PADDING_Y;
        let (fps_text, frame_text, slowest_text) = if let Some(snapshot) = &display.snapshot {
            let fps = if display.idle {
                "0 fps".to_string()
            } else {
                format_fps(snapshot.total_time_ms)
            };
            let frame = if display.idle {
                "idle".to_string()
            } else {
                format!(
                    "frame {} | {}",
                    snapshot.frame_index,
                    format_duration_ms(snapshot.total_time_ms)
                )
            };
            let renderer_work_ms = Self::snapshot_phase_duration(snapshot, FramePhase::Renderer);
            let surface_wait_ms = Self::snapshot_phase_duration(snapshot, FramePhase::SurfaceWait);
            let slowest = if renderer_work_ms > 0.0 || surface_wait_ms > 0.0 {
                format!(
                    "rend {} | wait {}",
                    format_duration_ms(renderer_work_ms),
                    format_duration_ms(surface_wait_ms),
                )
            } else {
                snapshot
                    .slowest_phase()
                    .map(|sample| {
                        format!(
                            "{} {}",
                            Self::stage_short_label(sample.phase),
                            format_duration_ms(sample.duration_ms)
                        )
                    })
                    .unwrap_or_else(|| "waiting for phases".to_string())
            };
            (fps, frame, slowest)
        } else {
            (
                "0 fps".to_string(),
                "waiting".to_string(),
                "waiting for first frame".to_string(),
            )
        };

        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + Self::PADDING_X,
                header_y,
                118.0,
                Self::HEADER_HEIGHT,
            ),
            fps_text,
            text_style(Color::rgba(0.98, 1.0, 1.0, 0.96), 22.0, 28.0),
        );
        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y + 2.0,
                ctx.bounds().width() - 148.0,
                15.0,
            ),
            frame_text,
            text_style(Color::rgba(0.92, 0.96, 1.0, 0.76), 11.0, 14.0),
        );
        ctx.draw_text(
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y + 17.0,
                ctx.bounds().width() - 148.0,
                15.0,
            ),
            slowest_text,
            text_style(Color::rgba(0.92, 0.96, 1.0, 0.66), 11.0, 14.0),
        );

        self.paint_graph(ctx, &display, Self::graph_bounds(ctx.bounds()));
        self.paint_legend(ctx, Self::legend_bounds(ctx.bounds()));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let display = self.display.borrow();
        let value = display
            .snapshot
            .as_ref()
            .map(|snapshot| {
                format!(
                    "{} | {} | {} samples",
                    if display.idle {
                        "0 fps".to_string()
                    } else {
                        format_fps(snapshot.total_time_ms)
                    },
                    format_duration_ms(snapshot.total_time_ms),
                    display.samples.len()
                )
            })
            .unwrap_or_else(|| "waiting for frames".to_string());
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Live performance overlay".to_string());
        node.description =
            Some("Transparent FPS overlay with rolling stacked frame phase costs.".to_string());
        node.value = Some(SemanticsValue::Text(value));
        ctx.push(node);
    }
}

fn text_style(color: Color, size: f32, line_height: f32) -> TextStyle {
    let mut style = TextStyle::new(color);
    style.font_size = size;
    style.line_height = line_height;
    style
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

fn format_fps(total_time_ms: f64) -> String {
    if total_time_ms <= 0.0 {
        "idle".to_string()
    } else {
        format!("{:.0} fps", 1000.0 / total_time_ms)
    }
}

fn format_duration_ms(duration_ms: f64) -> String {
    format!("{duration_ms:.1} ms")
}

impl Widget for WidgetBookSummary {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            return;
        }

        let current_state = self.state.borrow().clone();
        if current_state != self.last_seen_state {
            self.last_seen_state = current_state;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            320.0
        };
        constraints.clamp(Size::new(width, 270.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let state = self.state.borrow();
        let lines = [
            if state.name.trim().is_empty() {
                "Hello, stranger".to_string()
            } else {
                format!("Hello, {}", state.name)
            },
            format!(
                "buttons: primary={} icon={}",
                state.button_presses, state.icon_button_presses
            ),
            format!(
                "subscription: {} | snapping: {}",
                if state.subscribed { "on" } else { "off" },
                if state.switch_on { "on" } else { "off" }
            ),
            format!(
                "radio: standalone={} group={}",
                if state.standalone_radio_selected {
                    "selected"
                } else {
                    "idle"
                },
                if state.radio_choice.is_empty() {
                    "unset"
                } else {
                    state.radio_choice.as_str()
                }
            ),
            format!(
                "opacity: {:.0} | brush size: {:.0} | mode: {}",
                state.slider_value,
                state.number_value,
                if state.mode.is_empty() {
                    "unset"
                } else {
                    state.mode.as_str()
                }
            ),
            format!(
                "tabs: bar={} panel={}",
                if state.tab_bar_choice.is_empty() {
                    "unset"
                } else {
                    state.tab_bar_choice.as_str()
                },
                if state.tabs_choice.is_empty() {
                    "unset"
                } else {
                    state.tabs_choice.as_str()
                }
            ),
            format!(
                "menu: {} | context: {} | dialog apply: {}",
                if state.last_menu_action.is_empty() {
                    "idle"
                } else {
                    state.last_menu_action.as_str()
                },
                if state.last_context_action.is_empty() {
                    "idle"
                } else {
                    state.last_context_action.as_str()
                },
                state.dialog_apply_count,
            ),
            format!("notes lines: {}", state.notes.lines().count().max(1)),
        ];

        ctx.fill_bounds(palette.surface_raised);
        ctx.stroke_bounds(
            palette.border,
            StrokeStyle::new(theme.metrics.border_width.max(1.0)),
        );
        for (index, line) in lines.into_iter().enumerate() {
            ctx.label(
                Rect::new(
                    ctx.bounds().x() + 14.0,
                    ctx.bounds().y() + 14.0 + (index as f32 * 28.0),
                    ctx.bounds().width() - 28.0,
                    22.0,
                ),
                line,
                if index == 0 {
                    palette.text
                } else {
                    palette.text_muted
                },
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(SUMMARY_NAME.to_string());
        node.description = Some(format!(
            "name: {}; subscription: {}; button presses: {}; icon actions: {}; switch: {}; standalone radio: {}; radio choice: {}; slider: {:.0}; brush size: {:.0}; mode: {}; tab bar: {}; tabs: {}; menu: {}; context menu: {}; dialog apply: {}; notes lines: {}",
            if state.name.is_empty() {
                "stranger"
            } else {
                state.name.as_str()
            },
            if state.subscribed { "on" } else { "off" },
            state.button_presses,
            state.icon_button_presses,
            if state.switch_on { "on" } else { "off" },
            if state.standalone_radio_selected {
                "selected"
            } else {
                "off"
            },
            if state.radio_choice.is_empty() {
                "unset"
            } else {
                state.radio_choice.as_str()
            },
            state.slider_value,
            state.number_value,
            if state.mode.is_empty() {
                "unset"
            } else {
                state.mode.as_str()
            },
            if state.tab_bar_choice.is_empty() {
                "unset"
            } else {
                state.tab_bar_choice.as_str()
            },
            if state.tabs_choice.is_empty() {
                "unset"
            } else {
                state.tabs_choice.as_str()
            },
            if state.last_menu_action.is_empty() {
                "idle"
            } else {
                state.last_menu_action.as_str()
            },
            if state.last_context_action.is_empty() {
                "idle"
            } else {
                state.last_context_action.as_str()
            },
            state.dialog_apply_count,
            state.notes.lines().count().max(1),
        ));
        ctx.push(node);
    }
}

fn option_index(options: &[&str], value: &str) -> Option<usize> {
    options.iter().position(|option| *option == value)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        fs,
        path::{Path, PathBuf},
        rc::Rc,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::visual_artifacts::{
        StoryCase, artifact_root, configured_widget_book_state, scroll_to_story_target,
    };
    use super::{
        ANIMATION_BENCHMARK_REPAINT_NAME, ANIMATION_BENCHMARK_RETAINED_NAME,
        ANIMATION_BENCHMARK_SCALE_NAME, ANIMATION_BENCHMARK_TITLE, COLOR_PICKER_NAME, DIALOG_TITLE,
        DIALOG_TRIGGER_LABEL, GALLERY_SCROLL_BAR_NAME, GALLERY_SCROLL_NAME,
        LIGHT_PREVIEW_ACTION_LABEL, LIGHT_PREVIEW_INPUT_LABEL, LIGHT_THEME_PREVIEW_CARD_NAME,
        LivePerformanceDisplay, LivePerformanceFrameSample, LivePerformancePanel, NAME_INPUT_LABEL,
        NUMBER_INPUT_NAME, POPOVER_NAME, POPOVER_TRIGGER_LABEL, RADIO_BUTTON_LABEL,
        RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME, RETAINED_TEXT_BENCHMARK_SCROLL_NAME,
        RETAINED_TEXT_BENCHMARK_TITLE, SELECT_NAME, SLIDER_NAME, SUMMARY_NAME, SWITCH_LABEL,
        TEXT_AREA_LABEL, TEXT_EDITING_BENCHMARK_EDITOR_NAME, TEXT_EDITING_BENCHMARK_SPLIT_NAME,
        TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME, TEXT_EDITING_BENCHMARK_TITLE,
        TEXT_RENDERING_COMPARISON_SCROLL_NAME, TEXT_RENDERING_COMPARISON_TITLE,
        TEXT_VALIDATION_EDITOR_NAME, TEXT_VALIDATION_SCROLL_NAME, TEXT_VALIDATION_VIEW_TITLE,
        THEME_DEMO_SCROLL_NAME, THEME_DEMO_TITLE, THEME_PREVIEW_TOGGLE_LABEL, TOOLTIP_TEXT,
        TOOLTIP_TRIGGER_LABEL, WIDGET_STATES_BUTTON_LABEL, WIDGET_STATES_CHECKBOX_LABEL,
        WIDGET_STATES_GALLERY_NAME, WIDGET_STATES_MENU_NAME, WIDGET_STATES_POPOVER_NAME,
        WIDGET_STATES_SELECT_NAME, WIDGET_STATES_SLIDER_NAME, WIDGET_STATES_SWITCH_LABEL,
        WIDGET_STATES_TABS_NAME, WIDGET_STATES_TEXT_AREA_LABEL, WIDGET_STATES_TEXT_INPUT_LABEL,
        WINDOW_TITLE, build_animation_benchmark_application, build_color_and_imagery_story,
        build_retained_text_benchmark_application, build_text_editing_benchmark_application,
        build_text_rendering_comparison_application, build_text_validation_surface,
        build_theme_demo_application, build_widget_book_application, build_widget_book_gallery,
        default_widget_book_state, frame_phase_index, register_widget_book_images,
        text_editing_benchmark_document, text_editing_benchmark_style_overlays,
        text_editing_benchmark_style_spans, theme_preview_card,
    };
    use sui::{
        App, Application, DefaultTheme, Event, FramePhase, FramePhaseSample, ImeEvent, KeyState,
        KeyboardEvent, Point, PointerEvent, PointerEventKind, PresentationLatencyDiagnostics,
        RenderOutput, RendererSubmissionDiagnostics, Result, SceneStatistics,
        SceneStatisticsDetailMode, ScrollDelta, SemanticsRole, SemanticsValue, Size, SizedBox,
        TextCacheDeltaDiagnostics, TextCacheDiagnostics, TextSurfaceOverlayKind, Vector, Widget,
        WidgetPod, WidgetPodVisitor, Window, WindowBuilder, WindowEvent, WindowId,
        WindowPerformanceSnapshot, set_window_scene_statistics_detail_mode,
        window_scene_statistics_detail_mode,
    };
    use sui_runtime::publish_window_performance_snapshot;
    use sui_scene::{Brush, SceneCommand, SceneLayerUpdateKind};
    use sui_testing::prelude::*;

    fn build_default_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| {
            build_widget_book_application_with_overlay(default_widget_book_state()).build()
        })
    }

    fn build_default_theme_demo_app() -> Result<TestApp> {
        TestApp::new(|| build_theme_demo_application(default_widget_book_state()).build())
    }

    fn build_configured_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| build_widget_book_application(configured_widget_book_state()).build())
    }

    fn combo_box_text_value(window: &TestWindow, name: &str) -> Result<String> {
        window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| node.role == SemanticsRole::ComboBox && node.name.as_deref() == Some(name))
            .and_then(|node| match node.value {
                Some(SemanticsValue::Text(value)) => Some(value),
                _ => None,
            })
            .ok_or_else(|| sui::Error::new(format!("missing {name} combo box text value")))
    }

    fn build_widget_book_application_with_overlay(
        state: Rc<RefCell<super::WidgetBookState>>,
    ) -> Application {
        super::set_widget_book_hdr_theme_mode(sui::HdrThemeMode::Disabled);

        App::new()
            .with_resources(|resources| {
                register_widget_book_images(resources);
                Ok(())
            })
            .expect("widget-book image resources should be valid")
            .window(
                Window::new(WINDOW_TITLE).root(
                    super::LivePerformanceRoot::new(
                        WINDOW_TITLE,
                        super::WINDOW_DESCRIPTION,
                        build_widget_book_gallery(Rc::clone(&state)),
                    )
                    .show_performance_overlay()
                    .watch_widget_book_state(state),
                ),
            )
            .into_application()
    }

    #[cfg(feature = "artifacts")]
    fn build_gallery_only_widget_book_app() -> Result<TestApp> {
        TestApp::from_runtime(
            App::new()
                .with_resources(|resources| {
                    register_widget_book_images(resources);
                    Ok(())
                })?
                .window(
                    Window::new(WINDOW_TITLE)
                        .root(build_widget_book_gallery(default_widget_book_state())),
                )
                .build()?,
        )
    }

    #[cfg(feature = "artifacts")]
    fn headless_benchmark_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn build_text_validation_app() -> Result<TestApp> {
        TestApp::new(|| {
            Application::new()
                .window(
                    WindowBuilder::new()
                        .title(TEXT_VALIDATION_VIEW_TITLE)
                        .root(build_text_validation_surface()),
                )
                .build()
        })
    }

    #[test]
    fn text_editing_benchmark_exercises_rich_code_style_ranges() {
        let document = text_editing_benchmark_document();
        let spans = text_editing_benchmark_style_spans(&document);
        let overlays = text_editing_benchmark_style_overlays(&document);

        assert!(spans.len() > 500);
        assert!(
            overlays
                .iter()
                .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::SearchMatch))
        );
        assert!(
            overlays
                .iter()
                .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::Diagnostic))
        );
        assert!(
            overlays
                .iter()
                .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::RichTextPreview))
        );
        assert!(
            spans
                .iter()
                .all(|span| span.range.start < span.range.end && span.range.end <= document.len())
        );
        assert!(
            overlays
                .iter()
                .all(|overlay| overlay.range.start < overlay.range.end
                    && overlay.range.end <= document.len())
        );
    }

    #[test]
    fn retained_text_benchmark_exposes_vertical_scroll_bar() -> Result<()> {
        let mut runtime = build_retained_text_benchmark_runtime()?;
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;

        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(RETAINED_TEXT_BENCHMARK_SCROLL_NAME)
            })
            .expect("retained text scroll view should be present");
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME)
            })
            .expect("retained text vertical scroll bar should be present");
        let max = match scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => max,
            _ => 0.0,
        };

        assert!(max > 0.0);
        assert!(scroll_bar.bounds.x() >= scroll.bounds.max_x());
        Ok(())
    }

    #[test]
    fn text_editing_benchmark_exposes_named_splitter() -> Result<()> {
        let mut runtime = build_text_editing_benchmark_runtime()?;
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;

        let splitter = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Splitter
                    && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_SPLIT_NAME)
            })
            .expect("text editing splitter should be present");
        let editor = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
            })
            .expect("text editing editor should be present");
        let syntax_preview = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME)
            })
            .expect("text editing syntax preview should be present");

        assert!(matches!(
            splitter.value,
            Some(SemanticsValue::Number(value)) if (value - 0.54).abs() < 0.01
        ));
        assert!(editor.bounds.max_x() <= syntax_preview.bounds.x());
        Ok(())
    }

    fn build_text_validation_runtime() -> Result<sui::Runtime> {
        Application::new()
            .window(
                WindowBuilder::new().title(TEXT_VALIDATION_VIEW_TITLE).root(
                    SizedBox::new()
                        .size(Size::new(460.0, 380.0))
                        .with_child(build_text_validation_surface()),
                ),
            )
            .build()
    }

    fn build_retained_text_benchmark_runtime() -> Result<sui::Runtime> {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title(RETAINED_TEXT_BENCHMARK_TITLE)
                    .root(
                        SizedBox::new()
                            .size(Size::new(520.0, 360.0))
                            .with_child(super::build_retained_text_benchmark()),
                    ),
            )
            .build()
    }

    fn build_text_editing_benchmark_runtime() -> Result<sui::Runtime> {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title(TEXT_EDITING_BENCHMARK_TITLE)
                    .root(
                        SizedBox::new()
                            .size(Size::new(900.0, 520.0))
                            .with_child(super::build_text_editing_benchmark()),
                    ),
            )
            .build()
    }

    fn build_text_rendering_comparison_runtime() -> Result<sui::Runtime> {
        build_text_rendering_comparison_application().build()
    }

    fn build_narrow_text_rendering_comparison_runtime() -> Result<sui::Runtime> {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title(TEXT_RENDERING_COMPARISON_TITLE)
                    .root(
                        SizedBox::new()
                            .size(Size::new(430.0, 320.0))
                            .with_child(super::build_text_rendering_comparison_surface()),
                    ),
            )
            .build()
    }

    fn build_color_validation_runtime() -> Result<sui::Runtime> {
        super::build_color_validation_application().build()
    }

    fn build_narrow_color_validation_runtime() -> Result<sui::Runtime> {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title(super::COLOR_VALIDATION_VIEW_TITLE)
                    .root(
                        SizedBox::new()
                            .size(Size::new(430.0, 320.0))
                            .with_child(super::build_color_validation_surface()),
                    ),
            )
            .build()
    }

    fn assert_semantics_omit_live_performance_overlay(semantics: &[sui::SemanticsNode]) {
        assert!(
            semantics
                .iter()
                .all(|node| node.name.as_deref() != Some("Live performance overlay")),
            "expected semantics tree to omit the floating live performance overlay outside sui-demo"
        );
    }

    #[cfg(feature = "artifacts")]
    fn unique_visual_artifact_test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sui-demo-widget-book-artifacts-{}-{}-{}",
            std::process::id(),
            nonce,
            name
        ))
    }

    fn solid_fill_max_channel(output: &RenderOutput) -> f32 {
        let mut max_channel = 0.0_f32;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    brush: Brush::Solid(color),
                    ..
                }
                | SceneCommand::FillPath {
                    brush: Brush::Solid(color),
                    ..
                } => {
                    max_channel = max_channel.max(color.red.max(color.green.max(color.blue)));
                }
                _ => {}
            });
        max_channel
    }

    fn solid_fill_colors(output: &RenderOutput) -> Vec<sui::Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    brush: Brush::Solid(color),
                    ..
                }
                | SceneCommand::FillPath {
                    brush: Brush::Solid(color),
                    ..
                } => colors.push(*color),
                _ => {}
            });
        colors
    }

    fn solid_fill_bounds(output: &RenderOutput, expected: sui::Color) -> Vec<sui::Rect> {
        let mut bounds = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    rect,
                    brush: Brush::Solid(color),
                }
                | SceneCommand::FillRoundedRect {
                    rect,
                    brush: Brush::Solid(color),
                    ..
                } if *color == expected => bounds.push(*rect),
                SceneCommand::FillPath {
                    path,
                    brush: Brush::Solid(color),
                } if *color == expected => bounds.push(path.bounds()),
                _ => {}
            });
        bounds
    }

    fn build_overlay_placeholder_app() -> Result<TestApp> {
        TestApp::new(|| {
            Application::new()
                .window(
                    WindowBuilder::new()
                        .title("Overlay")
                        .root(LivePerformancePanel::new()),
                )
                .build()
        })
    }

    #[cfg(feature = "artifacts")]
    fn build_light_theme_preview_reference_app(card_width: f32) -> Result<TestApp> {
        TestApp::from_runtime(
            Application::new()
                .window(
                    WindowBuilder::new().title("Theme preview reference").root(
                        sui::containers::Padding::all(
                            24.0,
                            SizedBox::new()
                                .width(card_width)
                                .height(super::ThemePreviewShowcase::card_height())
                                .with_child(super::NamedSection::new(
                                    LIGHT_THEME_PREVIEW_CARD_NAME,
                                    theme_preview_card(
                                        DefaultTheme::light(),
                                        "Light",
                                        LIGHT_PREVIEW_ACTION_LABEL,
                                        LIGHT_PREVIEW_INPUT_LABEL,
                                    ),
                                )),
                        ),
                    ),
                )
                .build()?,
        )
    }

    #[cfg(feature = "artifacts")]
    fn build_headless_default_widget_book_app() -> Result<TestApp> {
        TestApp::from_runtime(build_widget_book_application(default_widget_book_state()).build()?)
    }

    #[cfg(feature = "artifacts")]
    fn build_headless_default_theme_demo_app() -> Result<TestApp> {
        TestApp::from_runtime(build_theme_demo_application(default_widget_book_state()).build()?)
    }

    #[cfg(feature = "artifacts")]
    fn viewport_size(window: &TestWindow) -> Result<Size> {
        let snapshot = window.snapshot()?;
        if let Some(scene) = snapshot.scene_summary {
            return Ok(scene.viewport);
        }

        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == SemanticsRole::Window)
            .map(|node| node.bounds.size)
            .ok_or_else(|| sui::Error::new("window viewport is missing from snapshot"))
    }

    #[cfg(feature = "artifacts")]
    fn percentile(sorted: &[f64], quantile: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let rank = ((sorted.len() - 1) as f64 * quantile).round() as usize;
        sorted[rank]
    }

    #[cfg(feature = "artifacts")]
    fn print_widget_book_headless_scroll_benchmark_summary(
        label: &str,
        samples: &[WindowPerformanceSnapshot],
    ) {
        let frame_count = samples.len().max(1) as f64;
        let mut totals = samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .collect::<Vec<_>>();
        totals.sort_by(|a, b| a.total_cmp(b));
        let avg_total_ms = totals.iter().sum::<f64>() / frame_count;
        let avg_visible_layers = samples
            .iter()
            .map(|sample| sample.renderer_submission.visible_layer_count as f64)
            .sum::<f64>()
            / frame_count;
        let avg_direct_packets = samples
            .iter()
            .map(|sample| sample.renderer_submission.direct_packet_count as f64)
            .sum::<f64>()
            / frame_count;
        let avg_packet_rebuilds = samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .total_count() as f64
            })
            .sum::<f64>()
            / frame_count;
        let avg_scene_layers = samples
            .iter()
            .map(|sample| sample.scene.scene_layer_count as f64)
            .sum::<f64>()
            / frame_count;
        let avg_repaint_boundaries = samples
            .iter()
            .map(|sample| sample.scene.repaint_boundary_count as f64)
            .sum::<f64>()
            / frame_count;
        let avg_dirty_coverage = samples
            .iter()
            .map(|sample| sample.scene.dirty_coverage as f64)
            .sum::<f64>()
            / frame_count;
        let max_total_ms = totals.last().copied().unwrap_or(0.0);

        println!("\n=== {label} ===");
        println!("frames:                 {}", samples.len());
        println!(
            "avg frame time:         {avg_total_ms:.3} ms ({:.1} fps)",
            1000.0 / avg_total_ms.max(0.001)
        );
        println!(
            "p95 frame time:         {:.3} ms",
            percentile(&totals, 0.95)
        );
        println!("max frame time:         {max_total_ms:.3} ms");
        println!("avg visible layers:     {avg_visible_layers:.2}");
        println!("avg direct packets:     {avg_direct_packets:.2}");
        println!("avg packet rebuilds:    {avg_packet_rebuilds:.2}");
        println!("avg repaint boundaries: {avg_repaint_boundaries:.2}");
        println!("avg scene layers:       {avg_scene_layers:.2}");
        println!("avg dirty coverage:     {avg_dirty_coverage:.2}%");

        // Per-frame-phase breakdown (Event / MeasureArrange / Paint / Renderer / ...).
        // Shows where wall-clock time actually goes within a frame.
        let mut phase_totals: std::collections::BTreeMap<&'static str, f64> =
            std::collections::BTreeMap::new();
        for sample in samples {
            for timing in &sample.phase_timings {
                *phase_totals.entry(timing.phase.label()).or_default() += timing.duration_ms;
            }
        }
        if !phase_totals.is_empty() {
            let mut phases = phase_totals
                .into_iter()
                .map(|(label, total)| (label, total / frame_count))
                .collect::<Vec<_>>();
            phases.sort_by(|a, b| b.1.total_cmp(&a.1));
            println!("--- avg frame-phase breakdown ---");
            for (label, avg_ms) in phases {
                let pct = if avg_total_ms > 0.0 {
                    (avg_ms / avg_total_ms) * 100.0
                } else {
                    0.0
                };
                println!("  {label:<22} {avg_ms:>8.3} ms ({pct:>5.1}%)");
            }
        }

        // Per-widget measure/arrange/paint timings, only populated when the runtime
        // env var SUI_PROFILE_WIDGET_TIMINGS is set. Surfaces the hottest widgets.
        let mut widget_totals: std::collections::BTreeMap<
            (&'static str, &'static str),
            (f64, usize),
        > = std::collections::BTreeMap::new();
        for sample in samples {
            for timing in &sample.widget_timings {
                let entry = widget_totals
                    .entry((timing.widget_name, timing.phase.label()))
                    .or_default();
                entry.0 += timing.duration_ms;
                entry.1 += timing.calls;
            }
        }
        if !widget_totals.is_empty() {
            let mut widgets = widget_totals
                .into_iter()
                .map(|((name, phase), (total, calls))| {
                    (name, phase, total / frame_count, calls as f64 / frame_count)
                })
                .collect::<Vec<_>>();
            widgets.sort_by(|a, b| b.2.total_cmp(&a.2));
            println!("--- top widget timings (avg/frame) ---");
            for (name, phase, avg_ms, avg_calls) in widgets.into_iter().take(15) {
                println!("  {name:<28} {phase:<8} {avg_ms:>8.4} ms  x{avg_calls:>6.1}");
            }
        }
    }

    #[cfg(feature = "artifacts")]
    fn set_detailed_scene_statistics_mode(window: &TestWindow) -> Result<()> {
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
        window.run_until_idle()
    }

    #[cfg(feature = "artifacts")]
    fn collect_headless_scroll_benchmark_samples(
        window: &TestWindow,
        scroll_name: &str,
        samples: usize,
    ) -> Result<Vec<WindowPerformanceSnapshot>> {
        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(scroll_name);
        let mut collected = Vec::with_capacity(samples);
        let mut previous_frame_index = 0;
        let mut attempts = 0;
        let max_attempts = samples * 8;
        while collected.len() < samples && attempts < max_attempts {
            scroll.scroll_pixels(Vector::new(0.0, -180.0))?;
            let snapshot = window.performance_snapshot()?;
            if snapshot.frame_index > previous_frame_index {
                previous_frame_index = snapshot.frame_index;
                collected.push(snapshot);
            }
            attempts += 1;
        }
        assert_eq!(
            collected.len(),
            samples,
            "headless scroll benchmark collected {} frames after {} attempts",
            collected.len(),
            attempts,
        );
        Ok(collected)
    }

    #[cfg(feature = "artifacts")]
    fn next_headless_benchmark_frame(
        window: &TestWindow,
        previous_frame_index: &mut u64,
        benchmark_name: &str,
        stage: &str,
        step: usize,
    ) -> Result<WindowPerformanceSnapshot> {
        let snapshot = window.performance_snapshot()?;
        if snapshot.frame_index <= *previous_frame_index {
            return Err(sui::Error::new(format!(
                "{benchmark_name} did not render a new frame during {stage} step {}",
                step + 1,
            )));
        }

        *previous_frame_index = snapshot.frame_index;
        Ok(snapshot)
    }

    #[cfg(feature = "artifacts")]
    fn collect_headless_text_editing_benchmark_samples(
        window: &TestWindow,
    ) -> Result<Vec<WindowPerformanceSnapshot>> {
        const EDIT_COMMITS: [&str; 10] = [
            " // typed atlas reuse",
            "\nlet pending_frame = cache_hits + 1;",
            "\n// bidi check: abc אבג 123 مرحبا",
            "\nlet emoji = \"🙂✅🎨\";",
            "\nlet ime_probe = \"候補\";",
            "\nlet syntax_band = highlight_rows.len();",
            "\n// fallback sample: Ж 中 नमस्ते",
            "\nrecord_selection_delta(cursor, viewport);",
            "\nlet scroll_budget_ms = 16.67;",
            "\ncommit_overlay_sample(frame_index);",
        ];
        const IME_PREEDIT_UPDATES: [(&str, Option<(usize, usize)>); 3] = [
            ("候", Some((0, 1))),
            ("候補", Some((1, 2))),
            ("候補を", Some((2, 3))),
        ];
        const EDITOR_SCROLL_FRAMES: usize = 18;
        const SYNTAX_SCROLL_FRAMES: usize = 28;
        const SCROLL_STEP_PX: f32 = -34.0;

        let editor = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(TEXT_EDITING_BENCHMARK_EDITOR_NAME);
        let syntax_scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME);
        editor.focus()?;

        let mut collected = Vec::with_capacity(
            IME_PREEDIT_UPDATES.len()
                + 1
                + EDIT_COMMITS.len()
                + EDITOR_SCROLL_FRAMES
                + SYNTAX_SCROLL_FRAMES,
        );
        let mut previous_frame_index = window.performance_snapshot()?.frame_index;

        editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
        for (step, (text, cursor_range)) in IME_PREEDIT_UPDATES.iter().enumerate() {
            editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
                text: (*text).to_string(),
                cursor_range: cursor_range.map(|(start, end)| start..end),
            }))?;
            collected.push(next_headless_benchmark_frame(
                window,
                &mut previous_frame_index,
                "headless text editing benchmark",
                "composition preedit",
                step,
            )?);
        }
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
            text: "候補を".to_string(),
        }))?;
        collected.push(next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless text editing benchmark",
            "composition commit",
            IME_PREEDIT_UPDATES.len(),
        )?);
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;

        for (step, text) in EDIT_COMMITS.iter().enumerate() {
            let text = (*text).to_string();
            editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
            editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
                text: text.clone(),
                cursor_range: None,
            }))?;
            editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit { text }))?;
            editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;
            collected.push(next_headless_benchmark_frame(
                window,
                &mut previous_frame_index,
                "headless text editing benchmark",
                "typing",
                step,
            )?);
        }

        for step in 0..EDITOR_SCROLL_FRAMES {
            editor.scroll_pixels(Vector::new(0.0, SCROLL_STEP_PX))?;
            collected.push(next_headless_benchmark_frame(
                window,
                &mut previous_frame_index,
                "headless text editing benchmark",
                "editor scroll",
                step,
            )?);
        }

        for step in 0..SYNTAX_SCROLL_FRAMES {
            syntax_scroll.scroll_pixels(Vector::new(0.0, SCROLL_STEP_PX))?;
            collected.push(next_headless_benchmark_frame(
                window,
                &mut previous_frame_index,
                "headless text editing benchmark",
                "syntax scroll",
                step,
            )?);
        }

        Ok(collected)
    }

    #[cfg(feature = "artifacts")]
    fn collect_headless_animation_benchmark_samples(
        window: &TestWindow,
    ) -> Result<Vec<WindowPerformanceSnapshot>> {
        const WARMUP_FRAMES: usize = 12;
        const MEASURED_FRAMES: usize = 120;
        const FRAME_DELTA_SECONDS: f64 = 1.0 / 60.0;

        for name in [
            ANIMATION_BENCHMARK_RETAINED_NAME,
            ANIMATION_BENCHMARK_REPAINT_NAME,
            ANIMATION_BENCHMARK_SCALE_NAME,
        ] {
            window
                .get_by_role(SemanticsRole::Button)
                .with_name(name)
                .click()?;
        }

        let mut collected = Vec::with_capacity(MEASURED_FRAMES);
        let mut previous_frame_index = window.performance_snapshot()?.frame_index;
        for step in 0..(WARMUP_FRAMES + MEASURED_FRAMES) {
            window.advance_time(FRAME_DELTA_SECONDS)?;
            let snapshot = next_headless_benchmark_frame(
                window,
                &mut previous_frame_index,
                "headless animation benchmark",
                "animation frame",
                step,
            )?;
            if step >= WARMUP_FRAMES {
                collected.push(snapshot);
            }
        }

        Ok(collected)
    }

    #[cfg(feature = "artifacts")]
    fn set_window_scale_factor(window: &TestWindow, scale_factor: f64, raw_dpi: f32) -> Result<()> {
        let viewport = viewport_size(window)?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi: Some(raw_dpi),
                suggested_size: Some(viewport),
            }))?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::Resized(viewport)))?;
        window.run_until_idle()
    }

    #[cfg(feature = "artifacts")]
    fn write_screenshot(
        path: impl AsRef<Path>,
        screenshot: &sui_testing::Screenshot,
    ) -> Result<()> {
        screenshot.write_png(path)
    }

    #[cfg(feature = "artifacts")]
    const SCREENSHOT_CHANNEL_TOLERANCE: u8 = 1;

    #[cfg(feature = "artifacts")]
    fn screenshot_pixels_match(left: &[u8], right: &[u8]) -> bool {
        left.iter()
            .zip(right.iter())
            .all(|(left, right)| left.abs_diff(*right) <= SCREENSHOT_CHANNEL_TOLERANCE)
    }

    #[cfg(feature = "artifacts")]
    fn screenshot_diff_count(
        left: &sui_testing::Screenshot,
        right: &sui_testing::Screenshot,
    ) -> usize {
        assert_eq!(left.width(), right.width(), "screenshot widths differ");
        assert_eq!(left.height(), right.height(), "screenshot heights differ");

        left.pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .filter(|(left_px, right_px)| !screenshot_pixels_match(left_px, right_px))
            .count()
    }

    #[cfg(feature = "artifacts")]
    fn screenshot_diff_image(
        left: &sui_testing::Screenshot,
        right: &sui_testing::Screenshot,
    ) -> Result<sui_testing::Screenshot> {
        assert_eq!(left.width(), right.width(), "screenshot widths differ");
        assert_eq!(left.height(), right.height(), "screenshot heights differ");

        let pixels = left
            .pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .flat_map(|(left_px, right_px)| {
                if screenshot_pixels_match(left_px, right_px) {
                    [left_px[0], left_px[1], left_px[2], 96]
                } else {
                    [255, 0, 0, 255]
                }
            })
            .collect::<Vec<_>>();

        sui_testing::Screenshot::new(left.width(), left.height(), pixels)
    }

    #[cfg(feature = "artifacts")]
    fn normalize_screenshot_pair(
        left: &sui_testing::Screenshot,
        right: &sui_testing::Screenshot,
    ) -> Result<(sui_testing::Screenshot, sui_testing::Screenshot)> {
        let width = left.width().min(right.width()) as f32;
        let height = left.height().min(right.height()) as f32;
        let crop = sui::Rect::new(0.0, 0.0, width, height);
        Ok((left.crop(crop)?, right.crop(crop)?))
    }

    #[cfg(feature = "artifacts")]
    #[test]
    fn screenshot_diff_helpers_tolerate_one_channel_value_per_channel() -> Result<()> {
        let left = sui_testing::Screenshot::new(2, 1, vec![10, 20, 30, 40, 100, 110, 120, 130])?;
        let right = sui_testing::Screenshot::new(2, 1, vec![11, 19, 31, 39, 99, 111, 119, 131])?;

        assert_eq!(screenshot_diff_count(&left, &right), 0);

        let diff = screenshot_diff_image(&left, &right)?;
        assert_eq!(diff.pixels(), &[10, 20, 30, 96, 100, 110, 120, 96]);

        Ok(())
    }

    #[test]
    fn text_rendering_comparison_surface_exposes_all_render_modes() {
        let mut runtime =
            build_text_rendering_comparison_runtime().expect("comparison runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("comparison surface should render");

        let semantics = runtime
            .semantics(window_id)
            .expect("comparison semantics should exist");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Window
                && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_TITLE)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_SCROLL_NAME)
        }));

        for mode_name in [
            "Grayscale baseline",
            "Grayscale + hinting",
            "Grayscale + stem darkening",
            "LCD subpixel",
            "LCD subpixel + hinting",
            "LCD subpixel + hinting + stem darkening",
        ] {
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(mode_name)
            }));
        }
    }

    #[test]
    fn text_rendering_comparison_surface_uses_two_axis_scroll_when_narrow() {
        let mut runtime = build_narrow_text_rendering_comparison_runtime()
            .expect("narrow comparison runtime should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("narrow comparison surface should render");

        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_SCROLL_NAME)
            })
            .expect("text comparison scroll view should be present");
        let horizontal_scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref()
                        == Some(super::TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME)
            })
            .expect("horizontal text comparison scroll bar should be present");
        let vertical_scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref()
                        == Some(super::TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME)
            })
            .expect("vertical text comparison scroll bar should be present");

        let horizontal_max = match horizontal_scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => max,
            _ => 0.0,
        };
        let vertical_max = match vertical_scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => max,
            _ => 0.0,
        };

        assert!(horizontal_max > 0.0);
        assert!(vertical_max > 0.0);
        assert!(horizontal_scroll_bar.bounds.y() >= scroll.bounds.max_y());
        assert!(vertical_scroll_bar.bounds.x() >= scroll.bounds.max_x());
    }

    #[test]
    fn text_validation_scroll_repaints_visible_content() -> Result<()> {
        let mut runtime = build_text_validation_runtime()?;
        let window_id = runtime.window_ids()[0];
        let before = runtime.render(window_id)?;
        let scroll_node = before
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(TEXT_VALIDATION_SCROLL_NAME)
            })
            .expect("text validation scroll semantics present");
        let scroll_point = Point::new(
            scroll_node.bounds.x() + 24.0,
            scroll_node.bounds.y() + (scroll_node.bounds.height() * 0.5),
        );

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, scroll_point);
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -220.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll))?;
        let after = runtime.render(window_id)?;

        assert_ne!(before.frame.scene, after.frame.scene);
        assert!(after.frame.layer_updates.iter().any(|update| {
            update.owner == scroll_node.id && update.kind == SceneLayerUpdateKind::Content
        }));

        Ok(())
    }

    #[test]
    fn color_validation_surface_exposes_wide_gamut_reference_swatches() {
        let mut runtime =
            build_color_validation_runtime().expect("color validation runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("color validation surface should render");

        let semantics = runtime
            .semantics(window_id)
            .expect("color validation semantics should exist");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Window
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_VIEW_TITLE)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_SCROLL_NAME)
        }));

        for swatch_name in [
            "sRGB reference red",
            "Display P3 reference red",
            "sRGB clipped lime",
            "Display P3 vivid lime",
            "sRGB accent cyan",
            "Display P3 accent cyan",
            "Reference white 1.0",
            "Highlight white 2.0",
            "Highlight white 4.0",
            "Highlight white 8.0",
            "Orange highlight 1.0",
            "Orange highlight 2.0",
            "Cyan highlight 1.0",
            "Cyan highlight 2.0",
            "SDR white baseline",
            "SDR clipped white 2.0",
        ] {
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some(swatch_name)
            }));
        }
    }

    #[test]
    fn color_validation_surface_keeps_swatch_labels_readable_when_narrow() {
        let mut runtime = build_narrow_color_validation_runtime()
            .expect("narrow color validation runtime should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("narrow color validation surface should render");

        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(super::COLOR_VALIDATION_SCROLL_NAME)
            })
            .expect("color validation scroll view should be present");
        let horizontal_scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref()
                        == Some(super::COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
            })
            .expect("horizontal color validation scroll bar should be present");
        let vertical_scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref()
                        == Some(super::COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
            })
            .expect("vertical color validation scroll bar should be present");
        let cyan_label = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Cyan highlight 2.0")
            })
            .expect("final HDR color label should be present");
        let hdr_description = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("Colored highlights help catch cases"))
            })
            .expect("HDR color description should be present");

        let horizontal_max = match horizontal_scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => max,
            _ => 0.0,
        };
        let vertical_max = match vertical_scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => max,
            _ => 0.0,
        };

        assert!(horizontal_max > 0.0);
        assert!(vertical_max > 0.0);
        assert!(horizontal_scroll_bar.bounds.y() >= scroll.bounds.max_y());
        assert!(vertical_scroll_bar.bounds.x() >= scroll.bounds.max_x());
        assert!(cyan_label.bounds.width() >= 80.0);
        assert!(cyan_label.bounds.height() <= 40.0);
        assert!(hdr_description.bounds.height() > 20.0);
        assert!(hdr_description.bounds.width() < 900.0);
    }

    #[test]
    fn color_validation_surface_omits_live_performance_overlay() {
        let mut runtime =
            build_color_validation_runtime().expect("color validation runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("color validation surface should render");

        let semantics = runtime
            .semantics(window_id)
            .expect("color validation semantics should exist");
        assert_semantics_omit_live_performance_overlay(&semantics);
    }

    #[test]
    fn widget_book_application_omits_live_performance_overlay() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render");

        let semantics = runtime
            .semantics(window_id)
            .expect("widget book semantics should exist");
        assert_semantics_omit_live_performance_overlay(&semantics);
    }

    #[test]
    fn hdr_theme_lab_exposes_mode_comparison_sections() {
        let mut runtime = build_theme_demo_application(default_widget_book_state())
            .build()
            .expect("theme demo runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("theme demo should render for HDR lab semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("theme demo semantics should exist");

        for section_name in [
            super::HDR_THEME_LAB_NAME,
            super::HDR_THEME_LAB_ACTIVE_PREVIEW_NAME,
            super::hdr_theme_lab_section_name(super::HdrThemeMode::Disabled),
            super::hdr_theme_lab_section_name(super::HdrThemeMode::WideGamutOnly),
            super::hdr_theme_lab_section_name(super::HdrThemeMode::ConstrainedHdr),
            super::hdr_theme_lab_section_name(super::HdrThemeMode::FullHdr),
        ] {
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(section_name)
            }));
        }

        for (button_name, switch_name) in [
            (
                format!(
                    "{} sample action",
                    super::hdr_theme_mode_title(super::HdrThemeMode::Disabled)
                ),
                format!(
                    "{} sample live indicator",
                    super::hdr_theme_mode_title(super::HdrThemeMode::Disabled)
                ),
            ),
            (
                format!(
                    "{} sample action",
                    super::hdr_theme_mode_title(super::HdrThemeMode::WideGamutOnly)
                ),
                format!(
                    "{} sample live indicator",
                    super::hdr_theme_mode_title(super::HdrThemeMode::WideGamutOnly)
                ),
            ),
            (
                format!(
                    "{} sample action",
                    super::hdr_theme_mode_title(super::HdrThemeMode::ConstrainedHdr)
                ),
                format!(
                    "{} sample live indicator",
                    super::hdr_theme_mode_title(super::HdrThemeMode::ConstrainedHdr)
                ),
            ),
            (
                format!(
                    "{} sample action",
                    super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr)
                ),
                format!(
                    "{} sample live indicator",
                    super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr)
                ),
            ),
        ] {
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(button_name.as_str())
            }));
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::Switch
                    && node.name.as_deref() == Some(switch_name.as_str())
            }));
        }
    }

    #[test]
    fn widget_book_gallery_omits_theme_demo_sections() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("widget book semantics should exist");

        for removed_section in [super::THEME_PREVIEW_NAME, super::HDR_THEME_LAB_NAME] {
            assert!(
                semantics.iter().all(|node| {
                    node.role != SemanticsRole::GenericContainer
                        || node.name.as_deref() != Some(removed_section)
                }),
                "expected the main widget book gallery to omit {removed_section:?}"
            );
        }
    }

    #[test]
    fn widget_book_exposes_widget_states_gallery() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render for widget states semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("widget book semantics should exist");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(WIDGET_STATES_GALLERY_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(WIDGET_STATES_BUTTON_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(super::WIDGET_STATES_ICON_BUTTON_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(WIDGET_STATES_TEXT_INPUT_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(WIDGET_STATES_TEXT_AREA_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ComboBox
                && node.name.as_deref() == Some(WIDGET_STATES_SELECT_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::CheckBox
                && node.name.as_deref() == Some(WIDGET_STATES_CHECKBOX_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Switch
                && node.name.as_deref() == Some(WIDGET_STATES_SWITCH_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(WIDGET_STATES_SLIDER_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Tabs
                && node.name.as_deref() == Some(WIDGET_STATES_TABS_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Menu
                && node.name.as_deref() == Some(WIDGET_STATES_MENU_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Popover
                && node.name.as_deref() == Some(WIDGET_STATES_POPOVER_NAME)
        }));
    }

    #[test]
    fn widget_book_state_matrix_rows_share_a_single_surface() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("widget book should render for widget state row surfaces");

        let button = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(WIDGET_STATES_BUTTON_LABEL)
            })
            .expect("state action button should be visible");
        let text_input = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(WIDGET_STATES_TEXT_INPUT_LABEL)
            })
            .expect("state text input should be visible");
        let button_center = Point::new(
            button.bounds.x() + button.bounds.width() * 0.5,
            button.bounds.y() + button.bounds.height() * 0.5,
        );
        let input_center = Point::new(
            text_input.bounds.x() + text_input.bounds.width() * 0.5,
            text_input.bounds.y() + text_input.bounds.height() * 0.5,
        );
        let raised_surfaces =
            solid_fill_bounds(&output, DefaultTheme::default().palette.surface_raised);

        assert!(
            raised_surfaces
                .iter()
                .any(|bounds| { bounds.contains(button_center) && bounds.contains(input_center) }),
            "the action and text-entry state columns should share one raised row surface"
        );
    }

    #[test]
    fn widget_book_size_presets_section_exposes_density_samples() {
        let root = SizedBox::new().width(1040.0).height(760.0).with_child(
            super::build_size_presets_gallery_with_theme(super::default_widget_book_theme_reader()),
        );
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Size presets").root(root))
            .build()
            .expect("size preset section runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("size preset section should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("size preset section semantics should exist");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(super::SIZE_PRESETS_GALLERY_NAME)
        }));

        let button_height = |name: &str| {
            semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(name)
                })
                .map(|node| node.bounds.height())
                .unwrap_or_else(|| panic!("missing {name} preset action button"))
        };
        let compact_button = button_height(super::SIZE_PRESET_COMPACT_ACTION_LABEL);
        let comfortable_button = button_height(super::SIZE_PRESET_COMFORTABLE_ACTION_LABEL);
        let touch_button = button_height(super::SIZE_PRESET_TOUCH_ACTION_LABEL);

        assert!(compact_button < comfortable_button);
        assert!(comfortable_button < touch_button);

        for name in [
            super::SIZE_PRESET_COMPACT_INPUT_LABEL,
            super::SIZE_PRESET_COMFORTABLE_INPUT_LABEL,
            super::SIZE_PRESET_TOUCH_INPUT_LABEL,
        ] {
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::TextInput && node.name.as_deref() == Some(name)
            }));
        }
    }

    #[test]
    fn widget_book_choices_ranges_and_selects_use_consistent_heights() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        scroll_to_story_target(&window, StoryCase::Slider, 12)?;
        let snapshot = window.snapshot()?;
        let semantics = &snapshot.accessibility.nodes;
        let theme = DefaultTheme::default();
        let style = theme.body_text_style();
        let padding = theme.metrics.text_input_padding;
        let expected_height =
            (style.line_height + padding.top + padding.bottom).max(theme.metrics.min_height);

        for (role, name) in [
            (SemanticsRole::Switch, SWITCH_LABEL),
            (SemanticsRole::RadioButton, RADIO_BUTTON_LABEL),
            (SemanticsRole::Slider, SLIDER_NAME),
            (SemanticsRole::SpinBox, NUMBER_INPUT_NAME),
            (SemanticsRole::ComboBox, SELECT_NAME),
        ] {
            let node = semantics
                .iter()
                .find(|node| node.role == role && node.name.as_deref() == Some(name))
                .unwrap_or_else(|| panic!("missing {role:?} named {name:?}"));
            assert!(
                (node.bounds.height() - expected_height).abs() < 0.01,
                "expected {role:?} named {name:?} to use the theme control height {expected_height}, got {:?}",
                node.bounds
            );
        }

        Ok(())
    }

    #[test]
    fn widget_book_exposes_animation_demo_panel() {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Animation demo semantics")
                    .root(super::build_animation_demo_panel()),
            )
            .build()
            .expect("animation demo runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("animation demo should render for semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("animation demo semantics should exist");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_BUTTON_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Switch
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_SWITCH_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(super::TIMELINE_ANIMATION_PREVIEW_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(super::ANIMATION_EDITOR_SURFACE_NAME)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_TEXT_INPUT_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_POPOVER_TRIGGER_LABEL)
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Popover
                && node.name.as_deref() == Some(super::ANIMATION_DEMO_POPOVER_NAME)
        }));
    }

    #[test]
    fn hdr_theme_lab_includes_emissive_indicator_and_popup_examples() {
        let mut runtime = build_theme_demo_application(default_widget_book_state())
            .build()
            .expect("theme demo runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("theme demo should render for HDR lab semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("theme demo semantics should exist");
        let full_hdr_title = super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr);
        let swatch_name = format!("{full_hdr_title} emissive indicator");
        let popover_name = format!("{full_hdr_title} attention popover");
        let popover_trigger = format!("{full_hdr_title} attention trigger");

        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ColorSwatch
                && node.name.as_deref() == Some(swatch_name.as_str())
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(popover_trigger.as_str())
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Popover
                && node.name.as_deref() == Some(popover_name.as_str())
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.description.as_deref().is_some_and(|description| {
                    description.contains("button, switch, emissive indicator, and popup trigger")
                })
                && node.name.as_deref() == Some(super::HDR_THEME_LAB_NAME)
        }));
    }

    #[test]
    fn hdr_theme_lab_full_hdr_emits_stronger_headroom_than_constrained() {
        let mut constrained_runtime = Application::new()
            .window(WindowBuilder::new().title("Constrained HDR lab").root(
                super::hdr_theme_lab_card(
                    "Constrained HDR isolated",
                    super::HdrThemeMode::ConstrainedHdr,
                    "Constrained HDR isolated",
                    "Constrained HDR isolated preview",
                ),
            ))
            .build()
            .expect("constrained HDR lab runtime should build");
        let constrained_window = constrained_runtime.window_ids()[0];
        let constrained_output = constrained_runtime
            .render(constrained_window)
            .expect("constrained HDR lab should render");

        let mut full_runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Full HDR lab")
                    .root(super::hdr_theme_lab_card(
                        "Full HDR isolated",
                        super::HdrThemeMode::FullHdr,
                        "Full HDR isolated",
                        "Full HDR isolated preview",
                    )),
            )
            .build()
            .expect("full HDR lab runtime should build");
        let full_window = full_runtime.window_ids()[0];
        let full_output = full_runtime
            .render(full_window)
            .expect("full HDR lab should render");

        let constrained_max = solid_fill_max_channel(&constrained_output);
        let full_max = solid_fill_max_channel(&full_output);

        assert!(
            constrained_max > 1.0,
            "constrained HDR lab should emit above-reference-white colors, got {constrained_max}"
        );
        assert!(
            full_max > constrained_max,
            "full HDR lab should exceed constrained HDR scene headroom, got full={full_max} constrained={constrained_max}"
        );
        assert!(
            full_max >= 2.0,
            "full HDR lab should emit clearly HDR-bright values, got {full_max}"
        );
    }

    #[test]
    fn widget_book_theme_preview_toggle_hides_dark_card() -> Result<()> {
        let app = build_default_theme_demo_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::ThemePreview, 2)?;
        let before = window.capture_screenshot()?;

        window
            .get_by_role(SemanticsRole::Switch)
            .with_name(THEME_PREVIEW_TOGGLE_LABEL)
            .click()?;

        let after = window.capture_screenshot()?;
        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_popover_click_repaints_gallery() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::PopoverOpen, 12)?;
        let before = window.capture_screenshot()?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(POPOVER_TRIGGER_LABEL)
            .click()?;

        window
            .get_by_role(SemanticsRole::Popover)
            .with_name(POPOVER_NAME)
            .capture_screenshot()?;
        let after = window.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_project_settings_click_repaints_gallery() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::Dialog, 12)?;
        let before = window.capture_screenshot()?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(DIALOG_TRIGGER_LABEL)
            .click()?;

        window
            .get_by_role(SemanticsRole::Dialog)
            .with_name(DIALOG_TITLE)
            .capture_screenshot()?;
        let after = window.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_tooltip_hides_after_pointer_moves_to_another_control() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::TooltipVisible, 12)?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(TOOLTIP_TRIGGER_LABEL)
            .hover()?;
        assert_eq!(
            window
                .get_by_role(SemanticsRole::Tooltip)
                .with_name(TOOLTIP_TEXT)
                .count()?,
            1
        );

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(POPOVER_TRIGGER_LABEL)
            .hover()?;
        assert_eq!(
            window
                .get_by_role(SemanticsRole::Tooltip)
                .with_name(TOOLTIP_TEXT)
                .count()?,
            0
        );

        Ok(())
    }

    #[test]
    fn widget_book_text_input_accepts_plain_keyboard_typing() -> Result<()> {
        let baseline_summary = {
            let baseline_app = build_default_widget_book_app()?;
            let baseline_window = baseline_app.main_window()?;
            scroll_to_story_target(&baseline_window, StoryCase::Summary, 12)?;
            baseline_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?
        };

        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::FilledInput, 12)?;
        let input = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(NAME_INPUT_LABEL);
        input.focus()?;
        input.press("Z")?;
        let input_value = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(NAME_INPUT_LABEL)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Text(value)) => Some(value),
                _ => None,
            })
            .expect("text input semantics value present after typing");
        assert_eq!(input_value, "AdaZ");

        scroll_to_story_target(&window, StoryCase::Summary, 12)?;
        let summary_description = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(SUMMARY_NAME)
            })
            .and_then(|node| node.description)
            .expect("summary semantics description present after typing");
        assert!(
            summary_description.contains("AdaZ"),
            "summary semantics did not reflect the typed name: {summary_description}"
        );
        let edited_summary = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(SUMMARY_NAME)
            .capture_screenshot()?;

        assert!(
            edited_summary != baseline_summary,
            "summary screenshot did not change after typing"
        );

        Ok(())
    }

    #[test]
    fn widget_book_summary_uses_live_dark_theme_tokens() -> Result<()> {
        let theme = DefaultTheme::dark();
        let theme_reader: super::WidgetBookThemeReader = Rc::new(move || theme);
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Widget book summary").root(
                super::WidgetBookSummary::new(default_widget_book_state(), theme_reader),
            ))
            .build()?;
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let fills = solid_fill_colors(&output);

        assert!(fills.contains(&theme.palette.surface_raised));
        assert!(
            !fills.contains(&sui::Color::rgba(0.985, 0.99, 1.0, 1.0)),
            "dark live summary should not use the old hardcoded light panel fill"
        );
        Ok(())
    }

    #[test]
    fn text_validation_surface_supports_ime_and_selection() -> Result<()> {
        let app = build_text_validation_app()?;
        let window = app.main_window()?;
        let editor = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(TEXT_VALIDATION_EDITOR_NAME);

        editor.focus()?;
        let before_selection = editor.capture_screenshot()?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
            text: " // validated🙂".to_string(),
            cursor_range: None,
        }))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
            text: " // validated🙂".to_string(),
        }))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;

        let mut shift_left = KeyboardEvent::new("ArrowLeft", KeyState::Pressed);
        shift_left.modifiers.shift = true;
        for _ in 0..6 {
            editor.dispatch_event(Event::Keyboard(shift_left.clone()))?;
        }

        let after_selection = editor.capture_screenshot()?;
        assert_ne!(before_selection, after_selection);

        let editor_value = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(TEXT_VALIDATION_EDITOR_NAME)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Text(value)) => Some(value),
                _ => None,
            })
            .expect("validation editor semantics value present after IME commit");
        assert!(editor_value.contains("validated🙂"));

        Ok(())
    }

    #[test]
    fn widget_book_gallery_wheel_scroll_updates_screenshot_and_reveals_lower_story() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_gallery_scroll_redraws_when_split_view_is_visible() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        scroll_to_story_target(&window, StoryCase::SplitView, 12)?;

        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -48.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_text_area_focus_does_not_trap_gallery_wheel_scroll() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        scroll_to_story_target(&window, StoryCase::TextArea, 12)?;
        let text_area = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(TEXT_AREA_LABEL);

        text_area.click()?;
        let before = window.capture_screenshot()?;
        text_area.scroll_pixels(Vector::new(0.0, -240.0))?;
        let after = window.capture_screenshot()?;

        assert_ne!(
            before, after,
            "wheel scrolling over the focused multiline editor should still move the gallery"
        );
        Ok(())
    }

    #[test]
    fn widget_book_gallery_small_wheel_scroll_updates_screenshot() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -12.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_gallery_exposes_visible_scroll_bar() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("widget book should render");
        let gallery = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
            })
            .expect("widget book gallery scroll view should be present");
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(GALLERY_SCROLL_BAR_NAME)
            })
            .expect("widget book gallery scroll bar should be present");

        assert!(scroll_bar.bounds.x() >= gallery.bounds.max_x());
        assert!(scroll_bar.bounds.height() >= gallery.bounds.height() - 1.0);
    }

    #[test]
    fn widget_book_and_theme_roots_start_at_scroll_view_top() -> Result<()> {
        fn assert_title_flush_with_scroll(output: &RenderOutput, scroll_name: &str, title: &str) {
            let scroll = output
                .semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::ScrollView
                        && node.name.as_deref() == Some(scroll_name)
                })
                .expect("root scroll view should be present");
            let title_node = output
                .semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Text && node.name.as_deref() == Some(title)
                })
                .expect("root title text should be present");

            assert!(
                (title_node.bounds.y() - scroll.bounds.y()).abs() < 0.01,
                "{title} should start at the scroll viewport top: title={:?}, scroll={:?}",
                title_node.bounds,
                scroll.bounds
            );
        }

        let mut widget_runtime =
            build_widget_book_application(default_widget_book_state()).build()?;
        let widget_window = widget_runtime.window_ids()[0];
        let widget_output = widget_runtime.render(widget_window)?;
        assert_title_flush_with_scroll(&widget_output, GALLERY_SCROLL_NAME, WINDOW_TITLE);

        let mut theme_runtime =
            build_theme_demo_application(default_widget_book_state()).build()?;
        let theme_window = theme_runtime.window_ids()[0];
        let theme_output = theme_runtime.render(theme_window)?;
        assert_title_flush_with_scroll(&theme_output, THEME_DEMO_SCROLL_NAME, THEME_DEMO_TITLE);

        Ok(())
    }

    #[test]
    fn widget_book_gallery_exposes_color_picker_story() -> Result<()> {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Color story")
                    .root(build_color_and_imagery_story()),
            )
            .build()?;
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorPicker
                    && node.name.as_deref() == Some(COLOR_PICKER_NAME)
            })
            .expect("widget book gallery should expose the color picker story");

        assert!(picker.bounds.width() >= 420.0);
        assert!(picker.bounds.height() >= 424.0);
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    fn widget_book_visual_artifacts_include_hdr_widget_book_capture() -> Result<()> {
        let artifact_root = unique_visual_artifact_test_dir("hdr-widget-book");
        let output_root = super::visual_artifacts::write_visual_artifacts_to(&artifact_root)?;
        let hdr_dir = output_root.join("hdr-widget-book");

        assert!(hdr_dir.join("window.png").exists());
        assert!(hdr_dir.join("hdr-intermediate.exr").exists());
        assert!(hdr_dir.join("hdr-intermediate.avif").exists());
        assert!(hdr_dir.join("luminance-map.png").exists());
        assert!(hdr_dir.join("headroom-map.png").exists());
        assert!(hdr_dir.join("clip-mask.png").exists());
        assert!(hdr_dir.join("output-diagnostics.txt").exists());
        assert!(hdr_dir.join("capture-metrics.txt").exists());
        assert!(
            hdr_dir.join("final-composed.exr").exists()
                || hdr_dir.join("final-composed.avif").exists()
                || hdr_dir.join("final-composed.png").exists()
        );

        fs::remove_dir_all(&artifact_root).ok();
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "slow; run `cargo run -p sui-demo --bin sui-demo-artifacts` to generate artifacts"]
    fn widget_book_generates_visual_artifacts() -> Result<()> {
        let artifact_root = super::write_visual_artifacts()?;

        for story in StoryCase::ALL {
            assert!(
                artifact_root
                    .join(story.id())
                    .join("screenshot.png")
                    .exists()
            );
        }

        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    fn widget_book_theme_preview_switch_matches_reference_at_fractional_dpi() -> Result<()> {
        let artifact_dir = artifact_root().join("theme-preview-150-dpi");
        if artifact_dir.exists() {
            fs::remove_dir_all(&artifact_dir).map_err(|error| {
                sui::Error::new(format!(
                    "failed to clear {}: {error}",
                    artifact_dir.display()
                ))
            })?;
        }
        fs::create_dir_all(&artifact_dir).map_err(|error| {
            sui::Error::new(format!(
                "failed to create {}: {error}",
                artifact_dir.display()
            ))
        })?;

        let live_app = build_headless_default_theme_demo_app()?;
        let live_window = live_app.main_window()?;
        set_window_scale_factor(&live_window, 1.5, 144.0)?;
        scroll_to_story_target(&live_window, StoryCase::ThemePreview, 12)?;

        let live_artifacts = live_window.capture_artifacts()?;
        live_artifacts.write_to_dir(artifact_dir.join("live-window"))?;

        let live_light_card_locator = live_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(LIGHT_THEME_PREVIEW_CARD_NAME);
        let live_light_card = live_light_card_locator.capture_screenshot()?;
        let live_switch = live_window
            .get_by_role(SemanticsRole::Switch)
            .with_name("Light preview live updates")
            .capture_screenshot()?;
        write_screenshot(artifact_dir.join("live-light-card.png"), &live_light_card)?;
        write_screenshot(artifact_dir.join("live-light-switch.png"), &live_switch)?;

        let live_snapshot = live_window.snapshot()?;
        let live_card_bounds = live_snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(LIGHT_THEME_PREVIEW_CARD_NAME)
            })
            .map(|node| node.bounds)
            .ok_or_else(|| sui::Error::new("light theme preview card is missing"))?;

        let reference_app = build_light_theme_preview_reference_app(live_card_bounds.width())?;
        let reference_window = reference_app.main_window()?;
        set_window_scale_factor(&reference_window, 1.5, 144.0)?;

        let reference_artifacts = reference_window.capture_artifacts()?;
        reference_artifacts.write_to_dir(artifact_dir.join("reference-window"))?;

        let reference_light_card = reference_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(LIGHT_THEME_PREVIEW_CARD_NAME)
            .capture_screenshot()?;
        let reference_switch = reference_window
            .get_by_role(SemanticsRole::Switch)
            .with_name("Light preview live updates")
            .capture_screenshot()?;
        write_screenshot(
            artifact_dir.join("reference-light-card.png"),
            &reference_light_card,
        )?;
        write_screenshot(
            artifact_dir.join("reference-light-switch.png"),
            &reference_switch,
        )?;

        let (normalized_live_switch, normalized_reference_switch) =
            normalize_screenshot_pair(&live_switch, &reference_switch)?;
        write_screenshot(
            artifact_dir.join("live-light-switch-normalized.png"),
            &normalized_live_switch,
        )?;
        write_screenshot(
            artifact_dir.join("reference-light-switch-normalized.png"),
            &normalized_reference_switch,
        )?;

        let diff = screenshot_diff_image(&normalized_live_switch, &normalized_reference_switch)?;
        write_screenshot(artifact_dir.join("switch-diff.png"), &diff)?;
        let diff_count =
            screenshot_diff_count(&normalized_live_switch, &normalized_reference_switch);
        let switch_control_crop = sui::Rect::new(
            0.0,
            0.0,
            56.0_f32.min(normalized_live_switch.width() as f32),
            normalized_live_switch.height() as f32,
        );
        let live_switch_control = normalized_live_switch.crop(switch_control_crop)?;
        let reference_switch_control = normalized_reference_switch.crop(switch_control_crop)?;
        write_screenshot(
            artifact_dir.join("live-light-switch-control.png"),
            &live_switch_control,
        )?;
        write_screenshot(
            artifact_dir.join("reference-light-switch-control.png"),
            &reference_switch_control,
        )?;
        let control_diff = screenshot_diff_image(&live_switch_control, &reference_switch_control)?;
        write_screenshot(artifact_dir.join("switch-control-diff.png"), &control_diff)?;
        let control_diff_count =
            screenshot_diff_count(&live_switch_control, &reference_switch_control);
        fs::write(
            artifact_dir.join("comparison.txt"),
            format!(
                "live card: {}\nreference card: isolated {}\nlive switch: {}x{}\nreference switch: {}x{}\nnormalized switch: {}x{}\nfull-row diff pixels: {}\nswitch-control diff pixels: {}\n",
                LIGHT_THEME_PREVIEW_CARD_NAME,
                LIGHT_THEME_PREVIEW_CARD_NAME,
                live_switch.width(),
                live_switch.height(),
                reference_switch.width(),
                reference_switch.height(),
                normalized_live_switch.width(),
                normalized_live_switch.height(),
                diff_count,
                control_diff_count,
            ),
        )
        .map_err(|error| {
            sui::Error::new(format!(
                "failed to write comparison metadata in {}: {error}",
                artifact_dir.display()
            ))
        })?;

        assert!(
            control_diff_count <= 550,
            "theme preview switch control differed from isolated reference at 150% DPI; diff pixels={control_diff_count}; see {}",
            artifact_dir.display()
        );

        Ok(())
    }

    #[test]
    fn widget_book_configured_story_renders_expected_visual_state() -> Result<()> {
        let (
            _default_slider,
            default_number_value,
            default_select_value,
            default_summary,
            default_slider_value,
        ) = {
            let default_app = build_default_widget_book_app()?;
            let default_window = default_app.main_window()?;
            scroll_to_story_target(&default_window, StoryCase::Slider, 12)?;
            let default_slider = default_window
                .get_by_role(SemanticsRole::Slider)
                .with_name(SLIDER_NAME)
                .capture_screenshot()?;
            let default_slider_value = default_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::Slider && node.name.as_deref() == Some(SLIDER_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Range { value, .. }) => Some(value),
                    _ => None,
                })
                .expect("default slider semantics value present");
            scroll_to_story_target(&default_window, StoryCase::NumberInput, 12)?;
            let default_number_value = default_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::SpinBox
                        && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Number(value)) => Some(value),
                    _ => None,
                })
                .expect("default number input semantics value present");
            scroll_to_story_target(&default_window, StoryCase::SelectExpanded, 12)?;
            let default_select_value = combo_box_text_value(&default_window, SELECT_NAME)?;
            scroll_to_story_target(&default_window, StoryCase::Summary, 12)?;
            let default_summary = default_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                default_slider,
                default_number_value,
                default_select_value,
                default_summary,
                default_slider_value,
            )
        };

        let (
            _configured_slider,
            configured_number_value,
            configured_select_value,
            configured_summary,
            configured_slider_value,
        ) = {
            let configured_app = build_configured_widget_book_app()?;
            let configured_window = configured_app.main_window()?;
            scroll_to_story_target(&configured_window, StoryCase::Slider, 12)?;
            let configured_slider = configured_window
                .get_by_role(SemanticsRole::Slider)
                .with_name(SLIDER_NAME)
                .capture_screenshot()?;
            let configured_slider_value = configured_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::Slider && node.name.as_deref() == Some(SLIDER_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Range { value, .. }) => Some(value),
                    _ => None,
                })
                .expect("configured slider semantics value present");
            scroll_to_story_target(&configured_window, StoryCase::NumberInput, 12)?;
            let configured_number_value = configured_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::SpinBox
                        && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Number(value)) => Some(value),
                    _ => None,
                })
                .expect("configured number input semantics value present");
            scroll_to_story_target(&configured_window, StoryCase::SelectExpanded, 12)?;
            let configured_select_value = combo_box_text_value(&configured_window, SELECT_NAME)?;
            scroll_to_story_target(&configured_window, StoryCase::Summary, 12)?;
            let configured_summary = configured_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                configured_slider,
                configured_number_value,
                configured_select_value,
                configured_summary,
                configured_slider_value,
            )
        };

        assert_eq!(default_slider_value, 72.0);
        assert_eq!(configured_slider_value, 35.0);
        assert_eq!(default_number_value, 12.0);
        assert_eq!(configured_number_value, 24.0);
        assert_eq!(default_select_value, "Normal");
        assert_eq!(configured_select_value, "Multiply");

        assert!(
            configured_summary != default_summary,
            "configured summary screenshot matched default state"
        );

        Ok(())
    }

    #[test]
    fn live_performance_frame_sample_records_snapshot_phase_costs() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        assert!(display.borrow().samples.is_empty());

        let snapshot = sample_detailed_window_performance_snapshot_record(WindowId::new(11));
        display
            .borrow_mut()
            .samples
            .push(LivePerformanceFrameSample::from_snapshot(&snapshot));
        let sample = display.borrow().samples[0].clone();

        assert_eq!(sample.frame_index, snapshot.frame_index);
        assert_eq!(
            sample.stage_costs[frame_phase_index(FramePhase::Paint)],
            0.8
        );
        assert_eq!(
            sample.stage_costs[frame_phase_index(FramePhase::Renderer)],
            1.9
        );
    }

    #[test]
    fn live_performance_panel_does_not_create_child_widgets_when_snapshot_updates() {
        struct CountingVisitor {
            count: usize,
        }

        impl WidgetPodVisitor for CountingVisitor {
            fn visit(&mut self, _child: &WidgetPod) {
                self.count += 1;
            }
        }

        let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        let panel = LivePerformancePanel::with_display(Rc::clone(&display));
        let mut visitor = CountingVisitor { count: 0 };
        Widget::visit_children(&panel, &mut visitor);
        assert_eq!(visitor.count, 0);

        display.borrow_mut().snapshot =
            Some(sample_window_performance_snapshot_record(WindowId::new(11)));

        let mut visitor = CountingVisitor { count: 0 };
        Widget::visit_children(&panel, &mut visitor);
        assert_eq!(visitor.count, 0);
    }

    #[test]
    fn live_performance_panel_measures_to_compact_width() {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Overlay")
                    .root(LivePerformancePanel::new()),
            )
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).expect("panel should render");
        let graph = runtime
            .widget_graph(window_id)
            .expect("widget graph should exist");
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .expect("panel root node present");

        assert!(root.bounds.width() <= LivePerformancePanel::WIDTH);
        assert!(root.bounds.height() > 0.0);
    }

    #[test]
    fn live_performance_panel_reports_zero_fps_when_idle() {
        let snapshot = sample_window_performance_snapshot_record(WindowId::new(11));
        let display = Rc::new(RefCell::new(LivePerformanceDisplay {
            snapshot: Some(snapshot.clone()),
            idle: true,
            samples: vec![LivePerformanceFrameSample::from_snapshot(&snapshot)],
        }));
        let panel = LivePerformancePanel::with_display(display);
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Overlay").root(panel))
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];

        runtime.render(window_id).expect("panel should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("semantics snapshot should exist");
        let overlay = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            })
            .expect("overlay semantics node present");

        assert_eq!(
            overlay.value,
            Some(SemanticsValue::Text(
                "0 fps | 1.5 ms | 1 samples".to_string()
            ))
        );
    }

    #[test]
    fn widget_book_root_requests_paint_when_a_published_snapshot_arrives() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];

        runtime
            .render(window_id)
            .expect("initial render should succeed");
        assert!(
            !runtime
                .needs_render(window_id)
                .expect("window should be idle after initial render")
        );

        publish_window_performance_snapshot(sample_window_performance_snapshot_record(window_id));
        runtime
            .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
            .expect("redraw event should be handled");

        assert!(runtime.needs_render(window_id).expect(
            "widget-book root should request a paint when the published performance snapshot changes"
        ));
    }

    #[test]
    fn widget_book_startup_bootstraps_live_performance_overlay() -> Result<()> {
        let placeholder_image = {
            let placeholder = build_overlay_placeholder_app()?;
            let placeholder_window = placeholder.main_window()?;
            placeholder_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name("Live performance overlay")
                .capture_screenshot()?
        };

        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");

        let live_image = overlay.capture_screenshot()?;
        let performance = window.performance_snapshot()?;

        assert_ne!(live_image, placeholder_image);
        assert!(performance.frame_index >= 2);

        Ok(())
    }

    #[test]
    fn widget_book_overlay_enables_detail_mode_while_visible() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");
        let before = overlay.capture_screenshot()?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;
        let after = overlay.capture_screenshot()?;
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "visible overlay should enable detailed scene statistics mode"
        );
        assert!(
            before != after,
            "overlay screenshot did not change after publishing detailed diagnostics"
        );

        Ok(())
    }

    #[test]
    fn widget_book_scroll_updates_performance_overlay_without_extra_frame() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");
        let before = overlay.capture_screenshot()?;
        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
        let after = overlay.capture_screenshot()?;
        assert_ne!(after, before);

        Ok(())
    }

    #[test]
    fn widget_book_scroll_updates_performance_overlay_visuals() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");

        let before = overlay.capture_screenshot()?;
        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
        let after = overlay.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "diagnostic benchmark for current headless widget-book scroll status"]
    fn widget_book_headless_scroll_current_status_benchmark() -> Result<()> {
        let _guard = headless_benchmark_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let app = build_headless_default_widget_book_app()?;
        let window = app.main_window()?;
        set_detailed_scene_statistics_mode(&window)?;
        let samples = collect_headless_scroll_benchmark_samples(&window, GALLERY_SCROLL_NAME, 24)?;

        print_widget_book_headless_scroll_benchmark_summary(
            "Widget Book Headless Scroll Benchmark",
            &samples,
        );
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "diagnostic benchmark for current headless overlay-free widget-book gallery status"]
    fn widget_book_headless_gallery_only_scroll_current_status_benchmark() -> Result<()> {
        let _guard = headless_benchmark_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let app = build_gallery_only_widget_book_app()?;
        let window = app.main_window()?;
        set_detailed_scene_statistics_mode(&window)?;
        let samples = collect_headless_scroll_benchmark_samples(&window, GALLERY_SCROLL_NAME, 24)?;

        print_widget_book_headless_scroll_benchmark_summary(
            "Widget Book Headless Gallery-Only Scroll Benchmark",
            &samples,
        );
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "diagnostic benchmark for current headless retained text scroll status"]
    fn retained_text_headless_scroll_current_status_benchmark() -> Result<()> {
        let _guard = headless_benchmark_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let app = TestApp::from_runtime(build_retained_text_benchmark_application().build()?)?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;
        assert_eq!(snapshot.title, RETAINED_TEXT_BENCHMARK_TITLE);
        set_detailed_scene_statistics_mode(&window)?;
        let samples = collect_headless_scroll_benchmark_samples(
            &window,
            RETAINED_TEXT_BENCHMARK_SCROLL_NAME,
            24,
        )?;

        print_widget_book_headless_scroll_benchmark_summary(
            "Retained Text Headless Scroll Benchmark",
            &samples,
        );
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "diagnostic benchmark for current headless text editing status"]
    fn text_editing_headless_current_status_benchmark() -> Result<()> {
        let _guard = headless_benchmark_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let app = TestApp::from_runtime(build_text_editing_benchmark_application().build()?)?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;
        assert_eq!(snapshot.title, TEXT_EDITING_BENCHMARK_TITLE);
        set_detailed_scene_statistics_mode(&window)?;
        let samples = collect_headless_text_editing_benchmark_samples(&window)?;

        print_widget_book_headless_scroll_benchmark_summary(
            "Text Editing Headless Benchmark",
            &samples,
        );
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "diagnostic benchmark for current headless animation status"]
    fn animation_headless_current_status_benchmark() -> Result<()> {
        let _guard = headless_benchmark_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let app = TestApp::from_runtime(build_animation_benchmark_application().build()?)?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;
        assert_eq!(snapshot.title, ANIMATION_BENCHMARK_TITLE);
        set_detailed_scene_statistics_mode(&window)?;
        let samples = collect_headless_animation_benchmark_samples(&window)?;

        print_widget_book_headless_scroll_benchmark_summary(
            "Animation Headless Benchmark",
            &samples,
        );
        Ok(())
    }

    #[test]
    fn widget_book_exposes_compact_performance_overlay_semantics() {
        let mut runtime = build_widget_book_application_with_overlay(default_widget_book_state())
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("semantics snapshot should exist");

        let overlay = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            })
            .expect("overlay semantics node present");

        let expected_left_edge =
            1280.0 - super::LivePerformanceRoot::OVERLAY_MARGIN.right - LivePerformancePanel::WIDTH;
        assert!(overlay.bounds.width() <= LivePerformancePanel::WIDTH);
        assert!(overlay.bounds.x() >= expected_left_edge);
        assert!(
            overlay.bounds.max_x()
                <= 1280.0 - super::LivePerformanceRoot::OVERLAY_MARGIN.right + 1.0
        );
        assert!(overlay.bounds.y() <= 24.0);
    }

    fn sample_window_performance_snapshot_record(window_id: WindowId) -> WindowPerformanceSnapshot {
        WindowPerformanceSnapshot::new(
            window_id,
            7,
            vec![FramePhaseSample::new(FramePhase::Renderer, 1.5)],
            RendererSubmissionDiagnostics::new(
                2,
                6,
                2048,
                24,
                1536,
                3,
                6,
                420,
                160,
                210,
                120,
                3,
                sui_runtime::RetainedPacketRebuildDiagnostics::new(1, 0, 1, 1, 0),
                4,
                90,
                440,
                210,
                130,
                15,
                95,
                4,
                32768,
                115,
                85,
                22,
                16384,
                920,
                640,
                180,
                70,
                560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: Default::default(),
                viewport: Size::new(1280.0, 720.0),
                total_widget_count: 4,
                active_animated_widget_count: 0,
                animation_frame_wake_count: 0,
                animation_repaint_frame_count: 0,
                animation_transform_effect_only_frame_count: 0,
                dirty_region_count: 0,
                dirty_regions: Vec::new(),
                dirty_area: 0.0,
                dirty_coverage: 0.0,
                command_count: 0,
                command_breakdown: Vec::new(),
                repaint_boundary_count: 0,
                scene_layer_count: 0,
                stack_surface_count: 0,
                overlay_layer_count: 0,
                layer_update_count: 0,
                layer_update_breakdown: Vec::new(),
                text_command_count: 0,
                image_command_count: 0,
                clip_command_count: 0,
                transform_command_count: 0,
            },
        )
        .with_presentation_latency(PresentationLatencyDiagnostics::new(1.1, 4.8, 3.2))
    }

    fn sample_detailed_window_performance_snapshot_record(
        window_id: WindowId,
    ) -> WindowPerformanceSnapshot {
        WindowPerformanceSnapshot::new(
            window_id,
            8,
            vec![
                FramePhaseSample::new(FramePhase::Paint, 0.8),
                FramePhaseSample::new(FramePhase::Renderer, 1.9),
            ],
            RendererSubmissionDiagnostics::new(
                2,
                6,
                2048,
                24,
                1536,
                3,
                6,
                420,
                160,
                210,
                120,
                3,
                sui_runtime::RetainedPacketRebuildDiagnostics::new(1, 0, 1, 1, 0),
                4,
                90,
                440,
                210,
                130,
                15,
                95,
                4,
                32768,
                115,
                85,
                22,
                16384,
                920,
                640,
                180,
                70,
                560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: SceneStatisticsDetailMode::Detailed,
                viewport: Size::new(1280.0, 720.0),
                total_widget_count: 9,
                active_animated_widget_count: 3,
                animation_frame_wake_count: 2,
                animation_repaint_frame_count: 1,
                animation_transform_effect_only_frame_count: 1,
                dirty_region_count: 2,
                dirty_regions: Vec::new(),
                dirty_area: 128.0,
                dirty_coverage: 3.0,
                command_count: 14,
                command_breakdown: vec![("FillRect".to_string(), 8), ("Layer".to_string(), 6)],
                repaint_boundary_count: 6,
                scene_layer_count: 6,
                stack_surface_count: 2,
                overlay_layer_count: 1,
                layer_update_count: 4,
                layer_update_breakdown: vec![("Repaint".to_string(), 4)],
                text_command_count: 3,
                image_command_count: 1,
                clip_command_count: 2,
                transform_command_count: 1,
            },
        )
        .with_presentation_latency(PresentationLatencyDiagnostics::new(2.4, 7.1, 4.5))
    }
}
