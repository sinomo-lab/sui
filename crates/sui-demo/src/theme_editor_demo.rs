use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use sui::{SemanticTone, StatusBadge, WidgetId, WidgetPodMutVisitor, WidgetPodVisitor, prelude::*};
use sui_runtime::{LayerOptions, PaintBoundaryMode};

use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when, dev_theme_color,
    request_widget_layout_refresh, request_widget_visual_refresh, request_window_refresh,
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

const THEME_COLOR_LAYER_NAME: &str = "Color layer";
const THEME_COLOR_LAYER_OPTIONS: [&str; 2] = ["Source", "Roles"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ThemeColorGroup {
    Source,
    Controls,
}

impl ThemeColorGroup {
    const ALL: [Self; 2] = [Self::Source, Self::Controls];

    const fn index(self) -> usize {
        match self {
            Self::Source => 0,
            Self::Controls => 1,
        }
    }

    const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Controls,
            _ => Self::Source,
        }
    }

    const fn label(self) -> &'static str {
        THEME_COLOR_LAYER_OPTIONS[self.index()]
    }

    fn variables(self) -> impl Iterator<Item = ThemeColorVariable> {
        ThemeColorVariable::ALL
            .iter()
            .copied()
            .filter(move |variable| variable.group() == self)
    }

    fn first_variable(self) -> ThemeColorVariable {
        self.variables()
            .next()
            .expect("every theme color group has at least one variable")
    }
}

macro_rules! define_theme_color_variables {
    ($( $variant:ident => $group:ident, $label:literal, $root:ident.$field:ident; )+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum ThemeColorVariable {
            $( $variant, )+
        }

        impl ThemeColorVariable {
            const ALL: &'static [Self] = &[$( Self::$variant, )+];

            const fn group(self) -> ThemeColorGroup {
                match self {
                    $( Self::$variant => ThemeColorGroup::$group, )+
                }
            }

            const fn label(self) -> &'static str {
                match self {
                    $( Self::$variant => $label, )+
                }
            }

            fn color(self, theme: &DefaultTheme) -> Color {
                match self {
                    $( Self::$variant => theme.$root.$field, )+
                }
            }

            fn set_color(self, theme: &mut DefaultTheme, color: Color) {
                let color = color.clamped();
                match self {
                    $( Self::$variant => theme.$root.$field = color, )+
                }
            }
        }
    };
}

define_theme_color_variables! {
    Base100 => Source, "Base 100", colors.base_100;
    Base200 => Source, "Base 200", colors.base_200;
    Base300 => Source, "Base 300", colors.base_300;
    BaseContent => Source, "Base content", colors.base_content;
    Primary => Source, "Primary", colors.primary;
    PrimaryContent => Source, "On primary", colors.primary_content;
    Secondary => Source, "Secondary", colors.secondary;
    SecondaryContent => Source, "On secondary", colors.secondary_content;
    Accent => Source, "Accent", colors.accent;
    AccentContent => Source, "On accent", colors.accent_content;
    Neutral => Source, "Neutral", colors.neutral;
    NeutralContent => Source, "On neutral", colors.neutral_content;
    Info => Source, "Info", colors.info;
    InfoContent => Source, "On info", colors.info_content;
    Success => Source, "Success", colors.success;
    SuccessContent => Source, "On success", colors.success_content;
    Warning => Source, "Warning", colors.warning;
    WarningContent => Source, "On warning", colors.warning_content;
    Error => Source, "Error", colors.error;
    ErrorContent => Source, "On error", colors.error_content;

    ControlText => Controls, "Text", palette.text;
    ControlTextMuted => Controls, "Text muted", palette.text_muted;
    ControlPlaceholder => Controls, "Placeholder", palette.placeholder;
    ControlSurface => Controls, "Surface", palette.surface;
    ControlSurfaceRaised => Controls, "Surface raised", palette.surface_raised;
    ControlFill => Controls, "Control", palette.control;
    ControlHover => Controls, "Control hover", palette.control_hover;
    ControlActive => Controls, "Control active", palette.control_active;
    ControlField => Controls, "Field", palette.field;
    ControlSurfaceHover => Controls, "Surface hover", palette.surface_hover;
    ControlSurfacePressed => Controls, "Surface pressed", palette.surface_pressed;
    ControlSurfaceFocus => Controls, "Surface focus", palette.surface_focus;
    ControlBorder => Controls, "Border", palette.border;
    ControlBorderStrong => Controls, "Border strong", palette.border_strong;
    ControlBorderHover => Controls, "Border hover", palette.border_hover;
    ControlBorderFocus => Controls, "Border focus", palette.border_focus;
    ControlFocus => Controls, "Focus", palette.focus;
    ControlFocusRing => Controls, "Focus ring", palette.focus_ring;
    ControlCaret => Controls, "Caret", palette.caret;
    ControlSelection => Controls, "Selection", palette.selection;
    ControlSelectionBorder => Controls, "Selection border", palette.selection_border;
    ControlAccent => Controls, "Accent", palette.accent;
    ControlAccentHover => Controls, "Accent hover", palette.accent_hover;
    ControlAccentPressed => Controls, "Accent pressed", palette.accent_pressed;
    ControlAccentBorder => Controls, "Accent border", palette.accent_border;
    ControlAccentBorderHover => Controls, "Accent border hover", palette.accent_border_hover;
    ControlAccentBorderFocus => Controls, "Accent border focus", palette.accent_border_focus;
    ControlAccentText => Controls, "On accent", palette.accent_text;
    ControlAccentSoft => Controls, "Accent soft", palette.accent_soft;
    ControlAccentSoftText => Controls, "Accent soft text", palette.accent_soft_text;
    ControlInfo => Controls, "Info", palette.info;
    ControlInfoText => Controls, "On info", palette.info_text;
    ControlInfoSoft => Controls, "Info soft", palette.info_soft;
    ControlInfoSoftText => Controls, "Info soft text", palette.info_soft_text;
    ControlSuccess => Controls, "Success", palette.success;
    ControlSuccessText => Controls, "On success", palette.success_text;
    ControlSuccessSoft => Controls, "Success soft", palette.success_soft;
    ControlSuccessSoftText => Controls, "Success soft text", palette.success_soft_text;
    ControlWarning => Controls, "Warning", palette.warning;
    ControlWarningText => Controls, "On warning", palette.warning_text;
    ControlWarningSoft => Controls, "Warning soft", palette.warning_soft;
    ControlWarningSoftText => Controls, "Warning soft text", palette.warning_soft_text;
    ControlDanger => Controls, "Danger", palette.danger;
    ControlDangerText => Controls, "On danger", palette.danger_text;
    ControlDangerSoft => Controls, "Danger soft", palette.danger_soft;
    ControlDangerSoftText => Controls, "Danger soft text", palette.danger_soft_text;
    ControlDangerHover => Controls, "Danger hover", palette.danger_hover;

}

#[derive(Clone, Default)]
struct ThemeEditorRefreshTarget {
    widget_id: Rc<Cell<Option<WidgetId>>>,
}

impl ThemeEditorRefreshTarget {
    fn set(&self, widget_id: WidgetId) {
        self.widget_id.set(Some(widget_id));
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.widget_id.get()
    }

    fn request_layout(&self, ctx: &mut EventCtx) -> bool {
        let Some(widget_id) = self.widget_id() else {
            return false;
        };

        request_widget_layout_refresh(ctx, widget_id);
        true
    }

    fn request_visual(&self, ctx: &mut EventCtx) -> bool {
        let Some(widget_id) = self.widget_id() else {
            return false;
        };

        request_widget_visual_refresh(ctx, widget_id);
        true
    }
}

#[derive(Clone)]
struct ThemeEditorRefreshTargets {
    controls: ThemeEditorRefreshTarget,
    preview: ThemeEditorRefreshTarget,
}

struct ThemeEditorRefreshAnchor {
    target: ThemeEditorRefreshTarget,
    child: SingleChild,
}

impl ThemeEditorRefreshAnchor {
    fn new<W>(target: ThemeEditorRefreshTarget, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            target,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ThemeEditorRefreshAnchor {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.target.set(ctx.widget_id());
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.target.set(ctx.widget_id());
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.target.set(ctx.widget_id());
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.target.set(ctx.widget_id());
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            ..Default::default()
        }
    }
}

fn request_theme_editor_refresh(
    ctx: &mut EventCtx,
    targets: &ThemeEditorRefreshTargets,
    preview_layout_changed: bool,
) {
    let controls_ready = targets.controls.request_visual(ctx);
    let preview_ready = if preview_layout_changed {
        targets.preview.request_layout(ctx)
    } else {
        targets.preview.request_visual(ctx)
    };

    if !controls_ready || !preview_ready {
        request_window_refresh(ctx, false);
    }
}

fn request_theme_editor_controls_refresh(ctx: &mut EventCtx, target: &ThemeEditorRefreshTarget) {
    if !target.request_visual(ctx) {
        request_window_refresh(ctx, false);
    }
}

fn request_theme_editor_controls_layout_refresh(
    ctx: &mut EventCtx,
    target: &ThemeEditorRefreshTarget,
) {
    if !target.request_layout(ctx) {
        request_window_refresh(ctx, true);
    }
}

#[derive(Clone)]
struct ThemeEditorState {
    inner: Rc<RefCell<ThemeEditorStateInner>>,
}

struct ThemeEditorStateInner {
    theme: DefaultTheme,
    preset: ThemeEditorPreset,
    selected_color_group: ThemeColorGroup,
    selected_color: ThemeColorVariable,
    color_overrides: HashMap<ThemeColorVariable, Color>,
    control_size: ControlSize,
    radius_scale: f32,
    text_scale: f32,
    motion_scale: f32,
    controls_scroll: ScrollState,
    preview_scroll: ScrollState,
    controls_target: ThemeEditorRefreshTarget,
    preview_target: ThemeEditorRefreshTarget,
}

impl ThemeEditorState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ThemeEditorStateInner {
                theme: DefaultTheme::sui(),
                preset: ThemeEditorPreset::SuiLight,
                selected_color_group: ThemeColorGroup::Source,
                selected_color: ThemeColorVariable::Primary,
                color_overrides: HashMap::new(),
                control_size: ControlSize::Medium,
                radius_scale: 1.0,
                text_scale: 1.0,
                motion_scale: 1.0,
                controls_scroll: ScrollState::new(),
                preview_scroll: ScrollState::new(),
                controls_target: ThemeEditorRefreshTarget::default(),
                preview_target: ThemeEditorRefreshTarget::default(),
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

    fn controls_target(&self) -> ThemeEditorRefreshTarget {
        self.inner.borrow().controls_target.clone()
    }

    fn preview_target(&self) -> ThemeEditorRefreshTarget {
        self.inner.borrow().preview_target.clone()
    }

    fn refresh_targets(&self) -> ThemeEditorRefreshTargets {
        let inner = self.inner.borrow();
        ThemeEditorRefreshTargets {
            controls: inner.controls_target.clone(),
            preview: inner.preview_target.clone(),
        }
    }

    fn preset_index(&self) -> usize {
        self.inner.borrow().preset.index()
    }

    fn set_preset(&self, index: usize) {
        let preset = ThemeEditorPreset::from_index(index);
        let mut inner = self.inner.borrow_mut();
        inner.theme = preset.theme();
        inner.preset = preset;
        inner.color_overrides.clear();
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
        let mut inner = self.inner.borrow_mut();
        inner.selected_color_group = variable.group();
        inner.selected_color = variable;
    }

    fn color_group_index(&self) -> usize {
        self.inner.borrow().selected_color_group.index()
    }

    fn set_color_group(&self, index: usize) {
        let group = ThemeColorGroup::from_index(index);
        let mut inner = self.inner.borrow_mut();
        inner.selected_color_group = group;
        if inner.selected_color.group() != group {
            inner.selected_color = group.first_variable();
        }
    }

    fn is_selected_color(&self, variable: ThemeColorVariable) -> bool {
        self.inner.borrow().selected_color == variable
    }

    fn color_variable(&self, variable: ThemeColorVariable) -> Color {
        let inner = self.inner.borrow();
        variable.color(&inner.theme)
    }

    fn selected_color(&self) -> Color {
        let inner = self.inner.borrow();
        inner.selected_color.color(&inner.theme)
    }

    fn selected_color_summary(&self) -> String {
        let inner = self.inner.borrow();
        let color = inner.selected_color.color(&inner.theme).clamped();
        format!(
            "{} / {}  #{:02X}{:02X}{:02X}  A {:.0}%",
            inner.selected_color_group.label(),
            inner.selected_color.label(),
            (color.red * 255.0).round() as u8,
            (color.green * 255.0).round() as u8,
            (color.blue * 255.0).round() as u8,
            color.alpha * 100.0,
        )
    }

    fn set_selected_color(&self, color: Color) {
        let mut inner = self.inner.borrow_mut();
        let selected = inner.selected_color;
        selected.set_color(&mut inner.theme, color);
        if selected.group() == ThemeColorGroup::Source {
            sync_editor_derived_fields(&mut inner);
        } else {
            inner.color_overrides.insert(selected, color.clamped());
        }
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
        sync_editor_derived_fields(&mut inner);
    }

    fn radius_scale(&self) -> f64 {
        f64::from(self.inner.borrow().radius_scale)
    }

    fn set_radius_scale(&self, scale: f32) {
        let scale = scale.clamp(0.0, 2.0);
        let mut inner = self.inner.borrow_mut();
        inner.radius_scale = scale;
        inner.theme.radius = scaled_radii(scale);
        sync_editor_derived_fields(&mut inner);
    }

    fn text_scale(&self) -> f64 {
        f64::from(self.inner.borrow().text_scale)
    }

    fn set_text_scale(&self, scale: f32) {
        let scale = scale.clamp(0.75, 1.5);
        let mut inner = self.inner.borrow_mut();
        inner.text_scale = scale;
        inner.theme.text = scaled_text_scale(scale);
        sync_editor_derived_fields(&mut inner);
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

fn sync_editor_derived_fields(inner: &mut ThemeEditorStateInner) {
    inner.theme.sync_derived_fields();
    let overrides = inner
        .color_overrides
        .iter()
        .map(|(variable, color)| (*variable, *color))
        .collect::<Vec<_>>();
    for (variable, color) in overrides {
        variable.set_color(&mut inner.theme, color);
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
    let controls_target = state.controls_target();
    let reset_targets = state.refresh_targets();
    ThemeEditorRefreshAnchor::new(
        controls_target,
        Surface::sidebar(
            ScrollView::vertical(Padding::all(
                16.0,
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(editor_title(
                        "Theme variables",
                        "Edit source colors or common semantic roles; widgets derive their own appearance from these defaults.",
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
                                request_theme_editor_refresh(ctx, &reset_targets, true);
                            }),
                    ),
            ))
            .state(controls_scroll)
            .name(THEME_EDITOR_CONTROLS_SCROLL_NAME)
            .theme_when(clone_dev_theme_reader(&shell_theme)),
        )
        .theme_when(clone_dev_theme_reader(&shell_theme))
        .fill(),
    )
}

fn editor_title(
    title: &'static str,
    description: &'static str,
    theme_reader: DevThemeReader,
) -> impl Widget {
    Stack::vertical()
        .spacing(5.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new(title).style_when(demo_text_style_when(
            &theme_reader,
            DemoTextRole::SectionTitle,
            |theme| theme.palette.text,
        )))
        .with_child(Label::new(description).style_when(demo_text_style_when(
            &theme_reader,
            DemoTextRole::Supporting,
            |theme| theme.palette.text_muted,
        )))
}

fn build_preset_section(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let preset_reader = state.clone();
    let preset_change = state.clone();
    let size_reader = state.clone();
    let size_change = state;
    let preset_targets = preset_reader.refresh_targets();
    let size_targets = size_reader.refresh_targets();

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
                            request_theme_editor_refresh(ctx, &preset_targets, true);
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
                            request_theme_editor_refresh(ctx, &size_targets, true);
                        }),
                )
                .theme_when(clone_dev_theme_reader(&shell_theme))
                .stacked(),
            ),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
}

fn build_color_section(state: ThemeEditorState, shell_theme: DevThemeReader) -> impl Widget {
    let group_reader = state.clone();
    let group_change = state.clone();
    let group_target = state.controls_target();
    let summary_state = state.clone();
    let picker_reader = state.clone();
    let picker_change = state.clone();
    let picker_targets = state.refresh_targets();

    PanelSection::new(
        "Color",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SegmentedControl::new(THEME_COLOR_LAYER_NAME)
                    .segments(THEME_COLOR_LAYER_OPTIONS)
                    .selected_when(move || Some(group_reader.color_group_index()))
                    .theme_when(clone_dev_theme_reader(&shell_theme))
                    .on_change_with_ctx(move |index, _, ctx| {
                        group_change.set_color_group(index);
                        request_theme_editor_controls_layout_refresh(ctx, &group_target);
                    }),
            )
            .with_child(
                Label::dynamic("Primary  #000000", move || {
                    summary_state.selected_color_summary()
                })
                .style_when(demo_text_style_when(
                    &shell_theme,
                    DemoTextRole::Body,
                    |theme| theme.palette.text,
                )),
            )
            .with_child(
                SimpleColorPicker::from_color(
                    THEME_COLOR_PICKER_NAME,
                    picker_reader.selected_color(),
                )
                .mode(SimpleColorPickerMode::Rgb)
                .show_alpha(true)
                .color_when(move || picker_reader.selected_color())
                .theme_when(clone_dev_theme_reader(&shell_theme))
                .on_change_with_ctx(move |ctx, color| {
                    picker_change.set_selected_color(color);
                    request_theme_editor_refresh(ctx, &picker_targets, false);
                }),
            )
            .with_child(build_color_swatch_layers(state, Rc::clone(&shell_theme))),
    )
    .theme_when(clone_dev_theme_reader(&shell_theme))
}

fn build_color_swatch_layers(state: ThemeEditorState, shell_theme: DevThemeReader) -> SwitchView {
    let selected_reader = state.clone();
    let mut layers = SwitchView::new().selected_when(move || selected_reader.color_group_index());
    for group in ThemeColorGroup::ALL {
        layers = layers.with_child(build_color_swatch_list(
            group,
            state.clone(),
            Rc::clone(&shell_theme),
        ));
    }
    layers
}

fn build_color_swatch_list(
    group: ThemeColorGroup,
    state: ThemeEditorState,
    shell_theme: DevThemeReader,
) -> impl Widget {
    let mut swatches = Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start);

    for variable in group.variables() {
        swatches = swatches.with_item(
            build_color_token_swatch(variable, state.clone(), Rc::clone(&shell_theme)),
            FlexItem::fixed(92.0),
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
    let controls_target = select_state.controls_target();
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
            .size(Size::new(46.0, 26.0))
            .on_press_with_ctx(move |ctx, _| {
                select_state.select_color_variable(variable);
                request_theme_editor_controls_refresh(ctx, &controls_target);
            }),
        )
        .with_child(
            Label::new(variable.label()).style_when(demo_text_style_when(
                &shell_theme,
                DemoTextRole::Metadata,
                move |theme| {
                    if label_state.is_selected_color(variable) {
                        theme.palette.accent
                    } else {
                        theme.palette.text_muted
                    }
                },
            )),
        )
}

fn theme_color_swatch_name(variable: ThemeColorVariable) -> String {
    if variable.group() == ThemeColorGroup::Source {
        format!("{} theme color", variable.label())
    } else {
        format!(
            "{} {} theme color",
            variable.group().label(),
            variable.label()
        )
    }
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
    let spacing_targets = spacing_reader.refresh_targets();
    let radius_targets = radius_reader.refresh_targets();
    let text_targets = text_reader.refresh_targets();
    let motion_targets = motion_reader.refresh_targets();

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
                    request_theme_editor_refresh(ctx, &spacing_targets, true);
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
                    request_theme_editor_refresh(ctx, &radius_targets, true);
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
                    request_theme_editor_refresh(ctx, &text_targets, true);
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
                    request_theme_editor_refresh(ctx, &motion_targets, true);
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
    let preview_target = state.preview_target();
    ThemeEditorRefreshAnchor::new(
        preview_target,
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
        .fill(),
    )
}

fn preview_header(state: ThemeEditorState, theme_reader: DevThemeReader) -> impl Widget {
    let summary_state = state;
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new("Live preview").style_when(demo_text_style_when(
            &theme_reader,
            DemoTextRole::PageTitle,
            |theme| theme.palette.text,
        )))
        .with_child(
            Label::dynamic("Light · medium controls", move || {
                summary_state.theme_summary()
            })
            .style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::Supporting,
                |theme| theme.palette.text_muted,
            )),
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
            .with_child(Label::new(title).style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::SectionTitle,
                |theme| theme.palette.text,
            )))
            .with_child(Label::new(description).style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::Supporting,
                |theme| theme.palette.text_muted,
            )))
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
    fn color_edits_refresh_derived_palette_without_overwriting_content_pairs() {
        let state = ThemeEditorState::new();
        let original_content = state.theme().colors.primary_content;
        state.select_color_variable(ThemeColorVariable::Primary);
        state.set_selected_color(Color::rgba(0.95, 0.85, 0.20, 1.0));

        let theme = state.theme();
        assert_eq!(theme.palette.accent, theme.colors.primary);
        assert_eq!(theme.colors.primary_content, original_content);
        assert_eq!(theme.palette.accent_text, original_content);
        assert_eq!(
            theme.palette.selection_border,
            theme
                .colors
                .primary
                .with_alpha(0.35)
                .over(theme.palette.surface_raised)
        );

        state.select_color_variable(ThemeColorVariable::PrimaryContent);
        state.set_selected_color(Color::rgba(0.12, 0.16, 0.22, 1.0));
        let theme = state.theme();
        assert_eq!(
            theme.colors.primary_content,
            Color::rgba(0.12, 0.16, 0.22, 1.0)
        );
        assert_eq!(theme.palette.accent_text, theme.colors.primary_content);
    }

    #[test]
    fn every_theme_color_component_is_independently_editable() {
        let marker = Color::rgba(0.123, 0.456, 0.789, 0.625);
        for (selected_index, selected) in ThemeColorVariable::ALL.iter().copied().enumerate() {
            let mut theme = DefaultTheme::light();
            let before = ThemeColorVariable::ALL
                .iter()
                .map(|variable| variable.color(&theme))
                .collect::<Vec<_>>();
            selected.set_color(&mut theme, marker);
            let after = ThemeColorVariable::ALL
                .iter()
                .map(|variable| variable.color(&theme))
                .collect::<Vec<_>>();

            for (index, value) in after.into_iter().enumerate() {
                if index == selected_index {
                    assert_eq!(value, marker, "{} should be editable", selected.label());
                } else {
                    assert_eq!(
                        value,
                        before[index],
                        "editing {} should not rewrite {}",
                        selected.label(),
                        ThemeColorVariable::ALL[index].label()
                    );
                }
            }
        }
    }

    #[test]
    fn color_variable_inventory_covers_every_current_theme_layer() {
        assert_eq!(ThemeColorVariable::ALL.len(), 67);
        assert_eq!(ThemeColorGroup::Source.variables().count(), 20);
        assert_eq!(ThemeColorGroup::Controls.variables().count(), 47);
    }

    #[test]
    fn derived_color_overrides_survive_recomputation_and_reset_with_the_preset() {
        let state = ThemeEditorState::new();
        let hover = Color::rgba(0.88, 0.18, 0.52, 0.42);
        let highlight = Color::rgba(0.12, 0.72, 0.94, 0.86);

        state.select_color_variable(ThemeColorVariable::ControlHover);
        state.set_selected_color(hover);
        state.select_color_variable(ThemeColorVariable::ControlSelectionBorder);
        state.set_selected_color(highlight);
        state.set_spacing(7.0);
        state.set_radius_scale(1.25);
        state.set_text_scale(1.1);

        let theme = state.theme();
        assert_eq!(theme.palette.control_hover, hover);
        assert_eq!(theme.palette.selection_border, highlight);

        state.select_color_variable(ThemeColorVariable::Primary);
        state.set_selected_color(Color::rgba(0.75, 0.20, 0.16, 1.0));
        let theme = state.theme();
        assert_eq!(theme.palette.control_hover, hover);
        assert_eq!(theme.palette.selection_border, highlight);

        state.reset_current_preset();
        let theme = state.theme();
        assert_ne!(theme.palette.control_hover, hover);
        assert_ne!(theme.palette.selection_border, highlight);
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
    fn color_editor_lists_every_editable_color_swatch_by_layer() -> Result<()> {
        for group in ThemeColorGroup::ALL {
            let shell_theme: DevThemeReader = Rc::new(DefaultTheme::sui);
            let state = ThemeEditorState::new();
            let mut runtime = Application::new()
                .window(
                    WindowBuilder::new()
                        .title("Theme editor")
                        .root(build_color_swatch_list(group, state, shell_theme)),
                )
                .build()?;
            let output = runtime.render(runtime.window_ids()[0])?;

            for variable in group.variables() {
                let name = theme_color_swatch_name(variable);
                assert!(
                    output.semantics.iter().any(|node| {
                        node.role == sui::SemanticsRole::ColorSwatch
                            && node.name.as_deref() == Some(name.as_str())
                    }),
                    "expected {} layer to expose {name:?}",
                    group.label()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn color_layer_selection_targets_a_variable_in_the_selected_group() {
        let state = ThemeEditorState::new();
        for group in ThemeColorGroup::ALL {
            state.set_color_group(group.index());
            let inner = state.inner.borrow();
            assert_eq!(inner.selected_color_group, group);
            assert_eq!(inner.selected_color.group(), group);
        }
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
