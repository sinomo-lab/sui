use std::{cell::RefCell, rc::Rc};

use sui::{SemanticTone, StatusBadge, prelude::*};

use crate::app::{
    DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color, request_window_refresh,
};

pub(crate) const THEME_EDITOR_TAB_LABEL: &str = "Theme editor";
pub(crate) const THEME_EDITOR_CONTROLS_SCROLL_NAME: &str = "Theme editor controls";
pub(crate) const THEME_EDITOR_PREVIEW_SCROLL_NAME: &str = "Theme editor preview";

const THEME_PRESET_NAME: &str = "Theme preset";
pub(crate) const THEME_COLOR_PICKER_NAME: &str = "Selected theme color picker";
const THEME_CONTROL_SIZE_NAME: &str = "Control size";
pub(crate) const THEME_SPACING_NAME: &str = "Base spacing";
const THEME_RADIUS_SCALE_NAME: &str = "Corner radius scale";
const THEME_TEXT_SCALE_NAME: &str = "Typography scale";
const THEME_MOTION_SCALE_NAME: &str = "Motion speed";
const THEME_RESET_NAME: &str = "Reset current preset";
const THEME_PRESET_OPTIONS: [&str; 5] = [
    "SUI light",
    "Neutral light",
    "SUI dark",
    "Neutral dark",
    "SUI true black",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeEditorPreset {
    SuiLight,
    Neutral,
    SuiDark,
    NeutralDark,
    SuiTrueBlack,
}

impl ThemeEditorPreset {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Neutral,
            2 => Self::SuiDark,
            3 => Self::NeutralDark,
            4 => Self::SuiTrueBlack,
            _ => Self::SuiLight,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::SuiLight => 0,
            Self::Neutral => 1,
            Self::SuiDark => 2,
            Self::NeutralDark => 3,
            Self::SuiTrueBlack => 4,
        }
    }

    fn theme(self) -> DefaultTheme {
        match self {
            Self::SuiLight => DefaultTheme::sui(),
            Self::Neutral => DefaultTheme::neutral(),
            Self::SuiDark => DefaultTheme::dark(),
            Self::NeutralDark => DefaultTheme::neutral_dark(),
            Self::SuiTrueBlack => DefaultTheme::high_contrast(),
        }
    }

    fn label(self) -> &'static str {
        THEME_PRESET_OPTIONS[self.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeColorVariable {
    Surface,
    SurfaceSubtle,
    Border,
    Text,
    Primary,
    Secondary,
    Info,
    Success,
    Warning,
    Danger,
}

impl ThemeColorVariable {
    const ALL: [Self; 10] = [
        Self::Surface,
        Self::SurfaceSubtle,
        Self::Border,
        Self::Text,
        Self::Primary,
        Self::Secondary,
        Self::Info,
        Self::Success,
        Self::Warning,
        Self::Danger,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::SurfaceSubtle => "Surface subtle",
            Self::Border => "Border",
            Self::Text => "Text",
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Info => "Info",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Danger => "Danger",
        }
    }

    const fn color(self, colors: &ThemeColors) -> Color {
        match self {
            Self::Surface => colors.base_100,
            Self::SurfaceSubtle => colors.base_200,
            Self::Border => colors.base_300,
            Self::Text => colors.base_content,
            Self::Primary => colors.primary,
            Self::Secondary => colors.secondary,
            Self::Info => colors.info,
            Self::Success => colors.success,
            Self::Warning => colors.warning,
            Self::Danger => colors.error,
        }
    }

    fn set_color(self, colors: &mut ThemeColors, color: Color) {
        let color = color.clamped().with_alpha(1.0);
        match self {
            Self::Surface => colors.base_100 = color,
            Self::SurfaceSubtle => colors.base_200 = color,
            Self::Border => colors.base_300 = color,
            Self::Text => colors.base_content = color,
            Self::Primary => {
                colors.primary = color;
                colors.accent = color;
                let content = readable_content_color(color);
                colors.primary_content = content;
                colors.accent_content = content;
            }
            Self::Secondary => {
                colors.secondary = color;
                colors.secondary_content = readable_content_color(color);
            }
            Self::Info => {
                colors.info = color;
                colors.info_content = readable_content_color(color);
            }
            Self::Success => {
                colors.success = color;
                colors.success_content = readable_content_color(color);
            }
            Self::Warning => {
                colors.warning = color;
                colors.warning_content = readable_content_color(color);
            }
            Self::Danger => {
                colors.error = color;
                colors.error_content = readable_content_color(color);
            }
        }
    }
}

fn readable_content_color(color: Color) -> Color {
    let linear = color.to_linear_srgb();
    let luminance = 0.2126 * linear.red + 0.7152 * linear.green + 0.0722 * linear.blue;
    if luminance > 0.38 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

#[derive(Clone)]
struct ThemeEditorState {
    inner: Rc<RefCell<ThemeEditorStateInner>>,
}

struct ThemeEditorStateInner {
    theme: DefaultTheme,
    preset: ThemeEditorPreset,
    selected_color: ThemeColorVariable,
    control_size: ControlSize,
    radius_scale: f32,
    text_scale: f32,
    motion_scale: f32,
    controls_scroll: ScrollState,
    preview_scroll: ScrollState,
}

impl ThemeEditorState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ThemeEditorStateInner {
                theme: DefaultTheme::sui(),
                preset: ThemeEditorPreset::SuiLight,
                selected_color: ThemeColorVariable::Primary,
                control_size: ControlSize::Medium,
                radius_scale: 1.0,
                text_scale: 1.0,
                motion_scale: 1.0,
                controls_scroll: ScrollState::new(),
                preview_scroll: ScrollState::new(),
            })),
        }
    }

    fn theme(&self) -> DefaultTheme {
        self.inner.borrow().theme
    }

    fn theme_reader(&self) -> DevThemeReader {
        let state = self.clone();
        Rc::new(move || state.theme())
    }

    fn controls_scroll_state(&self) -> ScrollState {
        self.inner.borrow().controls_scroll.clone()
    }

    fn preview_scroll_state(&self) -> ScrollState {
        self.inner.borrow().preview_scroll.clone()
    }

    fn preset_index(&self) -> usize {
        self.inner.borrow().preset.index()
    }

    fn set_preset(&self, index: usize) {
        let preset = ThemeEditorPreset::from_index(index);
        let mut inner = self.inner.borrow_mut();
        inner.theme = preset.theme();
        inner.preset = preset;
        inner.control_size = ControlSize::Medium;
        inner.radius_scale = 1.0;
        inner.text_scale = 1.0;
        inner.motion_scale = 1.0;
    }

    fn reset_current_preset(&self) {
        let index = self.preset_index();
        self.set_preset(index);
    }

    fn select_color_variable(&self, variable: ThemeColorVariable) {
        self.inner.borrow_mut().selected_color = variable;
    }

    fn is_selected_color(&self, variable: ThemeColorVariable) -> bool {
        self.inner.borrow().selected_color == variable
    }

    fn color_variable(&self, variable: ThemeColorVariable) -> Color {
        let inner = self.inner.borrow();
        variable.color(&inner.theme.colors)
    }

    fn selected_color(&self) -> Color {
        let inner = self.inner.borrow();
        inner.selected_color.color(&inner.theme.colors)
    }

    fn selected_color_summary(&self) -> String {
        let inner = self.inner.borrow();
        let color = inner.selected_color.color(&inner.theme.colors).clamped();
        format!(
            "{}  #{:02X}{:02X}{:02X}",
            inner.selected_color.label(),
            (color.red * 255.0).round() as u8,
            (color.green * 255.0).round() as u8,
            (color.blue * 255.0).round() as u8,
        )
    }

    fn set_selected_color(&self, color: Color) {
        let mut inner = self.inner.borrow_mut();
        let selected = inner.selected_color;
        selected.set_color(&mut inner.theme.colors, color);
        inner.theme.sync_derived_fields();
    }

    fn control_size_index(&self) -> usize {
        match self.inner.borrow().control_size {
            ControlSize::Small => 0,
            ControlSize::Medium => 1,
            ControlSize::Large => 2,
        }
    }

    fn set_control_size(&self, index: usize) {
        let size = match index {
            0 => ControlSize::Small,
            2 => ControlSize::Large,
            _ => ControlSize::Medium,
        };
        let mut inner = self.inner.borrow_mut();
        inner.control_size = size;
        inner.theme = inner.theme.with_size(size);
    }

    fn spacing(&self) -> f64 {
        f64::from(self.inner.borrow().theme.spacing)
    }

    fn set_spacing(&self, spacing: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.theme.spacing = spacing.clamp(2.0, 12.0);
        inner.theme.sync_derived_fields();
    }

    fn radius_scale(&self) -> f64 {
        f64::from(self.inner.borrow().radius_scale)
    }

    fn set_radius_scale(&self, scale: f32) {
        let scale = scale.clamp(0.0, 2.0);
        let mut inner = self.inner.borrow_mut();
        inner.radius_scale = scale;
        inner.theme.radius = scaled_radii(scale);
        inner.theme.sync_derived_fields();
    }

    fn text_scale(&self) -> f64 {
        f64::from(self.inner.borrow().text_scale)
    }

    fn set_text_scale(&self, scale: f32) {
        let scale = scale.clamp(0.75, 1.5);
        let mut inner = self.inner.borrow_mut();
        inner.text_scale = scale;
        inner.theme.text = scaled_text_scale(scale);
        inner.theme.sync_derived_fields();
    }

    fn motion_scale(&self) -> f64 {
        f64::from(self.inner.borrow().motion_scale)
    }

    fn set_motion_scale(&self, scale: f32) {
        let scale = scale.clamp(0.0, 2.0);
        let mut inner = self.inner.borrow_mut();
        inner.motion_scale = scale;
        inner.theme.motion = scaled_motion(scale);
    }

    fn theme_summary(&self) -> String {
        let inner = self.inner.borrow();
        let preset = inner.preset.label();
        let size = match inner.control_size {
            ControlSize::Small => "small",
            ControlSize::Medium => "medium",
            ControlSize::Large => "large",
        };
        format!(
            "{preset} · {size} controls · {:.0}% type · {:.0}% motion",
            inner.text_scale * 100.0,
            inner.motion_scale * 100.0,
        )
    }
}

fn scaled_radii(scale: f32) -> ThemeRadii {
    let base = ThemeRadii::default();
    ThemeRadii {
        xs: base.xs * scale,
        sm: base.sm * scale,
        md: base.md * scale,
        lg: base.lg * scale,
        xl: base.xl * scale,
        _2xl: base._2xl * scale,
        _3xl: base._3xl * scale,
        _4xl: base._4xl,
    }
}

fn scaled_text_token(token: ThemeTextToken, scale: f32) -> ThemeTextToken {
    ThemeTextToken {
        size: token.size * scale,
        line_height: token.line_height * scale,
    }
}

fn scaled_text_scale(scale: f32) -> ThemeTextScale {
    let base = ThemeTextScale::default();
    ThemeTextScale {
        xs: scaled_text_token(base.xs, scale),
        sm: scaled_text_token(base.sm, scale),
        base: scaled_text_token(base.base, scale),
        lg: scaled_text_token(base.lg, scale),
        xl: scaled_text_token(base.xl, scale),
        _2xl: scaled_text_token(base._2xl, scale),
        _3xl: scaled_text_token(base._3xl, scale),
        _4xl: scaled_text_token(base._4xl, scale),
        _5xl: scaled_text_token(base._5xl, scale),
        _6xl: scaled_text_token(base._6xl, scale),
        _7xl: scaled_text_token(base._7xl, scale),
        _8xl: scaled_text_token(base._8xl, scale),
        _9xl: scaled_text_token(base._9xl, scale),
    }
}

fn scaled_motion(scale: f32) -> ThemeMotion {
    let mut motion = ThemeMotion::standard();
    motion.duration_fast *= scale;
    motion.duration_normal *= scale;
    motion.duration_slow *= scale;
    motion.duration_slower *= scale;
    motion
}

pub(crate) fn build_theme_editor_demo_with_theme(shell_theme: DevThemeReader) -> impl Widget {
    let state = ThemeEditorState::new();
    let preview_theme = state.theme_reader();
    let split_theme = Rc::clone(&shell_theme);
    let background_theme = Rc::clone(&shell_theme);

    Background::new(
        shell_theme().palette.surface,
        SplitView::horizontal(
            build_editor_controls(state.clone(), Rc::clone(&shell_theme)),
            build_live_preview(state, preview_theme),
        )
        .name("Theme editor workspace")
        .theme_when(clone_dev_theme_reader(&split_theme))
        .ratio(0.34)
        .min_first(330.0)
        .min_second(520.0),
    )
    .brush_when(dev_theme_color(&background_theme, |theme| {
        theme.palette.surface
    }))
}

fn build_editor_controls(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let controls_scroll = state.controls_scroll_state();
    Surface::sidebar(
        ScrollView::vertical(Padding::all(
            16.0,
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(editor_title(
                    "Theme variables",
                    "Edit source tokens; derived palettes and control metrics update automatically.",
                    Rc::clone(&shell_theme),
                ))
                .with_child(build_preset_section(
                    state.clone(),
                    Rc::clone(&shell_theme),
                ))
                .with_child(build_color_section(
                    state.clone(),
                    Rc::clone(&shell_theme),
                ))
                .with_child(build_scale_section(
                    state.clone(),
                    Rc::clone(&shell_theme),
                ))
                .with_child(
                    Button::new(THEME_RESET_NAME)
                        .icon(IconGlyph::Restore)
                        .theme_when(clone_dev_theme_reader(&shell_theme))
                        .on_press_with_ctx(move |ctx| {
                            state.reset_current_preset();
                            request_window_refresh(ctx, false);
                        }),
                ),
        ))
        .state(controls_scroll)
        .name(THEME_EDITOR_CONTROLS_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(&shell_theme)),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
    .fill()
}

fn editor_title(
    title: &'static str,
    description: &'static str,
    theme_reader: DevThemeReader,
) -> impl Widget {
    Stack::vertical()
        .spacing(5.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(title)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.xl,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(
            Label::new(description)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.sm,
                    theme_reader().palette.text_muted,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
}

fn build_preset_section(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let preset_reader = state.clone();
    let preset_change = state.clone();
    let size_reader = state.clone();
    let size_change = state;

    PanelSection::new(
        "Foundation",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                PropertyRow::new(
                    "Preset",
                    Select::new(THEME_PRESET_NAME)
                        .options(THEME_PRESET_OPTIONS)
                        .selected_when(move || Some(preset_reader.preset_index()))
                        .theme_when(clone_dev_theme_reader(&shell_theme))
                        .on_change_with_ctx(move |ctx, index, _| {
                            preset_change.set_preset(index);
                            request_window_refresh(ctx, false);
                        }),
                )
                .theme_when(clone_dev_theme_reader(&shell_theme))
                .stacked(),
            )
            .with_child(
                PropertyRow::new(
                    "Control size",
                    SegmentedControl::new(THEME_CONTROL_SIZE_NAME)
                        .segments(["Small", "Medium", "Large"])
                        .selected_when(move || Some(size_reader.control_size_index()))
                        .theme_when(clone_dev_theme_reader(&shell_theme))
                        .on_change_with_ctx(move |index, _, ctx| {
                            size_change.set_control_size(index);
                            request_window_refresh(ctx, false);
                        }),
                )
                .theme_when(clone_dev_theme_reader(&shell_theme))
                .stacked(),
            ),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
}

fn build_color_section(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let summary_state = state.clone();
    let picker_reader = state.clone();
    let picker_change = state.clone();

    PanelSection::new(
        "Color",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(build_color_swatch_list(state, Rc::clone(&shell_theme)))
            .with_child(
                Label::dynamic("Primary  #000000", move || {
                    summary_state.selected_color_summary()
                })
                .style(dev_text_style(
                    shell_theme(),
                    shell_theme().text.sm,
                    shell_theme().palette.text,
                ))
                .color_when(dev_theme_color(&shell_theme, |theme| theme.palette.text)),
            )
            .with_child(
                SimpleColorPicker::from_color(
                    THEME_COLOR_PICKER_NAME,
                    picker_reader.selected_color(),
                )
                .mode(SimpleColorPickerMode::Rgb)
                .show_alpha(false)
                .color_when(move || picker_reader.selected_color())
                .theme_when(clone_dev_theme_reader(&shell_theme))
                .on_change_with_ctx(move |ctx, color| {
                    picker_change.set_selected_color(color);
                    request_window_refresh(ctx, false);
                }),
            ),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
}

fn build_color_swatch_list(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let mut swatches = Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start);

    for variable in ThemeColorVariable::ALL {
        swatches = swatches.with_item(
            build_color_token_swatch(variable, state.clone(), Rc::clone(&shell_theme)),
            FlexItem::fixed(118.0),
        );
    }

    swatches
}

fn build_color_token_swatch(
    variable: ThemeColorVariable,
    state: ThemeEditorState,
    shell_theme: DevThemeReader,
) -> impl Widget {
    let color_state = state.clone();
    let select_state = state.clone();
    let label_state = state;
    let label_theme = Rc::clone(&shell_theme);
    Stack::vertical()
        .spacing(4.0)
        .alignment(Alignment::Start)
        .with_child(
            ColorSwatch::new(
                theme_color_swatch_name(variable),
                color_state.color_variable(variable),
            )
            .theme_when(clone_dev_theme_reader(&shell_theme))
            .color_when(move || color_state.color_variable(variable))
            .size(Size::new(54.0, 30.0))
            .on_press_with_ctx(move |ctx, _| {
                select_state.select_color_variable(variable);
                request_window_refresh(ctx, false);
            }),
        )
        .with_child(
            Label::new(variable.label())
                .style(dev_text_style(
                    shell_theme(),
                    shell_theme().text.xs,
                    shell_theme().palette.text_muted,
                ))
                .color_when(move || {
                    let theme = label_theme();
                    if label_state.is_selected_color(variable) {
                        theme.palette.accent
                    } else {
                        theme.palette.text_muted
                    }
                }),
        )
}

fn theme_color_swatch_name(variable: ThemeColorVariable) -> String {
    format!("{} theme color", variable.label())
}

fn build_scale_section(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let spacing_reader = state.clone();
    let spacing_change = state.clone();
    let radius_reader = state.clone();
    let radius_change = state.clone();
    let text_reader = state.clone();
    let text_change = state.clone();
    let motion_reader = state.clone();
    let motion_change = state;

    PanelSection::new(
        "Scale",
        Stack::vertical()
            .spacing(9.0)
            .alignment(Alignment::Stretch)
            .with_child(editor_slider_row(
                "Spacing",
                THEME_SPACING_NAME,
                2.0,
                12.0,
                0.5,
                move || spacing_reader.spacing(),
                move |ctx, value| {
                    spacing_change.set_spacing(value as f32);
                    request_window_refresh(ctx, false);
                },
                Rc::clone(&shell_theme),
            ))
            .with_child(editor_slider_row(
                "Corner radius",
                THEME_RADIUS_SCALE_NAME,
                0.0,
                2.0,
                0.05,
                move || radius_reader.radius_scale(),
                move |ctx, value| {
                    radius_change.set_radius_scale(value as f32);
                    request_window_refresh(ctx, false);
                },
                Rc::clone(&shell_theme),
            ))
            .with_child(editor_slider_row(
                "Typography",
                THEME_TEXT_SCALE_NAME,
                0.75,
                1.5,
                0.05,
                move || text_reader.text_scale(),
                move |ctx, value| {
                    text_change.set_text_scale(value as f32);
                    request_window_refresh(ctx, false);
                },
                Rc::clone(&shell_theme),
            ))
            .with_child(editor_slider_row(
                "Motion",
                THEME_MOTION_SCALE_NAME,
                0.0,
                2.0,
                0.05,
                move || motion_reader.motion_scale(),
                move |ctx, value| {
                    motion_change.set_motion_scale(value as f32);
                    request_window_refresh(ctx, false);
                },
                Rc::clone(&shell_theme),
            )),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
}

#[allow(clippy::too_many_arguments)]
fn editor_slider_row<V, C>(
    label: &'static str,
    name: &'static str,
    min: f64,
    max: f64,
    step: f64,
    value: V,
    on_change: C,
    shell_theme: DevThemeReader,
) -> PropertyRow
where
    V: Fn() -> f64 + 'static,
    C: FnMut(&mut EventCtx, f64) + 'static,
{
    PropertyRow::new(
        label,
        Slider::new(name)
            .range(min, max)
            .step(step)
            .value_when(value)
            .theme_when(clone_dev_theme_reader(&shell_theme))
            .on_change_with_ctx(on_change),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
    .stacked()
}

fn build_live_preview(state: ThemeEditorState, preview_theme: DevThemeReader) -> impl Widget {
    let preview_scroll = state.preview_scroll_state();
    Surface::window(
        ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(preview_header(state, Rc::clone(&preview_theme)))
                .with_child(preview_section(
                    "Actions",
                    "Button emphasis and semantic tones",
                    build_preview_actions(Rc::clone(&preview_theme)),
                    Rc::clone(&preview_theme),
                ))
                .with_child(preview_section(
                    "Inputs",
                    "Fields, selection, choice controls, and progress",
                    build_preview_inputs(Rc::clone(&preview_theme)),
                    Rc::clone(&preview_theme),
                ))
                .with_child(preview_section(
                    "Semantic colors",
                    "Derived soft fills and readable status ink",
                    build_preview_statuses(Rc::clone(&preview_theme)),
                    Rc::clone(&preview_theme),
                )),
        ))
        .state(preview_scroll)
        .name(THEME_EDITOR_PREVIEW_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(&preview_theme)),
    )
    .theme_when(clone_dev_theme_reader(&preview_theme))
    .fill()
}

fn preview_header(state: ThemeEditorState, theme_reader: DevThemeReader) -> impl Widget {
    let summary_state = state;
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new("Live preview")
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text._2xl,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(
            Label::dynamic("Light · medium controls", move || {
                summary_state.theme_summary()
            })
            .style(dev_text_style(
                theme_reader(),
                theme_reader().text.sm,
                theme_reader().palette.text_muted,
            ))
            .color_when(dev_theme_color(&theme_reader, |theme| {
                theme.palette.text_muted
            })),
        )
}

fn preview_section<W>(
    title: &'static str,
    description: &'static str,
    body: W,
    theme_reader: DevThemeReader,
) -> impl Widget
where
    W: Widget + 'static,
{
    Surface::panel(
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new(title)
                    .style(dev_text_style(
                        theme_reader(),
                        theme_reader().text.lg,
                        theme_reader().palette.text,
                    ))
                    .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
            )
            .with_child(
                Label::new(description)
                    .style(dev_text_style(
                        theme_reader(),
                        theme_reader().text.sm,
                        theme_reader().palette.text_muted,
                    ))
                    .color_when(dev_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
            )
            .with_child(body),
    )
    .theme_when(clone_dev_theme_reader(&theme_reader))
    .appearance(SurfaceAppearance::Raised)
    .elevation(SurfaceElevation::Small)
    .padding(Insets::all(16.0))
    .fill_width()
}

fn build_preview_actions(theme_reader: DevThemeReader) -> impl Widget {
    Flex::horizontal()
        .gap(9.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center)
        .with_child(
            Button::primary("Create project")
                .icon(IconGlyph::Sparkles)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(Button::new("Secondary").theme_when(clone_dev_theme_reader(&theme_reader)))
        .with_child(
            Button::new("Outlined")
                .appearance(ButtonAppearance::Outline)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(
            Button::danger("Delete")
                .icon(IconGlyph::Trash)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(
            Button::new("Disabled")
                .enabled(false)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
}

fn build_preview_inputs(theme_reader: DevThemeReader) -> impl Widget {
    Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Flex::horizontal()
                .gap(10.0)
                .wrap(FlexWrap::Wrap)
                .with_item(
                    TextInput::new("Preview project name")
                        .placeholder("Project name")
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                    FlexItem::flex(1.0).min_width(220.0),
                )
                .with_item(
                    Select::new("Preview environment")
                        .options(["Production", "Staging", "Development"])
                        .selected(0)
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                    FlexItem::flex(1.0).min_width(190.0),
                ),
        )
        .with_child(
            Flex::horizontal()
                .gap(16.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Center)
                .with_child(
                    Checkbox::new("Include documentation")
                        .checked(true)
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    Switch::new("Automatic updates")
                        .on(true)
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                ),
        )
        .with_child(
            PropertyRow::new(
                "Capacity",
                Slider::new("Preview capacity")
                    .range(0.0, 100.0)
                    .step(1.0)
                    .value(68.0)
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
            )
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .inline()
            .label_width(88.0),
        )
        .with_child(
            ProgressBar::new("Preview progress")
                .value(0.68)
                .show_value(true)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
}

fn build_preview_statuses(theme_reader: DevThemeReader) -> impl Widget {
    Flex::horizontal()
        .gap(10.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center)
        .with_child(
            StatusBadge::new("Information")
                .tone(SemanticTone::Info)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(
            StatusBadge::new("Connected")
                .icon(IconGlyph::Check)
                .tone(SemanticTone::Success)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(
            StatusBadge::new("Attention")
                .icon(IconGlyph::Alert)
                .tone(SemanticTone::Warning)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
        .with_child(
            StatusBadge::new("Unavailable")
                .tone(SemanticTone::Danger)
                .theme_when(clone_dev_theme_reader(&theme_reader)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_edits_refresh_derived_palette_and_readable_content() {
        let state = ThemeEditorState::new();
        state.select_color_variable(ThemeColorVariable::Primary);
        state.set_selected_color(Color::rgba(0.95, 0.85, 0.20, 1.0));

        let theme = state.theme();
        assert_eq!(theme.palette.accent, theme.colors.primary);
        assert_eq!(theme.colors.primary_content, Color::BLACK);
        assert_eq!(theme.palette.accent_text, Color::BLACK);
    }

    #[test]
    fn scale_edits_update_text_radius_and_control_metrics() {
        let state = ThemeEditorState::new();
        state.set_spacing(8.0);
        state.set_radius_scale(1.5);
        state.set_text_scale(1.2);
        state.set_control_size(2);

        let theme = state.theme();
        assert_eq!(theme.spacing, 8.0);
        assert_eq!(theme.radius.md, ThemeRadii::default().md * 1.5);
        assert_eq!(
            theme.text.base.size,
            ThemeTextScale::default().base.size * 1.2
        );
        assert_eq!(theme.control_size, Some(ControlSize::Large));
        assert_eq!(theme.metrics.corner_radius, theme.radius.lg);
    }

    #[test]
    fn color_editor_lists_all_editable_color_swatches() -> Result<()> {
        let shell_theme: DevThemeReader = Rc::new(DefaultTheme::sui);
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Theme editor")
                    .root(build_theme_editor_demo_with_theme(shell_theme)),
            )
            .build()?;
        let output = runtime.render(runtime.window_ids()[0])?;

        for variable in ThemeColorVariable::ALL {
            let name = theme_color_swatch_name(variable);
            assert!(
                output.semantics.iter().any(|node| {
                    node.role == sui::SemanticsRole::ColorSwatch
                        && node.name.as_deref() == Some(name.as_str())
                }),
                "expected theme editor to expose {name:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn reset_restores_the_selected_preset() {
        let state = ThemeEditorState::new();
        state.set_preset(2);
        state.set_spacing(9.0);
        state.set_text_scale(1.4);
        state.reset_current_preset();

        assert_eq!(state.theme(), DefaultTheme::dark());
        assert_eq!(state.preset_index(), 2);
    }

    #[test]
    fn neutral_preset_is_selectable_and_resettable() {
        let state = ThemeEditorState::new();
        state.set_preset(1);
        state.set_selected_color(Color::rgba(0.8, 0.3, 0.2, 1.0));
        state.reset_current_preset();

        assert_eq!(state.theme(), DefaultTheme::neutral());
        assert_eq!(state.preset_index(), 1);
    }

    #[test]
    fn neutral_dark_preset_is_selectable_and_resettable() {
        let state = ThemeEditorState::new();
        state.set_preset(3);
        state.set_selected_color(Color::rgba(0.2, 0.3, 0.8, 1.0));
        state.reset_current_preset();

        assert_eq!(state.theme(), DefaultTheme::neutral_dark());
        assert_eq!(state.preset_index(), 3);
    }
}
