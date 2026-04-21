#![forbid(unsafe_code)]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{OnceLock, RwLock},
};

use sui::prelude::*;
use sui::{
    HdrLuminanceTokens, HdrThemeMode, HdrThemeTokens, InvalidationKind, InvalidationRequest,
    InvalidationTarget, Rect, SceneStatisticsDetailMode, SemanticColorToken, SemanticsNode,
    SemanticsRole, SemanticsValue, TextDirection, TextStyle, TextSurface, TextWrap, TimerToken,
    Vector, WidgetColorRole, WidgetLuminanceRole, WidgetMaterialRole, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowEvent, WindowId, WindowPerformanceSnapshot, resolve_semantic_color,
    resolve_widget_hdr_style, set_window_scene_statistics_detail_mode, window_performance_snapshot,
    window_scene_statistics_detail_mode,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::LayerCompositionMode;

#[cfg(feature = "artifacts")]
mod visual_artifacts;

#[cfg(feature = "artifacts")]
pub use visual_artifacts::write_visual_artifacts;

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
pub const BUTTON_GRID_BENCHMARK_TITLE: &str = "SUI 64 Button Grid Benchmark";
pub const RETAINED_TEXT_BENCHMARK_TITLE: &str = "SUI Retained Text Scroll Benchmark";
pub const TEXT_RENDERING_COMPARISON_TITLE: &str = "SUI Text Rendering Comparison";
pub const COLOR_VALIDATION_VIEW_TITLE: &str = "SUI HDR and Color Validation";
pub const TEXT_VALIDATION_VIEW_TITLE: &str = "SUI Text Validation";
pub const TEXT_EDITING_BENCHMARK_TITLE: &str = "SUI Text Editing Benchmark";
pub const BUTTON_GRID_ROWS: usize = 8;
pub const BUTTON_GRID_COLUMNS: usize = 8;
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
pub const DIALOG_TITLE: &str = "Project settings";
pub const DIALOG_TRIGGER_LABEL: &str = "Toggle project settings";
pub const PROGRESS_NAME: &str = "Export progress";
pub const SPINNER_NAME: &str = "Background work";
pub const SUMMARY_NAME: &str = "Widget book summary";
pub const GALLERY_SCROLL_NAME: &str = "Widget book gallery";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_NAME: &str = "Retained text benchmark scroll";
pub const TEXT_RENDERING_COMPARISON_SCROLL_NAME: &str = "Text rendering comparison scroll";
pub const COLOR_VALIDATION_SCROLL_NAME: &str = "Color validation scroll";
pub const TEXT_VALIDATION_SCROLL_NAME: &str = "Text validation scroll";
pub const TEXT_VALIDATION_EDITOR_NAME: &str = "Validation editor";
pub const TEXT_EDITING_BENCHMARK_EDITOR_NAME: &str = "Text editing benchmark editor";
pub const TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME: &str = "Text editing benchmark syntax preview";
pub const THEME_PREVIEW_NAME: &str = "Theme preview showcase";
pub const THEME_PREVIEW_TOGGLE_LABEL: &str = "Compare light and dark themes";
pub const LIGHT_THEME_PREVIEW_CARD_NAME: &str = "Light theme preview card";
pub const DARK_THEME_PREVIEW_CARD_NAME: &str = "Dark theme preview card";
pub const HDR_THEME_LAB_NAME: &str = "HDR theme mode lab";
pub const HDR_THEME_LAB_ACTIVE_PREVIEW_NAME: &str = "Current HDR theme mode preview";
pub const LIGHT_PREVIEW_ACTION_LABEL: &str = "Light preview action";
pub const DARK_PREVIEW_ACTION_LABEL: &str = "Dark preview action";
pub const LIGHT_PREVIEW_INPUT_LABEL: &str = "Light preview query";
pub const DARK_PREVIEW_INPUT_LABEL: &str = "Dark preview query";
pub const LIST_VIEW_NAME: &str = "Assets list";
pub const TREE_VIEW_NAME: &str = "Scene tree";
pub const TABLE_NAME: &str = "Material table";
pub const SPLIT_VIEW_NAME: &str = "Editor split";
pub const BREADCRUMB_NAME: &str = "Project path";
pub const COLOR_SWATCH_NAME: &str = "Primary swatch";
pub const COLOR_PICKER_NAME: &str = "Accent picker";
pub const DEMO_IMAGE_LABEL: &str = "Preview image";

const WIDGET_BOOK_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);

const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
const BLEND_MODE_OPTIONS: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];
const TAB_BAR_OPTIONS: [&str; 3] = ["Canvas", "Inspector", "Export"];
const TAB_PANEL_OPTIONS: [&str; 3] = ["Layout", "Data", "History"];

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
        }
    }

    pub fn watch_widget_book_state(mut self, state: Rc<RefCell<WidgetBookState>>) -> Self {
        self.last_seen_state = Some(state.borrow().clone());
        self.watched_state = Some(state);
        self
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSnapshot>,
        idle: bool,
    ) -> bool {
        let next = LivePerformanceDisplay { snapshot, idle };
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
/// application.  Call this before adding a window that contains the gallery
/// when you are assembling the application yourself (rather than using
/// [`build_widget_book_application`]).
pub fn register_widget_book_images(application: &mut Application) {
    application
        .register_image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");
}

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);

    let mut application = Application::new();
    register_widget_book_images(&mut application);

    application.window(
        WindowBuilder::new().title(WINDOW_TITLE).root(
            LivePerformanceRoot::new(
                WINDOW_TITLE,
                WINDOW_DESCRIPTION,
                build_widget_book_gallery(Rc::clone(&state)),
            )
            .watch_widget_book_state(state),
        ),
    )
}

#[cfg(feature = "native")]
pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for LivePerformanceRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(1280.0, 720.0));
        self.content.measure(ctx, Constraints::tight(viewport));
        self.performance_overlay
            .measure(ctx, Constraints::new(Size::ZERO, viewport));
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, Rect::from_origin_size(bounds.origin, bounds.size));

        let overlay_size = self.performance_overlay.child().measured_size();
        let overlay_x = (bounds.max_x() - overlay_size.width - Self::OVERLAY_MARGIN.right)
            .max(bounds.x() + Self::OVERLAY_MARGIN.left);
        let overlay_y = bounds.y() + Self::OVERLAY_MARGIN.top;
        self.performance_overlay.arrange(
            ctx,
            Rect::from_origin_size(Point::new(overlay_x, overlay_y), overlay_size),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.95, 0.968, 0.985, 1.0));
        self.content.paint(ctx);
        self.performance_overlay.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(self.window_title.clone());
        root.description = Some(self.window_description.clone());
        ctx.push(root);
        self.content.semantics(ctx);
        self.performance_overlay.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.performance_overlay.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.performance_overlay.visit_children_mut(visitor);
    }
}

struct ProjectSettingsPreview {
    trigger: SingleChild,
    dialog: SingleChild,
    dialog_open: bool,
    trigger_pressed: bool,
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
        }
    }

    fn card_height() -> f32 {
        272.0
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
            920.0
        };
        let toggle_size = self.toggle.measure(ctx, constraints.loosen());
        let top = toggle_size.height + 16.0;
        let gap = 16.0;
        let card_height = Self::card_height();

        if comparison_enabled {
            let stacked = max_width < 760.0;
            if stacked {
                let light_size = self
                    .light_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));
                let dark_size = self
                    .dark_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));

                return constraints.clamp(Size::new(
                    max_width,
                    top + light_size.height + gap + dark_size.height,
                ));
            }

            let card_width = ((max_width - gap) / 2.0).max(280.0);
            let light_size = self
                .light_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));
            let dark_size = self
                .dark_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));

            return constraints.clamp(Size::new(
                light_size.width + gap + dark_size.width,
                top + light_size.height.max(dark_size.height),
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
            if bounds.width() < 760.0 {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
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
            } else {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
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
            "Light and dark preview cards are visible side by side.".to_string()
        } else {
            "Only the light preview card is visible.".to_string()
        });
        ctx.push(node);
        self.toggle.semantics(ctx);
        self.light_card.semantics(ctx);
        if comparison_enabled {
            self.dark_card.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children(visitor);
        self.light_card.visit_children(visitor);
        if comparison_enabled {
            self.dark_card.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children_mut(visitor);
        self.light_card.visit_children_mut(visitor);
        if comparison_enabled {
            self.dark_card.visit_children_mut(visitor);
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
        Background::new(
            theme.palette.border.with_alpha(0.92),
            Padding::all(
                1.0,
                Background::new(
                    theme.palette.surface,
                    Padding::all(
                        16.0,
                        Stack::vertical()
                            .spacing(12.0)
                            .alignment(Alignment::Stretch)
                            .with_child(
                                Label::new(hdr_theme_mode_title(mode))
                                    .font_size(18.0)
                                    .line_height(22.0)
                                    .color(theme.palette.text),
                            )
                            .with_child(
                                Label::new(lead_text)
                                    .font_size(13.0)
                                    .line_height(18.0)
                                    .color(theme.palette.placeholder),
                            )
                            .with_child(
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
                            )
                            .with_child(
                                Stack::horizontal()
                                    .spacing(12.0)
                                    .alignment(Alignment::Center)
                                    .with_child(
                                        SizedBox::new().width(186.0).with_child(
                                            Button::new(button_label).min_width(176.0).theme(theme),
                                        ),
                                    )
                                    .with_child(
                                        ColorSwatch::new(swatch_name, indicator_color)
                                            .size(Size::new(64.0, 28.0)),
                                    )
                                    .with_child(
                                        Label::new("The swatch mirrors the accent token resolved for the current gamut/HDR mode.")
                                            .font_size(12.0)
                                            .line_height(17.0)
                                            .color(theme.palette.placeholder),
                                    ),
                            )
                            .with_child(
                                Switch::new(switch_label)
                                    .on(!matches!(mode, HdrThemeMode::Disabled))
                                    .theme(theme),
                            )
                            .with_child(
                                SizedBox::new().width(260.0).with_child(
                                    Popover::new(
                                        popover_name,
                                        Button::new(popover_trigger_label)
                                            .min_width(220.0)
                                            .theme(theme),
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Small popup surfaces are where constrained vs full HDR arrival cues become easiest to validate.")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color(theme.palette.text),
                                            )
                                            .with_child(
                                                Label::new("Use this trigger to compare popup chrome, border lift, and arrival emphasis against the matching button and switch.")
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

pub fn build_widget_book_gallery(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
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

    VirtualScrollView::new()
        .name(GALLERY_SCROLL_NAME)
        .padding(Insets::all(24.0))
        .spacing(18.0)
        .with_child(
                Stack::vertical()
                    .spacing(6.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new(WINDOW_TITLE)
                            .font_size(30.0)
                            .line_height(34.0)
                            .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                    )
                    .with_child(
                        Label::new(
                            "A dedicated widget book for exercising built-in controls, generating inspection artifacts, and providing stable screenshot stories.",
                        )
                        .font_size(15.0)
                        .line_height(20.0)
                        .color(Color::rgba(0.40, 0.48, 0.58, 1.0)),
                    ),
            )
            .with_child(panel(
                "Theme preview",
                "Flip the compare toggle to inspect the simplified light and dark daisy-style themes with the same control composition.",
                ThemePreviewShowcase::new(Rc::clone(&state)),
            ))
            .with_child(panel(
                "HDR theme lab",
                "Compare the same tokenized theme across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The first card follows the shared mode currently selected by the dev host.",
                HdrThemeLabShowcase::new(),
            ))
            .with_child(panel(
                "Common controls",
                "These defaults should feel contemporary and light, while still staying dense enough for inspectors, toolbars, and side panels.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            TextInput::new(NAME_INPUT_LABEL)
                                .value(initial_name)
                                .placeholder("Type your name")
                                .on_change(move |value| {
                                    name_state.borrow_mut().name = value;
                                }),
                        ),
                    )
                    .with_child(
                        Checkbox::new(SUBSCRIBE_LABEL)
                            .checked(initial_subscribed)
                            .on_toggle(move |checked| {
                                subscribed_state.borrow_mut().subscribed = checked;
                            }),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(12.0)
                            .alignment(Alignment::Center)
                            .with_child(
                                SizedBox::new().width(180.0).with_child(
                                    Button::new(PRIMARY_BUTTON_LABEL).on_press(move || {
                                        action_state.borrow_mut().button_presses += 1;
                                    }),
                                ),
                            )
                            .with_child(
                                Label::new(
                                    "Primary actions, boolean toggles, and text fields should feel related by default instead of looking like separate experiments.",
                                )
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                            ),
                    )
                    .with_child(
                        Label::new(
                            "The widget book tests capture these controls directly so visual regressions can be reviewed manually or compared automatically.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Toolbar pieces",
                "Compact controls, separators, and icons need to feel intentional before any themed application shell exists.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Stack::horizontal()
                            .spacing(14.0)
                            .alignment(Alignment::Center)
                            .with_child(Icon::new(IconGlyph::Search).label(ICON_LABEL).size(24.0))
                            .with_child(
                                IconButton::new(IconGlyph::MoreHorizontal, ICON_BUTTON_LABEL)
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
                                .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                            ),
                    )
                    .with_child(SizedBox::new().width(260.0).with_child(
                        Separator::horizontal()
                            .name(TOOLBAR_SEPARATOR_NAME)
                            .inset(12.0),
                    )),
            ))
            .with_child(panel(
                "Choices and ranges",
                "Desktop-style inspectors rely on switches, radio groups, sliders, numeric inputs, and selects more than oversized form controls.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Switch::new(SWITCH_LABEL)
                            .on(initial_switch_on)
                            .on_toggle(move |checked| {
                                switch_state.borrow_mut().switch_on = checked;
                            }),
                    )
                    .with_child(
                        RadioButton::new(RADIO_BUTTON_LABEL)
                            .selected(initial_standalone_radio)
                            .on_select(move || {
                                radio_button_state.borrow_mut().standalone_radio_selected = true;
                            }),
                    )
                    .with_child(
                        SizedBox::new().width(280.0).with_child(
                            RadioGroup::new(RADIO_GROUP_NAME)
                                .options(RADIO_OPTIONS)
                                .selected(option_index(&RADIO_OPTIONS, &initial_radio_choice).unwrap_or(0))
                                .on_change(move |_, value| {
                                    radio_group_state.borrow_mut().radio_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            Slider::new(SLIDER_NAME)
                                .range(0.0, 100.0)
                                .step(1.0)
                                .value(initial_slider_value)
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
                                .selected(option_index(&BLEND_MODE_OPTIONS, &initial_mode).unwrap_or(0))
                                .on_change(move |_, value| {
                                    select_state.borrow_mut().mode = value;
                                }),
                        ),
                    ),
            ))
            .with_child(panel(
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
                        .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Typography",
                "Static text is now a real widget too, so the dev host no longer needs to hand-paint every heading and caption.",
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("Section heading")
                            .font_size(22.0)
                            .line_height(26.0)
                            .color(Color::rgba(0.13, 0.17, 0.23, 1.0)),
                    )
                    .with_child(
                        Label::new("Body copy can use the same widget with different size and color settings.")
                            .font_size(15.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.38, 0.46, 0.56, 1.0)),
                    )
                    .with_child(
                        Label::new("Secondary note")
                            .font_size(13.0)
                            .line_height(18.0)
                            .color(Color::rgba(0.50, 0.57, 0.66, 1.0)),
                    ),
            ))
            .with_child(panel(
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
                                .on_change(move |_, value| {
                                    tab_bar_state.borrow_mut().tab_bar_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(540.0).height(220.0).with_child(
                            Tabs::new(TABS_NAME)
                                .selected(option_index(&TAB_PANEL_OPTIONS, &initial_tabs_choice).unwrap_or(0))
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
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                ProgressBar::new("Layout completion")
                                                    .range(0.0, 100.0)
                                                    .value(initial_slider_value)
                                                    .show_value(true),
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
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Selection: 4 layers, 2 masks, 1 smart object")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color(Color::rgba(0.46, 0.54, 0.63, 1.0)),
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
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Replaying history cache")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                                            ),
                                    ),
                                )
                                .on_change(move |_, value| {
                                    tabs_state.borrow_mut().tabs_choice = value;
                                }),
                        ),
                    ),
            ))
            .with_child(panel(
                "Menus and overlays",
                "App menus, context menus, popovers, tooltips, and dialogs are the small but high-value surfaces that make desktop workflows feel complete.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(300.0).with_child(
                            Menu::new(MENU_NAME)
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
                                    Color::rgba(0.96, 0.975, 0.995, 1.0),
                                    Padding::all(
                                        14.0,
                                        Label::new("Right-click this layer tile")
                                            .font_size(14.0)
                                            .line_height(18.0)
                                            .color(Color::rgba(0.16, 0.21, 0.29, 1.0)),
                                    ),
                                ),
                            )
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
                                Button::new(TOOLTIP_TRIGGER_LABEL).min_width(180.0),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(360.0).with_child(
                            Popover::new(
                                POPOVER_NAME,
                                Button::new(POPOVER_TRIGGER_LABEL).min_width(190.0),
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        Label::new("Inline inspector content can stay lightweight instead of forcing a full modal.")
                                            .font_size(14.0)
                                            .line_height(19.0)
                                            .color(Color::rgba(0.34, 0.42, 0.52, 1.0)),
                                    )
                                    .with_child(
                                        Label::new("Blend preview: Screen @ 72%")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color(Color::rgba(0.46, 0.54, 0.63, 1.0)),
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
            .with_child(panel(
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
                                .show_value(true),
                        ),
                    )
                    .with_child(
                        Label::new(SPINNER_NAME)
                            .font_size(13.0)
                            .line_height(18.0)
                            .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Live state",
                "This summary reads state produced by reusable controls so screenshot stories can cover both isolated widgets and composed UI.",
                WidgetBookSummary::new(state),
            ))
            .with_child(panel(
                "Live performance overlay",
                "The stats card now floats over the gallery so frame timing stays visible while you inspect any part of the widget book.",
                Label::new(
                    "Use the compact panel pinned in the top-right corner while you scroll and interact with the rest of the gallery.",
                )
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
            ))
            .with_child(panel(
                "Debugging and inspection",
                "The sui-debug crate composes reusable diagnostics chrome with SUI-specific views over focus, semantics, widget graph, and scene summaries.",
                Label::new(
                    "Debug inspector available via sui-debug crate. Open the standalone debug view for full semantics, widget graph, and scene inspection."
                )
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
            ))
            .with_child(panel(
                "Collections and hierarchy",
                "Foundational editor widgets need to cover lists, trees, and structured tables without requiring app-specific shells first.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(360.0).height(220.0).with_child(
                            ListView::new(LIST_VIEW_NAME)
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
            .with_child(panel(
                "Layout and pathing",
                "Editor shells need split panes and breadcrumb-style navigation before the rest of the UI can settle into place.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(620.0).with_child(
                            Breadcrumb::new(BREADCRUMB_NAME)
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
                                    Color::rgba(0.97, 0.981, 0.992, 1.0),
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Viewport")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color(Color::rgba(0.12, 0.16, 0.22, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Resizable panes let editor shells settle into familiar two-up and inspector layouts.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                                            ),
                                    ),
                                ),
                                Background::new(
                                    Color::rgba(0.985, 0.99, 1.0, 1.0),
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Inspector")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color(Color::rgba(0.12, 0.16, 0.22, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Drag the divider to rebalance the viewport and detail pane without custom shell code.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                                            ),
                                    ),
                                ),
                            )
                            .name(SPLIT_VIEW_NAME)
                            .ratio(0.62),
                        ),
                    ),
            ))
            .with_child(build_color_and_imagery_story())
}

fn build_color_and_imagery_story() -> impl Widget {
    panel(
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
                        .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                    ),
            )
            .with_child(
                Stack::horizontal()
                    .spacing(16.0)
                    .alignment(Alignment::Start)
                    .with_child(SizedBox::new().width(360.0).height(420.0).with_child(
                        ColorPicker::from_color(
                            COLOR_PICKER_NAME,
                            Color::new(sui::ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
                        ),
                    ))
                    .with_child(
                        SizedBox::new().width(220.0).height(220.0).with_child(
                            Image::new(WIDGET_BOOK_IMAGE_HANDLE)
                                .label(DEMO_IMAGE_LABEL)
                                .fit(ImageFit::Contain)
                                .background(Color::rgba(0.965, 0.975, 0.99, 1.0))
                                .corner_radius(12.0),
                        ),
                    ),
            ),
    )
}

pub fn build_button_grid_benchmark() -> impl Widget {
    let mut grid = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch);

    for row in 0..BUTTON_GRID_ROWS {
        let mut line = Stack::horizontal()
            .spacing(12.0)
            .alignment(Alignment::Start);
        for column in 0..BUTTON_GRID_COLUMNS {
            line = line.with_child(
                Button::new(format!("Button {row}:{column}"))
                    .min_width(112.0)
                    .min_height(24.0),
            );
        }
        grid = grid.with_child(line);
    }

    grid
}

pub fn build_button_grid_benchmark_application() -> Application {
    Application::new().window(
        WindowBuilder::new()
            .title(BUTTON_GRID_BENCHMARK_TITLE)
            .root(LivePerformanceRoot::new(
                BUTTON_GRID_BENCHMARK_TITLE,
                "Focused benchmark surface for measuring the initial frame cost of a 64-button grid.",
                build_button_grid_benchmark(),
            )),
    )
}

pub fn build_retained_text_benchmark() -> impl Widget {
    const SECTION_COUNT: usize = 72;
    const PARAGRAPHS_PER_SECTION: usize = 4;

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
                        "The outer scroll view stays cached, the visible content stays dominated by wrapped labels, and the benchmark scrolls through enough sections to keep retained tiles regenerating with mostly atlas text payloads.",
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

    ScrollView::vertical(Padding::all(
        24.0,
        SizedBox::new().width(948.0).with_child(content),
    ))
    .name(RETAINED_TEXT_BENCHMARK_SCROLL_NAME)
}

pub fn build_retained_text_benchmark_application() -> Application {
    Application::new().window(
        WindowBuilder::new()
            .title(RETAINED_TEXT_BENCHMARK_TITLE)
            .root(build_retained_text_benchmark()),
    )
}

pub fn build_text_rendering_comparison_surface() -> impl Widget {
    let mut mode_cards = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch);

    for (title, subtitle, notes) in [
        (
            "Grayscale baseline",
            "Baseline grayscale coverage for dark-on-light and light-on-dark UI text.",
            "Use this as the control sample for repeated stems like ill, scroll, minimum, Hello, Ж, and 中.",
        ),
        (
            "Grayscale + hinting",
            "Small-text hinting below the configured threshold.",
            "Compare 10–14 px labels, mixed-script captions, and medium UI text against the baseline.",
        ),
        (
            "Grayscale + stem darkening",
            "Conservative stroke-weight boost for thin dark-on-light text.",
            "Look for stronger stems without muddying medium-size text or emoji fallback.",
        ),
        (
            "LCD subpixel",
            "Subpixel coverage path for axis-aligned pixel-snapped text.",
            "Check repeated stems, color-fringe-aware edge detail, and automatic grayscale fallback expectations.",
        ),
        (
            "LCD subpixel + hinting",
            "Subpixel path plus small-text hinting.",
            "Focus on tiny Latin labels, mixed-script tool captions, and editor-like status lines.",
        ),
        (
            "LCD subpixel + hinting + stem darkening",
            "Most aggressive small-text experiment in the current plan.",
            "Validate tiny UI text, dark/light contrast pairs, and make sure medium text still reads cleanly.",
        ),
    ] {
        mode_cards = mode_cards.with_child(build_text_rendering_mode_card(title, subtitle, notes));
    }

    ScrollView::vertical(Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(panel(
                "Text rendering mode matrix",
                "Compare the same representative text samples across grayscale, hinted, darkened, and LCD-oriented rendering modes. The current surface is intended as a visual checklist for repeated stems, mixed scripts, and contrast-sensitive UI labels.",
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(980.0).with_child(
                            Label::new("Samples include dark text on light background, light text on dark background, small label text, medium UI copy, mixed-script runs, and repeated stems such as ill, scroll, minimum, Hello, Ж, and 中.")
                                .font_size(14.0)
                                .line_height(20.0)
                                .color(Color::rgba(0.38, 0.46, 0.56, 1.0)),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(980.0).with_child(
                            Label::new("Use the dev workspace renderer settings to switch the active mode, then compare how each reference card should look when the chosen policy is active. This keeps one stable validation surface for native and wasm runs.")
                                .font_size(14.0)
                                .line_height(20.0)
                                .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                        ),
                    ),
            ))
            .with_child(mode_cards),
    ))
    .name(TEXT_RENDERING_COMPARISON_SCROLL_NAME)
}

pub fn build_text_rendering_comparison_application() -> Application {
    Application::new().window(
        WindowBuilder::new().title(TEXT_RENDERING_COMPARISON_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_RENDERING_COMPARISON_TITLE,
                "Side-by-side validation surface for grayscale, hinted, darkened, and LCD-oriented text rendering modes.",
                build_text_rendering_comparison_surface(),
            ),
        ),
    )
}

pub fn build_color_validation_surface() -> impl Widget {
    ScrollView::vertical(Padding::all(
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
                    ))
                    .with_child(build_color_validation_row(
                        "SDR clipping reference",
                        "This pair makes SDR clipping easy to spot. If the boosted sample looks no brighter than the baseline, the path is still constrained to SDR output at this stage.",
                        [
                            ("SDR white baseline", Color::linear_rgba(1.0, 1.0, 1.0, 1.0)),
                            ("SDR clipped white 2.0", Color::linear_rgba(2.0, 2.0, 2.0, 1.0)),
                        ],
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
                    ))
                    .with_child(build_color_validation_row(
                        "Green primary",
                        "The Display-P3 green sample intentionally lives outside the sRGB gamut. Compare it against the clipped sRGB control when checking wide-gamut correctness.",
                        [
                            ("sRGB clipped lime", Color::rgba(0.0, 1.0, 0.0, 1.0)),
                            ("Display P3 vivid lime", Color::display_p3(0.0, 1.0, 0.0, 1.0)),
                        ],
                    ))
                    .with_child(build_color_validation_row(
                        "Cyan accent mix",
                        "A mixed-color sample helps catch cases where Display-P3 is incorrectly reduced to transfer decoding only. The P3 version should retain a more vivid cyan accent on wide-gamut outputs.",
                        [
                            ("sRGB accent cyan", Color::rgba(0.0, 0.78, 1.0, 1.0)),
                            ("Display P3 accent cyan", Color::display_p3(0.0, 0.78, 1.0, 1.0)),
                        ],
                    )),
            )),
    ))
    .name(COLOR_VALIDATION_SCROLL_NAME)
}

pub fn build_color_validation_application() -> Application {
    Application::new().window(
        WindowBuilder::new().title(COLOR_VALIDATION_VIEW_TITLE).root(
            LivePerformanceRoot::new(
                COLOR_VALIDATION_VIEW_TITLE,
                "Reference surface for validating wide-gamut color handling, HDR brightness separation, and SDR clipping behavior while native HDR support lands in phases.",
                build_color_validation_surface(),
            ),
        ),
    )
}

fn build_color_validation_row(
    title: &'static str,
    description: &'static str,
    swatches: [(&'static str, Color); 2],
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
                            .with_child(build_color_validation_swatch(swatches[0].0, swatches[0].1))
                            .with_child(build_color_validation_swatch(
                                swatches[1].0,
                                swatches[1].1,
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
                            .with_child(build_color_validation_swatch(swatches[0].0, swatches[0].1))
                            .with_child(build_color_validation_swatch(swatches[1].0, swatches[1].1))
                            .with_child(build_color_validation_swatch(swatches[2].0, swatches[2].1))
                            .with_child(build_color_validation_swatch(
                                swatches[3].0,
                                swatches[3].1,
                            )),
                    ),
            ),
        ),
    )
}

fn build_color_validation_swatch(name: &'static str, color: Color) -> impl Widget {
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Center)
        .with_child(ColorSwatch::new(name, color).size(Size::new(132.0, 56.0)))
        .with_child(
            Label::new(name)
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.16, 0.21, 0.28, 1.0)),
        )
}

fn build_text_rendering_mode_card(
    title: &'static str,
    subtitle: &'static str,
    notes: &'static str,
) -> impl Widget {
    NamedSection::new(
        title,
        Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(14.0)
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
                    .with_child(
                        Stack::horizontal()
                            .spacing(16.0)
                            .alignment(Alignment::Start)
                            .with_child(Background::new(
                                Color::rgba(0.995, 0.998, 1.0, 1.0),
                                Padding::all(
                                    16.0,
                                    Stack::vertical()
                                        .spacing(8.0)
                                        .alignment(Alignment::Stretch)
                                        .with_child(
                                            Label::new("Dark on light")
                                                .font_size(13.0)
                                                .line_height(18.0)
                                                .color(Color::rgba(0.43, 0.50, 0.58, 1.0)),
                                        )
                                        .with_child(
                                            Label::new("ill scroll minimum Hello Ж 中")
                                                .font_size(12.0)
                                                .line_height(16.0)
                                                .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                                        )
                                        .with_child(
                                            Label::new(
                                                "Toolbar 12 px · glyph atlas · Привет · 中文",
                                            )
                                            .font_size(14.0)
                                            .line_height(19.0)
                                            .color(Color::rgba(0.14, 0.19, 0.26, 1.0)),
                                        ),
                                ),
                            ))
                            .with_child(Background::new(
                                Color::rgba(0.14, 0.18, 0.24, 1.0),
                                Padding::all(
                                    16.0,
                                    Stack::vertical()
                                        .spacing(8.0)
                                        .alignment(Alignment::Stretch)
                                        .with_child(
                                            Label::new("Light on dark")
                                                .font_size(13.0)
                                                .line_height(18.0)
                                                .color(Color::rgba(0.70, 0.78, 0.86, 1.0)),
                                        )
                                        .with_child(
                                            Label::new("ill scroll minimum Hello Ж 中")
                                                .font_size(12.0)
                                                .line_height(16.0)
                                                .color(Color::rgba(0.95, 0.97, 1.0, 1.0)),
                                        )
                                        .with_child(
                                            Label::new("Status · שלום · مرحبا · नमस्ते · 中文")
                                                .font_size(14.0)
                                                .line_height(19.0)
                                                .color(Color::rgba(0.90, 0.94, 1.0, 1.0)),
                                        ),
                                ),
                            )),
                    )
                    .with_child(
                        SizedBox::new().width(980.0).with_child(
                            Label::new(notes)
                                .font_size(13.0)
                                .line_height(19.0)
                                .color(Color::rgba(0.41, 0.48, 0.56, 1.0)),
                        ),
                    ),
            ),
        ),
    )
}

pub fn build_text_validation_surface() -> impl Widget {
    let content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(panel(
            "Mixed scripts and fallback",
            "Validate mixed-script shaping, emoji fallback, and bidirectional runs against one stable visual surface.",
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    SizedBox::new().width(900.0).with_child(
                        Label::new("Latin, Cyrillic, Hebrew, Arabic, Devanagari, and Han: SUI validates editor text in English, Привет, שלום, مرحبا, नमस्ते, 中文.")
                            .font_size(15.0)
                            .line_height(22.0)
                            .color(Color::rgba(0.16, 0.22, 0.30, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(900.0).with_child(
                        Label::new("Emoji and fallback coverage: status ready 🙂, warning ⚠, success ✅, palette 🎨, atlas 🔤, mixed fallback 中 and Ж in one line.")
                            .font_size(15.0)
                            .line_height(22.0)
                            .color(Color::rgba(0.18, 0.25, 0.35, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(900.0).with_child(
                        Label::new("Bidirectional sample: layout anchor -> abc אבג 123 مرحبا <- editor overlay should preserve readable ordering.")
                            .font_size(15.0)
                            .line_height(22.0)
                            .color(Color::rgba(0.22, 0.30, 0.39, 1.0)),
                    ),
                ),
        ))
        .with_child(panel(
            "Wrapping and line breaking",
            "Constrained paragraphs keep the validation surface honest about line windows, wrapping, and caret placement near soft wraps.",
            Stack::horizontal()
                .spacing(18.0)
                .alignment(Alignment::Start)
                .with_child(
                    SizedBox::new().width(300.0).with_child(
                        Label::new("A narrow validation column should wrap mixed punctuation, inline numbers like 2026, and fallback text such as 漢字 without collapsing the selection geometry into one long strip.")
                            .font_size(14.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.29, 0.35, 0.43, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(300.0).with_child(
                        Label::new("A second constrained column helps compare where the renderer slices visible lines when long technical prose, bidi fragments, and emoji comments all sit in the same viewport.")
                            .font_size(14.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.29, 0.35, 0.43, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new().width(260.0).with_child(
                        Label::new("Expected focus: stable wraps, readable fallback glyphs, and no clipping around caret or selection overlays.")
                            .font_size(13.0)
                            .line_height(19.0)
                            .color(Color::rgba(0.44, 0.51, 0.60, 1.0)),
                    ),
                ),
        ))
        .with_child(panel(
            "Interactive text surface",
            "This editor-sized surface is the manual validation target for caret, selection, scrolling, IME commit, and mixed-script editing behavior.",
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    SizedBox::new().width(900.0).with_child(
                        Label::new("Focus the surface, type with IME or keyboard input, extend selection with Shift+Arrow, and wheel-scroll to inspect visible-line extraction.")
                            .font_size(13.0)
                            .line_height(19.0)
                            .color(Color::rgba(0.43, 0.50, 0.58, 1.0)),
                    ),
                )
                .with_child(
                    SizedBox::new()
                        .width(900.0)
                        .height(260.0)
                        .with_child(
                            TextSurface::new(TEXT_VALIDATION_EDITOR_NAME)
                                .value(text_validation_editor_seed())
                                .wrap(TextWrap::Word)
                                .direction(TextDirection::Auto)
                                .min_width(900.0)
                                .min_height(260.0)
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
        SizedBox::new().width(980.0).with_child(content),
    ))
    .name(TEXT_VALIDATION_SCROLL_NAME)
}

pub fn build_text_editing_benchmark() -> impl Widget {
    let editor_panel = panel(
        "Editable code surface",
        "Benchmark typing, selection, and wheel scrolling against one long text surface with editor-like line length and comment density.",
        SizedBox::new().width(560.0).height(700.0).with_child(
            TextSurface::new(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
                .value(text_editing_benchmark_document())
                .direction(TextDirection::LeftToRight)
                .min_width(560.0)
                .min_height(700.0)
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
            .ratio(0.54)
            .min_first(420.0)
            .min_second(360.0)
            .divider_thickness(12.0),
    )
}

pub fn build_text_editing_benchmark_application() -> Application {
    Application::new().window(
        WindowBuilder::new().title(TEXT_EDITING_BENCHMARK_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_EDITING_BENCHMARK_TITLE,
                "Focused benchmark surface for editor-style typing, selection, scrolling, and syntax-highlight preview cost.",
                build_text_editing_benchmark(),
            ),
        ),
    )
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
            "Wide paragraphs keep each visible tile loaded with enough glyph instances to show byte deltas clearly.",
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
        "Visible paragraphs change a little on each wheel tick, which keeps regenerated tile strips centered on text.",
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
        "- Mixed script: English, العربية, עברית, हिन्दी, 中文, and emoji 🙂 should stay readable.",
        "- Wrapping: long diagnostics must reflow without selection gaps when the viewport narrows.",
        "- IME: composition commits should land near the caret instead of invalidating the whole surface.",
        "- Caret: moving across bidi boundaries should preserve stable layout handles and visible overlays.",
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
            "// retained tiles should not rebuild unrelated code blocks",
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
    let type_name = [
        "EditorState",
        "GlyphRun",
        "SelectionOverlay",
        "SyntaxPalette",
        "VisibleWindow",
        "BenchmarkFrame",
    ][(line_index * 7) % 6];
    let method = [
        "shape_visible_window",
        "collect_cache_delta",
        "measure_cursor_band",
        "update_highlight_rows",
        "resolve_fallback_faces",
        "commit_frame_sample",
    ][(line_index * 11) % 6];
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
            Label::new(format!("sample_{line_index:03}: "))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.15, 0.19, 0.26, 1.0)),
        )
        .with_child(
            Label::new(format!("{type_name} "))
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.09, 0.43, 0.58, 1.0)),
        )
        .with_child(
            Label::new(format!("= {method}("))
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
            Label::new(") ")
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.21, 0.27, 0.35, 1.0)),
        )
        .with_child(
            Label::new(format!(
                "// {accent} tint, abc אבג 123, glyph set 🙂{}",
                line_index % 9
            ))
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
    Background::new(
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
    )
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
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(format!("{title} theme"))
                .font_size(18.0)
                .line_height(22.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new(format!(
                "{} base surface with {} accent for primary actions.",
                theme.colors.name, theme.colors.name
            ))
            .font_size(13.0)
            .line_height(18.0)
            .color(theme.palette.placeholder),
        )
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
                .with_child(
                    Label::new(
                        "Reusable controls should stay coherent across both theme variants.",
                    )
                    .font_size(13.0)
                    .line_height(18.0)
                    .color(theme.palette.placeholder),
                ),
        )
        .with_child(
            Checkbox::new(format!("{title} preview snap to grid"))
                .checked(true)
                .theme(theme),
        )
        .with_child(
            Switch::new(format!("{title} preview live updates"))
                .on(true)
                .theme(theme),
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

    Background::new(
        theme.palette.border,
        Padding::all(
            1.0,
            Background::new(theme.palette.surface, Padding::all(18.0, body)),
        ),
    )
}

struct WidgetBookSummary {
    state: Rc<RefCell<WidgetBookState>>,
    last_seen_state: WidgetBookState,
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        let last_seen_state = state.borrow().clone();
        Self {
            state,
            last_seen_state,
        }
    }
}

struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
    toggle_pressed: bool,
    detail_refresh_timer: Option<TimerToken>,
}

#[derive(Debug, Clone)]
struct LivePerformanceLine {
    y: f32,
    text: String,
    style: TextStyle,
}

#[derive(Debug, Clone)]
struct LivePerformanceLineSpec {
    text: String,
    style: TextStyle,
    spacing_after: f32,
}

impl LivePerformancePanel {
    const WIDTH: f32 = 252.0;
    const HEIGHT: f32 = 244.0;
    const PADDING_X: f32 = 12.0;
    const PADDING_Y: f32 = 10.0;
    const CORNER_RADIUS: f32 = 12.0;
    const TOGGLE_WIDTH: f32 = 76.0;
    const TOGGLE_HEIGHT: f32 = 18.0;

    #[cfg(test)]
    fn new() -> Self {
        Self::with_display(Rc::new(RefCell::new(LivePerformanceDisplay::default())))
    }

    fn with_display(display: Rc<RefCell<LivePerformanceDisplay>>) -> Self {
        Self {
            display,
            toggle_pressed: false,
            detail_refresh_timer: None,
        }
    }

    fn toggle_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.max_x() - Self::PADDING_X - Self::TOGGLE_WIDTH,
            bounds.y() + Self::PADDING_Y - 1.0,
            Self::TOGGLE_WIDTH,
            Self::TOGGLE_HEIGHT,
        )
    }

    fn toggle_label(detail_mode: SceneStatisticsDetailMode) -> &'static str {
        if detail_mode.is_detailed() {
            "detail on"
        } else {
            "detail off"
        }
    }

    fn rebuild_lines(
        &self,
        width: f32,
        specs: &[LivePerformanceLineSpec],
    ) -> Vec<LivePerformanceLine> {
        let _text_width = (width - Self::PADDING_X * 2.0).max(1.0);
        let mut y = Self::PADDING_Y;
        let mut lines = Vec::new();

        for spec in specs {
            let line_height = spec.style.line_height.max(spec.style.font_size);
            lines.push(LivePerformanceLine {
                y,
                text: spec.text.clone(),
                style: spec.style.clone(),
            });
            y += line_height + spec.spacing_after;
        }

        lines
    }

    fn content_specs(&self, _window_id: WindowId) -> Vec<LivePerformanceLineSpec> {
        let display = self.display.borrow().clone();

        match display.snapshot {
            Some(snapshot) => {
                let headline = format!(
                    "{}  |  {}",
                    if display.idle {
                        "0 fps".to_string()
                    } else {
                        format_fps(snapshot.total_time_ms)
                    },
                    if display.idle {
                        "idle".to_string()
                    } else {
                        format_duration_ms(snapshot.total_time_ms)
                    },
                );
                if !snapshot.scene.detail_mode.is_detailed() {
                    return vec![
                        LivePerformanceLineSpec::title("live performance".to_string()),
                        LivePerformanceLineSpec::headline(headline),
                        LivePerformanceLineSpec::metric(format!(
                            "lat present {}  |  redraw {}",
                            format_optional_duration_ms(
                                snapshot.presentation_latency.event_to_present_ms,
                            ),
                            format_optional_duration_ms(
                                snapshot.presentation_latency.redraw_request_to_callback_ms,
                            ),
                        )),
                        LivePerformanceLineSpec::metric(format!(
                            "lat render {}",
                            format_optional_duration_ms(
                                snapshot.presentation_latency.event_to_render_start_ms,
                            ),
                        )),
                    ];
                }

                let slowest_phase = snapshot.slowest_phase();
                let slowest_label = slowest_phase
                    .map(|sample| sample.phase.label())
                    .unwrap_or("idle");
                let slowest_duration = slowest_phase
                    .map(|sample| format_duration_ms(sample.duration_ms))
                    .unwrap_or_else(|| "0.0 ms".to_string());
                let scene_metric = format!(
                    "widgets {}  |  repaint(now) {}  |  scene {}",
                    snapshot.scene.total_widget_count,
                    snapshot.scene.repaint_boundary_count,
                    snapshot.scene.scene_layer_count,
                );
                let boundary_metric = format!(
                    "stack {}  |  overlay {}  |  updates {}",
                    snapshot.scene.stack_surface_count,
                    snapshot.scene.overlay_layer_count,
                    snapshot.scene.layer_update_count,
                );
                let trailing_metric = format!(
                    "dirty {}  |  txt {}  |  img {}",
                    snapshot.scene.dirty_region_count,
                    snapshot.scene.text_command_count,
                    snapshot.scene.image_command_count,
                );

                vec![
                    LivePerformanceLineSpec::title("live performance".to_string()),
                    LivePerformanceLineSpec::headline(headline),
                    LivePerformanceLineSpec::metric(if display.idle {
                        format!(
                            "frame {}  |  last active {} {}",
                            snapshot.frame_index, slowest_label, slowest_duration,
                        )
                    } else {
                        format!(
                            "frame {}  |  slowest {} {}",
                            snapshot.frame_index, slowest_label, slowest_duration,
                        )
                    }),
                    LivePerformanceLineSpec::metric(format!(
                        "lat render {}  |  present {}",
                        format_optional_duration_ms(
                            snapshot.presentation_latency.event_to_render_start_ms,
                        ),
                        format_optional_duration_ms(
                            snapshot.presentation_latency.event_to_present_ms,
                        ),
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "redraw wait {}  |  gpu present {}",
                        format_optional_duration_ms(
                            snapshot.presentation_latency.redraw_request_to_callback_ms,
                        ),
                        format_optional_duration_ms(
                            snapshot.renderer_submission.surface_present_time_us as f64 / 1000.0,
                        ),
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "gpu {} passes  |  {} draws  |  {}",
                        snapshot.renderer_submission.pass_count,
                        snapshot.renderer_submission.draw_count,
                        format_byte_size(snapshot.renderer_submission.uploaded_vertex_bytes),
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "layers {} vis  |  packets {} direct",
                        snapshot.renderer_submission.visible_layer_count,
                        snapshot.renderer_submission.direct_packet_count,
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "upload {}  |  packets {}  |  layers {}",
                        format_byte_size(snapshot.renderer_submission.uploaded_vertex_bytes),
                        snapshot.renderer_submission.direct_packet_count,
                        snapshot.renderer_submission.visible_layer_count,
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "state {}  |  compose {}",
                        format_duration_ms(
                            snapshot.renderer_submission.retained_state_update_time_us as f64
                                / 1000.0
                        ),
                        format_duration_ms(
                            snapshot.renderer_submission.composition_time_us as f64 / 1000.0
                        ),
                    )),
                    LivePerformanceLineSpec::metric(scene_metric),
                    LivePerformanceLineSpec::metric(boundary_metric),
                    LivePerformanceLineSpec::metric(trailing_metric),
                ]
            }
            None => vec![
                LivePerformanceLineSpec::title("live performance".to_string()),
                LivePerformanceLineSpec::body("waiting for the first completed frame".to_string()),
                LivePerformanceLineSpec::body("from the desktop loop".to_string()),
            ],
        }
    }
}

impl LivePerformanceLineSpec {
    fn title(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.14, 0.20, 0.29, 1.0), 11.0, 14.0),
            spacing_after: 6.0,
        }
    }

    fn headline(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.07, 0.34, 0.52, 1.0), 14.0, 18.0),
            spacing_after: 6.0,
        }
    }

    fn body(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.42, 0.49, 0.58, 1.0), 12.0, 16.0),
            spacing_after: 0.0,
        }
    }

    fn metric(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.18, 0.24, 0.32, 1.0), 12.0, 16.0),
            spacing_after: 4.0,
        }
    }
}

impl Widget for LivePerformancePanel {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(
            ctx.phase(),
            sui::EventPhase::Capture | sui::EventPhase::Target
        ) {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Down
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                self.toggle_pressed = self.toggle_bounds(ctx.bounds()).contains(pointer.position);
                if self.toggle_pressed {
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Up
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                let was_pressed = self.toggle_pressed;
                let activate =
                    was_pressed && self.toggle_bounds(ctx.bounds()).contains(pointer.position);
                self.toggle_pressed = false;

                if activate {
                    let current_mode = window_scene_statistics_detail_mode(ctx.window_id());
                    let next_mode = if current_mode.is_detailed() {
                        SceneStatisticsDetailMode::Lightweight
                    } else {
                        SceneStatisticsDetailMode::Detailed
                    };
                    set_window_scene_statistics_detail_mode(ctx.window_id(), next_mode);
                    if let Some(token) = self.detail_refresh_timer.take() {
                        ctx.cancel_timer(token);
                    }
                    self.detail_refresh_timer = Some(ctx.schedule_timer_after(0.0));
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if was_pressed {
                    ctx.request_paint();
                }
            }
            Event::Wake(WakeEvent::Timer { token, .. })
                if self.detail_refresh_timer == Some(*token) =>
            {
                self.detail_refresh_timer = None;
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Cancel => {
                if self.toggle_pressed {
                    self.toggle_pressed = false;
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

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
        let detail_mode = window_scene_statistics_detail_mode(ctx.window_id());
        let lines = self.rebuild_lines(ctx.bounds().width(), &self.content_specs(ctx.window_id()));
        let shadow = ctx.bounds().translate(Vector::new(0.0, 4.0));
        ctx.fill(
            rounded_rect_path(shadow, Self::CORNER_RADIUS),
            Color::rgba(0.10, 0.16, 0.22, 0.12),
        );

        let frame = rounded_rect_path(ctx.bounds(), Self::CORNER_RADIUS);
        ctx.fill(frame.clone(), Color::rgba(0.985, 0.993, 1.0, 0.96));
        ctx.stroke(
            frame,
            Color::rgba(0.78, 0.84, 0.90, 1.0),
            StrokeStyle::new(1.0),
        );

        let toggle_bounds = self.toggle_bounds(ctx.bounds());
        let toggle_shape = rounded_rect_path(toggle_bounds, toggle_bounds.height() * 0.5);
        let toggle_fill = if detail_mode.is_detailed() {
            if self.toggle_pressed {
                Color::rgba(0.07, 0.34, 0.52, 0.92)
            } else {
                Color::rgba(0.09, 0.40, 0.60, 0.88)
            }
        } else if self.toggle_pressed {
            Color::rgba(0.88, 0.92, 0.96, 1.0)
        } else {
            Color::rgba(0.94, 0.96, 0.98, 1.0)
        };
        let toggle_stroke = if detail_mode.is_detailed() {
            Color::rgba(0.05, 0.28, 0.44, 1.0)
        } else {
            Color::rgba(0.76, 0.82, 0.88, 1.0)
        };
        ctx.fill(toggle_shape.clone(), toggle_fill);
        ctx.stroke(toggle_shape, toggle_stroke, StrokeStyle::new(1.0));
        ctx.draw_text(
            toggle_bounds,
            Self::toggle_label(detail_mode).to_string(),
            text_style(
                if detail_mode.is_detailed() {
                    Color::rgba(0.98, 0.995, 1.0, 1.0)
                } else {
                    Color::rgba(0.33, 0.40, 0.48, 1.0)
                },
                10.0,
                14.0,
            ),
        );

        ctx.push_clip_rect(ctx.bounds());
        for line in &lines {
            let line_rect = Rect::new(
                ctx.bounds().x() + Self::PADDING_X,
                ctx.bounds().y() + line.y,
                (ctx.bounds().width() - Self::PADDING_X * 2.0).max(1.0),
                line.style.line_height.max(line.style.font_size),
            );
            ctx.draw_text(line_rect, line.text.clone(), line.style.clone());
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Live performance overlay".to_string());
        node.description =
            Some("Compact floating renderer and scene performance statistics with a scene detail toggle.".to_string());
        node.value = Some(SemanticsValue::Text(
            Self::toggle_label(window_scene_statistics_detail_mode(ctx.window_id())).to_string(),
        ));
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

fn format_optional_duration_ms(duration_ms: f64) -> String {
    if duration_ms <= 0.0 {
        "n/a".to_string()
    } else {
        format_duration_ms(duration_ms)
    }
}

fn format_byte_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;

    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
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

        ctx.fill_bounds(Color::rgba(0.985, 0.99, 1.0, 1.0));
        ctx.stroke_bounds(Color::rgba(0.80, 0.85, 0.91, 1.0), StrokeStyle::new(1.0));
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
                    Color::rgba(0.11, 0.15, 0.21, 1.0)
                } else {
                    Color::rgba(0.41, 0.49, 0.58, 1.0)
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
    use std::{cell::RefCell, fs, path::Path, rc::Rc};

    use super::visual_artifacts::{
        StoryCase, artifact_root, configured_widget_book_state, scroll_to_story_target,
    };
    use super::{
        COLOR_PICKER_NAME, DIALOG_TITLE, DIALOG_TRIGGER_LABEL, GALLERY_SCROLL_NAME,
        LIGHT_PREVIEW_ACTION_LABEL, LIGHT_PREVIEW_INPUT_LABEL, LIGHT_THEME_PREVIEW_CARD_NAME,
        LivePerformanceDisplay, LivePerformancePanel, NAME_INPUT_LABEL, NUMBER_INPUT_NAME,
        POPOVER_NAME, POPOVER_TRIGGER_LABEL, SELECT_NAME, SLIDER_NAME, SUMMARY_NAME,
        TEXT_RENDERING_COMPARISON_SCROLL_NAME, TEXT_RENDERING_COMPARISON_TITLE,
        TEXT_VALIDATION_EDITOR_NAME, TEXT_VALIDATION_SCROLL_NAME, TEXT_VALIDATION_VIEW_TITLE,
        THEME_PREVIEW_TOGGLE_LABEL, TOOLTIP_TEXT, TOOLTIP_TRIGGER_LABEL,
        build_color_and_imagery_story, build_text_rendering_comparison_application,
        build_text_validation_surface, build_widget_book_application, default_widget_book_state,
        theme_preview_card,
    };
    use sui::{
        Application, DefaultTheme, Event, FramePhase, FramePhaseSample, ImeEvent, KeyState,
        KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
        PresentationLatencyDiagnostics, RenderOutput, RendererSubmissionDiagnostics, Result,
        SceneStatistics, SceneStatisticsDetailMode, SemanticsRole, SemanticsValue, Size, SizedBox,
        TextCacheDeltaDiagnostics, TextCacheDiagnostics, Vector, Widget, WidgetPod,
        WidgetPodVisitor, WindowBuilder, WindowEvent, WindowId, WindowPerformanceSnapshot,
        window_scene_statistics_detail_mode,
    };
    use sui_runtime::publish_window_performance_snapshot;
    use sui_scene::{Brush, SceneCommand};
    use sui_testing::prelude::*;

    fn build_default_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| build_widget_book_application(default_widget_book_state()).build())
    }

    fn build_configured_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| build_widget_book_application(configured_widget_book_state()).build())
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

    fn build_text_rendering_comparison_runtime() -> Result<sui::Runtime> {
        build_text_rendering_comparison_application().build()
    }

    fn build_color_validation_runtime() -> Result<sui::Runtime> {
        super::build_color_validation_application().build()
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
                .window(WindowBuilder::new().title("Theme preview reference").root(
                    sui::containers::Padding::all(
                        24.0,
                        SizedBox::new().width(card_width).height(272.0).with_child(
                            super::NamedSection::new(
                                LIGHT_THEME_PREVIEW_CARD_NAME,
                                theme_preview_card(
                                    DefaultTheme::light(),
                                    "Light",
                                    LIGHT_PREVIEW_ACTION_LABEL,
                                    LIGHT_PREVIEW_INPUT_LABEL,
                                ),
                            ),
                        ),
                    ),
                ))
                .build()?,
        )
    }

    #[cfg(feature = "artifacts")]
    fn build_headless_default_widget_book_app() -> Result<TestApp> {
        TestApp::from_runtime(build_widget_book_application(default_widget_book_state()).build()?)
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
    fn screenshot_diff_count(
        left: &sui_testing::Screenshot,
        right: &sui_testing::Screenshot,
    ) -> usize {
        assert_eq!(left.width(), right.width(), "screenshot widths differ");
        assert_eq!(left.height(), right.height(), "screenshot heights differ");

        left.pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .filter(|(left_px, right_px)| left_px != right_px)
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
                if left_px == right_px {
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
    fn hdr_theme_lab_exposes_mode_comparison_sections() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render for HDR lab semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("widget book semantics should exist");

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
    fn hdr_theme_lab_includes_emissive_indicator_and_popup_examples() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("widget book runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("widget book should render for HDR lab semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("widget book semantics should exist");
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
        let app = build_default_widget_book_app()?;
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
    fn text_validation_surface_supports_ime_selection_and_scrolling() -> Result<()> {
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

        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(TEXT_VALIDATION_SCROLL_NAME);
        let before_scroll = scroll.capture_screenshot()?;
        scroll.scroll_pixels(Vector::new(0.0, -220.0))?;
        let after_scroll = scroll.capture_screenshot()?;

        assert_ne!(before_scroll, after_scroll);
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

        assert!(picker.bounds.width() >= 320.0);
        assert!(picker.bounds.height() >= 280.0);
        Ok(())
    }

    #[cfg(feature = "artifacts")]
    #[test]
    #[ignore = "slow; run `cargo run -p sui-widget-book` to generate artifacts"]
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

        let live_app = build_headless_default_widget_book_app()?;
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
        fs::write(
            artifact_dir.join("comparison.txt"),
            format!(
                "live card: {}\nreference card: isolated {}\nlive switch: {}x{}\nreference switch: {}x{}\nnormalized switch: {}x{}\ndiff pixels: {}\n",
                LIGHT_THEME_PREVIEW_CARD_NAME,
                LIGHT_THEME_PREVIEW_CARD_NAME,
                live_switch.width(),
                live_switch.height(),
                reference_switch.width(),
                reference_switch.height(),
                normalized_live_switch.width(),
                normalized_live_switch.height(),
                diff_count,
            ),
        )
        .map_err(|error| {
            sui::Error::new(format!(
                "failed to write comparison metadata in {}: {error}",
                artifact_dir.display()
            ))
        })?;

        assert_eq!(
            diff_count,
            0,
            "theme preview switch differed from isolated reference at 150% DPI; see {}",
            artifact_dir.display()
        );

        Ok(())
    }

    #[test]
    fn widget_book_configured_story_renders_expected_visual_state() -> Result<()> {
        let (
            _default_slider,
            default_number_value,
            default_select,
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
            let default_select = default_window
                .get_by_role(SemanticsRole::ComboBox)
                .with_name(SELECT_NAME)
                .capture_screenshot()?;
            scroll_to_story_target(&default_window, StoryCase::Summary, 12)?;
            let default_summary = default_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                default_slider,
                default_number_value,
                default_select,
                default_summary,
                default_slider_value,
            )
        };

        let (
            _configured_slider,
            configured_number_value,
            configured_select,
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
            let configured_select = configured_window
                .get_by_role(SemanticsRole::ComboBox)
                .with_name(SELECT_NAME)
                .capture_screenshot()?;
            scroll_to_story_target(&configured_window, StoryCase::Summary, 12)?;
            let configured_summary = configured_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                configured_slider,
                configured_number_value,
                configured_select,
                configured_summary,
                configured_slider_value,
            )
        };

        assert_eq!(default_slider_value, 72.0);
        assert_eq!(configured_slider_value, 35.0);
        assert_eq!(default_number_value, 12.0);
        assert_eq!(configured_number_value, 24.0);

        assert!(
            configured_select != default_select,
            "configured select screenshot matched default state"
        );
        assert!(
            configured_summary != default_summary,
            "configured summary screenshot matched default state"
        );

        Ok(())
    }

    #[test]
    fn live_performance_panel_replaces_snapshot_without_creating_children() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        let panel = LivePerformancePanel::with_display(Rc::clone(&display));

        assert_eq!(panel.content_specs(WindowId::new(11)).len(), 3);

        display.borrow_mut().snapshot =
            Some(sample_window_performance_snapshot_record(WindowId::new(11)));

        assert_eq!(panel.content_specs(WindowId::new(11)).len(), 4);
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

        assert!(root.bounds.width() <= 252.0);
        assert!(root.bounds.height() > 0.0);
    }

    #[test]
    fn live_performance_panel_reports_zero_fps_when_idle() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay {
            snapshot: Some(sample_window_performance_snapshot_record(WindowId::new(11))),
            idle: true,
        }));
        let panel = LivePerformancePanel::with_display(display);

        let lines = panel.content_specs(WindowId::new(11));
        assert_eq!(lines[1].text, "0 fps  |  idle");
        assert!(lines.iter().any(|line| line.text.contains("lat present")));
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn live_performance_panel_renders_detailed_scene_metrics() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay {
            snapshot: Some(sample_detailed_window_performance_snapshot_record(
                WindowId::new(11),
            )),
            idle: false,
        }));
        let panel = LivePerformancePanel::with_display(display);

        let lines = panel.content_specs(WindowId::new(11));
        assert!(lines.iter().any(|line| line.text.contains("lat render")));
        assert!(lines.iter().any(|line| line.text.contains("widgets")));
        assert!(lines.iter().any(|line| line.text.contains("repaint(now)")));
        assert!(lines.iter().any(|line| line.text.contains("stack")));
        assert!(lines.iter().any(|line| line.text.contains("overlay")));
        assert!(lines.iter().any(|line| line.text.contains("dirty")));
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
    fn widget_book_overlay_toggle_enables_detail_mode() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");
        let before = overlay.capture_screenshot()?;

        let overlay_node = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            })
            .expect("overlay semantics node present");
        let toggle_point = Point::new(
            overlay_node.bounds.max_x()
                - LivePerformancePanel::PADDING_X
                - LivePerformancePanel::TOGGLE_WIDTH * 0.5,
            overlay_node.bounds.y() + LivePerformancePanel::PADDING_Y - 1.0
                + LivePerformancePanel::TOGGLE_HEIGHT * 0.5,
        );

        overlay.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            toggle_point,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, toggle_point);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        overlay.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, toggle_point);
        up.button = Some(PointerButton::Primary);
        overlay.dispatch_event(Event::Pointer(up))?;

        let after = overlay.capture_screenshot()?;
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "overlay toggle did not enable detailed scene statistics mode"
        );
        assert!(
            before != after,
            "overlay screenshot did not change after toggling detail mode"
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

    #[test]
    fn widget_book_exposes_compact_performance_overlay_semantics() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
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

        assert!(overlay.bounds.width() <= 252.0);
        assert!(overlay.bounds.x() >= 1000.0);
        assert!(overlay.bounds.y() <= 24.0);
    }

    fn sample_window_performance_snapshot_record(window_id: WindowId) -> WindowPerformanceSnapshot {
        WindowPerformanceSnapshot::new(
            window_id,
            7,
            vec![FramePhaseSample::new(FramePhase::Renderer, 1.5)],
            RendererSubmissionDiagnostics::new(
                2, 6, 2048, 24, 1536, 3, 6, 420, 160, 210, 120, 3, 1, 0, 1, 1, 0, 4, 90, 440, 210,
                130, 15, 95, 4, 32768, 115, 85, 22, 16384, 920, 640, 180, 70, 560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: Default::default(),
                viewport: Size::new(1280.0, 720.0),
                total_widget_count: 4,
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
                2, 6, 2048, 24, 1536, 3, 6, 420, 160, 210, 120, 3, 1, 0, 1, 1, 0, 4, 90, 440, 210,
                130, 15, 95, 4, 32768, 115, 85, 22, 16384, 920, 640, 180, 70, 560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: SceneStatisticsDetailMode::Detailed,
                viewport: Size::new(1280.0, 720.0),
                total_widget_count: 9,
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
