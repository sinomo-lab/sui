use std::{cell::RefCell, rc::Rc};

#[cfg(test)]
use crate::widget_book::build_widget_book_gallery;
use crate::widget_book::{
    LivePerformanceRoot, build_color_validation_surface, build_retained_text_benchmark,
    build_text_editing_benchmark, build_text_rendering_comparison_surface,
    build_text_validation_surface, build_theme_demo_surface, build_widget_book_gallery_with_theme,
    default_widget_book_state, register_widget_book_images, set_widget_book_hdr_theme_mode,
    widget_book_hdr_theme_mode,
};
use sui::{
    HdrThemeMode, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    PointerButton, PointerEventKind, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue,
    TextCoveragePolicy, TextHinting, ToggleState, WgpuRenderer, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent, WindowId,
    WindowOutputColorPrimaries, WindowOutputDiagnostics, WindowRenderOptions, WindowStemDarkening,
    WindowTextCoveragePolicy, WindowTextHinting, WindowToneMappingMode, default_sui_logo_image,
    prelude::*, window_output_diagnostics,
};

#[cfg(test)]
use crate::animation_demo::{
    ANIMATION_DEMO_NAME, ANIMATION_DEMO_SCROLL_NAME, ANIMATION_EDITOR_SURFACE_NAME,
    ANIMATION_PAINT_INVALIDATION_NAME, ANIMATION_RETAINED_LAYER_NAME,
    ANIMATION_TIMELINE_PREVIEW_NAME,
};
use crate::animation_demo::{ANIMATION_DEMO_TAB_LABEL, build_animation_demo_with_theme};
#[cfg(test)]
use crate::drag_drop_demo::DRAG_DROP_DEMO_SCROLL_NAME;
use crate::drag_drop_demo::{DRAG_DROP_TAB_LABEL, build_drag_drop_demo_with_theme};
#[cfg(test)]
use crate::layout_demo::LAYOUT_DEMO_SCROLL_NAME;
use crate::layout_demo::{LAYOUT_TAB_LABEL, build_layout_demo_with_theme};
#[cfg(feature = "markdown")]
use crate::markdown_demo::build_markdown_render_demo_with_theme;
#[cfg(all(feature = "markdown", test))]
use crate::markdown_demo::{
    MARKDOWN_RENDER_COOLDOWN_SECONDS, MARKDOWN_RENDER_DEMO_NAME, MARKDOWN_RENDER_SCROLL_NAME,
    MARKDOWN_SOURCE_EDITOR_NAME,
};
use crate::paint_demo::{PAINT_TAB_LABEL, build_paint_demo_with_theme};
#[cfg(test)]
use crate::vector_demo::{
    VECTOR_DOCUMENT_WIDTH, VECTOR_FILL_RULE_NAME, VECTOR_MIN_OBJECT_SIZE, VECTOR_OPACITY_NAME,
    VECTOR_ROTATION_NAME, VECTOR_STROKE_WIDTH_NAME, VECTOR_WIDTH_NAME,
};
use crate::vector_demo::{VECTOR_EDITOR_TAB_LABEL, build_vector_editor_demo_with_theme};

const WINDOW_TITLE: &str = "SUI Demo";
const WINDOW_DESCRIPTION: &str =
    "Browser-style development workspace for the widget book and focused performance demos.";
#[cfg(any(target_arch = "wasm32", test))]
const DEV_WEB_FALLBACK_FONTS: &[(&str, &[u8])] = &[
    (
        "Noto Sans CJK SC",
        include_bytes!("../assets/NotoSansCJKsc-Regular.otf"),
    ),
    (
        "Noto Color Emoji",
        include_bytes!("../assets/NotoColorEmoji.ttf"),
    ),
];
const WIDGET_BOOK_TAB_LABEL: &str = "Widget book";
const THEMES_TAB_LABEL: &str = "Themes";
const RETAINED_TEXT_TAB_LABEL: &str = "Retained text";
const TEXT_RENDERING_COMPARISON_TAB_LABEL: &str = "Text comparison";
const TEXT_VALIDATION_TAB_LABEL: &str = "Text validation";
const TEXT_EDITING_TAB_LABEL: &str = "Text editing";
#[cfg(feature = "markdown")]
const MARKDOWN_RENDER_TAB_LABEL: &str = "Markdown";
const HDR_VALIDATION_TAB_LABEL: &str = "HDR validation";
const SETTINGS_TAB_LABEL: &str = "Settings";
const LIVE_PERFORMANCE_OVERLAY_LABEL: &str = "Show live performance overlay";
const FEATHERING_TOGGLE_LABEL: &str = "Enable renderer feathering";
const FEATHER_WIDTH_NAME: &str = "Feather width";
const OPTICAL_TEXT_CENTERING_TOGGLE_LABEL: &str = "Enable optical vertical text centering";
const TEXT_HINTING_TOGGLE_LABEL: &str = "Enable slight small-text hinting";
const TEXT_HINTING_MAX_PPEM_NAME: &str = "Hinting max ppem";
const TEXT_COVERAGE_POLICY_NAME: &str = "Text coverage policy";
const TEXT_COVERAGE_GAMMA_NAME: &str = "Text coverage gamma";
const STEM_DARKENING_TOGGLE_LABEL: &str = "Enable small-text stem darkening";
const STEM_DARKENING_AMOUNT_NAME: &str = "Stem darkening amount";
const STEM_DARKENING_MAX_PPEM_NAME: &str = "Stem darkening max ppem";
const DEMO_TEXT_HINTING_MAX_PPEM_LIMIT: f32 = 96.0;
pub(crate) const DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM: f32 = 18.0;
pub(crate) const DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT: f32 = 0.08;
const COLOR_MANAGEMENT_MODE_NAME: &str = "Color management";
const OUTPUT_PRIMARIES_NAME: &str = "Output primaries";
const DYNAMIC_RANGE_MODE_NAME: &str = "Dynamic range";
const TONE_MAPPING_MODE_NAME: &str = "Tone mapping";
const SDR_CONTENT_BRIGHTNESS_NAME: &str = "SDR content brightness";
const USE_SYSTEM_SDR_BRIGHTNESS_LABEL: &str = "Use system SDR brightness";
const HDR_THEME_MODE_NAME: &str = "HDR theme mode";
const OUTPUT_DIAGNOSTICS_TITLE: &str = "Output diagnostics";
const HDR_THEME_INSPECTION_TITLE: &str = "HDR theme mode inspection";
const SETTINGS_SCROLL_NAME: &str = "Settings controls";
const COLOR_MANAGEMENT_MODE_OPTIONS: [&str; 4] =
    ["Automatic", "Force SDR", "Prefer wide gamut", "Prefer HDR"];
const OUTPUT_PRIMARIES_OPTIONS: [&str; 3] = ["Automatic", "sRGB", "Display P3"];
const DYNAMIC_RANGE_MODE_OPTIONS: [&str; 3] = ["Automatic", "SDR", "HDR"];
const TONE_MAPPING_MODE_OPTIONS: [&str; 3] = ["Automatic", "Clamp", "Reinhard"];
const TEXT_COVERAGE_POLICY_OPTIONS: [&str; 5] = [
    "Perceptual",
    "Linear",
    "Gamma",
    "Coverage boost",
    "2c - c^2",
];
const HDR_THEME_MODE_OPTIONS: [&str; 4] = [
    "Disabled (SDR baseline)",
    "Wide-gamut only",
    "Constrained HDR",
    "Full HDR",
];
const DEV_SHELL_TOOLBAR_HEIGHT: f32 = 44.0;
const DEV_SHELL_LOGO_BUTTON_SIZE: f32 = 32.0;
const DEV_SHELL_LOGO_IMAGE_HANDLE: ImageHandle = ImageHandle::new(0x5355_4900_0000_0001);
const DEV_SHELL_LOGO_IMAGE_SIZE: u32 = 128;
const DEV_SHELL_TAB_HEIGHT: f32 = 32.0;
const DEV_SHELL_TAB_GAP: f32 = 6.0;
const DEV_SHELL_PLUS_BUTTON_SIZE: f32 = 30.0;
const DEV_SHELL_THEME_TOGGLE_WIDTH: f32 = 140.0;
const DEV_SHELL_THEME_TOGGLE_HEIGHT: f32 = 34.0;
const DEV_SHELL_PICKER_TILE_HEIGHT: f32 = 124.0;
const DEV_SHELL_PICKER_TILE_GAP: f32 = 16.0;
const DEV_SHELL_PICKER_SCROLL_NAME: &str = "Demo picker";
const DEV_SHELL_PICKER_SCROLL_BAR_NAME: &str = "Demo picker scroll bar";
const DEV_SHELL_SETTINGS_TITLE_HEIGHT: f32 = 38.0;
const DEV_SHELL_SETTINGS_RESIZE_HANDLE: f32 = 18.0;
const DEV_SHELL_MIN_SETTINGS_WIDTH: f32 = 320.0;
const DEV_SHELL_MIN_SETTINGS_HEIGHT: f32 = 260.0;
const DEV_SHELL_DEFAULT_SETTINGS_WIDTH: f32 = 460.0;
const DEV_SHELL_DEFAULT_SETTINGS_HEIGHT: f32 = 380.0;
const DEV_SHELL_DEFAULT_SETTINGS_X: f32 = 420.0;
const DEV_SHELL_DEFAULT_SETTINGS_Y: f32 = 96.0;
const DEV_SHELL_THEME_TOGGLE_NAME: &str = "Theme mode";
const DEV_SHELL_PICKER_TITLE: &str = "SUI Demo";

pub(crate) fn apply_demo_small_text_rendering_profile(
    options: WindowRenderOptions,
) -> WindowRenderOptions {
    options.with_stem_darkening(WindowStemDarkening::Enabled {
        max_ppem: DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM,
        amount: DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
pub enum DesktopAutomationMode {
    WidgetBookScroll,
}

pub(crate) type DevThemeReader = Rc<dyn Fn() -> DefaultTheme>;

#[cfg(test)]
pub(crate) fn default_dev_theme_reader() -> DevThemeReader {
    Rc::new(DefaultTheme::default)
}

pub(crate) fn clone_dev_theme_reader(
    theme_reader: &DevThemeReader,
) -> impl Fn() -> DefaultTheme + 'static {
    let theme_reader = Rc::clone(theme_reader);
    move || theme_reader()
}

pub(crate) fn dev_theme_color<F>(
    theme_reader: &DevThemeReader,
    color: F,
) -> impl Fn() -> Color + 'static
where
    F: Fn(DefaultTheme) -> Color + 'static,
{
    let theme_reader = Rc::clone(theme_reader);
    move || color(theme_reader())
}

fn next_dev_theme_scheme(scheme: ThemeColorScheme) -> ThemeColorScheme {
    match scheme {
        ThemeColorScheme::Light => ThemeColorScheme::Dark,
        ThemeColorScheme::Dark => ThemeColorScheme::HighContrast,
        ThemeColorScheme::HighContrast => ThemeColorScheme::Light,
    }
}

fn dev_theme_scheme_label(scheme: ThemeColorScheme) -> &'static str {
    match scheme {
        ThemeColorScheme::Light => "Light",
        ThemeColorScheme::Dark => "Dark",
        ThemeColorScheme::HighContrast => "True black",
    }
}

fn dev_theme_toggle_label(scheme: ThemeColorScheme) -> &'static str {
    match scheme {
        ThemeColorScheme::Light => "Light",
        ThemeColorScheme::Dark => "Dark",
        ThemeColorScheme::HighContrast => "OLED",
    }
}

fn dev_theme_toggle_position(scheme: ThemeColorScheme) -> f32 {
    match scheme {
        ThemeColorScheme::Light => 0.0,
        ThemeColorScheme::Dark => 0.5,
        ThemeColorScheme::HighContrast => 1.0,
    }
}

#[derive(Clone)]
struct DevShellState {
    inner: Rc<RefCell<DevShellStateInner>>,
}

struct DevShellStateInner {
    open_tabs: Vec<usize>,
    active_tab: Option<usize>,
    picker_open: bool,
    theme_scheme: ThemeColorScheme,
    performance_overlay_visible: bool,
    settings_visible: bool,
    settings_bounds: Rect,
    settings_host_bounds: Rect,
}

impl DevShellState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(DevShellStateInner {
                open_tabs: Vec::new(),
                active_tab: None,
                picker_open: true,
                theme_scheme: ThemeColorScheme::Light,
                performance_overlay_visible: false,
                settings_visible: false,
                settings_bounds: Rect::new(
                    DEV_SHELL_DEFAULT_SETTINGS_X,
                    DEV_SHELL_DEFAULT_SETTINGS_Y,
                    DEV_SHELL_DEFAULT_SETTINGS_WIDTH,
                    DEV_SHELL_DEFAULT_SETTINGS_HEIGHT,
                ),
                settings_host_bounds: Rect::ZERO,
            })),
        }
    }

    fn theme(&self) -> DefaultTheme {
        match self.theme_scheme() {
            ThemeColorScheme::Light => DefaultTheme::default(),
            ThemeColorScheme::Dark => DefaultTheme::dark(),
            ThemeColorScheme::HighContrast => DefaultTheme::high_contrast(),
        }
    }

    fn theme_reader(&self) -> DevThemeReader {
        let state = self.clone();
        Rc::new(move || state.theme())
    }

    fn performance_overlay_reader(&self) -> Rc<dyn Fn() -> bool> {
        let state = self.clone();
        Rc::new(move || state.performance_overlay_visible())
    }

    fn theme_scheme(&self) -> ThemeColorScheme {
        self.inner.borrow().theme_scheme
    }

    fn is_dark(&self) -> bool {
        matches!(
            self.inner.borrow().theme_scheme,
            ThemeColorScheme::Dark | ThemeColorScheme::HighContrast
        )
    }

    fn cycle_theme(&self) -> ThemeColorScheme {
        let mut inner = self.inner.borrow_mut();
        inner.theme_scheme = next_dev_theme_scheme(inner.theme_scheme);
        inner.theme_scheme
    }

    fn performance_overlay_visible(&self) -> bool {
        self.inner.borrow().performance_overlay_visible
    }

    fn set_performance_overlay_visible(&self, visible: bool) {
        self.inner.borrow_mut().performance_overlay_visible = visible;
    }

    fn open_tabs(&self) -> Vec<usize> {
        self.inner.borrow().open_tabs.clone()
    }

    fn active_tab(&self) -> Option<usize> {
        let inner = self.inner.borrow();
        inner
            .active_tab
            .filter(|index| inner.open_tabs.contains(index))
    }

    fn picker_visible(&self) -> bool {
        let inner = self.inner.borrow();
        inner.open_tabs.is_empty() || inner.picker_open || inner.active_tab.is_none()
    }

    fn open_demo(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();
        if !inner.open_tabs.contains(&index) {
            inner.open_tabs.push(index);
        }
        inner.active_tab = Some(index);
        inner.picker_open = false;
    }

    fn select_tab(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();
        if inner.open_tabs.contains(&index) {
            inner.active_tab = Some(index);
            inner.picker_open = false;
        }
    }

    fn close_tab(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();
        let Some(position) = inner.open_tabs.iter().position(|tab| *tab == index) else {
            return;
        };
        inner.open_tabs.remove(position);
        if inner.active_tab == Some(index) {
            inner.active_tab = inner
                .open_tabs
                .get(position.min(inner.open_tabs.len().saturating_sub(1)))
                .copied();
            inner.picker_open = inner.active_tab.is_none();
        }
        if inner.open_tabs.is_empty() {
            inner.active_tab = None;
            inner.picker_open = true;
        }
    }

    fn show_picker(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.active_tab = None;
        inner.picker_open = true;
    }

    fn show_settings(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.settings_visible = true;
        inner.settings_bounds =
            clamp_dev_shell_settings_bounds(inner.settings_host_bounds, inner.settings_bounds);
    }

    fn hide_settings(&self) {
        self.inner.borrow_mut().settings_visible = false;
    }

    fn settings_visible(&self) -> bool {
        self.inner.borrow().settings_visible
    }

    fn settings_bounds(&self) -> Rect {
        self.inner.borrow().settings_bounds
    }

    fn set_settings_bounds(&self, bounds: Rect) {
        let mut inner = self.inner.borrow_mut();
        inner.settings_bounds = clamp_dev_shell_settings_bounds(inner.settings_host_bounds, bounds);
    }

    fn set_settings_host_bounds(&self, bounds: Rect) {
        let mut inner = self.inner.borrow_mut();
        inner.settings_host_bounds = bounds;
        inner.settings_bounds = clamp_dev_shell_settings_bounds(bounds, inner.settings_bounds);
    }
}

struct DevDemo {
    title: &'static str,
    description: &'static str,
    icon: IconGlyph,
    accent: Color,
    child: WidgetPod,
}

struct DevBrowserShell {
    state: DevShellState,
    demos: Vec<DevDemo>,
    picker: SingleChild,
    tab_bar: SingleChild,
    main_menu: SingleChild,
    plus_button: SingleChild,
    theme_toggle: SingleChild,
    settings_window: SingleChild,
    content_bounds: Rect,
}

impl DevBrowserShell {
    fn new(render_options: WindowRenderOptions) -> Self {
        Self::with_initial_demo(render_options, None)
    }

    fn with_initial_demo(render_options: WindowRenderOptions, initial_demo: Option<&str>) -> Self {
        set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);
        let state = DevShellState::new();
        let theme_reader = state.theme_reader();
        let demos = build_dev_demo_entries(Rc::clone(&theme_reader));
        if let Some(index) =
            initial_demo.and_then(|title| demos.iter().position(|demo| demo.title == title))
        {
            state.open_demo(index);
        }
        let demo_titles = demos.iter().map(|demo| demo.title).collect::<Vec<_>>();

        let mut demo_buttons = WidgetChildren::with_capacity(demos.len());
        for (index, demo) in demos.iter().enumerate() {
            let open_state = state.clone();
            let theme_state = state.clone();
            demo_buttons.push(
                ActionCard::new(demo.title, demo.description)
                    .theme_when(move || theme_state.theme())
                    .icon(demo.icon)
                    .accent(demo.accent)
                    .min_width(260.0)
                    .min_height(DEV_SHELL_PICKER_TILE_HEIGHT)
                    .on_press_with_ctx(move |ctx| {
                        ctx.clear_focus();
                        open_state.open_demo(index);
                        request_window_refresh(ctx, true);
                    }),
            );
        }
        let picker_scroll_state = ScrollState::new();
        let picker = VerticalScrollPane::new(
            ScrollView::vertical(DevDemoPickerGrid::new(demo_buttons))
                .state(picker_scroll_state.clone())
                .name(DEV_SHELL_PICKER_SCROLL_NAME),
            ScrollBar::vertical(picker_scroll_state).name(DEV_SHELL_PICKER_SCROLL_BAR_NAME),
        );

        let tab_titles = demo_titles.clone();
        let tab_tabs_state = state.clone();
        let tab_bar = BrowserTabBar::new("Open demos")
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .tabs_when(move || {
                tab_tabs_state
                    .open_tabs()
                    .into_iter()
                    .filter_map(|index| tab_titles.get(index).map(|title| (*title).to_string()))
                    .collect()
            })
            .selected_when({
                let selected_state = state.clone();
                move || {
                    let tabs = selected_state.open_tabs();
                    selected_state
                        .active_tab()
                        .and_then(|active| tabs.iter().position(|index| *index == active))
                }
            })
            .on_change_with_ctx({
                let select_state = state.clone();
                move |tab_index, _, ctx| {
                    if let Some(demo_index) = select_state.open_tabs().get(tab_index).copied() {
                        select_state.select_tab(demo_index);
                        request_window_refresh(ctx, true);
                    }
                }
            })
            .on_close_with_ctx({
                let close_state = state.clone();
                move |tab_index, _, ctx| {
                    if let Some(demo_index) = close_state.open_tabs().get(tab_index).copied() {
                        close_state.close_tab(demo_index);
                        request_window_refresh(ctx, true);
                    }
                }
            });

        let picker_state = state.clone();
        let plus_button = IconButton::new(IconGlyph::Add, "Open demo")
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .size(DEV_SHELL_PLUS_BUTTON_SIZE)
            .icon_size(16.0)
            .on_press_with_ctx(move |ctx| {
                picker_state.show_picker();
                request_window_refresh(ctx, true);
            });

        let menu_state = state.clone();
        let main_menu = ContextMenu::new("SUI menu", SuiLogoButton::new())
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .activation_button(PointerButton::Primary)
            .item(MenuItem::new("Settings"))
            .on_activate_with_ctx(move |ctx, _, _| {
                menu_state.show_settings();
                ctx.clear_focus();
                request_window_refresh(ctx, true);
            });

        Self {
            state: state.clone(),
            demos,
            picker: SingleChild::new(picker),
            tab_bar: SingleChild::new(tab_bar),
            main_menu: SingleChild::new(main_menu),
            plus_button: SingleChild::new(plus_button),
            theme_toggle: SingleChild::new(ThemeToggleButton::new(state.clone())),
            settings_window: SingleChild::new(FloatingSettingsWindow::new(
                state.clone(),
                build_render_settings_tab_with_options(
                    render_options,
                    Rc::clone(&theme_reader),
                    state,
                ),
            )),
            content_bounds: Rect::ZERO,
        }
    }

    fn performance_overlay_reader(&self) -> Rc<dyn Fn() -> bool> {
        self.state.performance_overlay_reader()
    }

    fn root_size_for_constraints(constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1280.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                820.0
            },
        ))
    }

    fn toolbar_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            DEV_SHELL_TOOLBAR_HEIGHT,
        )
    }

    fn logo_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + 8.0,
            bounds.y() + ((DEV_SHELL_TOOLBAR_HEIGHT - DEV_SHELL_LOGO_BUTTON_SIZE) * 0.5),
            DEV_SHELL_LOGO_BUTTON_SIZE,
            DEV_SHELL_LOGO_BUTTON_SIZE,
        )
    }

    fn theme_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.max_x() - DEV_SHELL_THEME_TOGGLE_WIDTH - 12.0,
            bounds.y() + ((DEV_SHELL_TOOLBAR_HEIGHT - DEV_SHELL_THEME_TOGGLE_HEIGHT) * 0.5),
            DEV_SHELL_THEME_TOGGLE_WIDTH,
            DEV_SHELL_THEME_TOGGLE_HEIGHT,
        )
    }

    fn tab_zone_rect(bounds: Rect) -> Rect {
        let left = bounds.x() + 50.0;
        let right = Self::theme_rect(bounds).x() - 10.0;
        Rect::new(
            left,
            bounds.y() + 6.0,
            (right - left).max(0.0),
            DEV_SHELL_TAB_HEIGHT,
        )
    }

    fn picker_grid_rect(content: Rect) -> Rect {
        let width = (content.width() - 72.0).clamp(0.0, 1180.0);
        let x = content.x() + ((content.width() - width) * 0.5);
        Rect::new(
            x,
            content.y() + 112.0,
            width,
            (content.height() - 152.0).max(0.0),
        )
    }

    fn arrange_tab_strip(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) -> Rect {
        let tab_zone = Self::tab_zone_rect(bounds);
        let max_tabs_width =
            (tab_zone.width() - DEV_SHELL_TAB_GAP - DEV_SHELL_PLUS_BUTTON_SIZE).max(0.0);
        let tabs_size = self.tab_bar.child().measured_size();
        let tabs_width = tabs_size.width.min(max_tabs_width);
        let tabs_rect = Rect::new(tab_zone.x(), tab_zone.y(), tabs_width, tab_zone.height());
        self.tab_bar.arrange(ctx, tabs_rect);

        let plus_x = (tabs_rect.max_x() + DEV_SHELL_TAB_GAP)
            .min(tab_zone.max_x() - DEV_SHELL_PLUS_BUTTON_SIZE)
            .max(tab_zone.x());

        Rect::new(
            plus_x,
            tab_zone.y() + ((tab_zone.height() - DEV_SHELL_PLUS_BUTTON_SIZE) * 0.5),
            DEV_SHELL_PLUS_BUTTON_SIZE,
            DEV_SHELL_PLUS_BUTTON_SIZE,
        )
    }

    fn paint_toolbar(&self, ctx: &mut PaintCtx, theme: &DefaultTheme) {
        let bounds = ctx.bounds();
        let toolbar = Self::toolbar_rect(bounds);
        let palette = theme.palette;
        let toolbar_background = if self.state.is_dark() {
            Color::rgba(0.085, 0.105, 0.13, 1.0)
        } else {
            Color::rgba(0.95, 0.965, 0.985, 1.0)
        };
        ctx.fill_rect(toolbar, toolbar_background);
        ctx.stroke_rect(
            Rect::new(toolbar.x(), toolbar.max_y() - 1.0, toolbar.width(), 1.0),
            palette.border.with_alpha(0.85),
            StrokeStyle::new(1.0),
        );
    }

    fn paint_picker(&self, ctx: &mut PaintCtx, theme: &DefaultTheme) {
        let content = self.content_bounds;
        let palette = theme.palette;
        ctx.fill_rect(content, palette.surface);
        let header = Rect::new(content.x(), content.y(), content.width(), 96.0);
        ctx.fill_rect(header, palette.surface_hover.with_alpha(0.45));
        ctx.draw_text(
            Rect::new(
                content.x() + 40.0,
                content.y() + 24.0,
                content.width() - 80.0,
                32.0,
            ),
            DEV_SHELL_PICKER_TITLE,
            TextStyle {
                font_size: 28.0,
                line_height: 34.0,
                color: palette.text,
                ..TextStyle::default()
            },
        );
        ctx.draw_text(
            Rect::new(
                content.x() + 40.0,
                content.y() + 56.0,
                content.width() - 80.0,
                20.0,
            ),
            "Renderer, text, color, editor, and widget surfaces.",
            TextStyle {
                font_size: 14.0,
                line_height: 20.0,
                color: palette.text_muted,
                ..TextStyle::default()
            },
        );
    }
}

struct DevDemoPickerGrid {
    children: WidgetChildren,
}

impl DevDemoPickerGrid {
    fn new(children: WidgetChildren) -> Self {
        Self { children }
    }

    fn columns_for_width(width: f32) -> usize {
        if width >= 960.0 {
            3
        } else if width >= 620.0 {
            2
        } else {
            1
        }
    }

    fn column_width(width: f32, columns: usize) -> f32 {
        if columns == 0 {
            return 0.0;
        }
        ((width - (DEV_SHELL_PICKER_TILE_GAP * (columns.saturating_sub(1) as f32))).max(0.0)
            / columns as f32)
            .max(0.0)
    }

    fn content_height(child_count: usize, columns: usize) -> f32 {
        if child_count == 0 {
            return 0.0;
        }
        let rows = child_count.div_ceil(columns) as f32;
        (rows * DEV_SHELL_PICKER_TILE_HEIGHT) + ((rows - 1.0).max(0.0) * DEV_SHELL_PICKER_TILE_GAP)
    }
}

impl Widget for DevDemoPickerGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1120.0)
        };
        let columns = Self::columns_for_width(width);
        let column_width = Self::column_width(width, columns);
        let child_constraints =
            Constraints::tight(Size::new(column_width, DEV_SHELL_PICKER_TILE_HEIGHT));
        for index in 0..self.children.len() {
            self.children.measure_child(index, ctx, child_constraints);
        }
        constraints.clamp(Size::new(
            width,
            Self::content_height(self.children.len(), columns),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            for index in 0..self.children.len() {
                self.children.arrange_child(index, ctx, Rect::ZERO);
            }
            return;
        }

        let columns = Self::columns_for_width(bounds.width());
        let column_width = Self::column_width(bounds.width(), columns);
        for index in 0..self.children.len() {
            let column = index % columns;
            let row = index / columns;
            let rect = Rect::new(
                bounds.x() + column as f32 * (column_width + DEV_SHELL_PICKER_TILE_GAP),
                bounds.y()
                    + row as f32 * (DEV_SHELL_PICKER_TILE_HEIGHT + DEV_SHELL_PICKER_TILE_GAP),
                column_width,
                DEV_SHELL_PICKER_TILE_HEIGHT,
            );
            self.children.arrange_child(index, ctx, rect);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

impl Widget for DevBrowserShell {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Target {
            return;
        }

        match event {
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                match key.key.as_str() {
                    "+" | "N" => self.state.show_picker(),
                    _ => return,
                }
                request_window_refresh(ctx, true);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let root_size = Self::root_size_for_constraints(constraints);
        let content_size = Size::new(
            root_size.width,
            (root_size.height - DEV_SHELL_TOOLBAR_HEIGHT).max(0.0),
        );
        let content_constraints = Constraints::tight(content_size);

        let tab_zone = Self::tab_zone_rect(Rect::new(0.0, 0.0, root_size.width, root_size.height));
        self.tab_bar.measure(
            ctx,
            Constraints::new(
                Size::ZERO,
                Size::new(
                    (tab_zone.width() - DEV_SHELL_TAB_GAP - DEV_SHELL_PLUS_BUTTON_SIZE).max(0.0),
                    DEV_SHELL_TAB_HEIGHT,
                ),
            ),
        );

        self.main_menu.measure(ctx, constraints.loosen());
        self.plus_button.measure(
            ctx,
            Constraints::tight(Size::new(
                DEV_SHELL_PLUS_BUTTON_SIZE,
                DEV_SHELL_PLUS_BUTTON_SIZE,
            )),
        );
        self.theme_toggle.measure(
            ctx,
            Constraints::tight(Size::new(
                DEV_SHELL_THEME_TOGGLE_WIDTH,
                DEV_SHELL_THEME_TOGGLE_HEIGHT,
            )),
        );

        if self.state.picker_visible() {
            let content = Rect::new(
                0.0,
                DEV_SHELL_TOOLBAR_HEIGHT,
                root_size.width,
                content_size.height,
            );
            let grid = Self::picker_grid_rect(content);
            self.picker.measure(ctx, Constraints::tight(grid.size));
        } else if let Some(active) = self.state.active_tab() {
            self.demos[active].child.measure(ctx, content_constraints);
        }

        if self.state.settings_visible() {
            let settings_bounds = self.state.settings_bounds();
            self.settings_window
                .measure(ctx, Constraints::tight(settings_bounds.size));
        }

        root_size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let plus_rect = self.arrange_tab_strip(ctx, bounds);
        self.plus_button.arrange(ctx, plus_rect);
        self.theme_toggle.arrange(ctx, Self::theme_rect(bounds));
        let menu_origin = Self::logo_rect(bounds).origin;
        let menu_size = self.main_menu.child().measured_size();
        self.main_menu.arrange(
            ctx,
            Rect::from_origin_size(
                menu_origin,
                Size::new(
                    menu_size.width.max(DEV_SHELL_LOGO_BUTTON_SIZE),
                    menu_size.height.max(DEV_SHELL_LOGO_BUTTON_SIZE),
                ),
            ),
        );

        self.content_bounds = Rect::new(
            bounds.x(),
            bounds.y() + DEV_SHELL_TOOLBAR_HEIGHT,
            bounds.width(),
            (bounds.height() - DEV_SHELL_TOOLBAR_HEIGHT).max(0.0),
        );
        self.state.set_settings_host_bounds(self.content_bounds);

        if self.state.picker_visible() {
            let grid = Self::picker_grid_rect(self.content_bounds);
            self.picker.arrange(ctx, grid);
        } else {
            self.picker.arrange(ctx, Rect::ZERO);
            for (index, demo) in self.demos.iter_mut().enumerate() {
                if self.state.active_tab() == Some(index) {
                    demo.child.arrange(ctx, self.content_bounds);
                }
            }
        }

        if self.state.settings_visible() {
            self.settings_window
                .arrange(ctx, self.state.settings_bounds());
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.state.theme();
        let palette = theme.palette;
        ctx.fill_bounds(palette.surface);

        if self.state.picker_visible() {
            self.paint_picker(ctx, &theme);
            self.picker.paint(ctx);
        } else if let Some(active) = self.state.active_tab() {
            self.demos[active].child.paint(ctx);
        }

        self.paint_toolbar(ctx, &theme);
        self.tab_bar.paint(ctx);
        self.plus_button.paint(ctx);
        self.theme_toggle.paint(ctx);
        self.settings_window.paint(ctx);
        self.main_menu.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tabs, ctx.bounds());
        node.name = Some("SUI demo browser".to_string());
        node.value = self
            .state
            .active_tab()
            .map(|index| SemanticsValue::Text(self.demos[index].title.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        self.tab_bar.semantics(ctx);
        self.plus_button.semantics(ctx);
        self.theme_toggle.semantics(ctx);
        if self.state.picker_visible() {
            self.picker.semantics(ctx);
        } else if let Some(active) = self.state.active_tab() {
            self.demos[active].child.semantics(ctx);
        }
        self.settings_window.semantics(ctx);
        self.main_menu.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.tab_bar.visit_children(visitor);
        if self.state.picker_visible() {
            self.picker.visit_children(visitor);
        } else if let Some(active) = self.state.active_tab() {
            visitor.visit(&self.demos[active].child);
        }
        self.plus_button.visit_children(visitor);
        self.theme_toggle.visit_children(visitor);
        if self.state.settings_visible() {
            self.settings_window.visit_children(visitor);
        }
        self.main_menu.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.tab_bar.visit_children_mut(visitor);
        if self.state.picker_visible() {
            self.picker.visit_children_mut(visitor);
        } else if let Some(active) = self.state.active_tab() {
            visitor.visit(&mut self.demos[active].child);
        }
        self.plus_button.visit_children_mut(visitor);
        self.theme_toggle.visit_children_mut(visitor);
        if self.state.settings_visible() {
            self.settings_window.visit_children_mut(visitor);
        }
        self.main_menu.visit_children_mut(visitor);
    }
}

struct SuiLogoButton {
    image: SingleChild,
}

impl SuiLogoButton {
    fn new() -> Self {
        Self {
            image: SingleChild::new(
                Image::new(DEV_SHELL_LOGO_IMAGE_HANDLE)
                    .fit(ImageFit::Contain)
                    .size(Size::new(
                        DEV_SHELL_LOGO_BUTTON_SIZE,
                        DEV_SHELL_LOGO_BUTTON_SIZE,
                    ))
                    .without_border()
                    .corner_radius(0.0),
            ),
        }
    }
}

impl Widget for SuiLogoButton {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(
            DEV_SHELL_LOGO_BUTTON_SIZE,
            DEV_SHELL_LOGO_BUTTON_SIZE,
        ));
        self.image.measure(ctx, Constraints::tight(size));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.image.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.image.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("SUI menu".to_string());
        node.actions = vec![SemanticsAction::Activate];
        ctx.push(node);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.image.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.image.visit_children_mut(visitor);
    }
}

struct ThemeToggleButton {
    state: DevShellState,
    hovered: bool,
    pressed: bool,
    animation_from: ThemeColorScheme,
    animation_progress: f32,
    animation: Option<Transition<f32>>,
}

impl ThemeToggleButton {
    fn new(state: DevShellState) -> Self {
        let scheme = state.theme_scheme();
        Self {
            state,
            hovered: false,
            pressed: false,
            animation_from: scheme,
            animation_progress: 1.0,
            animation: None,
        }
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        self.animation_from = self.state.theme_scheme();
        self.animation_progress = 0.0;
        self.state.cycle_theme();
        let theme = self.state.theme();
        self.animation = Some(Transition::new(
            0.0,
            1.0,
            ctx.current_time(),
            theme.motion.toggle_duration(),
            theme.motion.toggle_easing(),
        ));
        ctx.request_animation_frame();
        ctx.request_paint();
        ctx.request_semantics();
        request_window_refresh(ctx, true);
    }

    fn advance_animation(&mut self, time: f64) -> bool {
        let Some(animation) = self.animation else {
            return false;
        };

        self.animation_progress = animation.sample(time);
        if animation.is_complete(time) {
            self.animation_progress = 1.0;
            self.animation = None;
            return false;
        }

        true
    }

    fn visual_transition(&self) -> (ThemeColorScheme, ThemeColorScheme, f32) {
        let target = self.state.theme_scheme();
        if self.animation.is_some() {
            (
                self.animation_from,
                target,
                self.animation_progress.clamp(0.0, 1.0),
            )
        } else {
            (target, target, 1.0)
        }
    }

    fn track_color(scheme: ThemeColorScheme) -> Color {
        match scheme {
            ThemeColorScheme::Light => Color::rgba(0.97, 0.985, 1.0, 1.0),
            ThemeColorScheme::Dark => Color::rgba(0.11, 0.15, 0.20, 1.0),
            ThemeColorScheme::HighContrast => Color::rgba(0.0, 0.0, 0.0, 1.0),
        }
    }

    fn knob_color(scheme: ThemeColorScheme) -> Color {
        match scheme {
            ThemeColorScheme::Light => Color::rgba(0.98, 0.74, 0.24, 1.0),
            ThemeColorScheme::Dark => Color::rgba(0.33, 0.74, 0.88, 1.0),
            ThemeColorScheme::HighContrast => Color::rgba(0.0, 0.84, 1.0, 1.0),
        }
    }

    fn label_rect(bounds: Rect, knob: Rect, scheme: ThemeColorScheme) -> Rect {
        if matches!(scheme, ThemeColorScheme::HighContrast) {
            let x = bounds.x() + 10.0;
            Rect::new(x, bounds.y() + 8.0, (knob.x() - x - 6.0).max(0.0), 18.0)
        } else {
            let x = knob.max_x() + 8.0;
            Rect::new(
                x,
                bounds.y() + 8.0,
                (bounds.max_x() - x - 10.0).max(0.0),
                18.0,
            )
        }
    }
}

impl Widget for ThemeToggleButton {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                if self.hovered != hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.hovered = false;
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed = true;
                self.hovered = true;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let activate = self.pressed && ctx.bounds().contains(pointer.position);
                self.pressed = false;
                self.hovered = ctx.bounds().contains(pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                } else {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animation(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            DEV_SHELL_THEME_TOGGLE_WIDTH,
            DEV_SHELL_THEME_TOGGLE_HEIGHT,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.state.theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let scheme = self.state.theme_scheme();
        let (from_scheme, to_scheme, transition) = self.visual_transition();
        let background = Color::interpolate(
            Self::track_color(from_scheme),
            Self::track_color(to_scheme),
            transition,
        );
        let border = if self.hovered || ctx.is_focused() {
            palette.border_focus
        } else {
            palette.border
        };
        ctx.fill(Path::rounded_rect(bounds, 17.0), background);
        ctx.stroke(
            Path::rounded_rect(bounds, 17.0),
            border,
            StrokeStyle::new(1.0),
        );

        let knob_progress = f32::interpolate(
            dev_theme_toggle_position(from_scheme),
            dev_theme_toggle_position(to_scheme),
            transition,
        );
        let knob_diameter = 28.0;
        let knob_x = bounds.x() + 3.0 + ((bounds.width() - knob_diameter - 6.0) * knob_progress);
        let knob = Rect::new(knob_x, bounds.y() + 3.0, 28.0, 28.0);
        ctx.fill(
            Path::circle(
                Point::new(
                    knob.x() + knob.width() * 0.5,
                    knob.y() + knob.height() * 0.5,
                ),
                14.0,
            ),
            Color::interpolate(
                Self::knob_color(from_scheme),
                Self::knob_color(to_scheme),
                transition,
            ),
        );
        let label_rect = Self::label_rect(bounds, knob, scheme);
        ctx.draw_text(
            label_rect,
            dev_theme_toggle_label(scheme),
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: palette.text,
                ..TextStyle::default()
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Switch, ctx.bounds());
        let scheme = self.state.theme_scheme();
        node.name = Some(DEV_SHELL_THEME_TOGGLE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(
            dev_theme_scheme_label(scheme).to_string(),
        ));
        node.state.checked = Some(match scheme {
            ThemeColorScheme::Light => ToggleState::Unchecked,
            ThemeColorScheme::Dark => ToggleState::Checked,
            ThemeColorScheme::HighContrast => ToggleState::Mixed,
        });
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
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

#[derive(Clone, Copy)]
enum FloatingSettingsGestureKind {
    Move,
    Resize,
}

struct FloatingSettingsGesture {
    pointer_id: u64,
    kind: FloatingSettingsGestureKind,
    pointer_origin: Point,
    initial_bounds: Rect,
}

struct FloatingSettingsWindow {
    state: DevShellState,
    content: SingleChild,
    gesture: Option<FloatingSettingsGesture>,
    close_pressed: bool,
    close_hovered: bool,
}

impl FloatingSettingsWindow {
    fn new<W>(state: DevShellState, content: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            state,
            content: SingleChild::new(content),
            gesture: None,
            close_pressed: false,
            close_hovered: false,
        }
    }

    fn title_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            DEV_SHELL_SETTINGS_TITLE_HEIGHT.min(bounds.height()),
        )
    }

    fn content_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + 1.0,
            bounds.y() + DEV_SHELL_SETTINGS_TITLE_HEIGHT,
            (bounds.width() - 2.0).max(0.0),
            (bounds.height() - DEV_SHELL_SETTINGS_TITLE_HEIGHT - 1.0).max(0.0),
        )
    }

    fn close_rect(bounds: Rect) -> Rect {
        Rect::new(bounds.max_x() - 34.0, bounds.y() + 4.0, 30.0, 30.0)
    }

    fn resize_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.max_x() - DEV_SHELL_SETTINGS_RESIZE_HANDLE,
            bounds.max_y() - DEV_SHELL_SETTINGS_RESIZE_HANDLE,
            DEV_SHELL_SETTINGS_RESIZE_HANDLE,
            DEV_SHELL_SETTINGS_RESIZE_HANDLE,
        )
    }

    fn update_gesture(&mut self, ctx: &mut EventCtx, position: Point) {
        let Some(gesture) = self.gesture.as_ref() else {
            return;
        };
        let delta = position - gesture.pointer_origin;
        let next = match gesture.kind {
            FloatingSettingsGestureKind::Move => Rect::new(
                gesture.initial_bounds.x() + delta.x,
                gesture.initial_bounds.y() + delta.y,
                gesture.initial_bounds.width(),
                gesture.initial_bounds.height(),
            ),
            FloatingSettingsGestureKind::Resize => Rect::new(
                gesture.initial_bounds.x(),
                gesture.initial_bounds.y(),
                gesture.initial_bounds.width() + delta.x,
                gesture.initial_bounds.height() + delta.y,
            ),
        };
        self.state.set_settings_bounds(next);
        request_window_refresh(ctx, true);
    }
}

impl Widget for FloatingSettingsWindow {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .gesture
                        .as_ref()
                        .is_some_and(|gesture| gesture.pointer_id == pointer.pointer_id) =>
            {
                self.update_gesture(ctx, pointer.position);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = Self::close_rect(ctx.bounds()).contains(pointer.position);
                if self.close_hovered != hovered {
                    self.close_hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let bounds = ctx.bounds();
                if Self::close_rect(bounds).contains(pointer.position) {
                    self.close_pressed = true;
                    self.close_hovered = true;
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if Self::resize_rect(bounds).contains(pointer.position) {
                    self.gesture = Some(FloatingSettingsGesture {
                        pointer_id: pointer.pointer_id,
                        kind: FloatingSettingsGestureKind::Resize,
                        pointer_origin: pointer.position,
                        initial_bounds: bounds,
                    });
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.set_handled();
                } else if Self::title_rect(bounds).contains(pointer.position) {
                    self.gesture = Some(FloatingSettingsGesture {
                        pointer_id: pointer.pointer_id,
                        kind: FloatingSettingsGestureKind::Move,
                        pointer_origin: pointer.position,
                        initial_bounds: bounds,
                    });
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let captured = self
                    .gesture
                    .as_ref()
                    .is_some_and(|gesture| gesture.pointer_id == pointer.pointer_id)
                    || self.close_pressed;
                if !captured {
                    return;
                }
                if self.close_pressed && Self::close_rect(ctx.bounds()).contains(pointer.position) {
                    self.state.hide_settings();
                }
                self.gesture = None;
                self.close_pressed = false;
                self.close_hovered = Self::close_rect(ctx.bounds()).contains(pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                request_window_refresh(ctx, true);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.gesture.is_some() || self.close_pressed {
                    self.gesture = None;
                    self.close_pressed = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    request_window_refresh(ctx, true);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Escape" =>
            {
                self.state.hide_settings();
                request_window_refresh(ctx, true);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(self.state.settings_bounds().size);
        let content = Self::content_rect(Rect::from_origin_size(Point::ZERO, size));
        self.content.measure(ctx, Constraints::tight(content.size));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, Self::content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.state.settings_visible() {
            return;
        }
        let theme = self.state.theme();
        let palette = theme.palette;
        let content_palette = palette;
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        ctx.fill(Path::rounded_rect(bounds, 8.0), content_palette.surface);
        ctx.stroke(
            Path::rounded_rect(bounds, 8.0),
            if self.state.is_dark() {
                palette.border.with_alpha(0.92)
            } else {
                content_palette.border.with_alpha(0.92)
            },
            StrokeStyle::new(1.0),
        );

        let title = Self::title_rect(bounds);
        let title_fill = if self.state.is_dark() {
            Color::rgba(0.12, 0.16, 0.21, 1.0)
        } else {
            Color::rgba(0.16, 0.20, 0.26, 1.0)
        };
        ctx.fill(top_rounded_rect_path(title, 8.0), title_fill);
        ctx.stroke_rect(
            Rect::new(title.x(), title.max_y(), title.width(), 1.0),
            content_palette.border.with_alpha(0.6),
            StrokeStyle::new(1.0),
        );
        ctx.draw_text(
            Rect::new(
                title.x() + 14.0,
                title.y() + 9.0,
                title.width() - 54.0,
                20.0,
            ),
            SETTINGS_TAB_LABEL,
            TextStyle {
                font_size: 13.0,
                line_height: 18.0,
                color: Color::rgba(0.96, 0.97, 0.99, 1.0),
                ..TextStyle::default()
            },
        );

        let close = Self::close_rect(bounds);
        if self.close_hovered || self.close_pressed {
            ctx.fill(
                Path::rounded_rect(close, 6.0),
                if self.close_pressed {
                    Color::rgba(1.0, 1.0, 1.0, 0.24)
                } else {
                    Color::rgba(1.0, 1.0, 1.0, 0.14)
                },
            );
        }
        let close_color = Color::rgba(0.96, 0.97, 0.99, 1.0);
        ctx.stroke(close_icon_path(close), close_color, StrokeStyle::new(1.5));

        self.content.paint(ctx);

        let handle = Self::resize_rect(bounds);
        let handle_color = content_palette.border.with_alpha(0.82);
        ctx.stroke(
            diagonal_handle_path(handle, 10.0, 1.0),
            handle_color,
            StrokeStyle::new(1.4),
        );
        ctx.stroke(
            diagonal_handle_path(handle, 6.0, 5.0),
            handle_color.with_alpha(0.72),
            StrokeStyle::new(1.4),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.state.settings_visible() {
            return;
        }
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        node.name = Some(SETTINGS_TAB_LABEL.to_string());
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
        self.content.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.state.settings_visible() {
            self.content.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.state.settings_visible() {
            self.content.visit_children_mut(visitor);
        }
    }
}

fn build_dev_demo_entries(theme_reader: DevThemeReader) -> Vec<DevDemo> {
    vec![
        DevDemo {
            title: WIDGET_BOOK_TAB_LABEL,
            description: "Catalog of controls, containers, media, and text surfaces.",
            icon: IconGlyph::MoreHorizontal,
            accent: Color::rgba(0.16, 0.48, 0.86, 1.0),
            child: WidgetPod::new(build_widget_book_gallery_with_theme(
                default_widget_book_state(),
                Rc::clone(&theme_reader),
            )),
        },
        DevDemo {
            title: THEMES_TAB_LABEL,
            description: "Theme previews and HDR theme mode comparisons.",
            icon: IconGlyph::PaintBucket,
            accent: Color::rgba(0.62, 0.28, 0.78, 1.0),
            child: WidgetPod::new(build_theme_demo_surface(default_widget_book_state())),
        },
        DevDemo {
            title: ANIMATION_DEMO_TAB_LABEL,
            description: "Timeline playback, retained layer, repaint, editor, and overlay examples.",
            icon: IconGlyph::Sparkles,
            accent: Color::rgba(0.18, 0.58, 0.74, 1.0),
            child: WidgetPod::new(build_animation_demo_with_theme(Rc::clone(&theme_reader))),
        },
        DevDemo {
            title: RETAINED_TEXT_TAB_LABEL,
            description: "Retained text layout and redraw benchmark.",
            icon: IconGlyph::Search,
            accent: Color::rgba(0.75, 0.42, 0.12, 1.0),
            child: WidgetPod::new(build_retained_text_benchmark()),
        },
        DevDemo {
            title: TEXT_RENDERING_COMPARISON_TAB_LABEL,
            description: "Side-by-side text rendering comparison surface.",
            icon: IconGlyph::FitView,
            accent: Color::rgba(0.20, 0.50, 0.62, 1.0),
            child: WidgetPod::new(build_text_rendering_comparison_surface()),
        },
        DevDemo {
            title: TEXT_VALIDATION_TAB_LABEL,
            description: "Validation surface for text metrics, alignment, and rasterization.",
            icon: IconGlyph::ActualSize,
            accent: Color::rgba(0.68, 0.26, 0.32, 1.0),
            child: WidgetPod::new(build_text_validation_surface()),
        },
        DevDemo {
            title: TEXT_EDITING_TAB_LABEL,
            description: "Single-line and multi-line text editing demos.",
            icon: IconGlyph::Restore,
            accent: Color::rgba(0.35, 0.38, 0.82, 1.0),
            child: WidgetPod::new(build_text_editing_benchmark()),
        },
        #[cfg(feature = "markdown")]
        DevDemo {
            title: MARKDOWN_RENDER_TAB_LABEL,
            description: "Feature-gated markdown rendering through SUI rich text documents.",
            icon: IconGlyph::File,
            accent: Color::rgba(0.12, 0.48, 0.70, 1.0),
            child: WidgetPod::new(build_markdown_render_demo_with_theme(Rc::clone(
                &theme_reader,
            ))),
        },
        DevDemo {
            title: HDR_VALIDATION_TAB_LABEL,
            description: "HDR, color-management, and tone-mapping validation surface.",
            icon: IconGlyph::Maximize,
            accent: Color::rgba(0.82, 0.52, 0.10, 1.0),
            child: WidgetPod::new(build_color_validation_surface()),
        },
        DevDemo {
            title: LAYOUT_TAB_LABEL,
            description: "Stack, Align, and Flex layout patterns for app composition.",
            icon: IconGlyph::Maximize,
            accent: Color::rgba(0.08, 0.58, 0.42, 1.0),
            child: WidgetPod::new(build_layout_demo_with_theme(Rc::clone(&theme_reader))),
        },
        DevDemo {
            title: DRAG_DROP_TAB_LABEL,
            description: "Internal drag-and-drop payloads, targets, scopes, and preview overlay.",
            icon: IconGlyph::Send,
            accent: Color::rgba(0.20, 0.48, 0.78, 1.0),
            child: WidgetPod::new(build_drag_drop_demo_with_theme(Rc::clone(&theme_reader))),
        },
        DevDemo {
            title: PAINT_TAB_LABEL,
            description: "Pixel canvas painting workspace with editor-style panels.",
            icon: IconGlyph::Brush,
            accent: Color::rgba(0.80, 0.22, 0.44, 1.0),
            child: WidgetPod::new(build_paint_demo_with_theme(Rc::clone(&theme_reader))),
        },
        DevDemo {
            title: VECTOR_EDITOR_TAB_LABEL,
            description: "Vector canvas drawing and editing demo.",
            icon: IconGlyph::ChevronRight,
            accent: Color::rgba(0.12, 0.56, 0.76, 1.0),
            child: WidgetPod::new(build_vector_editor_demo_with_theme(theme_reader)),
        },
    ]
}

pub(crate) fn dev_demo_label_for_slug(slug: &str) -> Option<&'static str> {
    match slug {
        "widget-book" | "widgets" => Some(WIDGET_BOOK_TAB_LABEL),
        "themes" | "theme" => Some(THEMES_TAB_LABEL),
        "animation" | "animations" | "animation-demo" => Some(ANIMATION_DEMO_TAB_LABEL),
        "retained-text" => Some(RETAINED_TEXT_TAB_LABEL),
        "text-comparison" | "comparison-surface" => Some(TEXT_RENDERING_COMPARISON_TAB_LABEL),
        "text-validation" => Some(TEXT_VALIDATION_TAB_LABEL),
        "text-editing" => Some(TEXT_EDITING_TAB_LABEL),
        #[cfg(feature = "markdown")]
        "markdown" | "markdown-render" | "markdown-renderer" => Some(MARKDOWN_RENDER_TAB_LABEL),
        "hdr-validation" | "color-validation" => Some(HDR_VALIDATION_TAB_LABEL),
        "layout" | "layouts" | "flex" => Some(LAYOUT_TAB_LABEL),
        "drag-drop" | "drag-and-drop" | "dnd" => Some(DRAG_DROP_TAB_LABEL),
        "paint" | "sui-paint" => Some(PAINT_TAB_LABEL),
        "vector-editor" | "vector" => Some(VECTOR_EDITOR_TAB_LABEL),
        _ => None,
    }
}

fn clamp_dev_shell_settings_bounds(host: Rect, bounds: Rect) -> Rect {
    if host.is_empty() {
        return Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width().max(DEV_SHELL_MIN_SETTINGS_WIDTH),
            bounds.height().max(DEV_SHELL_MIN_SETTINGS_HEIGHT),
        );
    }
    let width = bounds.width().clamp(
        DEV_SHELL_MIN_SETTINGS_WIDTH.min(host.width()),
        host.width().max(1.0),
    );
    let height = bounds.height().clamp(
        DEV_SHELL_MIN_SETTINGS_HEIGHT.min(host.height()),
        host.height().max(1.0),
    );
    let min_visible_width = width.min(64.0);
    let min_visible_height = DEV_SHELL_SETTINGS_TITLE_HEIGHT.min(height);
    let max_x = (host.max_x() - min_visible_width).max(host.x());
    let max_y = (host.max_y() - min_visible_height).max(host.y());
    Rect::new(
        bounds.x().clamp(host.x(), max_x),
        bounds.y().clamp(host.y(), max_y),
        width,
        height,
    )
}

fn close_icon_path(bounds: Rect) -> Path {
    let mut path = PathBuilder::new();
    let inset = bounds.width().min(bounds.height()) * 0.34;
    path.move_to(Point::new(bounds.x() + inset, bounds.y() + inset));
    path.line_to(Point::new(bounds.max_x() - inset, bounds.max_y() - inset));
    path.move_to(Point::new(bounds.max_x() - inset, bounds.y() + inset));
    path.line_to(Point::new(bounds.x() + inset, bounds.max_y() - inset));
    path.build()
}

fn diagonal_handle_path(bounds: Rect, inset: f32, offset: f32) -> Path {
    let mut path = PathBuilder::new();
    path.move_to(Point::new(bounds.max_x() - inset, bounds.max_y() - offset));
    path.line_to(Point::new(bounds.max_x() - offset, bounds.max_y() - inset));
    path.build()
}

fn top_rounded_rect_path(bounds: Rect, radius: f32) -> Path {
    let radius = radius
        .max(0.0)
        .min(bounds.width() * 0.5)
        .min(bounds.height());
    let kappa = 0.552_284_8;
    let x0 = bounds.x();
    let y0 = bounds.y();
    let x1 = bounds.max_x();
    let y1 = bounds.max_y();
    let r = radius;
    let c = r * kappa;
    let mut path = PathBuilder::new();
    path.move_to(Point::new(x0 + r, y0));
    path.line_to(Point::new(x1 - r, y0));
    path.cubic_to(
        Point::new(x1 - r + c, y0),
        Point::new(x1, y0 + r - c),
        Point::new(x1, y0 + r),
    );
    path.line_to(Point::new(x1, y1));
    path.line_to(Point::new(x0, y1));
    path.line_to(Point::new(x0, y0 + r));
    path.cubic_to(
        Point::new(x0, y0 + r - c),
        Point::new(x0 + r - c, y0),
        Point::new(x0 + r, y0),
    );
    path.close();
    path.build()
}

pub(crate) fn request_window_refresh(ctx: &mut EventCtx, include_ordering: bool) {
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

fn window_text_hinting_from_renderer(hinting: TextHinting) -> WindowTextHinting {
    match hinting.normalized() {
        TextHinting::None => WindowTextHinting::None,
        TextHinting::Slight { max_ppem } => WindowTextHinting::Slight { max_ppem },
    }
}

fn window_text_coverage_policy_from_renderer(
    policy: TextCoveragePolicy,
) -> WindowTextCoveragePolicy {
    match policy.normalized() {
        TextCoveragePolicy::Perceptual => WindowTextCoveragePolicy::Perceptual,
        TextCoveragePolicy::Linear => WindowTextCoveragePolicy::Linear,
        TextCoveragePolicy::Gamma(gamma) => WindowTextCoveragePolicy::Gamma(gamma),
        TextCoveragePolicy::CoverageBoost(amount) => {
            WindowTextCoveragePolicy::CoverageBoost(amount)
        }
        TextCoveragePolicy::TwoCoverageMinusCoverageSq => {
            WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq
        }
    }
}

fn text_coverage_policy_selected_index(policy: WindowTextCoveragePolicy) -> usize {
    match policy.normalized() {
        WindowTextCoveragePolicy::Perceptual => 0,
        WindowTextCoveragePolicy::Linear => 1,
        WindowTextCoveragePolicy::Gamma(_) => 2,
        WindowTextCoveragePolicy::CoverageBoost(_) => 3,
        WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq => 4,
    }
}

fn update_text_coverage_policy_selection(state: &mut WindowRenderOptions, index: usize) {
    state.text_coverage_policy = match index {
        0 => WindowTextCoveragePolicy::Perceptual,
        1 => WindowTextCoveragePolicy::Linear,
        2 => WindowTextCoveragePolicy::Gamma(match state.text_coverage_policy.normalized() {
            WindowTextCoveragePolicy::Gamma(gamma) => gamma,
            _ => 1.6,
        }),
        3 => {
            WindowTextCoveragePolicy::CoverageBoost(match state.text_coverage_policy.normalized() {
                WindowTextCoveragePolicy::CoverageBoost(amount) => amount,
                _ => 0.75,
            })
        }
        4 => WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq,
        _ => state.text_coverage_policy,
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

fn sdr_content_brightness_line(diagnostics: &WindowOutputDiagnostics) -> String {
    let source = if diagnostics.use_system_sdr_content_brightness
        && diagnostics.system_sdr_content_brightness_nits.is_some()
    {
        "system"
    } else if diagnostics.use_system_sdr_content_brightness {
        "manual fallback"
    } else {
        "manual"
    };
    let system = diagnostics
        .system_sdr_content_brightness_nits
        .map(|nits| format!("{nits:.0} nits"))
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "SDR content brightness: {:.0} nits ({source}; system {system}, manual {:.0} nits)",
        diagnostics.requested_sdr_content_brightness_nits,
        diagnostics.configured_sdr_content_brightness_nits,
    )
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
        lines.push(sdr_content_brightness_line(&diagnostics));
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
        sdr_content_brightness_line(&diagnostics),
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

pub(crate) fn labeled_settings_control<W>(
    theme_reader: DevThemeReader,
    label: &'static str,
    width: f32,
    control: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .control_width(width)
}

const SETTINGS_PANEL_MAX_WIDTH: f32 = 640.0;
const SETTINGS_PANEL_PADDING_X: f32 = 14.0;
const SETTINGS_PANEL_PADDING_TOP: f32 = 12.0;
const SETTINGS_PANEL_PADDING_BOTTOM: f32 = 12.0;
const SETTINGS_PANEL_TITLE_HEIGHT: f32 = 18.0;
const SETTINGS_PANEL_TITLE_GAP: f32 = 10.0;
const SETTINGS_PANEL_LINE_GAP: f32 = 3.0;

fn settings_panel_width(max_width: f32) -> f32 {
    if max_width.is_finite() {
        max_width.min(SETTINGS_PANEL_MAX_WIDTH).max(0.0)
    } else {
        SETTINGS_PANEL_MAX_WIDTH
    }
}

fn settings_panel_title_style(palette: ControlPalette) -> TextStyle {
    TextStyle {
        font_size: 14.0,
        line_height: SETTINGS_PANEL_TITLE_HEIGHT,
        color: palette.text,
        ..TextStyle::default()
    }
}

fn settings_panel_body_style(palette: ControlPalette) -> TextStyle {
    TextStyle {
        font_size: 11.0,
        line_height: 15.0,
        color: palette.text.with_alpha(0.9),
        ..TextStyle::default()
    }
}

fn settings_wrapped_text_height(
    ctx: &MeasureCtx,
    text: &str,
    style: &TextStyle,
    width: f32,
) -> f32 {
    ctx.layout()
        .shape_text(
            text.to_string(),
            Size::new(width.max(1.0), f32::INFINITY),
            style.clone(),
        )
        .map(|layout| layout.measurement().height.max(style.line_height))
        .unwrap_or(style.line_height)
}

fn settings_panel_height(
    ctx: &MeasureCtx,
    lines: &[String],
    width: f32,
    title_style: &TextStyle,
    body_style: &TextStyle,
) -> f32 {
    let text_width = (width - (SETTINGS_PANEL_PADDING_X * 2.0)).max(1.0);
    let body_height = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            settings_wrapped_text_height(ctx, line, body_style, text_width)
                + if index == 0 {
                    0.0
                } else {
                    SETTINGS_PANEL_LINE_GAP
                }
        })
        .sum::<f32>();

    SETTINGS_PANEL_PADDING_TOP
        + title_style.line_height
        + SETTINGS_PANEL_TITLE_GAP
        + body_height
        + SETTINGS_PANEL_PADDING_BOTTOM
}

fn paint_settings_panel(
    ctx: &mut PaintCtx,
    title: &str,
    lines: &[String],
    palette: ControlPalette,
) {
    let bounds = ctx.bounds();
    ctx.fill_rect(bounds, palette.surface.with_alpha(0.35));
    ctx.stroke_rect(
        bounds,
        palette.border.with_alpha(0.85),
        StrokeStyle::default(),
    );

    let text_x = bounds.x() + SETTINGS_PANEL_PADDING_X;
    let text_width = (bounds.width() - (SETTINGS_PANEL_PADDING_X * 2.0)).max(1.0);
    let title_style = settings_panel_title_style(palette);
    let body_style = settings_panel_body_style(palette);

    ctx.push_clip_rect(bounds);
    ctx.draw_text(
        Rect::new(
            text_x,
            bounds.y() + SETTINGS_PANEL_PADDING_TOP,
            text_width,
            title_style.line_height,
        ),
        title,
        title_style,
    );

    let mut y = bounds.y()
        + SETTINGS_PANEL_PADDING_TOP
        + SETTINGS_PANEL_TITLE_HEIGHT
        + SETTINGS_PANEL_TITLE_GAP;
    for line in lines {
        let line_height = ctx
            .shape_text(
                line.clone(),
                Size::new(text_width.max(1.0), f32::INFINITY),
                body_style.clone(),
            )
            .map(|layout| layout.measurement().height.max(body_style.line_height))
            .unwrap_or(body_style.line_height);
        ctx.draw_text(
            Rect::new(text_x, y, text_width, line_height),
            line.clone(),
            body_style.clone(),
        );
        y += line_height + SETTINGS_PANEL_LINE_GAP;
    }
    ctx.pop_clip();
}

struct HdrThemeInspectionPanel {
    theme_reader: DevThemeReader,
}

impl HdrThemeInspectionPanel {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self { theme_reader }
    }

    fn theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }
}

impl Widget for HdrThemeInspectionPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let palette = self.theme().palette;
        let width = settings_panel_width(constraints.max.width);
        let lines = hdr_theme_inspection_lines(ctx.window_id());
        let height = settings_panel_height(
            ctx,
            &lines,
            width,
            &settings_panel_title_style(palette),
            &settings_panel_body_style(palette),
        );
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme().palette;
        let lines = hdr_theme_inspection_lines(ctx.window_id());
        paint_settings_panel(ctx, HDR_THEME_INSPECTION_TITLE, &lines, palette);
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

struct OutputDiagnosticsPanel {
    theme_reader: DevThemeReader,
}

impl OutputDiagnosticsPanel {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self { theme_reader }
    }

    fn theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }
}

impl Widget for OutputDiagnosticsPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let palette = self.theme().palette;
        let width = settings_panel_width(constraints.max.width);
        let lines = output_diagnostics_lines(ctx.window_id());
        let height = settings_panel_height(
            ctx,
            &lines,
            width,
            &settings_panel_title_style(palette),
            &settings_panel_body_style(palette),
        );
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme().palette;
        let lines = output_diagnostics_lines(ctx.window_id());
        paint_settings_panel(ctx, OUTPUT_DIAGNOSTICS_TITLE, &lines, palette);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(OUTPUT_DIAGNOSTICS_TITLE.to_string());
        node.description = Some(output_diagnostics_lines(ctx.window_id()).join("\n"));
        ctx.push(node);
    }
}

struct SdrContentBrightnessStatus {
    theme_reader: DevThemeReader,
}

impl SdrContentBrightnessStatus {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self { theme_reader }
    }

    fn theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }
}

impl Widget for SdrContentBrightnessStatus {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text = window_output_diagnostics(ctx.window_id())
            .map(|diagnostics| sdr_content_brightness_line(&diagnostics))
            .unwrap_or_else(|| "SDR content brightness: waiting for first frame".to_string());
        let style = TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: self.theme().palette.text.with_alpha(0.78),
            ..TextStyle::default()
        };
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(420.0).max(0.0)
        } else {
            420.0
        };
        let height = settings_wrapped_text_height(ctx, &text, &style, width).max(34.0);
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme().palette;
        let text = window_output_diagnostics(ctx.window_id())
            .map(|diagnostics| sdr_content_brightness_line(&diagnostics))
            .unwrap_or_else(|| "SDR content brightness: waiting for first frame".to_string());
        ctx.draw_text(
            ctx.bounds(),
            text,
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: palette.text.with_alpha(0.78),
                ..TextStyle::default()
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let description = window_output_diagnostics(ctx.window_id())
            .map(|diagnostics| sdr_content_brightness_line(&diagnostics))
            .unwrap_or_else(|| "SDR content brightness waiting for first frame".to_string());
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(SDR_CONTENT_BRIGHTNESS_NAME.to_string());
        node.description = Some(description);
        ctx.push(node);
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
            spacing: 10.0,
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

struct RenderSettingsTab {
    content: SingleChild,
    state: Rc<RefCell<WindowRenderOptions>>,
    applied: Option<WindowRenderOptions>,
    last_hdr_theme_mode: HdrThemeMode,
}

impl RenderSettingsTab {
    fn default_options() -> WindowRenderOptions {
        let renderer = WgpuRenderer::new();
        apply_demo_small_text_rendering_profile(
            WindowRenderOptions::new(renderer.feathering_enabled(), renderer.feather_width())
                .with_text_hinting(window_text_hinting_from_renderer(renderer.text_hinting()))
                .with_text_coverage_policy(window_text_coverage_policy_from_renderer(
                    renderer.text_coverage_policy(),
                )),
        )
    }

    fn with_initial_options(
        initial: WindowRenderOptions,
        theme_reader: DevThemeReader,
        shell_state: DevShellState,
    ) -> Self {
        let state = Rc::new(RefCell::new(initial));
        let performance_overlay_state = shell_state.clone();
        let toggle_state = Rc::clone(&state);
        let width_state = Rc::clone(&state);
        let text_centering_state = Rc::clone(&state);
        let hinting_toggle_state = Rc::clone(&state);
        let hinting_max_ppem_state = Rc::clone(&state);
        let text_coverage_policy_state = Rc::clone(&state);
        let text_coverage_gamma_state = Rc::clone(&state);
        let stem_darkening_toggle_state = Rc::clone(&state);
        let stem_darkening_amount_state = Rc::clone(&state);
        let stem_darkening_max_ppem_state = Rc::clone(&state);
        let color_management_state = Rc::clone(&state);
        let output_primaries_state = Rc::clone(&state);
        let dynamic_range_state = Rc::clone(&state);
        let tone_mapping_state = Rc::clone(&state);
        let sdr_content_brightness_state = Rc::clone(&state);
        let system_sdr_content_brightness_state = Rc::clone(&state);
        let current_hdr_theme_mode = widget_book_hdr_theme_mode();
        let scroll_state = ScrollState::new();

        let content = VerticalScrollPane::new(
            ScrollView::vertical(Padding::all(
                28.0,
                Stack::vertical()
                    .spacing(18.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("Renderer settings")
                            .font_size(24.0)
                            .line_height(30.0)
                            .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                    )
                    .with_child(
                        Label::new(
                            "These controls update the active window's runtime presentation on the next redraw.",
                        )
                        .font_size(14.0)
                        .line_height(20.0)
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text_muted)),
                    )
                    .with_child(
                        Checkbox::new(LIVE_PERFORMANCE_OVERLAY_LABEL)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .checked(shell_state.performance_overlay_visible())
                            .on_toggle(move |checked| {
                                performance_overlay_state
                                    .set_performance_overlay_visible(checked);
                            }),
                    )
                    .with_child(
                        Checkbox::new(FEATHERING_TOGGLE_LABEL)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .checked(initial.feathering_enabled)
                            .on_toggle(move |checked| {
                                toggle_state.borrow_mut().feathering_enabled = checked;
                            }),
                    )
                    .with_child(
                        Checkbox::new(OPTICAL_TEXT_CENTERING_TOGGLE_LABEL)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .checked(initial.optical_vertical_text_alignment_enabled)
                            .on_toggle(move |checked| {
                                text_centering_state
                                    .borrow_mut()
                                    .optical_vertical_text_alignment_enabled = checked;
                            }),
                    )
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        FEATHER_WIDTH_NAME,
                        220.0,
                        NumberInput::new(FEATHER_WIDTH_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .range(0.0, 8.0)
                            .step(0.05)
                            .precision(2)
                            .value(initial.feather_width as f64)
                            .on_change(move |value| {
                                width_state.borrow_mut().feather_width = value.max(0.0) as f32;
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        TEXT_COVERAGE_POLICY_NAME,
                        240.0,
                        Select::new(TEXT_COVERAGE_POLICY_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(TEXT_COVERAGE_POLICY_OPTIONS)
                            .selected(text_coverage_policy_selected_index(
                                initial.text_coverage_policy,
                            ))
                            .on_change(move |index, _| {
                                let mut state = text_coverage_policy_state.borrow_mut();
                                update_text_coverage_policy_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        TEXT_COVERAGE_GAMMA_NAME,
                        220.0,
                        NumberInput::new(TEXT_COVERAGE_GAMMA_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .range(0.25, 4.0)
                            .step(0.05)
                            .precision(2)
                            .value(match initial.text_coverage_policy.normalized() {
                                WindowTextCoveragePolicy::Gamma(gamma) => gamma as f64,
                                _ => 1.6,
                            })
                            .on_change(move |value| {
                                text_coverage_gamma_state.borrow_mut().text_coverage_policy =
                                    WindowTextCoveragePolicy::Gamma(value.clamp(0.25, 4.0) as f32);
                            }),
                    ))
                    .with_child(
                        Checkbox::new(TEXT_HINTING_TOGGLE_LABEL)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .checked(!matches!(initial.text_hinting, WindowTextHinting::None))
                            .on_toggle(move |checked| {
                                let mut state = hinting_toggle_state.borrow_mut();
                                state.text_hinting = if checked {
                                    match state.text_hinting.normalized() {
                                        WindowTextHinting::Slight { max_ppem } => {
                                            WindowTextHinting::Slight { max_ppem }
                                        }
                                        WindowTextHinting::None => {
                                            WindowTextHinting::Slight {
                                                max_ppem: DEMO_TEXT_HINTING_MAX_PPEM_LIMIT,
                                            }
                                        }
                                    }
                                } else {
                                    WindowTextHinting::None
                                };
                            }),
                    )
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        TEXT_HINTING_MAX_PPEM_NAME,
                        220.0,
                        NumberInput::new(TEXT_HINTING_MAX_PPEM_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .range(1.0, DEMO_TEXT_HINTING_MAX_PPEM_LIMIT as f64)
                            .step(0.5)
                            .precision(1)
                            .value(match initial.text_hinting.normalized() {
                                WindowTextHinting::Slight { max_ppem } => max_ppem as f64,
                                WindowTextHinting::None => DEMO_TEXT_HINTING_MAX_PPEM_LIMIT as f64,
                            })
                            .on_change(move |value| {
                                let max_ppem =
                                    value.clamp(1.0, DEMO_TEXT_HINTING_MAX_PPEM_LIMIT as f64) as f32;
                                hinting_max_ppem_state.borrow_mut().text_hinting =
                                    WindowTextHinting::Slight { max_ppem };
                            }),
                    ))
                    .with_child(
                        Checkbox::new(STEM_DARKENING_TOGGLE_LABEL)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
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
                                                max_ppem: DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM,
                                                amount: DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT,
                                            }
                                        }
                                    }
                                } else {
                                    WindowStemDarkening::None
                                };
                            }),
                    )
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        STEM_DARKENING_AMOUNT_NAME,
                        220.0,
                        NumberInput::new(STEM_DARKENING_AMOUNT_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .range(0.0, 1.0)
                            .step(0.01)
                            .precision(2)
                            .value(match initial.stem_darkening.normalized() {
                                WindowStemDarkening::Enabled { amount, .. } => amount as f64,
                                WindowStemDarkening::None => {
                                    DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT as f64
                                }
                            })
                            .on_change(move |value| {
                                let amount = value.clamp(0.0, 1.0) as f32;
                                let max_ppem = match stem_darkening_amount_state
                                    .borrow()
                                    .stem_darkening
                                    .normalized()
                                {
                                    WindowStemDarkening::Enabled { max_ppem, .. } => max_ppem,
                                    WindowStemDarkening::None => {
                                        DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM
                                    }
                                };
                                stem_darkening_amount_state.borrow_mut().stem_darkening =
                                    WindowStemDarkening::Enabled { max_ppem, amount };
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        STEM_DARKENING_MAX_PPEM_NAME,
                        220.0,
                        NumberInput::new(STEM_DARKENING_MAX_PPEM_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .range(1.0, 64.0)
                            .step(0.5)
                            .precision(1)
                            .value(match initial.stem_darkening.normalized() {
                                WindowStemDarkening::Enabled { max_ppem, .. } => max_ppem as f64,
                                WindowStemDarkening::None => {
                                    DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM as f64
                                }
                            })
                            .on_change(move |value| {
                                let max_ppem = value.clamp(1.0, 64.0) as f32;
                                let amount = match stem_darkening_max_ppem_state
                                    .borrow()
                                    .stem_darkening
                                    .normalized()
                                {
                                    WindowStemDarkening::Enabled { amount, .. } => amount,
                                    WindowStemDarkening::None => {
                                        DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT
                                    }
                                };
                                stem_darkening_max_ppem_state.borrow_mut().stem_darkening =
                                    WindowStemDarkening::Enabled { max_ppem, amount };
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        COLOR_MANAGEMENT_MODE_NAME,
                        280.0,
                        Select::new(COLOR_MANAGEMENT_MODE_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(COLOR_MANAGEMENT_MODE_OPTIONS)
                            .selected(color_management_mode_selected_index(initial.color_management_mode))
                            .on_change(move |index, _| {
                                let mut state = color_management_state.borrow_mut();
                                update_color_management_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        OUTPUT_PRIMARIES_NAME,
                        240.0,
                        Select::new(OUTPUT_PRIMARIES_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(OUTPUT_PRIMARIES_OPTIONS)
                            .selected(output_primaries_selected_index(initial.output_color_primaries))
                            .on_change(move |index, _| {
                                let mut state = output_primaries_state.borrow_mut();
                                update_output_primaries_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        DYNAMIC_RANGE_MODE_NAME,
                        240.0,
                        Select::new(DYNAMIC_RANGE_MODE_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(DYNAMIC_RANGE_MODE_OPTIONS)
                            .selected(dynamic_range_mode_selected_index(initial.dynamic_range_mode))
                            .on_change(move |index, _| {
                                let mut state = dynamic_range_state.borrow_mut();
                                update_dynamic_range_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        TONE_MAPPING_MODE_NAME,
                        240.0,
                        Select::new(TONE_MAPPING_MODE_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(TONE_MAPPING_MODE_OPTIONS)
                            .selected(tone_mapping_mode_selected_index(initial.tone_mapping_mode))
                            .on_change(move |index, _| {
                                let mut state = tone_mapping_state.borrow_mut();
                                update_tone_mapping_mode_selection(&mut state, index);
                            }),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        SDR_CONTENT_BRIGHTNESS_NAME,
                        420.0,
                        Stack::vertical()
                            .spacing(8.0)
                            .alignment(Alignment::Start)
                            .with_child(SizedBox::new().width(220.0).with_child(
                                NumberInput::new(SDR_CONTENT_BRIGHTNESS_NAME)
                                    .theme_when(clone_dev_theme_reader(&theme_reader))
                                    .range(48.0, 1000.0)
                                    .step(1.0)
                                    .precision(0)
                                    .value(initial.sdr_content_brightness_nits as f64)
                                    .on_change(move |value| {
                                        sdr_content_brightness_state
                                            .borrow_mut()
                                            .sdr_content_brightness_nits =
                                            value.clamp(48.0, 1000.0) as f32;
                                    }),
                            ))
                            .with_child(
                                Checkbox::new(USE_SYSTEM_SDR_BRIGHTNESS_LABEL)
                                    .theme_when(clone_dev_theme_reader(&theme_reader))
                                    .checked(initial.use_system_sdr_content_brightness)
                                    .on_toggle(move |checked| {
                                        system_sdr_content_brightness_state
                                            .borrow_mut()
                                            .use_system_sdr_content_brightness = checked;
                                    }),
                            )
                            .with_child(SdrContentBrightnessStatus::new(Rc::clone(&theme_reader))),
                    ))
                    .with_child(labeled_settings_control(
                        Rc::clone(&theme_reader),
                        HDR_THEME_MODE_NAME,
                        280.0,
                        Select::new(HDR_THEME_MODE_NAME)
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .options(HDR_THEME_MODE_OPTIONS)
                            .selected(hdr_theme_mode_selected_index(current_hdr_theme_mode))
                            .on_change(move |index, _| {
                                set_widget_book_hdr_theme_mode(hdr_theme_mode_from_index(index));
                            }),
                    ))
                    .with_child(HdrThemeInspectionPanel::new(Rc::clone(&theme_reader)))
                    .with_child(OutputDiagnosticsPanel::new(Rc::clone(&theme_reader)))
                    .with_child(
                        Label::new(
                            "Optical centering uses cap height when available and a softened descent bias for Latin UI labels. Atlas glyphs are always snapped to physical pixels; fractional glyph phase is handled by quarter-pixel raster variants. The default perceptual text coverage policy applies a luminance-aware coverage curve to atlas and fallback glyph coverage; changing the gamma input selects and updates the Gamma policy. Slight hinting biases small-text rasterization below the configured ppem threshold. Stem darkening slightly boosts thin small-text coverage below its threshold. Phase 2 controls choose the preferred color-management policy, the HDR theme selector drives the shared widget-book preview mode, and the inspection panels show the detected monitor/output path after each redraw.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text_muted)),
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

fn build_render_settings_tab_with_options(
    options: WindowRenderOptions,
    theme_reader: DevThemeReader,
    shell_state: DevShellState,
) -> impl Widget {
    RenderSettingsTab::with_initial_options(options, theme_reader, shell_state)
}

pub(crate) fn build_dev_application_with_widget_book_bounds_and_render_options(
    _widget_book_bounds: Rect,
    render_options: WindowRenderOptions,
) -> Application {
    let shell = DevBrowserShell::new(render_options);
    let performance_overlay_reader = shell.performance_overlay_reader();
    finish_dev_application_with_performance_overlay_reader(shell, performance_overlay_reader)
        .with_window_render_options(render_options)
}

pub(crate) fn build_dev_application_with_initial_demo_and_render_options(
    initial_demo: Option<&str>,
    render_options: WindowRenderOptions,
) -> Application {
    let shell = DevBrowserShell::with_initial_demo(render_options, initial_demo);
    let performance_overlay_reader = shell.performance_overlay_reader();
    finish_dev_application_with_performance_overlay_reader(shell, performance_overlay_reader)
        .with_window_render_options(render_options)
}

#[cfg(not(target_arch = "wasm32"))]
fn build_dev_application_with_render_options_and_automation(
    render_options: WindowRenderOptions,
    automation: Option<DesktopAutomationMode>,
) -> Application {
    let initial_demo = automation.map(|mode| match mode {
        DesktopAutomationMode::WidgetBookScroll => WIDGET_BOOK_TAB_LABEL,
    });
    let shell = DevBrowserShell::with_initial_demo(render_options, initial_demo);
    let performance_overlay_reader = shell.performance_overlay_reader();
    finish_dev_application_with_performance_overlay_reader(shell, performance_overlay_reader)
        .with_window_render_options(render_options)
}

#[cfg(test)]
fn finish_dev_application<W: Widget + 'static>(root: W) -> Application {
    let mut app = App::new();
    {
        let mut resources = app.resources();
        register_dev_application_resources(&mut resources);
    }

    app.window(Window::new(WINDOW_TITLE).root(LivePerformanceRoot::new(
        WINDOW_TITLE,
        WINDOW_DESCRIPTION,
        root,
    )))
    .into_application()
}

fn finish_dev_application_with_performance_overlay_reader<W: Widget + 'static>(
    root: W,
    performance_overlay_reader: Rc<dyn Fn() -> bool>,
) -> Application {
    let mut app = App::new();
    {
        let mut resources = app.resources();
        register_dev_application_resources(&mut resources);
    }

    app.window(
        Window::new(WINDOW_TITLE).root(
            LivePerformanceRoot::new(WINDOW_TITLE, WINDOW_DESCRIPTION, root)
                .performance_overlay_enabled_when(move || performance_overlay_reader()),
        ),
    )
    .into_application()
}

fn register_dev_application_resources(resources: &mut sui::ResourceRegistry<'_>) {
    register_widget_book_images(resources);
    #[cfg(target_arch = "wasm32")]
    register_dev_web_fallback_fonts(resources);
    resources
        .image(
            DEV_SHELL_LOGO_IMAGE_HANDLE,
            default_sui_logo_image(DEV_SHELL_LOGO_IMAGE_SIZE)
                .expect("default SUI logo SVG should rasterize for dev shell"),
        )
        .expect("dev shell logo SVG should register exactly once");
}

#[cfg(target_arch = "wasm32")]
fn register_dev_web_fallback_fonts(resources: &mut sui::ResourceRegistry<'_>) {
    for (name, font) in DEV_WEB_FALLBACK_FONTS {
        resources
            .font_bytes(font.to_vec())
            .unwrap_or_else(|error| panic!("{name} fallback font should register: {error}"));
    }
}

pub fn build_dev_application_with_widget_book_bounds(widget_book_bounds: Rect) -> Application {
    build_dev_application_with_widget_book_bounds_and_render_options(
        widget_book_bounds,
        RenderSettingsTab::default_options(),
    )
}

pub fn build_dev_application() -> Application {
    build_dev_application_with_widget_book_bounds(Rect::new(24.0, 24.0, 680.0, 760.0))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn build_dev_application_with_automation(
    automation: Option<DesktopAutomationMode>,
) -> Application {
    build_dev_application_with_render_options_and_automation(
        RenderSettingsTab::default_options(),
        automation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::paint_demo::{
        PAINT_ACTUAL_SIZE_NAME, PAINT_BLEND_MODE_NAME, PAINT_BRUSH_COLOR_NAME,
        PAINT_BRUSH_OPACITY_NAME, PAINT_BRUSH_PREVIEW_NAME, PAINT_BRUSH_SHAPE_NAME,
        PAINT_BRUSH_SIZE_NAME, PAINT_BRUSH_SIZE_PRESETS_NAME, PAINT_COLOR_EDITOR_NAME,
        PAINT_COLOR_PRESETS_NAME, PAINT_DOCUMENT_BAR_NAME, PAINT_DOCUMENT_COMMANDS_NAME,
        PAINT_DOCUMENT_HEIGHT, PAINT_DOCUMENT_NAME, PAINT_DOCUMENT_VIEW_COMMANDS_NAME,
        PAINT_DOCUMENT_WIDTH, PAINT_ERASER_PREVIEW_NAME, PAINT_FILL_BLEND_MODE_NAME,
        PAINT_FILL_OPACITY_NAME, PAINT_FIT_VIEW_NAME, PAINT_HISTORY_COMMANDS_NAME,
        PAINT_HORIZONTAL_RULER_NAME, PAINT_INITIAL_BRUSH_SIZE, PAINT_LAYER_BLEND_MODE_NAME,
        PAINT_LAYER_OPACITY_NAME, PAINT_LAYERS_NAME, PAINT_PROPERTIES_NAME, PAINT_SCROLL_NAME,
        PAINT_SELECT_LAYER_ABOVE_NAME, PAINT_SELECT_LAYER_BELOW_NAME, PAINT_VERTICAL_RULER_NAME,
        PAINT_VIEW_COMMANDS_NAME, PAINT_ZOOM_IN_NAME, PAINT_ZOOM_OUT_NAME, PAINT_ZOOM_READOUT_NAME,
        build_paint_demo_with_state,
    };

    use std::{
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use sui::{
        Brush, Event, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, RenderOutput, Result, Runtime, SceneCommand,
        SceneStatisticsDetailMode, ScrollDelta, SemanticsNode, SemanticsRole, StackOrderPolicy,
        Vector, WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent,
        WindowOutputColorPrimaries, WindowPerformanceSnapshot, WindowRenderOptions,
        WindowToneMappingMode, set_window_scene_statistics_detail_mode,
        window_performance_snapshot, window_scene_statistics_detail_mode,
    };
    use sui_render_wgpu::{
        DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
        DebugSdrVisualization,
    };
    use sui_testing::{
        Screenshot, TestApp, TestWindow, WindowSnapshot, hdr_clip_mask, hdr_headroom_heatmap,
        hdr_luminance_heatmap, write_hdr_exr,
    };

    const FRONTING_TEST_TITLE: &str = "Fronting test";

    #[test]
    fn dev_web_fallback_fonts_shape_cjk_and_emoji_samples() {
        let cjk_handle = sui::FontHandle::new(41_001);
        let emoji_handle = sui::FontHandle::new(41_002);
        let mut fonts = sui::FontRegistry::new();
        fonts.insert(
            cjk_handle,
            sui::RegisteredFont::from_bytes(DEV_WEB_FALLBACK_FONTS[0].1.to_vec()),
        );
        fonts.insert(
            emoji_handle,
            sui::RegisteredFont::from_bytes(DEV_WEB_FALLBACK_FONTS[1].1.to_vec()),
        );

        let text_system = sui_text::TextSystem::new();
        let cjk_layout = text_system
            .shape_text(
                "你好 日本語 한국어",
                sui::Size::new(360.0, 32.0),
                sui::TextStyle {
                    font: Some(cjk_handle),
                    ..sui::TextStyle::new(sui::Color::BLACK)
                },
                &fonts,
            )
            .expect("CJK fallback font should shape markdown sample text");
        let emoji_layout = text_system
            .shape_text(
                "🙂 ✅ 🎨",
                sui::Size::new(180.0, 32.0),
                sui::TextStyle {
                    font: Some(emoji_handle),
                    ..sui::TextStyle::new(sui::Color::BLACK)
                },
                &fonts,
            )
            .expect("emoji fallback font should shape markdown sample text");

        for glyph in cjk_layout
            .glyphs()
            .iter()
            .chain(emoji_layout.glyphs().iter())
        {
            assert_ne!(glyph.glyph_id, 0, "fallback font emitted missing glyph");
        }
    }

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

    #[test]
    fn dev_demo_defaults_enable_small_text_quality_profile() {
        let options = RenderSettingsTab::default_options();

        assert!(matches!(
            options.text_hinting.normalized(),
            WindowTextHinting::Slight { max_ppem }
                if (max_ppem - DEMO_TEXT_HINTING_MAX_PPEM_LIMIT).abs() < f32::EPSILON
        ));
        assert!(matches!(
            options.stem_darkening.normalized(),
            WindowStemDarkening::Enabled { max_ppem, amount }
                if (max_ppem - DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM).abs() < f32::EPSILON
                    && (amount - DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT).abs() < f32::EPSILON
        ));
        assert_eq!(
            options.text_coverage_policy.normalized(),
            WindowTextCoveragePolicy::Linear
        );
    }

    #[test]
    fn dev_application_attaches_default_render_options() {
        let app = build_dev_application();
        let options = app
            .initial_window_render_options()
            .expect("dev application should publish initial render options");

        assert!(matches!(
            options.stem_darkening.normalized(),
            WindowStemDarkening::Enabled { max_ppem, amount }
                if (max_ppem - DEMO_SMALL_TEXT_STEM_DARKENING_MAX_PPEM).abs() < f32::EPSILON
                    && (amount - DEMO_SMALL_TEXT_STEM_DARKENING_AMOUNT).abs() < f32::EPSILON
        ));
    }

    fn primary_pointer_event(
        pointer_id: u64,
        kind: PointerEventKind,
        position: Point,
        pressed: bool,
    ) -> Event {
        let mut event = PointerEvent::new(kind, position);
        event.pointer_id = pointer_id;
        event.button = Some(PointerButton::Primary);
        if pressed {
            event.buttons = PointerButtons::new(1);
        }
        Event::Pointer(event)
    }

    fn drag_primary_pointer(
        runtime: &mut Runtime,
        window_id: WindowId,
        pointer_id: u64,
        from: Point,
        to: Point,
    ) -> Result<()> {
        runtime.handle_event(
            window_id,
            primary_pointer_event(pointer_id, PointerEventKind::Move, from, false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(pointer_id, PointerEventKind::Down, from, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(pointer_id, PointerEventKind::Move, to, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(pointer_id, PointerEventKind::Up, to, false),
        )
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
        let mut views = FloatingWorkspace::new(workspace).name(FRONTING_TEST_TITLE);
        views.push_view(
            FloatingViewConfig::new("First", Rect::new(24.0, 48.0, 320.0, 240.0))
                .min_size(Size::new(220.0, 160.0)),
            SolidFill::new(Color::rgba(0.86, 0.22, 0.18, 1.0)),
        );
        views.push_view(
            FloatingViewConfig::new("Second", Rect::new(220.0, 88.0, 320.0, 240.0))
                .min_size(Size::new(220.0, 160.0)),
            SolidFill::new(Color::rgba(0.16, 0.62, 0.28, 1.0)),
        );

        Application::new().window(
            WindowBuilder::new().title(FRONTING_TEST_TITLE).root(
                LivePerformanceRoot::new(
                    FRONTING_TEST_TITLE,
                    "Floating workspace fronting regression.",
                    views,
                )
                .show_performance_overlay(),
            ),
        )
    }

    fn build_floating_widget_book_test_application(widget_book_bounds: Rect) -> Application {
        set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);
        let workspace = FloatingWorkspaceState::new();
        let mut views = FloatingWorkspace::new(workspace).name("Widget book floating regression");
        views.push_view(
            FloatingViewConfig::new(WIDGET_BOOK_TAB_LABEL, widget_book_bounds)
                .min_size(Size::new(420.0, 320.0)),
            build_widget_book_gallery(default_widget_book_state()),
        );
        finish_dev_application(views)
    }

    fn build_floating_color_imagery_test_application(widget_book_bounds: Rect) -> Application {
        const WIDGET_BOOK_TEST_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);

        set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);
        let workspace = FloatingWorkspaceState::new();
        let mut views = FloatingWorkspace::new(workspace).name("Widget book floating regression");
        let gallery = ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    SizedBox::new()
                        .height(580.0)
                        .with_child(Label::new("Color imagery scroll spacer")),
                )
                .with_child(
                    Stack::vertical()
                        .spacing(16.0)
                        .alignment(Alignment::Start)
                        .with_child(
                            ColorSwatch::new(
                                crate::widget_book::COLOR_SWATCH_NAME,
                                Color::rgba(0.12, 0.55, 0.88, 1.0),
                            )
                            .size(Size::new(64.0, 36.0)),
                        )
                        .with_child(
                            SizedBox::new().width(220.0).height(220.0).with_child(
                                Image::new(WIDGET_BOOK_TEST_IMAGE_HANDLE)
                                    .label(crate::widget_book::DEMO_IMAGE_LABEL)
                                    .fit(ImageFit::Contain)
                                    .background(Color::rgba(0.92, 0.95, 0.98, 1.0))
                                    .corner_radius(12.0),
                            ),
                        ),
                )
                .with_child(
                    SizedBox::new()
                        .height(520.0)
                        .with_child(Label::new("Color imagery trailing spacer")),
                ),
        ))
        .name(crate::widget_book::GALLERY_SCROLL_NAME);
        views.push_view(
            FloatingViewConfig::new(WIDGET_BOOK_TAB_LABEL, widget_book_bounds)
                .min_size(Size::new(420.0, 320.0)),
            gallery,
        );
        finish_dev_application(views)
    }

    fn rects_overlap(a: Rect, b: Rect) -> bool {
        a.x() < b.max_x() && a.max_x() > b.x() && a.y() < b.max_y() && a.max_y() > b.y()
    }

    fn solid_stroke_colors_in_rect(output: &RenderOutput, region: Rect) -> Vec<Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect {
                    rect,
                    brush: Brush::Solid(color),
                    ..
                } if rects_overlap(*rect, region) => colors.push(*color),
                SceneCommand::StrokePath {
                    path,
                    brush: Brush::Solid(color),
                    ..
                } if rects_overlap(path.bounds(), region) => colors.push(*color),
                _ => {}
            });
        colors
    }

    fn solid_fill_bounds_for_color_in_rect(
        output: &RenderOutput,
        expected: Color,
        region: Rect,
    ) -> Vec<Rect> {
        let mut rects = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    rect,
                    brush: Brush::Solid(color),
                } if *color == expected && rects_overlap(*rect, region) => rects.push(*rect),
                SceneCommand::FillPath {
                    path,
                    brush: Brush::Solid(color),
                } if *color == expected && rects_overlap(path.bounds(), region) => {
                    rects.push(path.bounds());
                }
                _ => {}
            });
        rects
    }

    #[test]
    fn widget_book_scroll_does_not_repaint_pixels_outside_shrunken_floating_view() -> Result<()> {
        let initial_bounds = Rect::new(320.0, 28.0, 560.0, 520.0);
        let app = TestApp::new(move || {
            build_floating_widget_book_test_application(initial_bounds).build()
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
            .with_name(crate::widget_book::GALLERY_SCROLL_NAME);

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
            build_floating_color_imagery_test_application(initial_bounds).build()
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
            crate::widget_book::DEMO_IMAGE_LABEL,
        )?;
        assert_story_exit_does_not_repaint_outside_view(
            &window,
            SemanticsRole::ColorSwatch,
            crate::widget_book::COLOR_SWATCH_NAME,
        )?;

        Ok(())
    }

    #[test]
    fn settings_view_scrolls_without_repainting_outside_its_floating_bounds() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_settings(&window)?;

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
        open_dev_shell_demo(&window, HDR_VALIDATION_TAB_LABEL)?;
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(crate::widget_book::COLOR_VALIDATION_SCROLL_NAME)
            .expect()
            .to_be_visible()?;
        Ok(())
    }

    #[test]
    fn dev_workspace_omits_live_performance_overlay_by_default() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("dev application should render for overlay semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");

        assert!(
            semantics
                .iter()
                .all(|node| { node.name.as_deref() != Some("Live performance overlay") })
        );
    }

    #[test]
    fn dev_workspace_uses_picker_buttons_without_sidebar() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("dev application should render for picker semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");

        assert!(
            semantics.iter().all(|node| {
                !(node.role == SemanticsRole::List
                    && node.name.as_deref() == Some("Available views"))
            }),
            "expected the browser-style dev shell to omit the legacy sidebar"
        );
        for button in [
            WIDGET_BOOK_TAB_LABEL,
            THEMES_TAB_LABEL,
            HDR_VALIDATION_TAB_LABEL,
            LAYOUT_TAB_LABEL,
            DRAG_DROP_TAB_LABEL,
            PAINT_TAB_LABEL,
            VECTOR_EDITOR_TAB_LABEL,
        ] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(button)
                }),
                "expected the demo picker to expose {button:?} as a button"
            );
        }
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open demo")
            }),
            "expected the tab zone to expose the demo picker + button"
        );
    }

    #[test]
    fn dev_workspace_picker_exposes_scroll_bar_when_height_is_constrained() -> Result<()> {
        let mut runtime = build_dev_application().build()?;
        let window_id = runtime.window_ids()[0];
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(760.0, 360.0))),
        )?;

        let output = runtime.render(window_id)?;
        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(DEV_SHELL_PICKER_SCROLL_NAME)
            })
            .expect("demo picker scroll view should be present");
        let scroll_bar = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(DEV_SHELL_PICKER_SCROLL_BAR_NAME)
            })
            .expect("demo picker scroll bar should be present");

        let max = match &scroll_bar.value {
            Some(SemanticsValue::Range { max, .. }) => *max,
            _ => panic!("demo picker scroll bar should expose range semantics"),
        };
        assert!(
            max > 0.0,
            "expected constrained demo picker to overflow vertically"
        );
        assert!(
            scroll_bar.bounds.x() >= scroll.bounds.max_x(),
            "expected picker scroll bar to sit to the right of the scroll view"
        );

        Ok(())
    }

    #[test]
    fn dev_shell_theme_toggle_cycles_light_dark_high_contrast() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];

        let output = runtime
            .render(window_id)
            .expect("dev application should render");
        let toggle = assert_theme_toggle_state(
            &output.semantics,
            ThemeColorScheme::Light,
            ToggleState::Unchecked,
        );

        click_runtime_point(&mut runtime, window_id, center_of(toggle.bounds));
        let output = runtime
            .render(window_id)
            .expect("dev application should render dark theme");
        let toggle = assert_theme_toggle_state(
            &output.semantics,
            ThemeColorScheme::Dark,
            ToggleState::Checked,
        );

        click_runtime_point(&mut runtime, window_id, center_of(toggle.bounds));
        let output = runtime
            .render(window_id)
            .expect("dev application should render true black theme");
        let toggle = assert_theme_toggle_state(
            &output.semantics,
            ThemeColorScheme::HighContrast,
            ToggleState::Mixed,
        );

        click_runtime_point(&mut runtime, window_id, center_of(toggle.bounds));
        let output = runtime
            .render(window_id)
            .expect("dev application should render light theme");
        assert_theme_toggle_state(
            &output.semantics,
            ThemeColorScheme::Light,
            ToggleState::Unchecked,
        );
    }

    #[test]
    fn dev_shell_top_tabs_animate_selection_indicator() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        let switch_duration = DefaultTheme::default().motion.tab_switch_duration();

        let output = runtime
            .render(window_id)
            .expect("dev application should render picker");
        let widget_book_card = find_picker_button(&output.semantics, WIDGET_BOOK_TAB_LABEL);
        click_runtime_point(&mut runtime, window_id, center_of(widget_book_card.bounds));

        let output = runtime
            .render(window_id)
            .expect("dev application should render widget book tab");
        let open_demo = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open demo")
            })
            .expect("open demo button should remain in the tab strip")
            .clone();
        click_runtime_point(&mut runtime, window_id, center_of(open_demo.bounds));

        let output = runtime
            .render(window_id)
            .expect("dev application should render picker again");
        let themes_card = find_picker_button(&output.semantics, THEMES_TAB_LABEL);
        click_runtime_point(&mut runtime, window_id, center_of(themes_card.bounds));

        let output = runtime
            .render(window_id)
            .expect("dev application should render two top tabs");
        let from =
            top_tab_indicator_rect(find_top_tab_button(&output.semantics, THEMES_TAB_LABEL).bounds);
        let to = top_tab_indicator_rect(
            find_top_tab_button(&output.semantics, WIDGET_BOOK_TAB_LABEL).bounds,
        );
        let widget_book_tab = find_top_tab_button(&output.semantics, WIDGET_BOOK_TAB_LABEL);
        click_runtime_point(&mut runtime, window_id, center_of(widget_book_tab.bounds));

        runtime.tick(switch_duration * 0.5);
        let ready_events = dispatch_ready_events(&mut runtime);
        assert!(
            ready_events >= 1,
            "tab switch should schedule at least one animation frame"
        );
        let mid = runtime
            .render(window_id)
            .expect("dev application should render mid tab animation");
        let mid_indicator = dev_shell_indicator_rect(&mid);
        assert!(
            mid_indicator.x() > to.x() && mid_indicator.x() < from.x(),
            "mid-animation indicator should sit between target and previous tabs: from={from:?}, to={to:?}, mid={mid_indicator:?}"
        );

        runtime.tick(switch_duration);
        dispatch_ready_events(&mut runtime);
        let settled = runtime
            .render(window_id)
            .expect("dev application should render settled tab animation");
        let settled_indicator = dev_shell_indicator_rect(&settled);
        assert!(
            (settled_indicator.x() - to.x()).abs() < 0.01,
            "settled indicator should land under the selected tab: to={to:?}, settled={settled_indicator:?}"
        );
        assert!(
            (settled_indicator.width() - to.width()).abs() < 0.01,
            "settled indicator should match the selected tab width: to={to:?}, settled={settled_indicator:?}"
        );
    }

    #[test]
    fn dev_shell_picker_card_transient_state_clears_after_opening_and_returning() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];

        let output = runtime
            .render(window_id)
            .expect("dev application should render picker");
        let card = find_picker_button(&output.semantics, WIDGET_BOOK_TAB_LABEL);
        click_runtime_point(&mut runtime, window_id, center_of(card.bounds));

        runtime
            .render(window_id)
            .expect("dev application should render opened tab");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");
        let open_demo = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open demo")
            })
            .expect("open demo button should remain in the tab strip");
        let open_demo_bounds = open_demo.bounds;
        click_runtime_point(&mut runtime, window_id, center_of(open_demo_bounds));

        runtime
            .render(window_id)
            .expect("dev application should render picker again");
        settle_runtime_animations(&mut runtime, window_id);
        let output = runtime
            .render(window_id)
            .expect("dev application should render settled picker");
        let card = find_picker_button(&output.semantics, WIDGET_BOOK_TAB_LABEL);
        assert!(
            !card.state.hovered,
            "picker card hover state should clear while the card is hidden after activation"
        );
        assert!(
            !card.state.focused,
            "picker card focus state should clear while the card is hidden after activation"
        );
        assert!(
            !contains_approx_color(
                &solid_stroke_colors_in_rect(&output, card.bounds),
                DefaultTheme::default().palette.focus_ring,
            ),
            "picker card focus glow should clear while the card is hidden after activation"
        );

        click_runtime_point(&mut runtime, window_id, center_of(card.bounds));
        runtime
            .render(window_id)
            .expect("dev application should render reopened tab");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");
        let close_tab = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Close Widget book tab")
            })
            .expect("close-tab button should be available for the opened tab");
        let close_tab_bounds = close_tab.bounds;
        click_runtime_point(&mut runtime, window_id, center_of(close_tab_bounds));

        runtime
            .render(window_id)
            .expect("dev application should render picker after closing the only tab");
        settle_runtime_animations(&mut runtime, window_id);
        let output = runtime
            .render(window_id)
            .expect("dev application should render settled picker after closing the only tab");
        let card = find_picker_button(&output.semantics, WIDGET_BOOK_TAB_LABEL);
        assert!(
            !card.state.hovered,
            "picker card hover state should stay clear after returning by closing all tabs"
        );
        assert!(
            !card.state.focused,
            "picker card focus state should stay clear after returning by closing all tabs"
        );
        assert!(
            !contains_approx_color(
                &solid_stroke_colors_in_rect(&output, card.bounds),
                DefaultTheme::default().palette.focus_ring,
            ),
            "picker card focus glow should stay clear after returning by closing all tabs"
        );
    }

    #[test]
    fn dev_shell_active_tab_chrome_stays_neutral() {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            WindowRenderOptions::new(true, 1.0),
            Some(WIDGET_BOOK_TAB_LABEL),
        ))
        .build()
        .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("dev application should render");
        let theme = DefaultTheme::default();
        let toolbar = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            DEV_SHELL_TOOLBAR_HEIGHT,
        );

        assert!(
            !solid_stroke_colors_in_rect(&output, toolbar).contains(&theme.palette.border_focus),
            "active dev shell tab should not paint the focus border color"
        );
        assert!(
            !solid_stroke_colors_in_rect(&output, toolbar).contains(&theme.palette.focus_ring),
            "unfocused active dev shell tab should not paint a focus ring"
        );

        let active_tab = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(WIDGET_BOOK_TAB_LABEL)
            })
            .expect("active dev shell tab semantics should exist");
        let position = Point::new(
            active_tab.bounds.x() + active_tab.bounds.width() * 0.5,
            active_tab.bounds.y() + active_tab.bounds.height() * 0.5,
        );
        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("tab focus pointer event should be handled");
        settle_runtime_animations(&mut runtime, window_id);

        let focused = runtime
            .render(window_id)
            .expect("focused dev application should render");
        let focused_strokes = solid_stroke_colors_in_rect(&focused, toolbar);
        assert!(
            focused_strokes.contains(&theme.palette.focus_ring),
            "focused active dev shell tab should paint a focus ring; strokes={focused_strokes:?}"
        );
        assert!(
            !focused_strokes.contains(&theme.palette.border_focus),
            "focused active dev shell tab should still avoid selection border focus color"
        );
    }

    #[test]
    fn dev_shell_main_menu_uses_compact_toolbar_metrics() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("dev application should render for toolbar semantics");
        let semantics = runtime
            .semantics(window_id)
            .expect("dev application semantics should exist");

        let menu_button = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("SUI menu")
            })
            .expect("expected compact SUI menu trigger");
        let open_demo = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Open demo")
            })
            .expect("expected compact open-demo button");

        assert_eq!(menu_button.bounds.height(), DEV_SHELL_LOGO_BUTTON_SIZE);
        assert_eq!(open_demo.bounds.height(), DEV_SHELL_PLUS_BUTTON_SIZE);
        assert!(menu_button.bounds.max_y() <= DEV_SHELL_TOOLBAR_HEIGHT);
        assert!(open_demo.bounds.max_y() <= DEV_SHELL_TOOLBAR_HEIGHT);
    }

    #[test]
    fn dev_shell_logo_button_draws_registered_svg_logo_image() {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime
            .render(window_id)
            .expect("dev application should render");

        let mut logo_rect = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { rect, source } = command
                && source.image == DEV_SHELL_LOGO_IMAGE_HANDLE
            {
                logo_rect = Some(*rect);
            }
        });
        let logo_rect = logo_rect
            .expect("dev shell should render the SUI menu trigger from the registered logo image");
        let logo_image = output
            .frame
            .image_registry
            .get(DEV_SHELL_LOGO_IMAGE_HANDLE)
            .expect("dev shell logo image should be registered");

        assert_eq!(logo_rect.width(), DEV_SHELL_LOGO_BUTTON_SIZE);
        assert_eq!(logo_rect.height(), DEV_SHELL_LOGO_BUTTON_SIZE);
        assert_eq!(logo_image.width(), DEV_SHELL_LOGO_IMAGE_SIZE);
        assert_eq!(logo_image.height(), DEV_SHELL_LOGO_IMAGE_SIZE);
        assert!(
            logo_image.bytes().chunks_exact(4).any(|pixel| pixel[3] > 0),
            "the SVG-backed logo raster should contain visible pixels"
        );
    }

    #[test]
    fn paint_workspace_exposes_canvas_and_inspector_controls() {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("paint workspace should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("paint workspace semantics should exist");

        let canvas = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("expected the paint workspace to expose the pixel canvas");
        let properties = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_PROPERTIES_NAME)
            })
            .expect("expected the paint workspace to expose the properties dock");
        assert!(
            canvas.bounds.max_x() <= properties.bounds.x() + 0.5,
            "paint canvas should not overlap the properties dock: canvas={:?}, properties={:?}",
            canvas.bounds,
            properties.bounds
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_COLOR_EDITOR_NAME)
                    && node.state.expanded == Some(false)
            }),
            "expected the full color editor to start collapsed"
        );
        assert!(
            !semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorPicker
                    && node.name.as_deref() == Some(PAINT_BRUSH_COLOR_NAME)
            }),
            "expected the hidden full color picker to stay out of the initial semantics tree"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_COLOR_PRESETS_NAME)
                    && node.value == Some(SemanticsValue::Text("Ocean #1438C7FF".to_string()))
            }),
            "expected the color presets to expose the selected brush color"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some(PAINT_BRUSH_COLOR_NAME)
                    && node.value == Some(SemanticsValue::Text("#1438C7FF".to_string()))
                    && node.actions.is_empty()
            }),
            "expected the visible brush color well to expose the current brush color"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some("Ocean")
                    && node.state.selected
            }),
            "expected the initial Ocean color preset to be selected"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Image
                    && node.name.as_deref() == Some(PAINT_BRUSH_PREVIEW_NAME)
                    && node.value
                        == Some(SemanticsValue::Text(
                            "Round brush, 18 px, 100% opacity".to_string(),
                        ))
            }),
            "expected the brush preview to expose the current brush settings"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_BRUSH_SIZE_PRESETS_NAME)
                    && node.value == Some(SemanticsValue::Text("18 px".to_string()))
            }),
            "expected the brush size presets to expose the selected preset"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("18 px")
                    && node.state.selected
            }),
            "expected the initial 18 px brush preset to be selected"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::List
                    && node.name.as_deref() == Some(PAINT_LAYERS_NAME)
                    && node.value == Some(SemanticsValue::Text("Paint".to_string()))
            }),
            "expected the paint workspace to expose the selected layer list"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ListItem
                    && node.name.as_deref() == Some("Paint")
                    && node.description.as_deref() == Some("Normal / 100%")
                    && node.value
                        == Some(SemanticsValue::Text(
                            "Normal / 100%; Visible; Unlocked".to_string(),
                        ))
                    && node.state.selected
            }),
            "expected the visible paint layer row to expose its detail and selection state"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ListItem
                    && node.name.as_deref() == Some("Paper")
                    && node.description.as_deref() == Some("Normal / 100%")
                    && node.value
                        == Some(SemanticsValue::Text(
                            "Normal / 100%; Visible; Locked".to_string(),
                        ))
                    && !node.state.selected
            }),
            "expected the visible paper layer row to expose its detail"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paint layer")
                    && node.value == Some(SemanticsValue::Text("Visible".to_string()))
                    && node.state.checked == Some(ToggleState::Checked)
            }),
            "expected the paint layer to expose a visibility toggle"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Lock Paint layer")
                    && node.value == Some(SemanticsValue::Text("Unlocked".to_string()))
                    && node.state.checked == Some(ToggleState::Unchecked)
            }),
            "expected the paint layer to expose an unlocked lock toggle"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Unlock Paper layer")
                    && node.value == Some(SemanticsValue::Text("Locked".to_string()))
                    && node.state.checked == Some(ToggleState::Checked)
            }),
            "expected the paper layer to expose a locked lock toggle"
        );
        for group in [
            "Paint toolbar",
            PAINT_HISTORY_COMMANDS_NAME,
            PAINT_VIEW_COMMANDS_NAME,
            PAINT_DOCUMENT_COMMANDS_NAME,
            "Paint tools",
            PAINT_DOCUMENT_BAR_NAME,
            PAINT_DOCUMENT_VIEW_COMMANDS_NAME,
            PAINT_HORIZONTAL_RULER_NAME,
            PAINT_VERTICAL_RULER_NAME,
        ] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::GenericContainer
                        && node.name.as_deref() == Some(group)
                }),
                "expected the paint workspace to expose the {group} group"
            );
        }
        for text in [PAINT_DOCUMENT_NAME, PAINT_PROPERTIES_NAME, "1920 x 1080 px"] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::Text && node.name.as_deref() == Some(text)
                }),
                "expected the paint workspace to expose {text:?}"
            );
        }
        let document_zoom = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some(PAINT_ZOOM_READOUT_NAME)
            })
            .expect("expected the document bar to expose the zoom level");
        let Some(SemanticsValue::Text(document_zoom_value)) = &document_zoom.value else {
            panic!("expected the document zoom readout to expose its visible value");
        };
        assert!(
            document_zoom_value.starts_with("Zoom "),
            "expected the document zoom readout value to match visible zoom text, got {document_zoom_value:?}"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Brush tool")
                    && node.state.selected
            }),
            "expected the active paint tool to be exposed as selected"
        );
        for button in [PAINT_ZOOM_OUT_NAME, PAINT_ZOOM_IN_NAME] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(button)
                }),
                "expected the document bar to expose the {button} button"
            );
        }
        for button in [
            "Undo",
            "Redo",
            PAINT_FIT_VIEW_NAME,
            PAINT_ACTUAL_SIZE_NAME,
            "Clear",
            "Export",
        ] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(button)
                }),
                "expected the paint toolbar to expose the {button} button"
            );
        }
        for button in ["Undo", "Redo"] {
            let node = semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(button)
                })
                .expect("toolbar history button should exist");
            assert!(node.state.disabled, "expected {button} to start disabled");
        }
        let clear = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Clear")
            })
            .expect("clear button should exist");
        assert!(
            clear.state.disabled,
            "empty paint document should not be clearable"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Paint status")
            }),
            "expected the paint workspace to expose a status bar"
        );
        for status in [
            "Tool Brush",
            "Brush 18 px / 100%",
            "Blend Normal",
            "Layer Paint / Normal / 100% / Unlocked",
            "Document 1920 x 1080 px",
            "Cursor --",
        ] {
            assert!(
                semantics.iter().any(|node| {
                    node.role == SemanticsRole::Text && node.name.as_deref() == Some(status)
                }),
                "expected the paint status bar to expose {status:?}"
            );
        }
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("Zoom "))
            }),
            "expected the paint status bar to expose zoom"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SIZE_NAME)
                    && node.value == Some(SemanticsValue::Number(PAINT_INITIAL_BRUSH_SIZE as f64))
            }),
            "expected the brush size number input to expose its value"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_BRUSH_OPACITY_NAME)
            }),
            "expected the brush opacity slider to be accessible"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_LAYER_OPACITY_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: 1.0,
                            min: 0.0,
                            max: 1.0,
                        })
            }),
            "expected the selected layer opacity slider to expose its value"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_LAYER_BLEND_MODE_NAME)
                    && node.value == Some(SemanticsValue::Text("Normal".to_string()))
            }),
            "expected the selected layer blend mode select to expose its value"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SHAPE_NAME)
                    && node.value == Some(SemanticsValue::Text("Round".to_string()))
            }),
            "expected the brush shape select to expose the current brush shape"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BLEND_MODE_NAME)
                    && node.value == Some(SemanticsValue::Text("Normal".to_string()))
            }),
            "expected the blend mode select to expose the current blend mode"
        );
    }

    #[test]
    fn paint_workspace_layer_list_updates_selected_layer_status() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let layers = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::List && node.name.as_deref() == Some(PAINT_LAYERS_NAME)
            })
            .expect("layers list should exist");
        let position = Point::new(
            layers.bounds.x() + 104.0,
            layers.bounds.y() + layers.bounds.height() - 24.0,
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let layers = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::List && node.name.as_deref() == Some(PAINT_LAYERS_NAME)
            })
            .expect("layers list should still exist");
        assert_eq!(
            layers.value,
            Some(SemanticsValue::Text("Paper".to_string()))
        );
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should still exist");
        assert!(paper.state.selected);
        assert_eq!(
            paper.value,
            Some(SemanticsValue::Text(
                "Normal / 100%; Visible; Locked".to_string()
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paper / Normal / 100% / Locked")
            }),
            "expected the status bar to expose the selected Paper layer"
        );

        let above = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PAINT_SELECT_LAYER_ABOVE_NAME)
            })
            .expect("select layer above action should exist");
        assert!(
            above.actions.contains(&SemanticsAction::Activate),
            "select layer above should be enabled when Paper is selected"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PAINT_SELECT_LAYER_BELOW_NAME)
                    && node.state.disabled
            }),
            "select layer below should be disabled at the bottom layer"
        );
        let position = Point::new(
            above.bounds.x() + (above.bounds.width() * 0.5),
            above.bounds.y() + (above.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 2;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 2;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 2;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert!(
            paint.state.selected,
            "expected the Paint layer row to follow the header action selection"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paint / Normal / 100% / Unlocked")
            }),
            "expected the status bar to expose the selected Paint layer"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_layer_list_drag_reorders_layers() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(paint_state.display_above_paper());
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should exist");
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should exist");

        drag_primary_pointer(
            &mut runtime,
            window_id,
            41,
            Point::new(
                paint.bounds.x() + 104.0,
                paint.bounds.y() + paint.bounds.height() * 0.5,
            ),
            Point::new(paint.bounds.x() + 104.0, paper.bounds.max_y() + 8.0),
        )?;

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should still exist");
        assert!(paper.bounds.y() < paint.bounds.y());
        assert!(
            !paint_state.display_above_paper(),
            "paper should composite above paint after dragging it above the Paint layer"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paint / Normal / 100% / Unlocked")
            }),
            "reordering should not change the selected paint layer identity"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_layer_visibility_toggle_updates_semantics() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let paper_visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paper layer")
            })
            .expect("paper layer visibility button should exist");
        let position = Point::new(
            paper_visibility.bounds.x() + (paper_visibility.bounds.width() * 0.5),
            paper_visibility.bounds.y() + (paper_visibility.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 5;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 5;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 5;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let layers = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::List && node.name.as_deref() == Some(PAINT_LAYERS_NAME)
            })
            .expect("layers list should still exist");
        assert_eq!(
            layers.value,
            Some(SemanticsValue::Text("Paint".to_string()))
        );
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert!(paint.state.selected);
        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should still exist");
        assert!(!paper.state.selected);
        assert_eq!(
            paper.value,
            Some(SemanticsValue::Text(
                "Normal / 100%; Hidden; Locked".to_string()
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Show Paper layer")
                    && node.value == Some(SemanticsValue::Text("Hidden".to_string()))
                    && node.state.checked == Some(ToggleState::Unchecked)
            }),
            "expected the paper layer visibility toggle to expose the hidden state"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_paint_layer_display_controls_drive_canvas_state() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(paint_state.display_visible());
        assert_eq!(paint_state.display_opacity(), 1.0);
        assert_eq!(
            paint_state.display_blend_mode(),
            PixelCanvasBlendMode::Normal
        );

        let paint_visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paint layer")
            })
            .expect("paint layer visibility button should exist");
        let visibility_position = Point::new(
            paint_visibility.bounds.x() + (paint_visibility.bounds.width() * 0.5),
            paint_visibility.bounds.y() + (paint_visibility.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, visibility_position);
        move_event.pointer_id = 12;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, visibility_position);
        down.pointer_id = 12;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, visibility_position);
        up.pointer_id = 12;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(!paint_state.display_visible());
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("paint canvas semantics should exist");
        assert!(
            matches!(
                canvas.value.as_ref(),
                Some(SemanticsValue::Text(value)) if value.contains("layer hidden")
            ),
            "expected the paint canvas semantics to report the hidden paint layer"
        );

        paint_state.set_display_visible(true);
        let output = runtime.render(window_id)?;
        let opacity = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_LAYER_OPACITY_NAME)
            })
            .expect("layer opacity slider should exist");
        let opacity_position = Point::new(
            opacity.bounds.x() + (opacity.bounds.width() * 0.5),
            opacity.bounds.y() + (opacity.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, opacity_position);
        move_event.pointer_id = 13;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, opacity_position);
        down.pointer_id = 13;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, opacity_position);
        up.pointer_id = 13;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;
        assert_eq!(paint_state.display_opacity(), 0.5);

        let output = runtime.render(window_id)?;
        let blend = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_LAYER_BLEND_MODE_NAME)
            })
            .expect("layer blend mode select should exist");
        let blend_position = Point::new(
            blend.bounds.x() + (blend.bounds.width() * 0.5),
            blend.bounds.y() + (blend.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, blend_position);
        move_event.pointer_id = 14;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, blend_position);
        down.pointer_id = 14;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, blend_position);
        up.pointer_id = 14;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;
        let _ = runtime.render(window_id)?;

        let option_position = Point::new(
            blend.bounds.x() + (blend.bounds.width() * 0.5),
            blend.bounds.y() + blend.bounds.height() + 6.0 + (blend.bounds.height() * 1.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, option_position);
        move_event.pointer_id = 14;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, option_position);
        down.pointer_id = 14;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, option_position);
        up.pointer_id = 14;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;
        assert_eq!(
            paint_state.display_blend_mode(),
            PixelCanvasBlendMode::Multiply
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_paper_layer_display_controls_drive_canvas_state() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(paint_state.paper_visible());
        assert_eq!(paint_state.paper_opacity(), 1.0);
        assert!(paint_state.display_visible());
        assert_eq!(paint_state.display_opacity(), 1.0);

        let paper_visibility = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Hide Paper layer")
            })
            .expect("paper layer visibility button should exist");
        let visibility_position = Point::new(
            paper_visibility.bounds.x() + (paper_visibility.bounds.width() * 0.5),
            paper_visibility.bounds.y() + (paper_visibility.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, visibility_position);
        move_event.pointer_id = 15;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, visibility_position);
        down.pointer_id = 15;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, visibility_position);
        up.pointer_id = 15;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(!paint_state.paper_visible());
        assert!(paint_state.display_visible());
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("paint canvas semantics should exist");
        assert!(
            matches!(
                canvas.value.as_ref(),
                Some(SemanticsValue::Text(value)) if value.contains("paper layer hidden")
            ),
            "expected the paint canvas semantics to report the hidden paper layer"
        );

        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should exist");
        let paper_position = Point::new(
            paper.bounds.x() + (paper.bounds.width() * 0.5),
            paper.bounds.y() + (paper.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, paper_position);
        move_event.pointer_id = 16;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, paper_position);
        down.pointer_id = 16;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, paper_position);
        up.pointer_id = 16;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let opacity = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_LAYER_OPACITY_NAME)
            })
            .expect("layer opacity slider should exist");
        let opacity_position = Point::new(
            opacity.bounds.x() + (opacity.bounds.width() * 0.5),
            opacity.bounds.y() + (opacity.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, opacity_position);
        move_event.pointer_id = 17;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, opacity_position);
        down.pointer_id = 17;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, opacity_position);
        up.pointer_id = 17;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert_eq!(paint_state.paper_opacity(), 0.5);
        assert_eq!(paint_state.display_opacity(), 1.0);
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("paint canvas semantics should exist");
        assert!(
            matches!(
                canvas.value.as_ref(),
                Some(SemanticsValue::Text(value)) if value.contains("paper opacity 50%")
            ),
            "expected the paint canvas semantics to report the paper opacity"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_layer_lock_toggle_updates_semantics() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let paint_lock = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Lock Paint layer")
            })
            .expect("paint layer lock button should exist");
        let position = Point::new(
            paint_lock.bounds.x() + (paint_lock.bounds.width() * 0.5),
            paint_lock.bounds.y() + (paint_lock.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 9;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 9;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 9;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert!(paint.state.selected);
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Normal / 100%; Visible; Locked".to_string()
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Unlock Paint layer")
                    && node.value == Some(SemanticsValue::Text("Locked".to_string()))
                    && node.state.checked == Some(ToggleState::Checked)
            }),
            "expected the paint layer lock toggle to expose the locked state"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_selected_locked_layer_makes_canvas_read_only() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(paint_state.is_editable());

        let paper = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paper")
            })
            .expect("paper layer row should exist");
        let position = Point::new(
            paper.bounds.x() + paper.bounds.width() * 0.5,
            paper.bounds.y() + paper.bounds.height() * 0.5,
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 10;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 10;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 10;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(!paint_state.is_editable());
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("paint canvas semantics should exist");
        assert!(
            matches!(
                canvas.value.as_ref(),
                Some(SemanticsValue::Text(value)) if value.contains("read only")
            ),
            "expected selected locked layer to make the canvas read only"
        );
        assert!(
            !canvas
                .actions
                .contains(&SemanticsAction::Custom("Paint".into())),
            "read-only canvas should not expose a Paint action"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paper / Normal / 100% / Locked")
            }),
            "expected the status bar to expose the locked selected layer"
        );

        let unlock = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Unlock Paper layer")
            })
            .expect("paper layer unlock control should exist");
        let unlock_position = Point::new(
            unlock.bounds.x() + unlock.bounds.width() * 0.5,
            unlock.bounds.y() + unlock.bounds.height() * 0.5,
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, unlock_position);
        move_event.pointer_id = 11;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, unlock_position);
        down.pointer_id = 11;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, unlock_position);
        up.pointer_id = 11;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(paint_state.is_editable());
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("paint canvas semantics should exist");
        assert!(
            matches!(
                canvas.value.as_ref(),
                Some(SemanticsValue::Text(value)) if value.contains("editable")
            ),
            "expected unlocked selected layer to make the canvas editable"
        );
        assert!(
            canvas
                .actions
                .contains(&SemanticsAction::Custom("Paint".into())),
            "editable canvas should expose a Paint action"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paper / Normal / 100% / Unlocked")
            }),
            "expected the status bar to expose the unlocked selected layer"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_layer_opacity_updates_layer_detail() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let opacity = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_LAYER_OPACITY_NAME)
            })
            .expect("layer opacity slider should exist");
        let position = Point::new(
            opacity.bounds.x() + (opacity.bounds.width() * 0.5),
            opacity.bounds.y() + (opacity.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 6;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 6;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 6;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let opacity = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_LAYER_OPACITY_NAME)
            })
            .expect("layer opacity slider should still exist");
        assert_eq!(
            opacity.value,
            Some(SemanticsValue::Range {
                value: 0.5,
                min: 0.0,
                max: 1.0,
            })
        );
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert_eq!(paint.description.as_deref(), Some("Normal / 50%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Normal / 50%; Visible; Unlocked".to_string()
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paint / Normal / 50% / Unlocked")
            }),
            "expected the status bar to expose the edited layer opacity"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_layer_blend_mode_updates_layer_detail() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let blend = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_LAYER_BLEND_MODE_NAME)
            })
            .expect("layer blend mode select should exist");
        let position = Point::new(
            blend.bounds.x() + (blend.bounds.width() * 0.5),
            blend.bounds.y() + (blend.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 8;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 8;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 8;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let _ = runtime.render(window_id)?;
        press_runtime_key(&mut runtime, window_id, "ArrowDown");
        press_runtime_key(&mut runtime, window_id, "Enter");

        let output = runtime.render(window_id)?;
        let blend = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_LAYER_BLEND_MODE_NAME)
            })
            .expect("layer blend mode select should still exist");
        assert_eq!(
            blend.value,
            Some(SemanticsValue::Text("Multiply".to_string()))
        );
        let paint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Paint")
            })
            .expect("paint layer row should still exist");
        assert_eq!(paint.description.as_deref(), Some("Multiply / 100%"));
        assert_eq!(
            paint.value,
            Some(SemanticsValue::Text(
                "Multiply / 100%; Visible; Unlocked".to_string()
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Layer Paint / Multiply / 100% / Unlocked")
            }),
            "expected the status bar to expose the edited layer blend mode"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_brush_size_preset_updates_canvas_state() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let preset = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("36 px")
            })
            .expect("36 px brush preset should exist");
        let position = Point::new(
            preset.bounds.x() + (preset.bounds.width() * 0.5),
            preset.bounds.y() + (preset.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 3;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 3;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 3;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_BRUSH_SIZE_PRESETS_NAME)
                    && node.value == Some(SemanticsValue::Text("36 px".to_string()))
            }),
            "expected the brush size preset group to expose 36 px"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("36 px")
                    && node.state.selected
            }),
            "expected the 36 px preset to be selected"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SIZE_NAME)
                    && node.value == Some(SemanticsValue::Number(36.0))
            }),
            "expected the brush size number input to sync to 36"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Image
                    && node.name.as_deref() == Some(PAINT_BRUSH_PREVIEW_NAME)
                    && node.value
                        == Some(SemanticsValue::Text(
                            "Round brush, 36 px, 100% opacity".to_string(),
                        ))
            }),
            "expected the brush preview to reflect the selected 36 px preset"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_color_preset_updates_brush_color() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        // At the default test window height, the palette starts under the fixed status bar.
        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(PAINT_SCROLL_NAME)
            })
            .expect("paint controls scroll view should exist");
        let mut scroll_event = PointerEvent::new(
            PointerEventKind::Scroll,
            Point::new(scroll.bounds.x() + 16.0, scroll.bounds.y() + 16.0),
        );
        scroll_event.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -96.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll_event))?;

        let output = runtime.render(window_id)?;
        let preset = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("Coral")
            })
            .expect("Coral color preset should exist");
        let position = Point::new(
            preset.bounds.x() + (preset.bounds.width() * 0.5),
            preset.bounds.y() + (preset.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 4;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 4;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 4;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_COLOR_PRESETS_NAME)
                    && node.value == Some(SemanticsValue::Text("Coral #E6522EFF".to_string()))
            }),
            "expected the color presets to expose Coral"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some("Coral")
                    && node.state.selected
            }),
            "expected the Coral preset to be selected"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some(PAINT_BRUSH_COLOR_NAME)
                    && node.value == Some(SemanticsValue::Text("#E6522EFF".to_string()))
                    && node.actions.is_empty()
            }),
            "expected the visible brush color well to sync to Coral"
        );
        assert!(
            !output.semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorPicker
                    && node.name.as_deref() == Some(PAINT_BRUSH_COLOR_NAME)
            }),
            "expected the full color picker to stay hidden while the editor is collapsed"
        );

        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(PAINT_SCROLL_NAME)
            })
            .expect("paint controls scroll view should exist");
        let mut scroll_event = PointerEvent::new(
            PointerEventKind::Scroll,
            Point::new(scroll.bounds.x() + 16.0, scroll.bounds.max_y() - 16.0),
        );
        scroll_event.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -240.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll_event))?;

        let output = runtime.render(window_id)?;
        let editor = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_COLOR_EDITOR_NAME)
            })
            .expect("color editor section should exist");
        let position = Point::new(editor.bounds.x() + 20.0, editor.bounds.y() + 8.0);
        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 5;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 5;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(PAINT_COLOR_EDITOR_NAME)
                    && node.state.expanded == Some(true)
            }),
            "expected the color editor section to expand"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::ColorPicker
                    && node.name.as_deref() == Some(PAINT_BRUSH_COLOR_NAME)
                    && node.value == Some(SemanticsValue::Text("#E6522EFF".to_string()))
            }),
            "expected the expanded color picker to sync to the selected Coral color"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_tool_buttons_update_selected_tool() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let eraser = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
            })
            .expect("eraser tool button should exist");
        let position = Point::new(
            eraser.bounds.x() + (eraser.bounds.width() * 0.5),
            eraser.bounds.y() + (eraser.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let eraser = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Eraser tool")
            })
            .expect("eraser tool button should still exist");
        let brush = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Brush tool")
            })
            .expect("brush tool button should still exist");

        assert!(
            eraser.state.selected,
            "eraser state after click: selected={}, bounds={:?}; brush selected={}, bounds={:?}; click={:?}",
            eraser.state.selected, eraser.bounds, brush.state.selected, brush.bounds, position
        );
        assert!(!brush.state.selected);
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Image
                    && node.name.as_deref() == Some(PAINT_ERASER_PREVIEW_NAME)
                    && node.value
                        == Some(SemanticsValue::Text(
                            "Round eraser, 18 px, 100% opacity".to_string(),
                        ))
            }),
            "expected eraser selection to swap in the eraser preview pane"
        );
        assert!(
            !output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Image
                    && node.name.as_deref() == Some(PAINT_BRUSH_PREVIEW_NAME)
            }),
            "brush preview should not remain exposed when the Eraser tool is active"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Eraser 18 px / 100%")
            }),
            "expected the status bar to expose eraser-specific tool settings"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_fill_and_pan_tools_swap_property_panes() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let fill = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Fill tool")
            })
            .expect("fill tool button should exist");
        let position = Point::new(
            fill.bounds.x() + (fill.bounds.width() * 0.5),
            fill.bounds.y() + (fill.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 6;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 6;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 6;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_FILL_OPACITY_NAME)
            }),
            "expected Fill to expose fill opacity"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_FILL_BLEND_MODE_NAME)
                    && node.value == Some(SemanticsValue::Text("Normal".to_string()))
            }),
            "expected Fill to expose fill blend mode"
        );
        assert!(
            !output.semantics.iter().any(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SIZE_NAME)
            }),
            "Fill should not expose brush size controls"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Fill 100%")
            }),
            "expected the status bar to expose fill-specific tool settings"
        );

        let pan = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Pan tool")
            })
            .expect("pan tool button should exist");
        let position = Point::new(
            pan.bounds.x() + (pan.bounds.width() * 0.5),
            pan.bounds.y() + (pan.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 7;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 7;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 7;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PAINT_FIT_VIEW_NAME)
            }),
            "expected Pan to expose fit view"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PAINT_ACTUAL_SIZE_NAME)
            }),
            "expected Pan to expose actual size"
        );
        assert!(
            !output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(PAINT_FILL_OPACITY_NAME)
            }),
            "Pan should not expose fill opacity controls"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Pan view")
            }),
            "expected the status bar to expose pan-specific tool settings"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_export_button_creates_export_snapshot() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(
            paint_state.latest_export_snapshot().is_none(),
            "export snapshot should not exist before clicking Export"
        );
        let export = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Export")
            })
            .expect("export button should exist");
        let position = Point::new(
            export.bounds.x() + (export.bounds.width() * 0.5),
            export.bounds.y() + (export.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let snapshot = paint_state
            .latest_export_snapshot()
            .expect("export button should request a canvas snapshot");
        assert_eq!(snapshot.name(), PAINT_TAB_LABEL);
        assert_eq!(snapshot.width(), PAINT_DOCUMENT_WIDTH);
        assert_eq!(snapshot.height(), PAINT_DOCUMENT_HEIGHT);
        assert_eq!(
            snapshot.byte_len(),
            PAINT_DOCUMENT_WIDTH * PAINT_DOCUMENT_HEIGHT * 4
        );
        assert!(
            snapshot
                .rgba8()
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 0]),
            "paint demo should start with an empty transparent canvas"
        );
        assert!(
            !output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("Exported "))
            }),
            "export details should not take over the editor status bar"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_canvas_cursor_updates_status_bar() -> Result<()> {
        let paint_state = PixelCanvasState::new();
        let mut runtime = finish_dev_application(build_paint_demo_with_state(paint_state.clone()))
            .build()
            .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Cursor --")
            }),
            "cursor status should start empty"
        );
        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should exist");
        let position = Point::new(
            canvas.bounds.x() + canvas.bounds.width() * 0.5,
            canvas.bounds.y() + canvas.bounds.height() * 0.5,
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 9;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let output = runtime.render(window_id)?;
        assert_eq!(
            paint_state.cursor_position(),
            Some(Point::new(
                PAINT_DOCUMENT_WIDTH as f32 * 0.5,
                PAINT_DOCUMENT_HEIGHT as f32 * 0.5
            ))
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Cursor 960, 540")
            }),
            "cursor status should expose document coordinates"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_clear_button_records_undoable_canvas_change() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should exist");
        assert!(!canvas.actions.contains(&SemanticsAction::Undo));
        let clear = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Clear")
            })
            .expect("clear button should exist");
        assert!(clear.state.disabled);
        let paint_position = Point::new(
            canvas.bounds.x() + canvas.bounds.width() * 0.5,
            canvas.bounds.y() + canvas.bounds.height() * 0.5,
        );

        runtime.handle_event(
            window_id,
            primary_pointer_event(1, PointerEventKind::Move, paint_position, false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(1, PointerEventKind::Down, paint_position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(1, PointerEventKind::Up, paint_position, false),
        )?;

        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should still exist after painting");
        assert!(canvas.actions.contains(&SemanticsAction::Undo));
        let clear = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Clear")
            })
            .expect("clear button should be enabled after painting");
        assert!(!clear.state.disabled);
        let position = Point::new(
            clear.bounds.x() + (clear.bounds.width() * 0.5),
            clear.bounds.y() + (clear.bounds.height() * 0.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer_event(2, PointerEventKind::Move, position, false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(2, PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(2, PointerEventKind::Up, position, false),
        )?;

        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should still exist");
        assert!(canvas.actions.contains(&SemanticsAction::Undo));
        let clear = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button && node.name.as_deref() == Some("Clear")
            })
            .expect("clear button should still exist");
        assert!(clear.state.disabled);
        Ok(())
    }

    #[test]
    fn paint_workspace_blend_mode_select_updates_canvas_state() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let blend = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BLEND_MODE_NAME)
            })
            .expect("blend mode select should exist");
        let position = Point::new(
            blend.bounds.x() + (blend.bounds.width() * 0.5),
            blend.bounds.y() + (blend.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let _ = runtime.render(window_id)?;
        let option_position = Point::new(
            blend.bounds.x() + (blend.bounds.width() * 0.5),
            blend.bounds.y() + blend.bounds.height() + 6.0 + (blend.bounds.height() * 1.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, option_position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, option_position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, option_position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let blend = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BLEND_MODE_NAME)
            })
            .expect("blend mode select should still exist");
        assert_eq!(
            blend.value,
            Some(SemanticsValue::Text("Multiply".to_string()))
        );

        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should exist");
        let Some(SemanticsValue::Text(value)) = &canvas.value else {
            panic!("paint canvas should expose text value");
        };
        assert!(
            value.contains("blend Multiply"),
            "unexpected canvas value after blend selection: {value}"
        );
        Ok(())
    }

    #[test]
    fn paint_workspace_brush_shape_select_updates_canvas_state() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(PAINT_TAB_LABEL),
        ))
        .build()
        .expect("paint workspace application should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let shape = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SHAPE_NAME)
            })
            .expect("brush shape select should exist");
        let position = Point::new(
            shape.bounds.x() + (shape.bounds.width() * 0.5),
            shape.bounds.y() + (shape.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let _ = runtime.render(window_id)?;
        let option_position = Point::new(
            shape.bounds.x() + (shape.bounds.width() * 0.5),
            shape.bounds.y() + shape.bounds.height() + 6.0 + (shape.bounds.height() * 0.5),
        );

        let mut move_event = PointerEvent::new(PointerEventKind::Move, option_position);
        move_event.pointer_id = 1;
        runtime.handle_event(window_id, Event::Pointer(move_event))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, option_position);
        down.pointer_id = 1;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime.handle_event(window_id, Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, option_position);
        up.pointer_id = 1;
        up.button = Some(PointerButton::Primary);
        runtime.handle_event(window_id, Event::Pointer(up))?;

        let output = runtime.render(window_id)?;
        let shape = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(PAINT_BRUSH_SHAPE_NAME)
            })
            .expect("brush shape select should still exist");
        assert_eq!(
            shape.value,
            Some(SemanticsValue::Text("Square".to_string()))
        );

        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some(PAINT_TAB_LABEL)
            })
            .expect("paint canvas should exist");
        let Some(SemanticsValue::Text(value)) = &canvas.value else {
            panic!("paint canvas should expose text value");
        };
        assert!(
            value.contains("shape Square"),
            "unexpected canvas value after brush shape selection: {value}"
        );
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_automation_builder_uses_browser_shell_layout() {
        let mut runtime = build_dev_application_with_automation(None)
            .build()
            .expect("desktop dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("desktop dev application should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("desktop dev application semantics should exist");

        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Tabs && node.name.as_deref() == Some("SUI demo browser")
            }),
            "expected the desktop launch builder to use the browser-style dev shell"
        );
        assert!(
            semantics.iter().all(|node| {
                !(node.role == SemanticsRole::List
                    && node.name.as_deref() == Some("Available views"))
            }),
            "desktop launch builder should not expose the retired sidebar layout"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_automation_builder_opens_requested_demo() {
        let mut runtime =
            build_dev_application_with_automation(Some(DesktopAutomationMode::WidgetBookScroll))
                .build()
                .expect("desktop dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("desktop dev application should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("desktop dev application semantics should exist");
        let shell = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Tabs && node.name.as_deref() == Some("SUI demo browser")
            })
            .expect("desktop dev shell tab semantics should exist");
        assert_eq!(
            shell.value,
            Some(SemanticsValue::Text(WIDGET_BOOK_TAB_LABEL.to_string()))
        );
    }

    #[test]
    fn hdr_validation_surface_debug_capture_exports_intermediate_artifacts() -> Result<()> {
        let options = WindowRenderOptions::new(true, 1.0)
            .with_color_management_mode(WindowColorManagementMode::PreferHdr)
            .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
            .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
            .with_tone_mapping_mode(WindowToneMappingMode::Automatic)
            .with_system_sdr_content_brightness_enabled(false);
        let app = TestApp::new_no_vsync(move || {
            crate::widget_book::build_color_validation_application()
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
        assert!(artifact_dir.join("luminance-map.png").exists());
        assert!(artifact_dir.join("headroom-map.png").exists());
        assert!(artifact_dir.join("clip-mask.png").exists());
        assert!(artifact_dir.join("output-diagnostics.txt").exists());
        assert!(artifact_dir.join("capture-metrics.txt").exists());
        assert!(
            artifact_dir.join("final-composed.exr").exists()
                || artifact_dir.join("final-composed.png").exists()
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
            .with_tone_mapping_mode(WindowToneMappingMode::Automatic)
            .with_system_sdr_content_brightness_enabled(false);
        let app = TestApp::new_no_vsync(move || {
            crate::widget_book::build_color_validation_application()
                .with_window_render_options(options)
        })?;
        let window = app.main_window()?;
        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(crate::widget_book::COLOR_VALIDATION_SCROLL_NAME);
        scroll.scroll_pixels(Vector::new(0.0, -240.0))?;

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
    fn settings_view_exposes_visible_labels_for_render_selectors() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_settings(&window)?;
        let snapshot = window.snapshot()?;
        let semantics = &snapshot.accessibility.nodes;

        for label in [
            TEXT_COVERAGE_POLICY_NAME,
            TEXT_COVERAGE_GAMMA_NAME,
            COLOR_MANAGEMENT_MODE_NAME,
            OUTPUT_PRIMARIES_NAME,
            DYNAMIC_RANGE_MODE_NAME,
            TONE_MAPPING_MODE_NAME,
            SDR_CONTENT_BRIGHTNESS_NAME,
            USE_SYSTEM_SDR_BRIGHTNESS_LABEL,
            HDR_THEME_MODE_NAME,
            LIVE_PERFORMANCE_OVERLAY_LABEL,
        ] {
            assert!(
                semantics
                    .iter()
                    .any(|node| node.name.as_deref() == Some(label)),
                "expected semantics tree to expose settings control {label:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn settings_toggle_controls_live_performance_overlay() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        set_window_scene_statistics_detail_mode(
            window.id(),
            SceneStatisticsDetailMode::Lightweight,
        );
        open_dev_shell_settings(&window)?;

        let before_snapshot = window.snapshot()?;
        assert!(
            before_snapshot.accessibility.nodes.iter().all(|node| {
                node.role != SemanticsRole::GenericContainer
                    || node.name.as_deref() != Some("Live performance overlay")
            }),
            "live performance overlay should be hidden by default"
        );

        window
            .get_by_role(SemanticsRole::CheckBox)
            .with_name(LIVE_PERFORMANCE_OVERLAY_LABEL)
            .click()?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;

        let enabled_snapshot = window.snapshot()?;
        assert!(
            enabled_snapshot.accessibility.nodes.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            }),
            "settings checkbox should show the live performance overlay"
        );
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "visible live performance overlay should enable detailed frame diagnostics"
        );

        window
            .get_by_role(SemanticsRole::CheckBox)
            .with_name(LIVE_PERFORMANCE_OVERLAY_LABEL)
            .click()?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;

        let disabled_snapshot = window.snapshot()?;
        assert!(
            disabled_snapshot.accessibility.nodes.iter().all(|node| {
                node.role != SemanticsRole::GenericContainer
                    || node.name.as_deref() != Some("Live performance overlay")
            }),
            "settings checkbox should hide the live performance overlay"
        );
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Lightweight,
            "hiding the overlay should release the detailed frame diagnostics mode it enabled"
        );
        Ok(())
    }

    #[test]
    fn settings_output_diagnostics_panel_grows_for_wrapped_text() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_settings(&window)?;
        let _ = window.capture_screenshot()?;
        let snapshot = window.snapshot()?;
        let diagnostics = find_named_node(
            &snapshot,
            SemanticsRole::GenericContainer,
            OUTPUT_DIAGNOSTICS_TITLE,
        );

        assert!(
            diagnostics.bounds.height() > 200.0,
            "output diagnostics should grow beyond the old fixed height when wrapped diagnostic lines are present; bounds={:?}",
            diagnostics.bounds
        );
        Ok(())
    }

    #[test]
    fn settings_controls_repaint_when_theme_toggle_changes() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_settings(&window)?;

        let light_snapshot = window.snapshot()?;
        let feather_width =
            find_named_node(&light_snapshot, SemanticsRole::SpinBox, FEATHER_WIDTH_NAME);
        let probe = Rect::new(
            feather_width.bounds.x() + 8.0,
            feather_width.bounds.y() + feather_width.bounds.height() * 0.5,
            1.0,
            1.0,
        );
        let light_pixel = sample_pixel(&window.capture_screenshot()?, probe, &light_snapshot)?;

        window
            .get_by_role(SemanticsRole::Switch)
            .with_name(DEV_SHELL_THEME_TOGGLE_NAME)
            .click()?;

        let dark_snapshot = window.snapshot()?;
        let dark_pixel = sample_pixel(&window.capture_screenshot()?, probe, &dark_snapshot)?;

        assert_ne!(
            light_pixel, dark_pixel,
            "expected the settings number input surface to repaint after the theme switch toggles"
        );
        Ok(())
    }

    #[test]
    fn widget_book_controls_repaint_when_theme_toggle_changes() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_demo(&window, WIDGET_BOOK_TAB_LABEL)?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(crate::widget_book::GALLERY_SCROLL_NAME);
        scroll_story_until_visible(
            &window,
            &gallery,
            SemanticsRole::Button,
            crate::widget_book::PRIMARY_BUTTON_LABEL,
            240,
        )?;

        let light_snapshot = window.snapshot()?;
        let primary_button = find_named_node(
            &light_snapshot,
            SemanticsRole::Button,
            crate::widget_book::PRIMARY_BUTTON_LABEL,
        );
        let probe = Rect::new(
            primary_button.bounds.x() + 8.0,
            primary_button.bounds.y() + primary_button.bounds.height() * 0.5,
            1.0,
            1.0,
        );
        let light_pixel = sample_pixel(&window.capture_screenshot()?, probe, &light_snapshot)?;

        window
            .get_by_role(SemanticsRole::Switch)
            .with_name(DEV_SHELL_THEME_TOGGLE_NAME)
            .click()?;

        let dark_snapshot = window.snapshot()?;
        let dark_pixel = sample_pixel(&window.capture_screenshot()?, probe, &dark_snapshot)?;

        assert_ne!(
            light_pixel, dark_pixel,
            "expected the widget book primary button surface to repaint after the theme switch toggles"
        );
        Ok(())
    }

    #[test]
    fn settings_view_exposes_hdr_theme_mode_controls() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_settings(&window)?;
        let snapshot = window.snapshot()?;
        let semantics = &snapshot.accessibility.nodes;

        assert!(
            semantics
                .iter()
                .any(|node| { node.name.as_deref() == Some(HDR_THEME_MODE_NAME) })
        );

        let inspection = semantics
            .iter()
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
        Ok(())
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

    fn ensure_live_overlay_detail_mode(window: &TestWindow) -> Result<()> {
        if window_scene_statistics_detail_mode(window.id()) != SceneStatisticsDetailMode::Detailed {
            set_window_scene_statistics_detail_mode(
                window.id(),
                SceneStatisticsDetailMode::Detailed,
            );
        }

        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "expected live performance diagnostics to run in detailed mode"
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

    fn find_picker_button(nodes: &[SemanticsNode], name: &str) -> SemanticsNode {
        let matches = nodes
            .iter()
            .filter(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(name)
                    && node.bounds.y() >= DEV_SHELL_TOOLBAR_HEIGHT
            })
            .cloned()
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [node] => node.clone(),
            [] => panic!("missing picker button named {name:?}"),
            _ => panic!(
                "expected exactly one picker button named {name:?}, found {}",
                matches.len()
            ),
        }
    }

    fn find_top_tab_button(nodes: &[SemanticsNode], name: &str) -> SemanticsNode {
        let matches = nodes
            .iter()
            .filter(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(name)
                    && node.bounds.y() < DEV_SHELL_TOOLBAR_HEIGHT
            })
            .cloned()
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [node] => node.clone(),
            [] => panic!("missing top tab button named {name:?}"),
            _ => panic!(
                "expected exactly one top tab button named {name:?}, found {}",
                matches.len()
            ),
        }
    }

    fn top_tab_indicator_rect(tab: Rect) -> Rect {
        let theme = DefaultTheme::default();
        let padding = theme.metrics.tab_padding;
        let thickness = theme.interaction.active_indicator_thickness;
        Rect::new(
            tab.x() + padding.left,
            tab.max_y() - thickness,
            (tab.width() - padding.left - padding.right).max(0.0),
            thickness,
        )
    }

    fn dev_shell_indicator_rect(output: &RenderOutput) -> Rect {
        let theme = DefaultTheme::default();
        let toolbar_region = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            DEV_SHELL_TOOLBAR_HEIGHT,
        );
        let matches =
            solid_fill_bounds_for_color_in_rect(output, theme.palette.accent, toolbar_region)
                .into_iter()
                .filter(|rect| rect.height() <= 4.0 && rect.y() < DEV_SHELL_TOOLBAR_HEIGHT)
                .collect::<Vec<_>>();

        match matches.as_slice() {
            [rect] => *rect,
            [] => panic!("missing dev shell tab indicator"),
            _ => panic!("expected one dev shell tab indicator, found {matches:?}"),
        }
    }

    fn assert_theme_toggle_state(
        nodes: &[SemanticsNode],
        scheme: ThemeColorScheme,
        checked: ToggleState,
    ) -> SemanticsNode {
        let matches = nodes
            .iter()
            .filter(|node| {
                node.role == SemanticsRole::Switch
                    && node.name.as_deref() == Some(DEV_SHELL_THEME_TOGGLE_NAME)
            })
            .cloned()
            .collect::<Vec<_>>();

        let toggle = match matches.as_slice() {
            [node] => node.clone(),
            [] => panic!("missing theme toggle semantics node"),
            _ => panic!("expected exactly one theme toggle, found {}", matches.len()),
        };

        assert_eq!(
            toggle.value,
            Some(SemanticsValue::Text(
                dev_theme_scheme_label(scheme).to_string()
            )),
            "theme toggle should expose the active scheme"
        );
        assert_eq!(
            toggle.state.checked,
            Some(checked),
            "theme toggle checked state should reflect {scheme:?}"
        );
        toggle
    }

    fn center_of(bounds: Rect) -> Point {
        Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.5,
        )
    }

    fn click_runtime_point(runtime: &mut Runtime, window_id: WindowId, position: Point) {
        runtime
            .handle_event(
                window_id,
                Event::Pointer(PointerEvent::new(PointerEventKind::Move, position)),
            )
            .expect("pointer move should dispatch");

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .expect("pointer down should dispatch");

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.button = Some(PointerButton::Primary);
        runtime
            .handle_event(window_id, Event::Pointer(up))
            .expect("pointer up should dispatch");
    }

    fn press_runtime_key(runtime: &mut Runtime, window_id: WindowId, key: &str) {
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new(key, KeyState::Pressed)),
            )
            .expect("key press should dispatch");
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new(key, KeyState::Released)),
            )
            .expect("key release should dispatch");
    }

    fn dispatch_ready_events(runtime: &mut Runtime) -> usize {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window_id, event) in ready {
            runtime
                .handle_event(ready_window_id, event)
                .expect("ready event should dispatch");
        }
        count
    }

    fn settle_runtime_animations(runtime: &mut Runtime, window_id: WindowId) {
        runtime.tick(DefaultTheme::default().motion.focus_duration() + 0.01);
        dispatch_ready_events(runtime);
        runtime
            .render(window_id)
            .expect("runtime should render after settling animations");
    }

    fn contains_approx_color(colors: &[Color], expected: Color) -> bool {
        const CHANNEL_TOLERANCE: f32 = 1.0 / 255.0;

        colors.iter().any(|color| {
            color.space == expected.space
                && (color.red - expected.red).abs() <= CHANNEL_TOLERANCE
                && (color.green - expected.green).abs() <= CHANNEL_TOLERANCE
                && (color.blue - expected.blue).abs() <= CHANNEL_TOLERANCE
                && (color.alpha - expected.alpha).abs() <= CHANNEL_TOLERANCE
        })
    }

    fn assert_dev_shell_active_tab(window: &TestWindow, title: &str) -> Result<()> {
        let snapshot = window.snapshot()?;
        let shell = find_named_node(&snapshot, SemanticsRole::Tabs, "SUI demo browser");
        assert_eq!(
            shell.value,
            Some(SemanticsValue::Text(title.to_string())),
            "expected dev shell active tab to be {title:?}"
        );
        Ok(())
    }

    fn open_dev_shell_demo(window: &TestWindow, title: &str) -> Result<()> {
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(title)
            .click()?;
        assert_dev_shell_active_tab(window, title)
    }

    fn open_dev_shell_settings(window: &TestWindow) -> Result<()> {
        window
            .get_by_role(SemanticsRole::Button)
            .with_name("SUI menu")
            .click()?;
        window
            .get_by_role(SemanticsRole::MenuItem)
            .with_name(SETTINGS_TAB_LABEL)
            .click()?;
        window
            .get_by_role(SemanticsRole::Window)
            .with_name(SETTINGS_TAB_LABEL)
            .expect()
            .to_be_visible()
    }

    fn semantic_text_value(window: &TestWindow, role: SemanticsRole, name: &str) -> String {
        let snapshot = window
            .snapshot()
            .expect("window snapshot should be available");
        let node = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("expected {role:?} named {name:?}"));
        match &node.value {
            Some(SemanticsValue::Text(value)) => value.clone(),
            other => panic!("expected text value for {name:?}, got {other:?}"),
        }
    }

    fn percentile_index(count: usize, percentile: f64) -> usize {
        assert!(count > 0, "percentile requires at least one sample");
        let percentile = percentile.clamp(0.0, 1.0);
        let rank = (count as f64 * percentile).ceil().max(1.0) as usize;
        (rank - 1).min(count - 1)
    }

    #[test]
    fn percentile_index_uses_nearest_rank_without_promoting_p95_to_max() {
        assert_eq!(percentile_index(24, 0.95), 22);
        assert_eq!(percentile_index(1, 0.95), 0);
    }

    #[test]
    fn dev_workspace_exposes_retained_text_benchmark_view() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(RETAINED_TEXT_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        open_dev_shell_demo(&window, RETAINED_TEXT_TAB_LABEL)?;
        Ok(())
    }

    #[test]
    fn dev_workspace_registers_layout_demo() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(LAYOUT_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        open_dev_shell_demo(&window, LAYOUT_TAB_LABEL)?;
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(LAYOUT_DEMO_SCROLL_NAME)
            .expect()
            .to_be_visible()?;
        Ok(())
    }

    #[test]
    fn dev_workspace_registers_drag_drop_demo() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(DRAG_DROP_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        open_dev_shell_demo(&window, DRAG_DROP_TAB_LABEL)?;
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(DRAG_DROP_DEMO_SCROLL_NAME)
            .expect()
            .to_be_visible()?;
        Ok(())
    }

    #[test]
    fn dev_workspace_registers_animation_demo() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(ANIMATION_DEMO_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        open_dev_shell_demo(&window, ANIMATION_DEMO_TAB_LABEL)?;
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(ANIMATION_DEMO_SCROLL_NAME)
            .expect()
            .to_be_visible()?;

        let snapshot = window.snapshot()?;
        for name in [
            ANIMATION_DEMO_NAME,
            ANIMATION_TIMELINE_PREVIEW_NAME,
            ANIMATION_RETAINED_LAYER_NAME,
            ANIMATION_PAINT_INVALIDATION_NAME,
            ANIMATION_EDITOR_SURFACE_NAME,
        ] {
            assert!(
                snapshot
                    .accessibility
                    .nodes
                    .iter()
                    .any(|node| node.name.as_deref() == Some(name)),
                "expected animation demo to expose {name:?}"
            );
        }

        Ok(())
    }

    #[test]
    fn animation_demo_resumes_playback_after_tab_switch() -> Result<()> {
        let app = TestApp::new_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_demo(&window, ANIMATION_DEMO_TAB_LABEL)?;
        app.advance_time(0.18)?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name("Open demo")
            .click()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(LAYOUT_TAB_LABEL)
            .click()?;
        assert_dev_shell_active_tab(&window, LAYOUT_TAB_LABEL)?;
        app.advance_time(0.18)?;

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(ANIMATION_DEMO_TAB_LABEL)
            .click()?;
        assert_dev_shell_active_tab(&window, ANIMATION_DEMO_TAB_LABEL)?;
        let restored_before_advance = semantic_text_value(
            &window,
            SemanticsRole::GenericContainer,
            ANIMATION_TIMELINE_PREVIEW_NAME,
        );
        app.advance_time(0.18)?;
        let restored_after_advance = semantic_text_value(
            &window,
            SemanticsRole::GenericContainer,
            ANIMATION_TIMELINE_PREVIEW_NAME,
        );

        assert_ne!(
            restored_before_advance, restored_after_advance,
            "animation timeline should resume after switching back to the demo tab"
        );

        Ok(())
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn dev_workspace_registers_markdown_render_demo() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(MARKDOWN_RENDER_TAB_LABEL)
            .expect()
            .to_be_visible()?;
        open_dev_shell_demo(&window, MARKDOWN_RENDER_TAB_LABEL)?;
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(MARKDOWN_RENDER_SCROLL_NAME)
            .expect()
            .to_be_visible()?;
        window
            .get_by_role(SemanticsRole::Text)
            .with_name(MARKDOWN_RENDER_DEMO_NAME)
            .expect()
            .to_be_visible()?;
        window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(MARKDOWN_SOURCE_EDITOR_NAME)
            .expect()
            .to_be_visible()?;
        Ok(())
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn markdown_source_edit_updates_rendered_preview() -> Result<()> {
        let app = TestApp::new(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_demo(&window, MARKDOWN_RENDER_TAB_LABEL)?;

        let marker = "Live preview marker";
        window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(MARKDOWN_SOURCE_EDITOR_NAME)
            .fill(format!("\n\n{marker}"))?;
        window.run_until_idle()?;

        let snapshot = window.snapshot()?;
        let rendered = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some(MARKDOWN_RENDER_DEMO_NAME)
                    && matches!(node.value, Some(SemanticsValue::Text(_)))
            })
            .expect("rendered markdown text semantics present");
        let Some(SemanticsValue::Text(text)) = &rendered.value else {
            unreachable!("rendered markdown node was filtered by text value");
        };
        assert!(
            text.contains(marker),
            "rendered markdown preview did not include edited source; text={text:?}"
        );
        Ok(())
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn markdown_source_edits_during_cooldown_update_after_timer() -> Result<()> {
        let app = TestApp::new_no_vsync(|| build_dev_application().build())?;
        let window = app.main_window()?;
        open_dev_shell_demo(&window, MARKDOWN_RENDER_TAB_LABEL)?;

        let source = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(MARKDOWN_SOURCE_EDITOR_NAME);
        source.fill("\n\nFirst marker")?;
        source.fill("\n\nSecond marker")?;
        app.advance_time(MARKDOWN_RENDER_COOLDOWN_SECONDS + 0.01)?;
        window.run_until_idle()?;

        let snapshot = window.snapshot()?;
        let rendered = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some(MARKDOWN_RENDER_DEMO_NAME)
                    && matches!(node.value, Some(SemanticsValue::Text(_)))
            })
            .expect("rendered markdown text semantics present");
        let Some(SemanticsValue::Text(text)) = &rendered.value else {
            unreachable!("rendered markdown node was filtered by text value");
        };
        assert!(
            text.contains("Second marker"),
            "dirty markdown preview did not flush after cooldown; text={text:?}"
        );
        Ok(())
    }

    #[test]
    fn dev_workspace_registers_canvas_editor_demos() -> Result<()> {
        let mut runtime = build_dev_application()
            .build()
            .expect("dev application should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id)?;
        let semantics = runtime.semantics(window_id)?;

        for title in [PAINT_TAB_LABEL, VECTOR_EDITOR_TAB_LABEL] {
            assert!(
                semantics
                    .iter()
                    .any(|node| node.role == SemanticsRole::Button
                        && node.name.as_deref() == Some(title)),
                "expected browser-style demo picker to expose {title:?}"
            );
        }

        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(VECTOR_EDITOR_TAB_LABEL),
        ))
        .build()
        .expect("vector editor demo should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id)?;
        let semantics = runtime.semantics(window_id)?;

        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Canvas
                    && node.name.as_deref() == Some(VECTOR_EDITOR_TAB_LABEL)
            }),
            "expected vector editor to expose its canvas"
        );
        assert!(
            !semantics
                .iter()
                .any(|node| node.name.as_deref() == Some("Vector toolbar")
                    || node.name.as_deref() == Some("Select tool")),
            "expected vector editor to omit non-functional toolbar controls"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ListItem
                    && node.name.as_deref() == Some("Blue ellipse")
                    && node.value
                        == Some(SemanticsValue::Text(
                            "124 x 96 px / 78% fill; Visible; Unlocked".to_string(),
                        ))
            }),
            "expected vector object list to expose object values"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(VECTOR_WIDTH_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: 124.0,
                            min: f64::from(VECTOR_MIN_OBJECT_SIZE),
                            max: f64::from(VECTOR_DOCUMENT_WIDTH),
                        })
            }),
            "expected selected width slider"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(VECTOR_ROTATION_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: -12.0,
                            min: -180.0,
                            max: 180.0,
                        })
            }),
            "expected selected rotation slider"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(VECTOR_STROKE_WIDTH_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: 3.0,
                            min: 0.5,
                            max: 24.0,
                        })
            }),
            "expected stroke width slider"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(VECTOR_OPACITY_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: 0.78,
                            min: 0.0,
                            max: 1.0,
                        })
            }),
            "expected opacity slider"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(VECTOR_FILL_RULE_NAME)
                    && node.value == Some(SemanticsValue::Text("Nonzero".to_string()))
            }),
            "expected fill rule combo box"
        );
        assert!(
            semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Object Blue ellipse")
            }),
            "expected vector status bar to expose selected object"
        );

        Ok(())
    }

    #[test]
    fn vector_editor_objects_list_drag_reorders_objects() -> Result<()> {
        let mut runtime = finish_dev_application(DevBrowserShell::with_initial_demo(
            RenderSettingsTab::default_options(),
            Some(VECTOR_EDITOR_TAB_LABEL),
        ))
        .build()
        .expect("vector editor demo should build");
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id)?;
        let blue = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Blue ellipse")
            })
            .expect("blue ellipse row should exist");
        let amber = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem
                    && node.name.as_deref() == Some("Amber ellipse")
            })
            .expect("amber ellipse row should exist");
        assert!(
            amber.bounds.y() < blue.bounds.y(),
            "default vector object list should show frontmost layers above lower layers"
        );

        drag_primary_pointer(
            &mut runtime,
            window_id,
            51,
            Point::new(
                blue.bounds.x() + 104.0,
                blue.bounds.y() + blue.bounds.height() * 0.5,
            ),
            Point::new(blue.bounds.x() + 104.0, amber.bounds.y() + 4.0),
        )?;

        let output = runtime.render(window_id)?;
        let blue = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem && node.name.as_deref() == Some("Blue ellipse")
            })
            .expect("blue ellipse row should still exist");
        let amber = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ListItem
                    && node.name.as_deref() == Some("Amber ellipse")
            })
            .expect("amber ellipse row should still exist");
        assert!(blue.bounds.y() < amber.bounds.y());

        let amber_position = Point::new(
            amber.bounds.x() + amber.bounds.width() * 0.5,
            amber.bounds.y() + amber.bounds.height() * 0.5,
        );
        runtime.handle_event(
            window_id,
            primary_pointer_event(52, PointerEventKind::Down, amber_position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer_event(52, PointerEventKind::Up, amber_position, false),
        )?;

        let output = runtime.render(window_id)?;
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Text
                    && node.name.as_deref() == Some("Object Amber ellipse")
            }),
            "clicking the reordered Amber row should select the Amber object"
        );
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some(VECTOR_WIDTH_NAME)
                    && node.value
                        == Some(SemanticsValue::Range {
                            value: 142.0,
                            min: f64::from(VECTOR_MIN_OBJECT_SIZE),
                            max: f64::from(VECTOR_DOCUMENT_WIDTH),
                        })
            }),
            "selected object controls should follow the reordered visual row"
        );
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
        let p95_index = percentile_index(valid_count, 0.95);
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
                    .retained_packet_rebuilds
                    .scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .coordinate_space_count as f64
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

        println!("\n=== SUI Demo Visible No-Vsync Widget-Book Drag Benchmark ===");
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

        println!("\n=== SUI Demo Visible No-Vsync Idle Benchmark ===");
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
                .retained_packet_rebuilds
                .scene_count,
            final_snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .state_count,
            final_snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .coordinate_space_count,
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
            .with_name(crate::widget_book::GALLERY_SCROLL_NAME);

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
            println!("\n=== SUI Demo Visible No-Vsync Widget-Book Scroll Benchmark ===");
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
        let p95_index = percentile_index(valid_count, 0.95);
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
                    .retained_packet_rebuilds
                    .scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .coordinate_space_count as f64
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

        println!("\n=== SUI Demo Visible No-Vsync Widget-Book Scroll Benchmark ===");
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
            crate::widget_book::GALLERY_SCROLL_NAME,
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
            crate::widget_book::GALLERY_SCROLL_NAME,
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
        let p95_index = percentile_index(valid_count, 0.95);
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
                    .retained_packet_rebuilds
                    .scene_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_state = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .state_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;
        let avg_packet_rebuild_coordinate_space = frame_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .coordinate_space_count as f64
            })
            .sum::<f64>()
            / valid_count as f64;

        println!("\n=== SUI Demo Realtime Visible No-Vsync Widget-Book Scroll Benchmark ===");
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
            .with_name(crate::widget_book::GALLERY_SCROLL_NAME);

        scroll_story_until_visible(window, &gallery, role.clone(), name, 240)?;
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
            if story_is_visible_in_gallery(&snapshot, &role, name) {
                return Ok(());
            }

            gallery.scroll_pixels(Vector::new(0.0, -120.0))?;
        }

        for _ in 0..max_steps.saturating_mul(2) {
            let snapshot = window.snapshot()?;
            if story_is_visible_in_gallery(&snapshot, &role, name) {
                return Ok(());
            }

            gallery.scroll_pixels(Vector::new(0.0, 120.0))?;
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
                crate::widget_book::GALLERY_SCROLL_NAME,
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

    fn story_is_visible_in_gallery(
        snapshot: &WindowSnapshot,
        role: &SemanticsRole,
        name: &str,
    ) -> bool {
        let Some(story) = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == *role && node.name.as_deref() == Some(name))
        else {
            return false;
        };
        let Some(gallery) = snapshot.accessibility.nodes.iter().find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(crate::widget_book::GALLERY_SCROLL_NAME)
        }) else {
            return false;
        };

        visible_area_ratio(story.bounds, gallery.bounds) > 0.0
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
        let scaled = scale_bounds_for_screenshot(bounds, snapshot, screenshot);
        let pixel = screenshot.crop(scaled).map_err(|error| {
            sui::Error::new(format!(
                "sample pixel crop failed for bounds={bounds:?}, scaled={scaled:?}, screenshot={}x{}: {}",
                screenshot.width(),
                screenshot.height(),
                error
            ))
        })?;
        let rgba = pixel.pixels();
        Ok([rgba[0], rgba[1], rgba[2], rgba[3]])
    }
}
