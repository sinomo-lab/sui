use std::{cell::RefCell, rc::Rc};

use sui::{
    HdrThemeMode, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    PointerButton, PointerEventKind, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue,
    TextHinting, ToggleState, WgpuRenderer, WidgetId, WidgetPodMutVisitor, WidgetPodVisitor,
    WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent, WindowId,
    WindowOutputColorPrimaries, WindowOutputDiagnostics, WindowRenderOptions, WindowStemDarkening,
    WindowTextHinting, WindowToneMappingMode, prelude::*, window_output_diagnostics,
};
use sui_widget_book::{
    LivePerformanceRoot, build_button_grid_benchmark, build_color_validation_surface,
    build_retained_text_benchmark, build_text_editing_benchmark,
    build_text_rendering_comparison_surface, build_text_validation_surface,
    build_theme_demo_surface, build_widget_book_gallery, default_widget_book_state,
    register_widget_book_images, set_widget_book_hdr_theme_mode, widget_book_hdr_theme_mode,
};

const WINDOW_TITLE: &str = "SUI Dev";
const WINDOW_DESCRIPTION: &str =
    "Browser-style development workspace for the widget book and focused performance demos.";
const WIDGET_BOOK_TAB_LABEL: &str = "Widget book";
const THEMES_TAB_LABEL: &str = "Themes";
const BUTTON_GRID_TAB_LABEL: &str = "64 buttons";
const RETAINED_TEXT_TAB_LABEL: &str = "Retained text";
const TEXT_RENDERING_COMPARISON_TAB_LABEL: &str = "Text comparison";
const TEXT_VALIDATION_TAB_LABEL: &str = "Text validation";
const TEXT_EDITING_TAB_LABEL: &str = "Text editing";
const HDR_VALIDATION_TAB_LABEL: &str = "HDR validation";
const PAINT_TAB_LABEL: &str = "Paint";
const VECTOR_EDITOR_TAB_LABEL: &str = "Vector editor";
const SETTINGS_TAB_LABEL: &str = "Settings";
const FEATHERING_TOGGLE_LABEL: &str = "Enable renderer feathering";
const FEATHER_WIDTH_NAME: &str = "Feather width";
const OPTICAL_TEXT_CENTERING_TOGGLE_LABEL: &str = "Enable optical vertical text centering";
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
const HDR_THEME_MODE_OPTIONS: [&str; 4] = [
    "Disabled (SDR baseline)",
    "Wide-gamut only",
    "Constrained HDR",
    "Full HDR",
];

const DEV_SHELL_TOOLBAR_HEIGHT: f32 = 44.0;
const DEV_SHELL_LOGO_BUTTON_SIZE: f32 = 32.0;
const DEV_SHELL_TAB_HEIGHT: f32 = 32.0;
const DEV_SHELL_TAB_GAP: f32 = 6.0;
const DEV_SHELL_TAB_CLOSE_SIZE: f32 = 18.0;
const DEV_SHELL_TAB_CLOSE_MARGIN: f32 = 7.0;
const DEV_SHELL_PLUS_BUTTON_SIZE: f32 = 30.0;
const DEV_SHELL_THEME_TOGGLE_WIDTH: f32 = 92.0;
const DEV_SHELL_THEME_TOGGLE_HEIGHT: f32 = 34.0;
const DEV_SHELL_PICKER_TILE_HEIGHT: f32 = 72.0;
const DEV_SHELL_SETTINGS_TITLE_HEIGHT: f32 = 38.0;
const DEV_SHELL_SETTINGS_RESIZE_HANDLE: f32 = 18.0;
const DEV_SHELL_MIN_SETTINGS_WIDTH: f32 = 320.0;
const DEV_SHELL_MIN_SETTINGS_HEIGHT: f32 = 260.0;
const DEV_SHELL_DEFAULT_SETTINGS_WIDTH: f32 = 460.0;
const DEV_SHELL_DEFAULT_SETTINGS_HEIGHT: f32 = 380.0;
const DEV_SHELL_DEFAULT_SETTINGS_X: f32 = 420.0;
const DEV_SHELL_DEFAULT_SETTINGS_Y: f32 = 96.0;
const DEV_SHELL_PICKER_TITLE: &str = "Open a demo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
pub enum DesktopAutomationMode {
    ButtonGridResize,
    WidgetBookScroll,
}

#[derive(Clone)]
struct DevShellState {
    inner: Rc<RefCell<DevShellStateInner>>,
}

struct DevShellStateInner {
    open_tabs: Vec<usize>,
    active_tab: Option<usize>,
    picker_open: bool,
    dark_theme: bool,
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
                dark_theme: false,
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
        if self.inner.borrow().dark_theme {
            DefaultTheme::dark()
        } else {
            DefaultTheme::default()
        }
    }

    fn is_dark(&self) -> bool {
        self.inner.borrow().dark_theme
    }

    fn toggle_theme(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.dark_theme = !inner.dark_theme;
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
    child: WidgetPod,
}

struct DevBrowserShell {
    state: DevShellState,
    demos: Vec<DevDemo>,
    demo_buttons: WidgetChildren,
    main_menu: SingleChild,
    plus_button: SingleChild,
    theme_toggle: SingleChild,
    settings_window: SingleChild,
    tab_widths: Vec<f32>,
    tab_rects: Vec<(usize, Rect)>,
    hovered_tab: Option<usize>,
    pressed_tab: Option<usize>,
    hovered_close_tab: Option<usize>,
    pressed_close_tab: Option<usize>,
    content_bounds: Rect,
}

impl DevBrowserShell {
    fn new(render_options: WindowRenderOptions) -> Self {
        Self::with_initial_demo(render_options, None)
    }

    fn with_initial_demo(render_options: WindowRenderOptions, initial_demo: Option<&str>) -> Self {
        set_widget_book_hdr_theme_mode(HdrThemeMode::Disabled);
        let state = DevShellState::new();
        let demos = build_dev_demo_entries();
        if let Some(index) =
            initial_demo.and_then(|title| demos.iter().position(|demo| demo.title == title))
        {
            state.open_demo(index);
        }

        let mut demo_buttons = WidgetChildren::with_capacity(demos.len());
        for (index, demo) in demos.iter().enumerate() {
            let button_state = state.clone();
            demo_buttons.push(
                Button::new(demo.title)
                    .min_width(180.0)
                    .min_height(DEV_SHELL_PICKER_TILE_HEIGHT)
                    .on_press_with_ctx(move |ctx| {
                        button_state.open_demo(index);
                        request_window_refresh(ctx, true);
                    }),
            );
        }

        let picker_state = state.clone();
        let plus_button = IconButton::new(IconGlyph::Add, "Open demo")
            .size(DEV_SHELL_PLUS_BUTTON_SIZE)
            .icon_size(16.0)
            .on_press_with_ctx(move |ctx| {
                picker_state.show_picker();
                request_window_refresh(ctx, true);
            });

        let menu_state = state.clone();
        let main_menu = ContextMenu::new("SUI menu", SuiLogoButton)
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
            demo_buttons,
            main_menu: SingleChild::new(main_menu),
            plus_button: SingleChild::new(plus_button),
            theme_toggle: SingleChild::new(ThemeToggleButton::new(state.clone())),
            settings_window: SingleChild::new(FloatingSettingsWindow::new(
                state,
                build_render_settings_tab_with_options(render_options),
            )),
            tab_widths: Vec::new(),
            tab_rects: Vec::new(),
            hovered_tab: None,
            pressed_tab: None,
            hovered_close_tab: None,
            pressed_close_tab: None,
            content_bounds: Rect::ZERO,
        }
    }

    fn tab_at(&self, position: Point) -> Option<usize> {
        self.tab_rects
            .iter()
            .find_map(|(index, rect)| rect.contains(position).then_some(*index))
    }

    fn tab_close_rect(rect: Rect) -> Rect {
        Rect::new(
            rect.max_x() - DEV_SHELL_TAB_CLOSE_MARGIN - DEV_SHELL_TAB_CLOSE_SIZE,
            rect.y() + ((rect.height() - DEV_SHELL_TAB_CLOSE_SIZE) * 0.5),
            DEV_SHELL_TAB_CLOSE_SIZE,
            DEV_SHELL_TAB_CLOSE_SIZE,
        )
    }

    fn tab_close_at(&self, position: Point) -> Option<usize> {
        self.tab_rects.iter().find_map(|(index, rect)| {
            Self::tab_close_rect(*rect)
                .contains(position)
                .then_some(*index)
        })
    }

    fn tab_label_rect(rect: Rect) -> Rect {
        let close = Self::tab_close_rect(rect);
        let x = rect.x() + 12.0;
        let line_height = 18.0;
        Rect::new(
            x,
            rect.y() + ((rect.height() - line_height) * 0.5),
            (close.x() - x - 6.0).max(0.0),
            line_height,
        )
    }

    fn select_adjacent_tab(&mut self, direction: isize) {
        let tabs = self.state.open_tabs();
        if tabs.is_empty() {
            return;
        }
        let current = self
            .state
            .active_tab()
            .and_then(|active| tabs.iter().position(|index| *index == active))
            .unwrap_or(0);
        let last = tabs.len() as isize - 1;
        let next = (current as isize + direction).clamp(0, last) as usize;
        self.state.select_tab(tabs[next]);
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
        Rect::new(
            content.x() + 32.0,
            content.y() + 82.0,
            (content.width() - 64.0).max(0.0),
            (content.height() - 114.0).max(0.0),
        )
    }

    fn arrange_tab_strip(&mut self, bounds: Rect) -> Rect {
        self.tab_rects.clear();
        let tab_zone = Self::tab_zone_rect(bounds);
        let tabs = self.state.open_tabs();
        let mut x = tab_zone.x();
        for (order, demo_index) in tabs.into_iter().enumerate() {
            let width = self.tab_widths.get(order).copied().unwrap_or(132.0);
            if x + width > tab_zone.max_x() {
                break;
            }
            let rect = Rect::new(x, tab_zone.y(), width, DEV_SHELL_TAB_HEIGHT);
            self.tab_rects.push((demo_index, rect));
            x += width + DEV_SHELL_TAB_GAP;
        }

        Rect::new(
            x.min(tab_zone.max_x() - DEV_SHELL_PLUS_BUTTON_SIZE)
                .max(tab_zone.x()),
            tab_zone.y() + ((DEV_SHELL_TAB_HEIGHT - DEV_SHELL_PLUS_BUTTON_SIZE) * 0.5),
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

        let active = self.state.active_tab();
        for (demo_index, rect) in &self.tab_rects {
            let selected = active == Some(*demo_index);
            let hovered = self.hovered_tab == Some(*demo_index);
            let pressed = self.pressed_tab == Some(*demo_index);
            let tab_background = if selected {
                palette.surface
            } else if pressed {
                palette.surface_pressed
            } else if hovered {
                palette.surface_hover
            } else {
                Color::rgba(0.0, 0.0, 0.0, 0.0)
            };
            if selected || hovered || pressed {
                ctx.fill(
                    Path::rounded_rect(*rect, 7.0),
                    tab_background.with_alpha(if selected { 1.0 } else { 0.82 }),
                );
                ctx.stroke(
                    Path::rounded_rect(*rect, 7.0),
                    if selected {
                        palette.border_focus.with_alpha(0.72)
                    } else {
                        palette.border.with_alpha(0.62)
                    },
                    StrokeStyle::new(1.0),
                );
            }

            let label = self.demos[*demo_index].title;
            ctx.draw_text(
                Self::tab_label_rect(*rect),
                label,
                TextStyle {
                    font_size: 13.0,
                    line_height: 18.0,
                    color: if selected {
                        palette.border_focus
                    } else {
                        palette.text
                    },
                    ..TextStyle::default()
                },
            );

            let close = Self::tab_close_rect(*rect);
            let close_hovered = self.hovered_close_tab == Some(*demo_index);
            let close_pressed = self.pressed_close_tab == Some(*demo_index);
            if close_hovered || close_pressed {
                ctx.fill(
                    Path::rounded_rect(close, 5.0),
                    if close_pressed {
                        palette.surface_pressed
                    } else {
                        palette.surface_hover
                    },
                );
            }
            ctx.stroke(
                close_icon_path(close),
                if close_hovered || selected {
                    palette.text
                } else {
                    palette.placeholder
                }
                .with_alpha(if close_pressed { 0.95 } else { 0.78 }),
                StrokeStyle::new(1.4),
            );

            if selected {
                ctx.fill(
                    Path::rounded_rect(
                        Rect::new(
                            rect.x() + 12.0,
                            rect.max_y() - 3.0,
                            rect.width() - 24.0,
                            3.0,
                        ),
                        1.5,
                    ),
                    palette.accent,
                );
            }
        }
    }

    fn paint_picker(&self, ctx: &mut PaintCtx, theme: &DefaultTheme) {
        let content = self.content_bounds;
        let palette = theme.palette;
        ctx.fill_rect(content, palette.surface);
        ctx.draw_text(
            Rect::new(
                content.x() + 32.0,
                content.y() + 28.0,
                content.width() - 64.0,
                32.0,
            ),
            DEV_SHELL_PICKER_TITLE,
            TextStyle {
                font_size: 24.0,
                line_height: 30.0,
                color: palette.text,
                ..TextStyle::default()
            },
        );
        ctx.draw_text(
            Rect::new(
                content.x() + 32.0,
                content.y() + 58.0,
                content.width() - 64.0,
                20.0,
            ),
            "Choose a demo to open it as a tab.",
            TextStyle {
                font_size: 13.0,
                line_height: 18.0,
                color: palette.placeholder,
                ..TextStyle::default()
            },
        );
    }

    fn paint_picker_descriptions(&self, ctx: &mut PaintCtx, theme: &DefaultTheme) {
        let palette = theme.palette;
        for (index, demo) in self.demos.iter().enumerate() {
            let Some(tile) = self.demo_buttons.as_slice().get(index) else {
                continue;
            };
            let rect = tile.bounds();
            if rect.is_empty() {
                continue;
            }
            ctx.draw_text(
                Rect::new(rect.x() + 16.0, rect.y() + 46.0, rect.width() - 32.0, 18.0),
                demo.description,
                TextStyle {
                    font_size: 11.0,
                    line_height: 15.0,
                    color: palette.accent_text.with_alpha(0.82),
                    ..TextStyle::default()
                },
            );
        }
    }
}

fn dev_shell_tab_semantics_id(parent: WidgetId, demo_index: usize) -> WidgetId {
    const TAG: u64 = 2_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(313)
            .wrapping_add(demo_index as u64 + 1)
            & LOW_MASK),
    )
}

fn dev_shell_tab_close_semantics_id(parent: WidgetId, demo_index: usize) -> WidgetId {
    const TAG: u64 = 2_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(313)
            .wrapping_add(10_000 + demo_index as u64)
            & LOW_MASK),
    )
}

impl Widget for DevBrowserShell {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Target {
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.tab_at(pointer.position);
                let hovered_close = self.tab_close_at(pointer.position);
                if self.hovered_tab != hovered || self.hovered_close_tab != hovered_close {
                    self.hovered_tab = hovered;
                    self.hovered_close_tab = hovered_close;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered_tab.take().is_some() || self.hovered_close_tab.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
                if self.pressed_tab.is_some() && pointer.buttons.is_empty() {
                    self.pressed_tab = None;
                }
                if self.pressed_close_tab.is_some() && pointer.buttons.is_empty() {
                    self.pressed_close_tab = None;
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(tab) = self.tab_close_at(pointer.position) {
                    self.pressed_close_tab = Some(tab);
                    self.hovered_close_tab = Some(tab);
                    self.hovered_tab = Some(tab);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if let Some(tab) = self.tab_at(pointer.position) {
                    self.pressed_tab = Some(tab);
                    self.hovered_tab = Some(tab);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(pressed) = self.pressed_close_tab.take() {
                    let hovered_close = self.tab_close_at(pointer.position);
                    if hovered_close == Some(pressed) {
                        self.state.close_tab(pressed);
                    }
                    self.hovered_tab = self.tab_at(pointer.position);
                    self.hovered_close_tab = hovered_close;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    request_window_refresh(ctx, true);
                    ctx.set_handled();
                } else if let Some(pressed) = self.pressed_tab.take() {
                    let hovered = self.tab_at(pointer.position);
                    if hovered == Some(pressed) {
                        self.state.select_tab(pressed);
                    }
                    self.hovered_tab = hovered;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    request_window_refresh(ctx, true);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_tab.take().is_some() || self.pressed_close_tab.take().is_some() {
                    self.hovered_tab = None;
                    self.hovered_close_tab = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    request_window_refresh(ctx, true);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                match key.key.as_str() {
                    "ArrowLeft" => self.select_adjacent_tab(-1),
                    "ArrowRight" => self.select_adjacent_tab(1),
                    "Home" => {
                        if let Some(first) = self.state.open_tabs().first().copied() {
                            self.state.select_tab(first);
                        }
                    }
                    "End" => {
                        if let Some(last) = self.state.open_tabs().last().copied() {
                            self.state.select_tab(last);
                        }
                    }
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

        let tabs = self.state.open_tabs();
        let label_style = self.state.theme().body_text_style();
        self.tab_widths = tabs
            .iter()
            .map(|index| {
                ctx.layout()
                    .measure_text(self.demos[*index].title, label_style.clone())
                    .map(|measurement| {
                        measurement.width
                            + 12.0
                            + 6.0
                            + DEV_SHELL_TAB_CLOSE_SIZE
                            + DEV_SHELL_TAB_CLOSE_MARGIN
                    })
                    .unwrap_or(132.0)
                    .clamp(118.0, 240.0)
            })
            .collect();

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
            for index in 0..self.demo_buttons.len() {
                self.demo_buttons.measure_child(
                    index,
                    ctx,
                    Constraints::new(
                        Size::new(160.0, DEV_SHELL_PICKER_TILE_HEIGHT),
                        Size::new(260.0, DEV_SHELL_PICKER_TILE_HEIGHT),
                    ),
                );
            }
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
        let plus_rect = self.arrange_tab_strip(bounds);
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
            let columns: usize = if grid.width() >= 840.0 {
                3
            } else if grid.width() >= 560.0 {
                2
            } else {
                1
            };
            let gap = 14.0;
            let column_width = ((grid.width() - (gap * (columns.saturating_sub(1) as f32)))
                / columns as f32)
                .max(160.0);
            for index in 0..self.demo_buttons.len() {
                let column = index % columns;
                let row = index / columns;
                let rect = Rect::new(
                    grid.x() + column as f32 * (column_width + gap),
                    grid.y() + row as f32 * (DEV_SHELL_PICKER_TILE_HEIGHT + gap),
                    column_width,
                    DEV_SHELL_PICKER_TILE_HEIGHT,
                );
                self.demo_buttons.arrange_child(index, ctx, rect);
            }
        } else {
            for index in 0..self.demo_buttons.len() {
                self.demo_buttons.arrange_child(index, ctx, Rect::ZERO);
            }
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
            self.demo_buttons.paint(ctx);
            self.paint_picker_descriptions(ctx, &theme);
        } else if let Some(active) = self.state.active_tab() {
            self.demos[active].child.paint(ctx);
        }

        self.paint_toolbar(ctx, &theme);
        self.plus_button.paint(ctx);
        self.theme_toggle.paint(ctx);
        self.settings_window.paint(ctx);
        self.main_menu.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tabs, ctx.bounds());
        node.name = Some("SUI dev browser".to_string());
        node.value = self
            .state
            .active_tab()
            .map(|index| SemanticsValue::Text(self.demos[index].title.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (demo_index, rect) in &self.tab_rects {
            let tab_id = dev_shell_tab_semantics_id(ctx.widget_id(), *demo_index);
            let mut tab = SemanticsNode::new(tab_id, SemanticsRole::Button, *rect);
            tab.parent = Some(ctx.widget_id());
            tab.name = Some(self.demos[*demo_index].title.to_string());
            tab.state.selected = self.state.active_tab() == Some(*demo_index);
            tab.state.hovered = self.hovered_tab == Some(*demo_index);
            tab.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(tab);

            let mut close = SemanticsNode::new(
                dev_shell_tab_close_semantics_id(ctx.widget_id(), *demo_index),
                SemanticsRole::Button,
                Self::tab_close_rect(*rect),
            );
            close.parent = Some(tab_id);
            close.name = Some(format!("Close {} tab", self.demos[*demo_index].title));
            close.state.hovered = self.hovered_close_tab == Some(*demo_index);
            close.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(close);
        }

        self.plus_button.semantics(ctx);
        self.theme_toggle.semantics(ctx);
        if self.state.picker_visible() {
            self.demo_buttons.semantics(ctx);
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
        if self.state.picker_visible() {
            self.demo_buttons.visit_children(visitor);
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
        if self.state.picker_visible() {
            self.demo_buttons.visit_children_mut(visitor);
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

struct SuiLogoButton;

impl Widget for SuiLogoButton {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            DEV_SHELL_LOGO_BUTTON_SIZE,
            DEV_SHELL_LOGO_BUTTON_SIZE,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let center = Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.5,
        );
        let radius = bounds.width().min(bounds.height()) * 0.48;
        ctx.fill(Path::circle(center, radius), Color::WHITE);
        ctx.stroke(
            Path::circle(center, radius - 0.5),
            Color::rgba(0.08, 0.14, 0.20, 0.18),
            StrokeStyle::new(1.0),
        );

        let inner_radius = radius - 3.6;
        ctx.fill(
            Path::circle(center, inner_radius),
            Color::rgba(0.05, 0.52, 0.66, 1.0),
        );
        let wave_bounds = Rect::new(
            center.x - inner_radius,
            center.y - inner_radius,
            inner_radius * 2.0,
            inner_radius * 2.0,
        );
        draw_logo_wave(ctx, wave_bounds, 0.32, Color::rgba(0.48, 0.86, 0.93, 1.0));
        draw_logo_wave(ctx, wave_bounds, 0.52, Color::rgba(0.14, 0.64, 0.76, 1.0));
        draw_logo_wave(ctx, wave_bounds, 0.70, Color::rgba(0.02, 0.36, 0.50, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("SUI menu".to_string());
        node.actions = vec![SemanticsAction::Activate];
        ctx.push(node);
    }
}

struct ThemeToggleButton {
    state: DevShellState,
    hovered: bool,
    pressed: bool,
}

impl ThemeToggleButton {
    fn new(state: DevShellState) -> Self {
        Self {
            state,
            hovered: false,
            pressed: false,
        }
    }

    fn activate(&self, ctx: &mut EventCtx) {
        self.state.toggle_theme();
        request_window_refresh(ctx, true);
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
        let dark = self.state.is_dark();
        let theme = self.state.theme();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let background = if dark {
            Color::rgba(0.11, 0.15, 0.20, 1.0)
        } else {
            Color::rgba(0.97, 0.985, 1.0, 1.0)
        };
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

        let knob_x = if dark {
            bounds.max_x() - 31.0
        } else {
            bounds.x() + 3.0
        };
        let knob = Rect::new(knob_x, bounds.y() + 3.0, 28.0, 28.0);
        ctx.fill(
            Path::circle(
                Point::new(
                    knob.x() + knob.width() * 0.5,
                    knob.y() + knob.height() * 0.5,
                ),
                14.0,
            ),
            if dark {
                Color::rgba(0.33, 0.74, 0.88, 1.0)
            } else {
                Color::rgba(0.98, 0.74, 0.24, 1.0)
            },
        );
        let label_rect = if dark {
            Rect::new(
                bounds.x() + 12.0,
                bounds.y() + 8.0,
                bounds.width() - 48.0,
                18.0,
            )
        } else {
            Rect::new(
                bounds.x() + 38.0,
                bounds.y() + 8.0,
                bounds.width() - 50.0,
                18.0,
            )
        };
        ctx.draw_text(
            label_rect,
            if dark { "Dark" } else { "Light" },
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
        node.name = Some("Dark theme".to_string());
        node.state.checked = Some(if self.state.is_dark() {
            ToggleState::Checked
        } else {
            ToggleState::Unchecked
        });
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
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
        let content_palette = DefaultTheme::default().palette;
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
        ctx.fill(
            Path::rounded_rect(
                Rect::new(title.x(), title.y(), title.width(), title.height() + 8.0),
                8.0,
            ),
            if self.state.is_dark() {
                Color::rgba(0.12, 0.16, 0.21, 1.0)
            } else {
                Color::rgba(0.16, 0.20, 0.26, 1.0)
            },
        );
        ctx.fill_rect(
            Rect::new(title.x(), title.max_y() - 8.0, title.width(), 8.0),
            if self.state.is_dark() {
                Color::rgba(0.12, 0.16, 0.21, 1.0)
            } else {
                Color::rgba(0.16, 0.20, 0.26, 1.0)
            },
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

fn build_dev_demo_entries() -> Vec<DevDemo> {
    vec![
        DevDemo {
            title: WIDGET_BOOK_TAB_LABEL,
            description: "Catalog of controls, containers, media, and text surfaces.",
            child: WidgetPod::new(build_widget_book_gallery(default_widget_book_state())),
        },
        DevDemo {
            title: THEMES_TAB_LABEL,
            description: "Theme previews and HDR theme mode comparisons.",
            child: WidgetPod::new(build_theme_demo_surface(default_widget_book_state())),
        },
        DevDemo {
            title: BUTTON_GRID_TAB_LABEL,
            description: "Dense button grid used for interaction and resizing performance checks.",
            child: WidgetPod::new(build_button_grid_benchmark()),
        },
        DevDemo {
            title: RETAINED_TEXT_TAB_LABEL,
            description: "Retained text layout and redraw benchmark.",
            child: WidgetPod::new(build_retained_text_benchmark()),
        },
        DevDemo {
            title: TEXT_RENDERING_COMPARISON_TAB_LABEL,
            description: "Side-by-side text rendering comparison surface.",
            child: WidgetPod::new(build_text_rendering_comparison_surface()),
        },
        DevDemo {
            title: TEXT_VALIDATION_TAB_LABEL,
            description: "Validation surface for text metrics, alignment, and rasterization.",
            child: WidgetPod::new(build_text_validation_surface()),
        },
        DevDemo {
            title: TEXT_EDITING_TAB_LABEL,
            description: "Single-line and multi-line text editing demos.",
            child: WidgetPod::new(build_text_editing_benchmark()),
        },
        DevDemo {
            title: HDR_VALIDATION_TAB_LABEL,
            description: "HDR, color-management, and tone-mapping validation surface.",
            child: WidgetPod::new(build_color_validation_surface()),
        },
        DevDemo {
            title: PAINT_TAB_LABEL,
            description: "Pixel canvas painting demo.",
            child: WidgetPod::new(build_paint_demo()),
        },
        DevDemo {
            title: VECTOR_EDITOR_TAB_LABEL,
            description: "Vector canvas drawing and editing demo.",
            child: WidgetPod::new(build_vector_editor_demo()),
        },
    ]
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

fn draw_logo_wave(ctx: &mut PaintCtx, bounds: Rect, top_fraction: f32, color: Color) {
    let radius = bounds.width().min(bounds.height()) * 0.5;
    let center = Point::new(
        bounds.x() + bounds.width() * 0.5,
        bounds.y() + bounds.height() * 0.5,
    );
    let top = (bounds.y() + bounds.height() * top_fraction)
        .clamp(center.y - radius + 0.5, center.y + radius - 0.5);
    let chord_half_width = (radius * radius - (top - center.y).powi(2)).sqrt();
    let left = center.x - chord_half_width;
    let right = center.x + chord_half_width;
    let width = right - left;
    let amp = bounds.height() * 0.105;
    let mut path = PathBuilder::new();
    path.move_to(Point::new(left, top));
    path.cubic_to(
        Point::new(left + width * 0.18, top - amp),
        Point::new(left + width * 0.30, top - amp),
        Point::new(left + width * 0.50, top),
    );
    path.cubic_to(
        Point::new(left + width * 0.70, top + amp),
        Point::new(left + width * 0.82, top + amp),
        Point::new(right, top),
    );
    path.line_to(Point::new(center.x, center.y + radius));
    path.line_to(Point::new(left, top));
    path.close();
    ctx.fill(path.build(), color);
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

struct SdrContentBrightnessStatus;

impl Widget for SdrContentBrightnessStatus {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(constraints.max.width.min(420.0), 34.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = DefaultTheme::default().palette;
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
    fn default_options() -> WindowRenderOptions {
        let renderer = WgpuRenderer::new();
        WindowRenderOptions::new(renderer.feathering_enabled(), renderer.feather_width())
            .with_text_hinting(window_text_hinting_from_renderer(renderer.text_hinting()))
            .with_stem_darkening(window_stem_darkening_from_renderer(
                renderer.stem_darkening(),
            ))
    }

    fn with_initial_options(initial: WindowRenderOptions) -> Self {
        let state = Rc::new(RefCell::new(initial));
        let toggle_state = Rc::clone(&state);
        let width_state = Rc::clone(&state);
        let text_centering_state = Rc::clone(&state);
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
        let system_sdr_content_brightness_state = Rc::clone(&state);
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
                            "These controls update the active window's runtime presentation on the next redraw.",
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
                    .with_child(labeled_settings_control(
                        FEATHER_WIDTH_NAME,
                        220.0,
                        NumberInput::new(FEATHER_WIDTH_NAME)
                            .range(0.0, 8.0)
                            .step(0.05)
                            .precision(2)
                            .value(initial.feather_width as f64)
                            .on_change(move |value| {
                                width_state.borrow_mut().feather_width = value.max(0.0) as f32;
                            }),
                    ))
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
                    .with_child(labeled_settings_control(
                        TEXT_HINTING_MAX_PPEM_NAME,
                        220.0,
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
                    ))
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
                    .with_child(labeled_settings_control(
                        STEM_DARKENING_AMOUNT_NAME,
                        220.0,
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
                    ))
                    .with_child(labeled_settings_control(
                        STEM_DARKENING_MAX_PPEM_NAME,
                        220.0,
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
                    ))
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
                        420.0,
                        Stack::vertical()
                            .spacing(8.0)
                            .alignment(Alignment::Start)
                            .with_child(SizedBox::new().width(220.0).with_child(
                                NumberInput::new(SDR_CONTENT_BRIGHTNESS_NAME)
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
                                    .checked(initial.use_system_sdr_content_brightness)
                                    .on_toggle(move |checked| {
                                        system_sdr_content_brightness_state
                                            .borrow_mut()
                                            .use_system_sdr_content_brightness = checked;
                                    }),
                            )
                            .with_child(SdrContentBrightnessStatus),
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
                            "Optical centering uses cap height when available and a softened descent bias for Latin UI labels. Atlas glyphs are always snapped to physical pixels; fractional glyph phase is handled by quarter-pixel raster variants. The render policy applies to both atlas and fallback glyph coverage; the gamma input is only used when the Gamma policy is selected. Slight hinting biases small-text rasterization below the configured ppem threshold. Stem darkening slightly boosts thin small-text coverage below its threshold. Phase 2 controls choose the preferred color-management policy, the HDR theme selector drives the shared widget-book preview mode, and the inspection panels show the detected monitor/output path after each redraw.",
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

fn build_render_settings_tab_with_options(options: WindowRenderOptions) -> impl Widget {
    RenderSettingsTab::with_initial_options(options)
}

fn build_paint_demo() -> PixelCanvas {
    let width = 1920;
    let height = 1080;
    PixelCanvas::from_fn(PAINT_TAB_LABEL, width, height, |x, y| {
        let u = x as f32 / (width - 1) as f32;
        let v = y as f32 / (height - 1) as f32;
        let dx = u - 0.5;
        let dy = v - 0.5;
        let vignette = (1.0 - ((dx * dx + dy * dy).sqrt() * 1.45)).clamp(0.0, 1.0);
        let wave = ((u * 18.0).sin() * (v * 11.0).cos() * 0.5) + 0.5;
        Color::rgba(
            0.08 + (0.58 * u) + (0.18 * vignette),
            0.18 + (0.42 * v) + (0.14 * wave),
            0.38 + (0.36 * (1.0 - u)) + (0.20 * vignette),
            1.0,
        )
    })
    .brush_color(Color::rgba(0.08, 0.22, 0.78, 1.0))
    .viewport(CanvasViewport::new().zoom(0.28))
}

fn build_vector_editor_demo() -> Canvas {
    let mut curve = PathBuilder::new();
    curve.move_to(Point::new(-160.0, 72.0)).cubic_to(
        Point::new(-96.0, -96.0),
        Point::new(88.0, 124.0),
        Point::new(160.0, -64.0),
    );

    Canvas::new(VECTOR_EDITOR_TAB_LABEL)
        .viewport(CanvasViewport::new().zoom(1.05))
        .draw_stroke(CanvasStroke::new(Color::rgba(0.12, 0.28, 0.84, 1.0), 3.0))
        .shape(CanvasShape::rect(
            Rect::new(-180.0, -110.0, 360.0, 220.0),
            Some(Color::rgba(1.0, 1.0, 1.0, 0.82)),
            Some(CanvasStroke::new(Color::rgba(0.12, 0.16, 0.22, 1.0), 2.0)),
        ))
        .shape(CanvasShape::circle(
            Point::new(-72.0, -28.0),
            46.0,
            Some(Color::rgba(0.18, 0.54, 0.86, 0.78)),
            Some(CanvasStroke::new(Color::rgba(0.08, 0.22, 0.42, 1.0), 2.0)),
        ))
        .shape(CanvasShape::circle(
            Point::new(82.0, 34.0),
            58.0,
            Some(Color::rgba(0.94, 0.58, 0.16, 0.72)),
            Some(CanvasStroke::new(Color::rgba(0.42, 0.22, 0.06, 1.0), 2.0)),
        ))
        .shape(CanvasShape::path(curve.build()))
}

pub(crate) fn build_dev_application_with_widget_book_bounds_and_render_options(
    _widget_book_bounds: Rect,
    render_options: WindowRenderOptions,
) -> Application {
    finish_dev_application(DevBrowserShell::new(render_options))
}

#[cfg(not(target_arch = "wasm32"))]
fn build_dev_application_with_render_options_and_automation(
    render_options: WindowRenderOptions,
    automation: Option<DesktopAutomationMode>,
) -> Application {
    let initial_demo = automation.map(|mode| match mode {
        DesktopAutomationMode::ButtonGridResize => BUTTON_GRID_TAB_LABEL,
        DesktopAutomationMode::WidgetBookScroll => WIDGET_BOOK_TAB_LABEL,
    });
    finish_dev_application(DevBrowserShell::with_initial_demo(
        render_options,
        initial_demo,
    ))
}

fn finish_dev_application<W: Widget + 'static>(root: W) -> Application {
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

    use std::{
        collections::BTreeSet,
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use sui::{
        Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect, Result,
        SceneStatisticsDetailMode, SemanticsNode, SemanticsRole, StackOrderPolicy, Vector,
        WidgetId, WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent,
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

    fn build_floating_button_grid_test_application(initial_bounds: Rect) -> Application {
        let workspace = FloatingWorkspaceState::new();
        let mut views = FloatingWorkspace::new(workspace).name("Button grid floating regression");
        views.push_view(
            FloatingViewConfig::new(BUTTON_GRID_TAB_LABEL, initial_bounds)
                .min_size(Size::new(280.0, 220.0)),
            build_button_grid_benchmark(),
        );
        finish_dev_application(views)
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
            .with_name(sui_widget_book::COLOR_VALIDATION_SCROLL_NAME)
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
            BUTTON_GRID_TAB_LABEL,
            HDR_VALIDATION_TAB_LABEL,
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
                node.role == SemanticsRole::Tabs && node.name.as_deref() == Some("SUI dev browser")
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
    fn dev_shell_tab_semantics_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(17);
        let mut ids = BTreeSet::new();
        for demo_index in 0..12 {
            for id in [
                dev_shell_tab_semantics_id(parent, demo_index).get(),
                dev_shell_tab_close_semantics_id(parent, demo_index).get(),
            ] {
                assert!(id <= (1_u64 << 53) - 1, "{id} should be JS-safe");
                assert!(ids.insert(id), "{id} should be unique");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_automation_builder_opens_requested_demo() {
        for (mode, title) in [
            (
                DesktopAutomationMode::ButtonGridResize,
                BUTTON_GRID_TAB_LABEL,
            ),
            (
                DesktopAutomationMode::WidgetBookScroll,
                WIDGET_BOOK_TAB_LABEL,
            ),
        ] {
            let mut runtime = build_dev_application_with_automation(Some(mode))
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
                    node.role == SemanticsRole::Tabs
                        && node.name.as_deref() == Some("SUI dev browser")
                })
                .expect("desktop dev shell tab semantics should exist");
            assert_eq!(shell.value, Some(SemanticsValue::Text(title.to_string())));
        }
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
            sui_widget_book::build_color_validation_application()
                .with_window_render_options(options)
        })?;
        let window = app.main_window()?;
        let scroll = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(sui_widget_book::COLOR_VALIDATION_SCROLL_NAME);
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
            COLOR_MANAGEMENT_MODE_NAME,
            OUTPUT_PRIMARIES_NAME,
            DYNAMIC_RANGE_MODE_NAME,
            TONE_MAPPING_MODE_NAME,
            SDR_CONTENT_BRIGHTNESS_NAME,
            USE_SYSTEM_SDR_BRIGHTNESS_LABEL,
            HDR_THEME_MODE_NAME,
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

    fn assert_dev_shell_active_tab(window: &TestWindow, title: &str) -> Result<()> {
        let snapshot = window.snapshot()?;
        let shell = find_named_node(&snapshot, SemanticsRole::Tabs, "SUI dev browser");
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

    #[test]
    fn button_grid_resize_stays_at_stable_60_fps_in_dev_workspace() -> Result<()> {
        const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
        const DRAG_STEPS: usize = 28;
        const WARMUP_SAMPLES: usize = 4;

        let app = TestApp::new(|| {
            build_floating_button_grid_test_application(Rect::new(560.0, 72.0, 420.0, 340.0))
                .build()
        })?;
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
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .new_count as f64
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
        let avg_packet_rebuild_signature = measured_samples
            .iter()
            .map(|sample| {
                sample
                    .renderer_submission
                    .retained_packet_rebuilds
                    .signature_count as f64
            })
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
