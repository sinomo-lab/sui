use crate::{
    Blink, ControlMetrics, DefaultTheme, HdrThemeMode, Interpolate, MotionScalar,
    ResolvedEffectStyle, ResolvedHdrStyle, SemanticTone, ThemeColorScheme, WidgetColorRole,
    WidgetLuminanceRole, WidgetMaterialRole,
    editor::{EditorCommand, EditorCommandResult, EditorState, selection_range},
    overlay::{OverlayPlacement, OverlayPlacementRequest, place_overlay},
    paint_theme_shadow, resolve_luminance_role, resolve_widget_hdr_style,
    selection::{SelectionChange, SelectionOwnerId, SelectionScope},
    text_align::{
        HorizontalTextAlignmentMode, aligned_text_rect_for_layout,
        aligned_text_rect_for_layout_with_mode, aligned_text_rect_for_text, paint_aligned_text,
        paint_single_line_aligned_text,
    },
    text_command::TextCommand,
    text_surface::paste_command,
};
use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};
use sui_core::{
    Color, EditableTextSemantics, Event, ImeEvent, InvalidationKind, InvalidationRequest,
    InvalidationTarget, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsPopupKind, SemanticsRole,
    SemanticsTextRange, SemanticsValue, Size, TimerToken, ToggleState, Vector, WakeEvent, WidgetId,
};
use sui_layout::{Axis, Constraints, IntrinsicSize, Padding as Insets};
use sui_lucide::LucideIcon;
use sui_reactive::Observable;
use sui_runtime::{
    ArrangeCtx, Command, EventCtx, EventPhase, LayerOptions, MeasureCtx, OVERLAY_DISMISS_REQUEST,
    OverlayDismissPolicy, OverlayFocusBehavior, OverlayKind, OverlayOptions, PaintBoundaryMode,
    PaintCtx, SemanticsCtx, SingleChild, StackSurfaceOptions, Widget, WidgetPodMutVisitor,
    WidgetPodVisitor,
};
use sui_scene::{LayerCompositionMode, LayerProperties, StrokeStyle};
use sui_text::{
    FontFeature, PersistentTextLayout, TextCursor, TextMeasurement, TextSelection, TextStyle,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconGlyph {
    Add,
    Remove,
    Check,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    Close,
    Maximize,
    Restore,
    FitView,
    ActualSize,
    MoreHorizontal,
    MoreVertical,
    Search,
    Undo,
    Redo,
    Brush,
    Eraser,
    PaintBucket,
    Hand,
    Lock,
    Unlock,
    Trash,
    Download,
    // Content/object glyphs (used by application chrome: chat, file browser, etc.).
    Sparkles,
    Chat,
    History,
    Folder,
    File,
    FileText,
    Link,
    Send,
    ArrowUp,
    Stop,
    Attach,
    Hourglass,
    Alert,
    Storage,
    // Media/device glyphs for realtime call and device surfaces.
    AudioLines,
    Mic,
    MicOff,
    Camera,
    CameraOff,
    Video,
    VideoOff,
    Phone,
    PhoneOff,
    Monitor,
    ScreenShare,
}

impl IconGlyph {
    pub const fn lucide_icon(self) -> LucideIcon {
        match self {
            Self::Add => LucideIcon::Plus,
            Self::Remove => LucideIcon::Minus,
            Self::Check => LucideIcon::Check,
            Self::ChevronDown => LucideIcon::ChevronDown,
            Self::ChevronUp => LucideIcon::ChevronUp,
            Self::ChevronLeft => LucideIcon::ChevronLeft,
            Self::ChevronRight => LucideIcon::ChevronRight,
            Self::Close => LucideIcon::X,
            Self::Maximize => LucideIcon::Maximize,
            Self::Restore => LucideIcon::Copy,
            Self::FitView => LucideIcon::ScanSearch,
            Self::ActualSize => LucideIcon::Scan,
            Self::MoreHorizontal => LucideIcon::Ellipsis,
            Self::MoreVertical => LucideIcon::EllipsisVertical,
            Self::Search => LucideIcon::Search,
            Self::Undo => LucideIcon::Undo2,
            Self::Redo => LucideIcon::Redo2,
            Self::Brush => LucideIcon::Brush,
            Self::Eraser => LucideIcon::Eraser,
            Self::PaintBucket => LucideIcon::PaintBucket,
            Self::Hand => LucideIcon::Hand,
            Self::Lock => LucideIcon::Lock,
            Self::Unlock => LucideIcon::LockOpen,
            Self::Trash => LucideIcon::Trash2,
            Self::Download => LucideIcon::Download,
            Self::Sparkles => LucideIcon::Sparkles,
            Self::Chat => LucideIcon::MessageSquare,
            Self::History => LucideIcon::History,
            Self::Folder => LucideIcon::Folder,
            Self::File => LucideIcon::File,
            Self::FileText => LucideIcon::FileText,
            Self::Link => LucideIcon::Link,
            Self::Send => LucideIcon::Send,
            Self::ArrowUp => LucideIcon::ArrowUp,
            Self::Stop => LucideIcon::Square,
            Self::Attach => LucideIcon::Paperclip,
            Self::Hourglass => LucideIcon::Hourglass,
            Self::Alert => LucideIcon::TriangleAlert,
            Self::Storage => LucideIcon::HardDrive,
            Self::AudioLines => LucideIcon::AudioLines,
            Self::Mic => LucideIcon::Mic,
            Self::MicOff => LucideIcon::MicOff,
            Self::Camera => LucideIcon::Camera,
            Self::CameraOff => LucideIcon::CameraOff,
            Self::Video => LucideIcon::Video,
            Self::VideoOff => LucideIcon::VideoOff,
            Self::Phone => LucideIcon::Phone,
            Self::PhoneOff => LucideIcon::PhoneOff,
            Self::Monitor => LucideIcon::Monitor,
            Self::ScreenShare => LucideIcon::ScreenShare,
        }
    }
}

pub const BUILTIN_ICON_GLYPHS: &[IconGlyph] = &[
    IconGlyph::Add,
    IconGlyph::Remove,
    IconGlyph::Check,
    IconGlyph::ChevronDown,
    IconGlyph::ChevronUp,
    IconGlyph::ChevronLeft,
    IconGlyph::ChevronRight,
    IconGlyph::Close,
    IconGlyph::Maximize,
    IconGlyph::Restore,
    IconGlyph::FitView,
    IconGlyph::ActualSize,
    IconGlyph::MoreHorizontal,
    IconGlyph::MoreVertical,
    IconGlyph::Search,
    IconGlyph::Undo,
    IconGlyph::Redo,
    IconGlyph::Brush,
    IconGlyph::Eraser,
    IconGlyph::PaintBucket,
    IconGlyph::Hand,
    IconGlyph::Lock,
    IconGlyph::Unlock,
    IconGlyph::Trash,
    IconGlyph::Download,
    IconGlyph::Sparkles,
    IconGlyph::Chat,
    IconGlyph::History,
    IconGlyph::Folder,
    IconGlyph::File,
    IconGlyph::FileText,
    IconGlyph::Link,
    IconGlyph::Send,
    IconGlyph::ArrowUp,
    IconGlyph::Stop,
    IconGlyph::Attach,
    IconGlyph::Hourglass,
    IconGlyph::Alert,
    IconGlyph::Storage,
    IconGlyph::AudioLines,
    IconGlyph::Mic,
    IconGlyph::MicOff,
    IconGlyph::Camera,
    IconGlyph::CameraOff,
    IconGlyph::Video,
    IconGlyph::VideoOff,
    IconGlyph::Phone,
    IconGlyph::PhoneOff,
    IconGlyph::Monitor,
    IconGlyph::ScreenShare,
];

pub fn register_builtin_icon_resources(
    _application: &mut sui_runtime::Application,
) -> sui_core::Result<()> {
    // Built-in widgets paint Lucide geometry directly. Keep this compatibility hook so
    // applications do not need to change their startup path.
    Ok(())
}

pub struct Separator {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: Axis,
    name: Option<String>,
    inset: f32,
    thickness: Option<f32>,
    length: Option<f32>,
}

impl Separator {
    pub fn new(axis: Axis) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            axis,
            name: None,
            inset: 0.0,
            thickness: None,
            length: None,
        }
    }

    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset.max(0.0);
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness.max(0.0));
        self
    }

    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length.max(0.0));
        self
    }

    fn resolved_thickness(&self) -> f32 {
        self.thickness
            .unwrap_or(self.resolved_theme().metrics.separator_thickness)
            .max(1.0)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| *self.theme)
    }
}

impl Widget for Separator {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let thickness = self.resolved_thickness();
        let length = self.length.unwrap_or(64.0);
        let size = match self.axis {
            Axis::Horizontal => Size::new(length, thickness + (self.inset * 2.0)),
            Axis::Vertical => Size::new(thickness + (self.inset * 2.0), length),
        };

        constraints.clamp(Size::new(
            if self.axis == Axis::Horizontal && constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                size.width
            },
            if self.axis == Axis::Vertical && constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                size.height
            },
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let thickness = physical_pixels(ctx, self.resolved_thickness());
        let line = match self.axis {
            Axis::Horizontal => Rect::new(
                ctx.bounds().x() + self.inset,
                ctx.bounds().y() + ((ctx.bounds().height() - thickness) * 0.5),
                (ctx.bounds().width() - (self.inset * 2.0)).max(0.0),
                thickness,
            ),
            Axis::Vertical => Rect::new(
                ctx.bounds().x() + ((ctx.bounds().width() - thickness) * 0.5),
                ctx.bounds().y() + self.inset,
                thickness,
                (ctx.bounds().height() - (self.inset * 2.0)).max(0.0),
            ),
        };
        ctx.fill(
            rounded_rect_path(line, thickness * 0.5),
            theme.palette.border,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Separator, ctx.bounds());
        node.name = self.name.clone();
        ctx.push(node);
    }
}

pub struct Icon {
    theme: Box<DefaultTheme>,
    glyph: IconGlyph,
    size: Option<f32>,
    color: Option<Color>,
    color_reader: Option<Box<dyn Fn() -> Color>>,
    label: Option<String>,
}

impl Icon {
    pub fn new(glyph: IconGlyph) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            glyph,
            size: None,
            color: None,
            color_reader: None,
            label: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self.color_reader = None;
        self
    }

    pub fn color_when<F>(mut self, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.color_reader = Some(Box::new(color));
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn resolved_color(&self) -> Color {
        self.color_reader
            .as_ref()
            .map(|reader| reader())
            .or(self.color)
            .unwrap_or(self.theme.palette.text)
    }

    fn resolved_size(&self) -> f32 {
        self.size.unwrap_or(self.theme.metrics.icon_size)
    }
}

impl Widget for Icon {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let side = self.resolved_size();
        constraints.clamp(Size::new(side, side))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        draw_icon_glyph(
            ctx,
            self.glyph,
            center_square(ctx.bounds(), self.resolved_size()),
            self.resolved_color(),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(label) = &self.label {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Image, ctx.bounds());
            node.name = Some(label.clone());
            ctx.push(node);
        }
    }
}

const CARET_BLINK_PERIOD_SECONDS: f64 = 1.0;
const SELECT_CHEVRON_SLOT_WIDTH: f32 = 28.0;
const SELECT_CHEVRON_ICON_SIZE: f32 = 20.0;
#[cfg(test)]
const SELECT_MENU_GAP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectMenuPlacement {
    Below,
    Above,
}

type AnimatedScalar = MotionScalar;

fn request_child_invalidation(ctx: &mut EventCtx, widget_id: WidgetId, kind: InvalidationKind) {
    ctx.request(InvalidationRequest::new(
        InvalidationTarget::Widget(widget_id),
        kind,
    ));
}

fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) {
    animation.set_target_event(target, duration, easing, ctx);
}

fn set_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    );
}

fn set_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
        ctx,
    );
}

fn set_toggle_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_animation_target(
        animation,
        target,
        theme.motion.toggle_duration(),
        theme.motion.toggle_easing(),
        ctx,
    );
}

fn set_focus_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_animation_target(
        animation,
        target,
        theme.motion.focus_duration(),
        theme.motion.focus_easing(),
        ctx,
    );
}

fn mix_color(from: Color, to: Color, t: f32) -> Color {
    Color::interpolate(from, to, t)
}

/// The visual treatment used by pressable controls.
///
/// Appearance and semantic tone are deliberately independent: a destructive
/// action can be rendered as a filled, tonal, outlined, or low-emphasis ghost
/// control without remapping the application's theme palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonAppearance {
    /// A solid semantic-color fill for primary and high-emphasis actions.
    Filled,
    /// A soft semantic-color wash with semantic ink.
    #[default]
    Tonal,
    /// A transparent surface with a visible semantic outline.
    Outline,
    /// A borderless, transparent surface that reveals a wash on interaction.
    Ghost,
}

/// Whether an editor paints its own field chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldAppearance {
    /// Paint the standard field background, border, hover, and focus ring.
    #[default]
    Framed,
    /// Paint only editor content. Intended for use inside [`crate::FramedField`].
    Bare,
}

/// The whole-row visual treatment used by checkbox, switch, and radio controls.
///
/// This affects only the row surrounding the label and indicator. The checkbox,
/// switch track, or radio indicator always keeps its own stateful chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChoiceAppearance {
    /// A quiet row that is transparent at rest and reveals a soft wash while
    /// hovered, pressed, or focused.
    #[default]
    Plain,
    /// A filled, bordered row suitable for inspectors and dense settings panes.
    Framed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ChoiceFrameVisuals {
    background: Color,
    border: Color,
}

fn choice_frame_visuals(
    theme: &DefaultTheme,
    appearance: ChoiceAppearance,
    framed_background: Color,
    framed_border: Color,
    hover_progress: f32,
    press_progress: f32,
    focus_progress: f32,
) -> ChoiceFrameVisuals {
    if appearance == ChoiceAppearance::Framed {
        return ChoiceFrameVisuals {
            background: framed_background,
            border: framed_border,
        };
    }

    let hover_wash = theme.surfaces.hover;
    let press_wash = theme
        .palette
        .text
        .with_alpha(if theme.surfaces.dark { 0.10 } else { 0.07 });
    let focus_wash = theme
        .palette
        .accent
        .with_alpha(if theme.surfaces.dark { 0.12 } else { 0.08 });
    let background = mix_color(
        mix_color(
            mix_color(
                Color::TRANSPARENT,
                hover_wash,
                hover_progress.clamp(0.0, 1.0),
            ),
            focus_wash,
            focus_progress.clamp(0.0, 1.0),
        ),
        press_wash,
        press_progress.clamp(0.0, 1.0),
    );

    ChoiceFrameVisuals {
        background,
        border: Color::TRANSPARENT,
    }
}

fn field_background(
    theme: &DefaultTheme,
    read_only: bool,
    hover_progress: f32,
    focus_progress: f32,
) -> Color {
    let palette = theme.palette;
    let base = if read_only {
        palette.surface
    } else {
        palette.field
    };
    let hover_target = if !read_only && theme.colors.scheme == ThemeColorScheme::Light {
        palette.surface
    } else {
        base
    };
    let hovered = mix_color(
        base,
        hover_target,
        hover_progress.clamp(0.0, 1.0) * theme.interaction.hover_blend,
    );
    mix_color(
        hovered,
        palette.surface_focus,
        focus_progress.clamp(0.0, 1.0),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SemanticButtonVisuals {
    background: Color,
    border: Color,
    content: Color,
}

fn semantic_button_visuals(
    theme: &DefaultTheme,
    appearance: ButtonAppearance,
    tone: SemanticTone,
    enabled: bool,
    hover_progress: f32,
    press_progress: f32,
) -> SemanticButtonVisuals {
    let palette = theme.palette;
    let interaction = theme.interaction;
    let hover = if enabled {
        hover_progress.clamp(0.0, 1.0) * interaction.hover_blend
    } else {
        0.0
    };
    let press = if enabled {
        press_progress.clamp(0.0, 1.0) * interaction.pressed_blend
    } else {
        0.0
    };
    let (solid, solid_text) = theme.semantic_tone_colors(tone);
    let (soft, soft_text) = theme.semantic_tone_soft_colors(tone);
    let ink = if tone == SemanticTone::Neutral {
        palette.text
    } else {
        solid
    };
    let outline = if tone == SemanticTone::Neutral {
        palette.border
    } else {
        solid.with_alpha(0.72)
    };

    let (base, hovered, pressed, border, content) = match appearance {
        ButtonAppearance::Filled => {
            let hovered = if tone == SemanticTone::Accent {
                palette.accent_hover
            } else {
                mix_color(solid, solid_text, 0.10)
            };
            let pressed = if tone == SemanticTone::Accent {
                palette.accent_pressed
            } else {
                mix_color(solid, palette.text, 0.16)
            };
            (solid, hovered, pressed, solid, solid_text)
        }
        ButtonAppearance::Tonal => (
            soft,
            mix_color(soft, solid, 0.12),
            mix_color(soft, solid, 0.24),
            if tone == SemanticTone::Neutral {
                palette.border
            } else {
                solid.with_alpha(0.30)
            },
            soft_text,
        ),
        ButtonAppearance::Outline => (
            Color::TRANSPARENT,
            soft,
            mix_color(soft, solid, 0.16),
            outline,
            ink,
        ),
        ButtonAppearance::Ghost => (
            Color::TRANSPARENT,
            soft,
            mix_color(soft, solid, 0.16),
            Color::TRANSPARENT,
            ink,
        ),
    };
    let background = mix_color(mix_color(base, hovered, hover), pressed, press);

    if enabled {
        SemanticButtonVisuals {
            background,
            border,
            content,
        }
    } else {
        let background = if matches!(
            appearance,
            ButtonAppearance::Outline | ButtonAppearance::Ghost
        ) {
            Color::TRANSPARENT
        } else {
            mix_color(background, palette.control, 0.72).with_alpha(interaction.disabled_opacity)
        };
        SemanticButtonVisuals {
            background,
            border: border.with_alpha(interaction.disabled_content_opacity),
            content: palette
                .text_muted
                .with_alpha(interaction.disabled_content_opacity),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconButtonPaint {
    pub appearance: ButtonAppearance,
    pub tone: SemanticTone,
    pub selected: bool,
    pub enabled: bool,
    pub hover_progress: f32,
    pub press_progress: f32,
    pub focus_progress: f32,
    pub icon_size: Option<f32>,
}

impl IconButtonPaint {
    pub const fn new() -> Self {
        Self {
            appearance: ButtonAppearance::Tonal,
            tone: SemanticTone::Neutral,
            selected: false,
            enabled: true,
            hover_progress: 0.0,
            press_progress: 0.0,
            focus_progress: 0.0,
            icon_size: None,
        }
    }

    pub const fn appearance(mut self, appearance: ButtonAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub const fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hover_progress = if hovered { 1.0 } else { 0.0 };
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.press_progress = if pressed { 1.0 } else { 0.0 };
        self
    }

    pub const fn focused(mut self, focused: bool) -> Self {
        self.focus_progress = if focused { 1.0 } else { 0.0 };
        self
    }

    pub fn hover_progress(mut self, progress: f32) -> Self {
        self.hover_progress = progress;
        self
    }

    pub fn press_progress(mut self, progress: f32) -> Self {
        self.press_progress = progress;
        self
    }

    pub fn focus_progress(mut self, progress: f32) -> Self {
        self.focus_progress = progress;
        self
    }

    pub const fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = Some(icon_size);
        self
    }
}

impl Default for IconButtonPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_icon_button(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    icon: IconGlyph,
    style: IconButtonPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let metrics = theme.metrics;
    let interaction = theme.interaction;
    let selected = style.selected;
    let enabled = style.enabled;
    let hover_progress = if enabled {
        style.hover_progress.clamp(0.0, 1.0) * interaction.hover_blend
    } else {
        0.0
    };
    let press_progress = if enabled {
        style.press_progress.clamp(0.0, 1.0) * interaction.pressed_blend
    } else {
        0.0
    };
    let focus_progress = if enabled {
        style.focus_progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let legacy_default =
        style.appearance == ButtonAppearance::Tonal && style.tone == SemanticTone::Neutral;
    let (background, border, icon_color) = if legacy_default {
        let base_background = if selected {
            mix_color(palette.control, palette.accent, interaction.selected_blend)
        } else {
            palette.control
        };
        let hover_background = if selected {
            mix_color(base_background, palette.accent_hover, 0.18)
        } else {
            palette.control_hover
        };
        let press_background = if selected {
            mix_color(base_background, palette.control_active, 0.45)
        } else {
            palette.control_active
        };
        let background = mix_color(
            mix_color(base_background, hover_background, hover_progress),
            press_background,
            press_progress,
        );
        let border_base = if !enabled {
            palette.border.with_alpha(0.55)
        } else if selected {
            mix_color(palette.accent_border, palette.border_hover, hover_progress)
        } else {
            mix_color(palette.border, palette.border_hover, hover_progress)
        };
        let border = if enabled {
            mix_color(border_base, palette.border_focus, focus_progress)
        } else {
            border_base
        };
        let background = if enabled {
            background
        } else {
            mix_color(background, palette.control, 0.72).with_alpha(interaction.disabled_opacity)
        };
        let icon_color = if !enabled {
            palette
                .text
                .with_alpha(interaction.disabled_content_opacity)
        } else if selected {
            palette.accent
        } else {
            palette.text
        };
        (background, border, icon_color)
    } else {
        let mut visuals = semantic_button_visuals(
            theme,
            style.appearance,
            style.tone,
            enabled,
            style.hover_progress,
            style.press_progress,
        );
        if selected && enabled {
            let selection = if style.tone == SemanticTone::Neutral {
                palette.accent
            } else {
                theme.semantic_tone_color(style.tone)
            };
            visuals.background =
                mix_color(visuals.background, selection, interaction.selected_blend);
            visuals.border = mix_color(visuals.border, selection, 0.72);
            visuals.content = selection;
        }
        visuals.border = if enabled {
            mix_color(visuals.border, palette.border_focus, focus_progress)
        } else {
            visuals.border
        };
        (visuals.background, visuals.border, visuals.content)
    };
    let icon_size = style
        .icon_size
        .unwrap_or(metrics.icon_size)
        .min(rect.width().min(rect.height()))
        .max(0.0);

    draw_control_frame(
        ctx,
        rect,
        metrics.corner_radius,
        metrics,
        background,
        border,
        (focus_progress > 0.0).then_some(
            palette
                .focus_ring
                .with_alpha(palette.focus_ring.alpha * focus_progress),
        ),
    );
    draw_icon_glyph(ctx, icon, center_square(rect, icon_size), icon_color);
}

pub struct IconButton {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    icon: IconGlyph,
    label: String,
    semantic_description: Option<String>,
    appearance: ButtonAppearance,
    tone: SemanticTone,
    size: Option<f32>,
    icon_size: Option<f32>,
    selected: bool,
    selected_reader: Option<Box<dyn Fn() -> bool>>,
    enabled: bool,
    enabled_reader: Option<Box<dyn Fn() -> bool>>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    on_press: Option<Box<dyn FnMut()>>,
    on_press_with_ctx: Option<Box<dyn FnMut(&mut EventCtx)>>,
}

impl IconButton {
    pub fn new(icon: IconGlyph, label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            icon,
            label: label.into(),
            semantic_description: None,
            appearance: ButtonAppearance::Tonal,
            tone: SemanticTone::Neutral,
            size: None,
            icon_size: None,
            selected: false,
            selected_reader: None,
            enabled: true,
            enabled_reader: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            on_press: None,
            on_press_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.semantic_description = Some(description.into());
        self
    }

    pub fn appearance(mut self, appearance: ButtonAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = Some(icon_size.max(0.0));
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.enabled_reader = None;
        self
    }

    pub fn enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.enabled_reader = Some(Box::new(enabled));
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    pub fn on_press_with_ctx<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(&mut EventCtx) + 'static,
    {
        self.on_press_with_ctx = Some(Box::new(on_press));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_size(&self) -> f32 {
        let theme = self.resolved_theme();
        self.size
            .unwrap_or(theme.metrics.icon_button_size)
            .max(theme.metrics.min_height)
    }

    fn resolved_icon_size(&self) -> f32 {
        let theme = self.resolved_theme();
        self.icon_size
            .unwrap_or(theme.metrics.icon_size)
            .min(self.resolved_size())
            .max(0.0)
    }

    fn is_selected(&self) -> bool {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
    }

    fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        if !self.is_enabled() {
            return;
        }
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
        if let Some(on_press) = &mut self.on_press_with_ctx {
            on_press(ctx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for IconButton {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_enabled() {
            if self.hovered || self.pressed {
                let theme = self.resolved_theme();
                self.hovered = false;
                self.pressed = false;
                set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.request_paint();
                ctx.request_semantics();
            }
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && (pointer.button == Some(PointerButton::Primary) || self.pressed) =>
            {
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let side = self.resolved_size();
        constraints.clamp(Size::new(side, side))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        paint_icon_button(
            ctx,
            &theme,
            ctx.bounds(),
            self.icon,
            IconButtonPaint::new()
                .appearance(self.appearance)
                .tone(self.tone)
                .selected(self.is_selected())
                .enabled(self.is_enabled())
                .hover_progress(self.hover_animation.value)
                .press_progress(self.press_animation.value)
                .focus_progress(self.focus_animation.value)
                .icon_size(self.resolved_icon_size()),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.label.clone());
        node.description = self.semantic_description.clone();
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered && self.is_enabled();
        node.state.selected = self.is_selected();
        node.state.disabled = !self.is_enabled();
        node.actions = if self.is_enabled() {
            vec![SemanticsAction::Focus, SemanticsAction::Activate]
        } else {
            Vec::new()
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Label {
    text: String,
    text_reader: Option<Box<dyn Fn() -> String>>,
    text_source: Option<Arc<dyn Observable<String>>>,
    semantic_name: Option<String>,
    style: TextStyle,
    color_reader: Option<Box<dyn Fn() -> Color>>,
    measurement: Option<TextMeasurement>,
    layout: Option<PersistentTextLayout>,
    selection_scope: Option<SelectionScope>,
    selection: TextSelection,
    dragging_selection: Option<u64>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            text_reader: None,
            text_source: None,
            semantic_name: None,
            style: DefaultTheme::default().body_text_style(),
            color_reader: None,
            measurement: None,
            layout: None,
            selection_scope: None,
            selection: TextSelection::new(TextCursor::new(0), TextCursor::new(0)),
            dragging_selection: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self::new(fallback).text_when(reader)
    }

    pub fn text_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.text_reader = Some(Box::new(reader));
        self.text_source = None;
        self
    }

    /// Bind label text to an observable value.
    ///
    /// Changes automatically invalidate text measurement, paint, and
    /// semantics for this retained label instance.
    pub fn text_from<O>(mut self, source: O) -> Self
    where
        O: Observable<String> + 'static,
    {
        self.text = source.get();
        self.text_reader = None;
        self.text_source = Some(Arc::new(source));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn selection_scope(&self) -> Option<&SelectionScope> {
        self.selection_scope.as_ref()
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.style = theme.body_text_style();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.text_reader = None;
        self.text_source = None;
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self.color_reader = None;
        self
    }

    pub fn color_when<F>(mut self, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.color_reader = Some(Box::new(color));
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.style.font_size = font_size.max(1.0);
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.line_height = line_height.max(1.0);
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    fn current_text(&self) -> String {
        self.text_source
            .as_ref()
            .map(|source| source.get())
            .or_else(|| self.text_reader.as_ref().map(|reader| reader()))
            .unwrap_or_else(|| self.text.clone())
    }

    fn observed_text(&self, ctx: &MeasureCtx) -> String {
        self.text_source
            .as_ref()
            .map(|source| ctx.observe_with(source.as_ref(), InvalidationKind::Text))
            .unwrap_or_else(|| self.current_text())
    }

    fn resolved_style(&self) -> TextStyle {
        let mut style = self.style.clone();
        if let Some(color_reader) = &self.color_reader {
            style.color = color_reader();
        }
        style
    }

    fn has_explicit_line_break(text: &str) -> bool {
        text.bytes().any(|byte| matches!(byte, b'\n' | b'\r'))
    }

    fn owner_id(widget_id: WidgetId) -> SelectionOwnerId {
        SelectionOwnerId::from(widget_id)
    }

    fn selected_range(&self, text_len: usize) -> std::ops::Range<usize> {
        selection_range(&self.selection, text_len)
    }

    fn active_selection_range(
        &self,
        widget_id: WidgetId,
        text_len: usize,
    ) -> Option<std::ops::Range<usize>> {
        let owner = Self::owner_id(widget_id);
        let range = self.selected_range(text_len);
        self.selection_scope
            .as_ref()
            .is_some_and(|scope| scope.has_owner_selection(owner))
            .then_some(range)
            .filter(|range| !range.is_empty())
    }

    fn sync_selection_scope(&self, ctx: &mut EventCtx, text: &str) {
        let Some(scope) = &self.selection_scope else {
            return;
        };
        let owner = Self::owner_id(ctx.widget_id());
        let range = self.selected_range(text.len());
        let selected = text.get(range.clone()).unwrap_or("").to_string();
        let change = scope.replace_text(owner, owner, range, text.len(), selected);
        request_selection_change(ctx, change);
    }

    fn label_layout_origin_for_event(&self, bounds: Rect, layout: &PersistentTextLayout) -> Point {
        let measurement = layout.measurement();
        let height = self
            .resolved_style()
            .line_height
            .max(measurement.height)
            .min(bounds.height());
        Point::new(
            bounds.x() - measurement.bounds.x(),
            bounds.y() + ((bounds.height() - height).max(0.0) * 0.5),
        )
    }

    fn hit_test_offset(&self, bounds: Rect, position: Point, text_len: usize) -> usize {
        self.layout
            .as_ref()
            .map(|layout| {
                let origin = self.label_layout_origin_for_event(bounds, layout);
                layout
                    .hit_test_point(Point::new(position.x - origin.x, position.y - origin.y))
                    .utf8_offset
                    .min(text_len)
            })
            .unwrap_or(text_len)
    }

    fn set_selection(&mut self, anchor: usize, focus: usize, text_len: usize) {
        self.selection = TextSelection::new(
            TextCursor::new(anchor.min(text_len)),
            TextCursor::new(focus.min(text_len)),
        );
    }

    fn copy_selection(&self, ctx: &mut EventCtx) -> bool {
        let text = self.current_text();
        let range = self.selected_range(text.len());
        let Some(selected) = text.get(range).filter(|selected| !selected.is_empty()) else {
            return false;
        };
        ctx.set_clipboard_text(selected);
        true
    }
}

impl Widget for Label {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.selection_scope.is_none() {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != sui_runtime::EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                let text = self.current_text();
                let offset = self.hit_test_offset(ctx.bounds(), pointer.position, text.len());
                let anchor = if pointer.modifiers.shift {
                    self.selection.anchor.utf8_offset
                } else {
                    offset
                };
                self.set_selection(anchor, offset, text.len());
                self.dragging_selection = Some(pointer.pointer_id);
                self.sync_selection_scope(ctx, &text);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self.dragging_selection == Some(pointer.pointer_id)
                    && pointer.buttons.contains(PointerButton::Primary) =>
            {
                let text = self.current_text();
                let anchor = self.selection.anchor.utf8_offset;
                let focus = self.hit_test_offset(ctx.bounds(), pointer.position, text.len());
                self.set_selection(anchor, focus, text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && self.dragging_selection == Some(pointer.pointer_id) =>
            {
                self.dragging_selection = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.dragging_selection == Some(pointer.pointer_id) =>
            {
                self.dragging_selection = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                // Preserve the active selection while handing the same press
                // to a wrapping ContextMenu. Its activation can route a
                // TextCommand back to this label after focus moves to the menu.
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && (key.modifiers.control || key.modifiers.meta)
                    && matches!(key.key.as_str(), "a" | "A") =>
            {
                let text = self.current_text();
                self.set_selection(0, text.len(), text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && (key.modifiers.control || key.modifiers.meta)
                    && matches!(key.key.as_str(), "c" | "C") =>
            {
                if self.copy_selection(ctx) {
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Escape" =>
            {
                if let Some(scope) = &self.selection_scope {
                    let change = scope.clear_owner(Self::owner_id(ctx.widget_id()));
                    request_selection_change(ctx, change);
                }
                self.set_selection(0, 0, 0);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                match &semantics.action {
                    SemanticsActionRequest::SetSelection(selection) => {
                        let text = self.current_text();
                        self.set_selection(selection.start, selection.end, text.len());
                        self.sync_selection_scope(ctx, &text);
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                    SemanticsActionRequest::Copy if self.copy_selection(ctx) => {
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(command) = TextCommand::from_command(command) else {
            return;
        };
        match command {
            TextCommand::SelectAll => {
                let text = self.current_text();
                self.set_selection(0, text.len(), text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            TextCommand::Copy => {
                if self.copy_selection(ctx) {
                    ctx.set_handled();
                }
            }
            TextCommand::Cut | TextCommand::Paste => {}
        }
    }

    fn intrinsic_size(
        &mut self,
        ctx: &mut MeasureCtx,
        axis: Axis,
        available_cross: f32,
    ) -> IntrinsicSize {
        if axis == Axis::Horizontal {
            let text = self.observed_text(ctx);
            let style = self.resolved_style();
            let natural = measure_text(ctx, &text, &style).width;
            let minimum = text
                .split_whitespace()
                .map(|segment| measure_text(ctx, segment, &style).width)
                .fold(0.0_f32, f32::max);
            return IntrinsicSize::new(minimum, natural);
        }

        let width = if available_cross.is_finite() {
            available_cross.max(0.0)
        } else {
            f32::INFINITY
        };
        let size = self.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(width, f32::INFINITY)),
        );
        IntrinsicSize::fixed(size.height)
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text = self.observed_text(ctx);
        let style = self.resolved_style();
        let natural_measurement = measure_text(ctx, &text, &style);
        let max_width = constraints.max.width;
        let wraps_to_constraint = max_width.is_finite() && natural_measurement.width > max_width;
        let needs_layout = self.selection_scope.is_some()
            || wraps_to_constraint
            || Self::has_explicit_line_break(&text);
        let mut measured_width = if wraps_to_constraint {
            max_width.max(0.0)
        } else {
            natural_measurement.width
        };
        let mut measurement = natural_measurement;

        if needs_layout {
            let layout_width = if max_width.is_finite() {
                measured_width.min(max_width).max(1.0)
            } else {
                measured_width.max(1.0)
            };
            measurement = ctx
                .layout()
                .shape_text(
                    text.clone(),
                    Size::new(layout_width, f32::INFINITY),
                    style.clone(),
                )
                .map(|layout| layout.measurement())
                .unwrap_or(measurement);
            if !wraps_to_constraint {
                measured_width = if max_width.is_finite() {
                    measurement.width.min(max_width).max(0.0)
                } else {
                    measurement.width.max(0.0)
                };
            }
            self.layout = ctx
                .layout()
                .shape_text_persistent(
                    self.layout.as_ref().map(|layout| layout.handle()),
                    text,
                    Size::new(
                        measured_width.max(1.0),
                        measurement.height.max(style.line_height).max(1.0),
                    ),
                    style.clone(),
                )
                .ok();
        } else {
            self.layout = None;
        }
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measured_width,
            measurement.height.max(style.line_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let text = self.current_text();
        let style = self.resolved_style();
        if let Some(layout) = &self.layout {
            let layout_bounds = layout.measurement().bounds;
            let mut layout_rect = aligned_text_rect_for_layout_with_mode(
                ctx,
                ctx.bounds(),
                layout.layout(),
                style.line_height,
                0.0,
                HorizontalTextAlignmentMode::Optical,
            );
            if layout.lines().len() > 1 {
                let block_height = style
                    .line_height
                    .max(layout.measurement().height)
                    .min(ctx.bounds().height());
                layout_rect = Rect::new(
                    layout_rect.x(),
                    ctx.bounds().y() + ((ctx.bounds().height() - block_height).max(0.0) * 0.5),
                    layout_rect.width(),
                    block_height,
                );
            }
            let origin = Point::new(layout_rect.x() - layout_bounds.x(), layout_rect.y());
            if let Some(range) = self.active_selection_range(ctx.widget_id(), text.len()) {
                let theme = DefaultTheme::default();
                for rect in layout.selection_rects(range) {
                    ctx.fill_rect(rect.translate(origin.to_vector()), theme.palette.selection);
                }
            }
            ctx.draw_persistent_text_layout(origin, layout);
        } else {
            paint_aligned_text(ctx, ctx.bounds(), &text, &style, style.line_height, 0.0);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let text = self.current_text();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or_else(|| text.clone()));
        if self.semantic_name.is_some() {
            node.value = Some(SemanticsValue::Text(text));
        }
        if self.selection_scope.is_some() {
            node.actions = vec![
                SemanticsAction::Focus,
                SemanticsAction::SetSelection,
                SemanticsAction::Copy,
            ];
            node.state.focused = ctx.is_focused();
        }
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.selection_scope.is_some()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Link {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    label_reader: Option<Box<dyn Fn() -> String>>,
    url: String,
    url_reader: Option<Box<dyn Fn() -> String>>,
    semantic_name: Option<String>,
    text_style: Option<TextStyle>,
    enabled: bool,
    enabled_reader: Option<Box<dyn Fn() -> bool>>,
    hovered: bool,
    pressed: bool,
    measurement: Option<TextMeasurement>,
    on_open: Option<Box<dyn FnMut(&str)>>,
    on_open_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, &str)>>,
}

impl Link {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            url: url.into(),
            url_reader: None,
            semantic_name: None,
            text_style: None,
            enabled: true,
            enabled_reader: None,
            hovered: false,
            pressed: false,
            measurement: None,
            on_open: None,
            on_open_with_ctx: None,
        }
    }

    pub fn url(url: impl Into<String>) -> Self {
        let url = url.into();
        Self::new(url.clone(), url)
    }

    pub fn label_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.label_reader = Some(Box::new(reader));
        self
    }

    pub fn url_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.url_reader = Some(Box::new(reader));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.enabled_reader = None;
        self
    }

    pub fn enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.enabled_reader = Some(Box::new(enabled));
        self
    }

    pub fn on_open<F>(mut self, on_open: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_open = Some(Box::new(on_open));
        self
    }

    pub fn on_open_with_ctx<F>(mut self, on_open: F) -> Self
    where
        F: FnMut(&mut EventCtx, &str) + 'static,
    {
        self.on_open_with_ctx = Some(Box::new(on_open));
        self
    }

    fn current_label(&self) -> String {
        single_line_text(
            self.label_reader
                .as_ref()
                .map(|reader| reader())
                .unwrap_or_else(|| self.label.clone()),
        )
    }

    fn current_url(&self) -> String {
        single_line_text(
            self.url_reader
                .as_ref()
                .map(|reader| reader())
                .unwrap_or_else(|| self.url.clone()),
        )
        .trim()
        .to_string()
    }

    fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_text_style(&self, color: Color) -> TextStyle {
        let mut style = self
            .text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style());
        style.color = color;
        style
    }

    fn resolved_color(&self, theme: &DefaultTheme) -> Color {
        if !self.is_enabled() {
            return theme
                .palette
                .placeholder
                .with_alpha(theme.interaction.disabled_content_opacity);
        }
        if self.pressed {
            theme.palette.accent_pressed
        } else if self.hovered {
            theme.palette.accent_hover
        } else {
            theme.palette.accent
        }
    }

    fn is_visible_parts(label: &str, url: &str) -> bool {
        !label.trim().is_empty() && !url.trim().is_empty()
    }

    fn is_visible(&self) -> bool {
        Self::is_visible_parts(&self.current_label(), &self.current_url())
    }

    fn can_activate_parts(&self, label: &str, url: &str) -> bool {
        self.is_enabled() && Self::is_visible_parts(label, url)
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !self.can_activate_parts(&label, &url) {
            return;
        }
        if let Some(on_open) = &mut self.on_open {
            on_open(&url);
        }
        if let Some(on_open) = &mut self.on_open_with_ctx {
            on_open(ctx, &url);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn reset_interaction(&mut self, ctx: &mut EventCtx) {
        if self.hovered || self.pressed {
            self.hovered = false;
            self.pressed = false;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for Link {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_visible() || !self.is_enabled() {
            self.reset_interaction(ctx);
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if _pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
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
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
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
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            self.measurement = None;
            return constraints.clamp(Size::ZERO);
        }

        let theme = self.resolved_theme();
        let style = self.resolved_text_style(self.resolved_color(&theme));
        let measurement = measure_text(ctx, &label, &style);
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measurement.width,
            measurement.height.max(style.line_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            return;
        }

        let theme = self.resolved_theme();
        let color = self.resolved_color(&theme);
        let style = self.resolved_text_style(color);
        let bounds = ctx.bounds();
        ctx.push_clip_rect(bounds);
        paint_single_line_aligned_text(ctx, bounds, &label, &style, style.line_height, 0.0);
        ctx.pop_clip();

        let measured_width = self
            .measurement
            .map(|measurement| measurement.width)
            .unwrap_or(bounds.width())
            .min(bounds.width())
            .max(0.0);
        if measured_width > 0.0 {
            let underline_y = bounds.y() + bounds.height() - physical_pixels(ctx, 2.0);
            let mut underline = PathBuilder::new();
            underline
                .move_to(Point::new(bounds.x(), underline_y))
                .line_to(Point::new(bounds.x() + measured_width, underline_y));
            ctx.stroke(
                underline.build(),
                color,
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
        }

        if ctx.is_focused() && self.is_enabled() {
            ctx.stroke_rect(
                bounds.inflate(physical_pixels(ctx, 2.0), physical_pixels(ctx, 1.0)),
                theme.palette.focus_ring,
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            return;
        }

        let enabled = self.is_enabled();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Link, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or(label));
        node.value = Some(SemanticsValue::Text(url));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered && enabled;
        node.state.disabled = !enabled;
        node.actions = if enabled {
            vec![SemanticsAction::Focus, SemanticsAction::Activate]
        } else {
            Vec::new()
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled() && self.is_visible()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Button {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    semantic_name: Option<String>,
    semantic_description: Option<String>,
    appearance: ButtonAppearance,
    tone: SemanticTone,
    text_style: Option<TextStyle>,
    icon: Option<IconGlyph>,
    icon_size: Option<f32>,
    icon_gap: Option<f32>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurement: Option<TextMeasurement>,
    label_layout: Option<PersistentTextLayout>,
    enabled: bool,
    enabled_reader: Option<Box<dyn Fn() -> bool>>,
    on_press: Option<Box<dyn FnMut()>>,
    on_press_with_ctx: Option<Box<dyn FnMut(&mut EventCtx)>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ButtonVisuals {
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
    label_color: Color,
    label_peak_lift: f32,
    chrome_style: Option<ResolvedHdrStyle>,
}

impl Button {
    /// Creates a neutral, tonal button for an ordinary action.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            semantic_name: None,
            semantic_description: None,
            appearance: ButtonAppearance::Tonal,
            tone: SemanticTone::Neutral,
            text_style: None,
            icon: None,
            icon_size: None,
            icon_gap: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurement: None,
            label_layout: None,
            enabled: true,
            enabled_reader: None,
            on_press: None,
            on_press_with_ctx: None,
        }
    }

    /// Creates a filled accent button for the primary action on a surface.
    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label).primary_action()
    }

    /// Creates a filled danger button for a destructive or irreversible action.
    pub fn danger(label: impl Into<String>) -> Self {
        Self::new(label).danger_action()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.semantic_description = Some(description.into());
        self
    }

    /// Selects the button's visual emphasis without changing its semantic
    /// meaning. The default is [`ButtonAppearance::Tonal`].
    pub fn appearance(mut self, appearance: ButtonAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Selects the semantic color family used by this button. The default is
    /// [`SemanticTone::Neutral`].
    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    /// Promotes this button to a filled accent primary action.
    pub fn primary_action(mut self) -> Self {
        self.appearance = ButtonAppearance::Filled;
        self.tone = SemanticTone::Accent;
        self
    }

    /// Promotes this button to a filled danger action.
    pub fn danger_action(mut self) -> Self {
        self.appearance = ButtonAppearance::Filled;
        self.tone = SemanticTone::Danger;
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = Some(icon_size.max(0.0));
        self
    }

    pub fn icon_gap(mut self, icon_gap: f32) -> Self {
        self.icon_gap = Some(icon_gap.max(0.0));
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width.max(0.0));
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.enabled_reader = None;
        self
    }

    pub fn enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.enabled_reader = Some(Box::new(enabled));
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    pub fn on_press_with_ctx<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(&mut EventCtx) + 'static,
    {
        self.on_press_with_ctx = Some(Box::new(on_press));
        self
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        if !self.is_enabled() {
            return;
        }
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
        if let Some(on_press) = &mut self.on_press_with_ctx {
            on_press(ctx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().button_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.button_padding)
    }

    fn resolved_icon_size(&self) -> f32 {
        self.icon_size
            .unwrap_or(self.resolved_theme().metrics.icon_size)
            .max(0.0)
    }

    fn resolved_icon_gap(&self) -> f32 {
        self.icon_gap
            .unwrap_or(self.resolved_theme().metrics.icon_label_gap)
            .max(0.0)
    }

    fn icon_extent(&self) -> Option<(f32, f32)> {
        self.icon.map(|_| {
            let icon_size = self.resolved_icon_size();
            let gap = if self.label.is_empty() {
                0.0
            } else {
                self.resolved_icon_gap()
            };
            (icon_size, gap)
        })
    }

    fn resolved_min_size(&self) -> Size {
        let theme = self.resolved_theme();
        Size::new(
            self.min_width.unwrap_or(theme.metrics.button_min_width),
            self.min_height.unwrap_or(theme.metrics.min_height),
        )
    }

    #[cfg(test)]
    fn resolved_visuals(&self, focused: bool) -> ButtonVisuals {
        self.resolved_visuals_with_focus_progress(focused as u8 as f32)
    }

    fn resolved_visuals_with_focus_progress(&self, focus_progress: f32) -> ButtonVisuals {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let interaction = theme.interaction;
        let enabled = self.is_enabled();
        let focus_progress = if enabled {
            focus_progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let hover_progress = if enabled {
            self.hover_animation.value * interaction.hover_blend
        } else {
            0.0
        };
        let press_progress = if enabled {
            self.press_animation.value * interaction.pressed_blend
        } else {
            0.0
        };
        let legacy_default =
            self.appearance == ButtonAppearance::Filled && self.tone == SemanticTone::Accent;
        if !legacy_default {
            let semantic = semantic_button_visuals(
                &theme,
                self.appearance,
                self.tone,
                enabled,
                self.hover_animation.value,
                self.press_animation.value,
            );
            let label_peak_lift = resolve_luminance_role(&theme.hdr, WidgetLuminanceRole::Standard);
            let label_color = if enabled {
                apply_hdr_policy_cap(
                    self.text_style
                        .as_ref()
                        .map(|style| style.color)
                        .unwrap_or(semantic.content),
                    label_peak_lift,
                )
            } else {
                apply_hdr_policy_cap(semantic.content, label_peak_lift)
            };
            return ButtonVisuals {
                background: semantic.background,
                border: if enabled {
                    mix_color(semantic.border, palette.border_focus, focus_progress)
                } else {
                    semantic.border
                },
                focus_ring: (focus_progress > 0.0).then_some(
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                ),
                label_color,
                label_peak_lift,
                chrome_style: None,
            };
        }
        let background = if !enabled {
            mix_color(palette.control, palette.accent, 0.08)
                .with_alpha(interaction.disabled_opacity)
        } else {
            mix_color(
                mix_color(palette.accent, palette.accent_hover, hover_progress),
                palette.accent_pressed,
                press_progress,
            )
        };
        let border_base = if !enabled {
            palette
                .accent_border
                .with_alpha(interaction.disabled_content_opacity)
        } else {
            mix_color(
                palette.accent_border,
                palette.accent_border_hover,
                hover_progress,
            )
        };
        let border = if enabled {
            mix_color(border_base, palette.accent_border_focus, focus_progress)
        } else {
            border_base
        };
        let label_peak_lift = resolve_luminance_role(&theme.hdr, WidgetLuminanceRole::Standard);
        let label_color = if enabled {
            apply_hdr_policy_cap(self.resolved_text_style().color, label_peak_lift)
        } else {
            apply_hdr_policy_cap(palette.text_muted, label_peak_lift)
        };

        if matches!(theme.hdr.mode, HdrThemeMode::Disabled) {
            return ButtonVisuals {
                background,
                border,
                focus_ring: (focus_progress > 0.0).then_some(
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                ),
                label_color,
                label_peak_lift,
                chrome_style: None,
            };
        }

        let chrome_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::SemanticAccent,
            WidgetMaterialRole::Flat,
            None,
        ));
        let focus_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::Focused,
            WidgetMaterialRole::Flat,
            None,
        ));
        let hdr_background = if !enabled {
            background
        } else {
            mix_color(
                mix_color(chrome_style.color, palette.accent_hover, hover_progress),
                palette.accent_pressed,
                press_progress,
            )
        };
        let hdr_border_base = if !enabled {
            border
        } else {
            mix_color(
                palette.accent_border,
                palette.accent_border_hover,
                hover_progress,
            )
        };
        let hdr_border = if enabled {
            mix_color(hdr_border_base, focus_style.color, focus_progress)
        } else {
            hdr_border_base
        };

        ButtonVisuals {
            background: hdr_background,
            border: hdr_border,
            focus_ring: (focus_progress > 0.0).then_some(
                focus_style
                    .color
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
            label_color,
            label_peak_lift,
            chrome_style: Some(chrome_style),
        }
    }

    fn button_content_rects(&self, bounds: Rect, padding: Insets) -> (Option<Rect>, Rect, f32) {
        let content = inset_rect(bounds, padding);
        let Some((icon_size, icon_gap)) = self.icon_extent() else {
            return (None, content, 0.5);
        };

        let measurement = self.label_measurement;
        let natural_label_width = measurement.map(|value| value.width).unwrap_or(0.0);
        let icon_size = icon_size
            .min(content.width())
            .min(content.height())
            .max(0.0);
        let gap = if icon_size > 0.0 && natural_label_width > 0.0 {
            icon_gap.min(content.width())
        } else {
            0.0
        };
        let label_width = natural_label_width.min((content.width() - icon_size - gap).max(0.0));
        let group_width = icon_size + gap + label_width;
        let start_x = content.x() + ((content.width() - group_width).max(0.0) * 0.5);
        let icon_rect = Rect::new(
            start_x,
            content.y() + ((content.height() - icon_size) * 0.5),
            icon_size,
            icon_size,
        );
        let label_base = Rect::new(
            start_x + icon_size + gap,
            content.y(),
            label_width,
            content.height(),
        );
        (Some(icon_rect), label_base, 0.0)
    }
}

impl Widget for Button {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_enabled() {
            if self.hovered || self.pressed {
                let theme = self.resolved_theme();
                self.hovered = false;
                self.pressed = false;
                set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.request_paint();
                ctx.request_semantics();
            }
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && (pointer.button == Some(PointerButton::Primary) || self.pressed) =>
            {
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let min_size = self.resolved_min_size();
        let measured = measure_text(ctx, &self.label, &text_style);
        let label_layout = ctx
            .layout()
            .shape_text_persistent(
                self.label_layout.as_ref().map(|layout| layout.handle()),
                self.label.clone(),
                Size::new(
                    f32::INFINITY,
                    measured.height.max(text_style.line_height).max(1.0),
                ),
                text_style.clone(),
            )
            .ok();
        let measurement = label_layout
            .as_ref()
            .map(|layout| layout.measurement())
            .unwrap_or(measured);
        self.label_measurement = Some(measurement);
        self.label_layout = label_layout;

        let (icon_size, icon_gap) = self.icon_extent().unwrap_or((0.0, 0.0));
        let content_width = icon_size + icon_gap + measurement.width;
        let content_height = measurement
            .height
            .max(text_style.line_height)
            .max(icon_size);
        let width = (content_width + padding.left + padding.right).max(min_size.width);
        let height = (content_height + padding.top + padding.bottom).max(min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let visuals = self.resolved_visuals_with_focus_progress(self.focus_animation.value);
        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            visuals.background,
            visuals.border,
            visuals.focus_ring,
        );
        let (icon_rect, label_slot, label_alignment) =
            self.button_content_rects(ctx.bounds(), padding);
        if let (Some(icon), Some(icon_rect)) = (self.icon, icon_rect) {
            draw_icon_glyph(ctx, icon, icon_rect, visuals.label_color);
        }
        if self.is_enabled()
            && let Some(layout) = &self.label_layout
        {
            let layout_rect = aligned_text_rect_for_layout_with_mode(
                ctx,
                label_slot,
                layout.layout(),
                text_style.line_height,
                label_alignment,
                HorizontalTextAlignmentMode::Optical,
            );
            let layout_bounds = layout.measurement().bounds;
            ctx.push_clip_rect(layout_rect);
            ctx.draw_persistent_text_layout_with_color(
                Point::new(layout_rect.x() - layout_bounds.x(), layout_rect.y()),
                layout,
                visuals.label_color,
            );
            ctx.pop_clip();
            return;
        }
        let paint_style = TextStyle {
            color: visuals.label_color,
            ..text_style
        };
        paint_aligned_text(
            ctx,
            label_slot,
            &self.label,
            &paint_style,
            paint_style.line_height,
            label_alignment,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(
            self.semantic_name
                .clone()
                .unwrap_or_else(|| self.label.clone()),
        );
        node.description = self.semantic_description.clone();
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered && self.is_enabled();
        node.state.disabled = !self.is_enabled();
        node.actions = if self.is_enabled() {
            vec![SemanticsAction::Focus, SemanticsAction::Activate]
        } else {
            Vec::new()
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Checkbox {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    checked: bool,
    appearance: ChoiceAppearance,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    indicator_size: Option<f32>,
    gap: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    toggle_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurement: Option<TextMeasurement>,
    on_toggle: Option<Box<dyn FnMut(bool)>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckboxIndicatorState {
    pub checked: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
}

impl CheckboxIndicatorState {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            hovered: false,
            pressed: false,
            focused: false,
        }
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CheckboxIndicatorVisual {
    hover_progress: f32,
    press_progress: f32,
    toggle_progress: f32,
    focus_progress: f32,
}

pub fn paint_checkbox_indicator(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    state: CheckboxIndicatorState,
) {
    paint_checkbox_indicator_visual(
        ctx,
        theme,
        rect,
        CheckboxIndicatorVisual {
            hover_progress: state.hovered as u8 as f32,
            press_progress: state.pressed as u8 as f32,
            toggle_progress: state.checked as u8 as f32,
            focus_progress: state.focused as u8 as f32,
        },
    );
}

fn paint_checkbox_indicator_visual(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    visual: CheckboxIndicatorVisual,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let metrics = theme.metrics;
    let interaction = theme.interaction;
    let hover_blend = visual.hover_progress * interaction.hover_blend;
    let press_blend = visual.press_progress * interaction.pressed_blend;
    let indicator_background = mix_color(
        mix_color(palette.control_active, palette.surface_focus, hover_blend),
        mix_color(
            mix_color(palette.accent, palette.accent_hover, hover_blend),
            palette.accent_pressed,
            press_blend,
        ),
        visual.toggle_progress,
    );
    let border = mix_color(
        mix_color(palette.border, palette.border_hover, visual.hover_progress),
        palette.border_focus,
        visual.focus_progress,
    );
    let indicator_border = mix_color(
        border,
        palette.accent_border_focus,
        visual.toggle_progress.max(visual.focus_progress),
    );

    draw_control_shape(
        ctx,
        rect,
        metrics.indicator_corner_radius,
        metrics.border_width,
        indicator_background,
        indicator_border,
    );
    if visual.toggle_progress > 0.0 {
        let check_color = palette.accent_text.with_alpha(visual.toggle_progress);
        ctx.stroke(
            checkmark_path(rect.inflate(-4.0, -4.0)),
            check_color,
            StrokeStyle::new(physical_pixels(ctx, interaction.active_indicator_thickness)),
        );
    }
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            checked: false,
            appearance: ChoiceAppearance::Plain,
            text_style: None,
            padding: None,
            indicator_size: None,
            gap: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            toggle_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurement: None,
            on_toggle: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self.toggle_animation = AnimatedScalar::new(checked as u8 as f32);
        self
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Selects whether the complete checkbox row is plain or framed.
    pub fn appearance(mut self, appearance: ChoiceAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Uses the quiet, transparent-at-rest row treatment.
    pub fn plain(self) -> Self {
        self.appearance(ChoiceAppearance::Plain)
    }

    /// Uses the filled and bordered row treatment.
    pub fn framed(self) -> Self {
        self.appearance(ChoiceAppearance::Framed)
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn indicator_size(mut self, indicator_size: f32) -> Self {
        self.indicator_size = Some(indicator_size.max(0.0));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
        self.toggle_animation = AnimatedScalar::new(checked as u8 as f32);
    }

    pub fn on_toggle<F>(mut self, on_toggle: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    fn toggle(&mut self) {
        self.checked = !self.checked;
        if let Some(on_toggle) = &mut self.on_toggle {
            on_toggle(self.checked);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.toggle_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.checkbox_padding)
    }

    fn resolved_indicator_size(&self) -> f32 {
        self.indicator_size
            .unwrap_or(self.resolved_theme().metrics.checkbox_indicator_size)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.checkbox_gap)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for Checkbox {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let toggle = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if toggle {
                    self.toggle();
                    set_toggle_animation_target(
                        &mut self.toggle_animation,
                        self.checked as u8 as f32,
                        &theme,
                        ctx,
                    );
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                self.toggle();
                set_toggle_animation_target(
                    &mut self.toggle_animation,
                    self.checked as u8 as f32,
                    &theme,
                    ctx,
                );
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let measurement = measure_text(ctx, &self.label, &text_style);
        self.label_measurement = Some(measurement);

        let width = padding.left + indicator_size + gap + measurement.width + padding.right;
        let content_height = indicator_size.max(measurement.height.max(text_style.line_height));
        let height = choice_control_height(
            content_height,
            padding,
            default_form_control_height(&theme),
            self.padding.is_some(),
        );

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let hover_progress = self.hover_animation.value * interaction.hover_blend;
        let press_progress = self.press_animation.value * interaction.pressed_blend;
        let toggle_progress = self.toggle_animation.value;
        let focus_progress = self.focus_animation.value;
        let framed_background = mix_color(
            mix_color(palette.control, palette.control_hover, hover_progress),
            palette.control_active,
            press_progress,
        );
        let framed_border = mix_color(
            mix_color(
                palette.border,
                palette.border_hover,
                self.hover_animation.value,
            ),
            palette.border_focus,
            focus_progress,
        );
        let frame_visuals = choice_frame_visuals(
            &theme,
            self.appearance,
            framed_background,
            framed_border,
            hover_progress,
            press_progress,
            focus_progress,
        );
        let layout_padding = choice_control_layout_padding(padding, self.padding.is_some());
        let indicator = indicator_rect(ctx.bounds(), layout_padding, indicator_size);
        let label_rect = checkbox_label_rect(ctx.bounds(), layout_padding, indicator_size, gap);

        draw_choice_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            self.appearance,
            frame_visuals,
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );

        paint_checkbox_indicator_visual(
            ctx,
            &theme,
            indicator,
            CheckboxIndicatorVisual {
                hover_progress: self.hover_animation.value,
                press_progress: self.press_animation.value,
                toggle_progress,
                focus_progress,
            },
        );
        paint_aligned_text(
            ctx,
            label_rect,
            &self.label,
            &text_style,
            text_style.line_height,
            0.0,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::CheckBox, ctx.bounds());
        node.name = Some(self.label.clone());
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.state.checked = Some(if self.checked {
            ToggleState::Checked
        } else {
            ToggleState::Unchecked
        });
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Switch {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    on: bool,
    appearance: ChoiceAppearance,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    gap: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    toggle_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurement: Option<TextMeasurement>,
    on_toggle: Option<Box<dyn FnMut(bool)>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SwitchVisuals {
    frame_background: Color,
    frame_border: Color,
    track_color: Color,
    track_border: Color,
    thumb_color: Color,
    label_color: Color,
    label_peak_lift: f32,
    indicator_style: Option<ResolvedHdrStyle>,
}

impl Switch {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            on: false,
            appearance: ChoiceAppearance::Plain,
            text_style: None,
            padding: None,
            gap: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            toggle_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurement: None,
            on_toggle: None,
        }
    }

    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self.toggle_animation = AnimatedScalar::new(on as u8 as f32);
        self
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Selects whether the complete switch row is plain or framed.
    pub fn appearance(mut self, appearance: ChoiceAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Uses the quiet, transparent-at-rest row treatment.
    pub fn plain(self) -> Self {
        self.appearance(ChoiceAppearance::Plain)
    }

    /// Uses the filled and bordered row treatment.
    pub fn framed(self) -> Self {
        self.appearance(ChoiceAppearance::Framed)
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        self.toggle_animation = AnimatedScalar::new(on as u8 as f32);
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn on_toggle<F>(mut self, on_toggle: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.checkbox_padding)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.checkbox_gap)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn toggle(&mut self) {
        self.on = !self.on;
        if let Some(on_toggle) = &mut self.on_toggle {
            on_toggle(self.on);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.toggle_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    fn resolved_visuals_for_state(&self, on: bool, focused: bool) -> SwitchVisuals {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let interaction = theme.interaction;
        let hover_t = self.hover_animation.value * interaction.hover_blend;
        let press_t = self.press_animation.value * interaction.pressed_blend;
        let framed_background = mix_color(
            mix_color(
                palette.control,
                palette.control_hover,
                self.hover_animation.value,
            ),
            palette.control_active,
            press_t,
        );
        let framed_background = if focused {
            mix_color(framed_background, palette.surface_focus, 0.5)
        } else {
            framed_background
        };
        let framed_border = mix_color(
            mix_color(
                palette.border,
                palette.border_hover,
                self.hover_animation.value,
            ),
            palette.border_focus,
            focused as u8 as f32,
        );
        let frame_visuals = choice_frame_visuals(
            &theme,
            self.appearance,
            framed_background,
            framed_border,
            hover_t,
            press_t,
            focused as u8 as f32,
        );
        let baseline_track_color = if on {
            mix_color(
                mix_color(palette.accent, palette.accent_hover, hover_t),
                palette.accent_pressed,
                press_t,
            )
        } else {
            mix_color(palette.surface_focus, palette.control_active, hover_t)
        };
        let baseline_track_border = if on {
            palette.accent_border
        } else {
            mix_color(
                palette.border,
                palette.border_hover,
                self.hover_animation.value,
            )
        };
        let thumb_color = if matches!(
            theme.colors.scheme,
            ThemeColorScheme::Dark | ThemeColorScheme::HighContrast
        ) {
            palette.text
        } else {
            palette.accent_text
        };
        let label_peak_lift = resolve_luminance_role(&theme.hdr, WidgetLuminanceRole::Standard);
        let label_color = apply_hdr_policy_cap(self.resolved_text_style().color, label_peak_lift);

        if matches!(theme.hdr.mode, HdrThemeMode::Disabled) || !on {
            return SwitchVisuals {
                frame_background: frame_visuals.background,
                frame_border: frame_visuals.border,
                track_color: baseline_track_color,
                track_border: baseline_track_border,
                thumb_color,
                label_color,
                label_peak_lift,
                indicator_style: None,
            };
        }

        let indicator_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::EmissiveIndicator,
            WidgetMaterialRole::Flat,
            None,
        ));

        SwitchVisuals {
            frame_background: frame_visuals.background,
            frame_border: frame_visuals.border,
            track_color: mix_color(
                mix_color(indicator_style.color, palette.accent_hover, hover_t),
                palette.accent_pressed,
                press_t,
            ),
            track_border: if focused {
                indicator_style.color
            } else {
                palette.accent_border
            },
            thumb_color,
            label_color,
            label_peak_lift,
            indicator_style: Some(indicator_style),
        }
    }

    fn resolved_visuals(&self, focused: bool) -> SwitchVisuals {
        self.resolved_visuals_for_state(self.on, focused)
    }
}

impl Widget for Switch {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let toggle = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if toggle {
                    self.toggle();
                    set_toggle_animation_target(
                        &mut self.toggle_animation,
                        self.on as u8 as f32,
                        &theme,
                        ctx,
                    );
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                self.toggle();
                set_toggle_animation_target(
                    &mut self.toggle_animation,
                    self.on as u8 as f32,
                    &theme,
                    ctx,
                );
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let gap = self.resolved_gap();
        let measurement = measure_text(ctx, &self.label, &text_style);
        self.label_measurement = Some(measurement);
        let track_width = theme.metrics.switch_track_width;
        let track_height = theme.metrics.switch_track_height;

        let content_height = track_height.max(measurement.height.max(text_style.line_height));

        constraints.clamp(Size::new(
            padding.left + track_width + gap + measurement.width + padding.right,
            choice_control_height(
                content_height,
                padding,
                default_form_control_height(&theme),
                self.padding.is_some(),
            ),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let palette = theme.palette;
        let interaction = theme.interaction;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let gap = self.resolved_gap();
        let track = switch_track_rect(ctx.bounds(), padding, metrics);
        let label_rect = switch_label_rect(ctx.bounds(), padding, metrics, gap);
        let visuals = self.resolved_visuals(ctx.is_focused());
        let off_visuals = self.resolved_visuals_for_state(false, ctx.is_focused());
        let on_visuals = self.resolved_visuals_for_state(true, ctx.is_focused());
        let hover_progress = self.hover_animation.value * interaction.hover_blend;
        let press_progress = self.press_animation.value * interaction.pressed_blend;
        let toggle_progress = self.toggle_animation.value;
        let focus_progress = self.focus_animation.value;

        let framed_background = mix_color(
            mix_color(
                mix_color(palette.control, palette.control_hover, hover_progress),
                palette.surface_focus,
                focus_progress,
            ),
            palette.control_active,
            press_progress,
        );
        let framed_border = mix_color(
            mix_color(palette.border, palette.border_hover, hover_progress),
            palette.border_focus,
            focus_progress,
        );
        let frame_visuals = choice_frame_visuals(
            &theme,
            self.appearance,
            framed_background,
            framed_border,
            hover_progress,
            press_progress,
            focus_progress,
        );

        draw_choice_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            self.appearance,
            frame_visuals,
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );

        let thumb_inset = metrics.switch_thumb_inset;
        let thumb_size = (track.height() - (thumb_inset * 2.0)).max(0.0);
        let thumb_x_off = track.x() + thumb_inset;
        let thumb_x_on = track.max_x() - thumb_size - thumb_inset;
        let thumb = Rect::new(
            f32::interpolate(thumb_x_off, thumb_x_on, toggle_progress),
            track.y() + thumb_inset,
            thumb_size,
            thumb_size,
        );

        let track_color = if toggle_progress <= f32::EPSILON {
            off_visuals.track_color
        } else if (1.0 - toggle_progress) <= f32::EPSILON {
            on_visuals.track_color
        } else {
            mix_color(
                off_visuals.track_color,
                on_visuals.track_color,
                toggle_progress,
            )
        };
        let track_border = if toggle_progress <= f32::EPSILON {
            off_visuals.track_border
        } else if (1.0 - toggle_progress) <= f32::EPSILON {
            on_visuals.track_border
        } else {
            mix_color(
                off_visuals.track_border,
                on_visuals.track_border,
                toggle_progress,
            )
        };

        draw_control_shape(
            ctx,
            track,
            track.height() * 0.5,
            physical_pixels(ctx, metrics.border_width),
            track_color,
            track_border,
        );
        ctx.fill(
            Path::circle(rect_center(thumb), thumb.width() * 0.5),
            visuals.thumb_color,
        );
        let text_style = TextStyle {
            color: visuals.label_color,
            ..text_style
        };
        paint_aligned_text(
            ctx,
            label_rect,
            &self.label,
            &text_style,
            text_style.line_height,
            0.0,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Switch, ctx.bounds());
        node.name = Some(self.label.clone());
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.state.checked = Some(if self.on {
            ToggleState::Checked
        } else {
            ToggleState::Unchecked
        });
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct RadioButton {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    selected: bool,
    appearance: ChoiceAppearance,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    indicator_size: Option<f32>,
    gap: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    toggle_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurement: Option<TextMeasurement>,
    on_select: Option<Box<dyn FnMut()>>,
}

impl RadioButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            selected: false,
            appearance: ChoiceAppearance::Plain,
            text_style: None,
            padding: None,
            indicator_size: None,
            gap: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            toggle_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurement: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.toggle_animation = AnimatedScalar::new(selected as u8 as f32);
        self
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Selects whether the complete radio row is plain or framed.
    pub fn appearance(mut self, appearance: ChoiceAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Uses the quiet, transparent-at-rest row treatment.
    pub fn plain(self) -> Self {
        self.appearance(ChoiceAppearance::Plain)
    }

    /// Uses the filled and bordered row treatment.
    pub fn framed(self) -> Self {
        self.appearance(ChoiceAppearance::Framed)
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
        self.toggle_animation = AnimatedScalar::new(selected as u8 as f32);
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn indicator_size(mut self, indicator_size: f32) -> Self {
        self.indicator_size = Some(indicator_size.max(0.0));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn on_select<F>(mut self, on_select: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_select = Some(Box::new(on_select));
        self
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.checkbox_padding)
    }

    fn resolved_indicator_size(&self) -> f32 {
        self.indicator_size
            .unwrap_or(self.resolved_theme().metrics.checkbox_indicator_size)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.checkbox_gap)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        let theme = self.resolved_theme();
        let changed = !self.selected;
        self.selected = true;
        if changed {
            set_toggle_animation_target(&mut self.toggle_animation, 1.0, &theme, ctx);
        }
        if let Some(on_select) = &mut self.on_select {
            on_select();
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.toggle_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for RadioButton {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
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
                let theme = self.resolved_theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = self.resolved_theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let measurement = measure_text(ctx, &self.label, &text_style);
        self.label_measurement = Some(measurement);

        let theme = self.resolved_theme();
        let content_height = indicator_size.max(measurement.height.max(text_style.line_height));

        constraints.clamp(Size::new(
            padding.left + indicator_size + gap + measurement.width + padding.right,
            choice_control_height(
                content_height,
                padding,
                default_form_control_height(&theme),
                self.padding.is_some(),
            ),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let hover_progress = self.hover_animation.value * interaction.hover_blend;
        let press_progress = self.press_animation.value * interaction.pressed_blend;
        let toggle_progress = self.toggle_animation.value;
        let focus_progress = self.focus_animation.value;
        let layout_padding = choice_control_layout_padding(padding, self.padding.is_some());
        let indicator = indicator_rect(ctx.bounds(), layout_padding, indicator_size);
        let label_rect = checkbox_label_rect(ctx.bounds(), layout_padding, indicator_size, gap);
        let framed_background = mix_color(
            mix_color(
                mix_color(palette.control, palette.control_hover, hover_progress),
                palette.surface_focus,
                focus_progress,
            ),
            palette.control_active,
            press_progress,
        );
        let framed_border = mix_color(
            mix_color(palette.border, palette.border_hover, hover_progress),
            palette.border_focus,
            focus_progress,
        );
        let frame_visuals = choice_frame_visuals(
            &theme,
            self.appearance,
            framed_background,
            framed_border,
            hover_progress,
            press_progress,
            focus_progress,
        );

        draw_choice_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            self.appearance,
            frame_visuals,
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );

        ctx.fill(
            Path::circle(rect_center(indicator), indicator.width() * 0.5),
            mix_color(
                mix_color(
                    palette.control_active,
                    palette.surface_focus,
                    hover_progress,
                ),
                mix_color(
                    mix_color(palette.accent, palette.accent_hover, hover_progress),
                    palette.accent_pressed,
                    press_progress,
                ),
                toggle_progress,
            ),
        );
        ctx.stroke(
            Path::circle(rect_center(indicator), (indicator.width() * 0.5) - 0.5),
            mix_color(framed_border, palette.accent_border_focus, toggle_progress),
            StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
        );
        if toggle_progress > 0.0 {
            ctx.fill(
                Path::circle(
                    rect_center(indicator),
                    indicator.width() * 0.22 * toggle_progress,
                ),
                palette.accent_text.with_alpha(toggle_progress),
            );
        }
        paint_aligned_text(
            ctx,
            label_rect,
            &self.label,
            &text_style,
            text_style.line_height,
            0.0,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::RadioButton, ctx.bounds());
        node.name = Some(self.label.clone());
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.state.selected = self.selected;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct RadioGroup {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    options: Vec<String>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hover_visual: Option<usize>,
    press_visual: Option<usize>,
    selected_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    selection_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    label_measurements: Vec<TextMeasurement>,
    spacing: f32,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl RadioGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            options: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            hover_visual: None,
            press_visual: None,
            selected_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            selection_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            spacing: 6.0,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn option(mut self, option: impl Into<String>) -> Self {
        self.options.push(option.into());
        self
    }

    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.options.extend(options.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self.selected_visual = Some(selected);
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn row_height(&self) -> f32 {
        default_form_control_height(&self.resolved_theme())
    }

    fn row_rect(&self, bounds: Rect, index: usize) -> Rect {
        let y = bounds.y() + (index as f32 * (self.row_height() + self.spacing));
        Rect::new(bounds.x(), y, bounds.width(), self.row_height())
    }

    fn option_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.options.iter().enumerate().find_map(|(index, _)| {
            self.row_rect(bounds, index)
                .contains(position)
                .then_some(index)
        })
    }

    fn current_selected_index(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.options.len())
    }

    fn select(&mut self, index: usize, ctx: &mut EventCtx) {
        if self.options.is_empty() {
            return;
        }

        let selected = index.min(self.options.len().saturating_sub(1));
        let changed = self.current_selected_index() != Some(selected);
        self.selected = Some(selected);
        if changed || self.selected_visual != Some(selected) {
            let theme = self.resolved_theme();
            self.selected_visual = Some(selected);
            self.selection_animation = AnimatedScalar::new(0.0);
            set_toggle_animation_target(&mut self.selection_animation, 1.0, &theme, ctx);
        }
        if let Some(on_change) = &mut self.on_change {
            on_change(selected, self.options[selected].clone());
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered && self.hover_visual == hovered {
            return;
        }

        let theme = self.resolved_theme();
        self.hovered = hovered;
        match hovered {
            Some(index) => {
                if self.hover_visual != Some(index) {
                    self.hover_visual = Some(index);
                    self.hover_animation = AnimatedScalar::new(0.0);
                }
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
            }
            None => {
                set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
            }
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed && self.press_visual == pressed {
            return;
        }

        let theme = self.resolved_theme();
        self.pressed = pressed;
        match pressed {
            Some(index) => {
                if self.press_visual != Some(index) {
                    self.press_visual = Some(index);
                    self.press_animation = AnimatedScalar::new(0.0);
                }
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
            }
            None => {
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
            }
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_progress_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_progress_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn selection_progress_for(&self, index: usize) -> f32 {
        let selected = self.current_selected_index();
        if selected == Some(index) && self.selected_visual == Some(index) {
            self.selection_animation.value
        } else if selected == Some(index) {
            1.0
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> (bool, bool) {
        let previous_hover = self.hover_animation.value;
        let previous_press = self.press_animation.value;
        let previous_selection = self.selection_animation.value;
        let previous_focus = self.focus_animation.value;
        let active = self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.selection_animation.advance(time)
            | self.focus_animation.advance(time);
        let changed = self.hover_animation.changed_since(previous_hover)
            || self.press_animation.changed_since(previous_press)
            || self.selection_animation.changed_since(previous_selection)
            || self.focus_animation.changed_since(previous_focus);

        if self.hovered.is_none() && !self.hover_animation.is_presented() {
            self.hover_visual = None;
        }
        if self.pressed.is_none() && !self.press_animation.is_presented() {
            self.press_visual = None;
        }

        (changed, active)
    }
}

impl Widget for RadioGroup {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.option_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(self.option_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.option_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.option_at(ctx.bounds(), pointer.position);
                let activate = self
                    .pressed
                    .zip(hovered)
                    .filter(|(pressed, hovered)| pressed == hovered);
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if let Some((index, _)) = activate {
                    self.select(index, ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let SemanticsActionRequest::SetValue(SemanticsValue::Text(value)) =
                    &semantics.action
                else {
                    return;
                };
                let Some(index) = self.options.iter().position(|option| option == value) else {
                    return;
                };
                self.set_hovered(Some(index), ctx);
                self.select(index, ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.options.is_empty() {
                    return;
                }

                let current = self
                    .current_selected_index()
                    .unwrap_or(0)
                    .min(self.options.len() - 1);
                let next = match key.key.as_str() {
                    "ArrowUp" | "ArrowLeft" => Some(current.saturating_sub(1)),
                    "ArrowDown" | "ArrowRight" => Some((current + 1).min(self.options.len() - 1)),
                    "Home" => Some(0),
                    "End" => Some(self.options.len() - 1),
                    "Enter" | " " => Some(current),
                    _ => None,
                };

                if let Some(next) = next {
                    self.set_hovered(Some(next), ctx);
                    self.select(next, ctx);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let (changed, active) = self.advance_animations(*time);
                if changed {
                    ctx.request_paint();
                }
                if active {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let text_style = theme.body_text_style();
        let padding = theme.metrics.checkbox_padding;
        let indicator = theme.metrics.checkbox_indicator_size;
        let gap = theme.metrics.checkbox_gap;
        let mut width: f32 = 0.0;
        self.label_measurements.clear();

        for option in &self.options {
            let measurement = measure_text(ctx, option, &text_style);
            self.label_measurements.push(measurement);
            width = width.max(padding.left + indicator + gap + measurement.width + padding.right);
        }

        let count = self.options.len() as f32;
        let height = if self.options.is_empty() {
            self.row_height()
        } else {
            (count * self.row_height()) + ((count - 1.0) * self.spacing.max(0.0))
        };

        constraints.clamp(Size::new(width.max(theme.metrics.button_min_width), height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let focus_progress = self.focus_animation.value;
        let row_padding = Insets {
            top: 0.0,
            bottom: 0.0,
            ..metrics.checkbox_padding
        };

        if focus_progress > AnimatedScalar::EPSILON {
            let outset = physical_pixels(ctx, metrics.focus_ring_outset);
            ctx.stroke(
                rounded_rect_path(
                    ctx.bounds().inflate(outset, outset),
                    metrics.corner_radius + outset,
                ),
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
                StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
            );
        }

        for (index, option) in self.options.iter().enumerate() {
            let row = self.row_rect(ctx.bounds(), index);
            let indicator = indicator_rect(row, row_padding, metrics.checkbox_indicator_size);
            let label_rect = checkbox_label_rect(
                row,
                row_padding,
                metrics.checkbox_indicator_size,
                metrics.checkbox_gap,
            );
            let hover_progress = self.hover_progress_for(index);
            let press_progress = self.press_progress_for(index);
            let selection_progress = self.selection_progress_for(index);
            let hover_amount = hover_progress * interaction.hover_blend;
            let press_amount = press_progress * interaction.pressed_blend;
            let background = mix_color(
                mix_color(palette.control, palette.control_hover, hover_amount),
                palette.control_active,
                press_amount,
            );
            let border = mix_color(
                mix_color(palette.border, palette.border_hover, hover_progress),
                palette.accent_border,
                selection_progress,
            );
            let indicator_fill = mix_color(
                mix_color(palette.control_active, palette.surface_focus, hover_amount),
                mix_color(
                    mix_color(palette.accent, palette.accent_hover, hover_amount),
                    palette.accent_pressed,
                    press_amount,
                ),
                selection_progress,
            );

            draw_control_shape(
                ctx,
                row,
                metrics.corner_radius,
                physical_pixels(ctx, metrics.border_width),
                background,
                border,
            );
            ctx.fill(
                Path::circle(rect_center(indicator), indicator.width() * 0.5),
                indicator_fill,
            );
            ctx.stroke(
                Path::circle(rect_center(indicator), (indicator.width() * 0.5) - 0.5),
                border,
                StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
            );
            if selection_progress > AnimatedScalar::EPSILON {
                ctx.fill(
                    Path::circle(
                        rect_center(indicator),
                        indicator.width() * 0.22 * selection_progress,
                    ),
                    palette.accent_text.with_alpha(selection_progress),
                );
            }
            let text_style = theme.body_text_style();
            paint_aligned_text(
                ctx,
                label_rect,
                option,
                &text_style,
                text_style.line_height,
                0.0,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::RadioGroup, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .current_selected_index()
            .and_then(|index| self.options.get(index).cloned())
            .map(SemanticsValue::Text);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Slider {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    value_reader: Option<Box<dyn Fn() -> f64>>,
    hovered: bool,
    dragging: bool,
    hover_animation: AnimatedScalar,
    drag_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    on_change: Option<Box<dyn FnMut(f64)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, f64)>>,
}

impl Slider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            min: 0.0,
            max: 1.0,
            step: 0.01,
            value: 0.0,
            value_reader: None,
            hovered: false,
            dragging: false,
            hover_animation: AnimatedScalar::new(0.0),
            drag_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            on_change: None,
            on_change_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self.value = clamp_and_snap_value(self.value, self.min, self.max, self.step);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs();
        self.value = clamp_and_snap_value(self.value, self.min, self.max, self.step);
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = clamp_and_snap_value(value, self.min, self.max, self.step);
        self.value_reader = None;
        self
    }

    pub fn value_when<F>(mut self, value: F) -> Self
    where
        F: Fn() -> f64 + 'static,
    {
        self.value_reader = Some(Box::new(value));
        self
    }

    pub const fn current_value(&self) -> f64 {
        self.value
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(f64) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, f64) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    fn fraction(&self) -> f32 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            return 0.0;
        }

        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
    }

    fn sync_external_value(&mut self) {
        if self.dragging {
            return;
        }
        let Some(reader) = &self.value_reader else {
            return;
        };
        self.value = clamp_and_snap_value(reader(), self.min, self.max, self.step);
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn track_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.slider_padding;
        let height = theme.metrics.slider_track_height.max(1.0);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + ((bounds.height() - height) * 0.5),
            (bounds.width() - padding.left - padding.right).max(0.0),
            height,
        )
    }

    fn thumb_rect(&self, bounds: Rect) -> Rect {
        let track = self.track_rect(bounds);
        let theme = self.resolved_theme();
        let thumb = theme.metrics.slider_thumb_size;
        Rect::new(
            track.x() + (track.width() * self.fraction()) - (thumb * 0.5),
            bounds.y() + ((bounds.height() - thumb) * 0.5),
            thumb,
            thumb,
        )
    }

    fn emit_change(&mut self, ctx: &mut EventCtx) {
        if let Some(on_change) = &mut self.on_change {
            on_change(self.value);
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, self.value);
        }
    }

    fn set_from_position(&mut self, ctx: &mut EventCtx, bounds: Rect, position: Point) {
        let track = self.track_rect(bounds);
        if track.width() <= 0.0 {
            return;
        }

        let fraction = ((position.x - track.x()) / track.width()).clamp(0.0, 1.0);
        let raw = self.min + ((self.max - self.min) * f64::from(fraction));
        let next = clamp_and_snap_value(raw, self.min, self.max, self.step);
        if (next - self.value).abs() > f64::EPSILON {
            self.value = next;
            self.emit_change(ctx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.drag_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for Slider {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_external_value();

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                self.set_hovered(hovered, ctx);
                if self.dragging {
                    let previous = self.value;
                    self.set_from_position(ctx, ctx.bounds(), pointer.position);
                    if (self.value - previous).abs() > f64::EPSILON {
                        ctx.request_paint();
                        ctx.request_semantics();
                    }
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.dragging = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.drag_animation, 1.0, &theme, ctx);
                self.set_from_position(ctx, ctx.bounds(), pointer.position);
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
                let theme = self.resolved_theme();
                self.dragging = false;
                self.hovered = ctx.bounds().contains(pointer.position);
                set_hover_animation_target(
                    &mut self.hover_animation,
                    self.hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.drag_animation, 0.0, &theme, ctx);
                self.set_from_position(ctx, ctx.bounds(), pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging {
                    let theme = self.resolved_theme();
                    self.dragging = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.drag_animation, 0.0, &theme, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let next = match &semantics.action {
                    SemanticsActionRequest::Increment => Some(self.value + self.step.max(0.01)),
                    SemanticsActionRequest::Decrement => Some(self.value - self.step.max(0.01)),
                    SemanticsActionRequest::SetValue(SemanticsValue::Number(value)) => Some(*value),
                    SemanticsActionRequest::SetValue(SemanticsValue::Range { value, .. }) => {
                        Some(*value)
                    }
                    _ => None,
                };
                let Some(next) = next.filter(|value| value.is_finite()) else {
                    return;
                };
                let clamped = clamp_and_snap_value(next, self.min, self.max, self.step);
                if (clamped - self.value).abs() > f64::EPSILON {
                    self.value = clamped;
                    self.emit_change(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let next = match key.key.as_str() {
                    "ArrowLeft" | "ArrowDown" => Some(self.value - self.step.max(0.01)),
                    "ArrowRight" | "ArrowUp" => Some(self.value + self.step.max(0.01)),
                    "Home" => Some(self.min),
                    "End" => Some(self.max),
                    _ => None,
                };

                if let Some(next) = next {
                    let clamped = clamp_and_snap_value(next, self.min, self.max, self.step);
                    if (clamped - self.value).abs() > f64::EPSILON {
                        self.value = clamped;
                        self.emit_change(ctx);
                    }
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_external_value();
        let theme = self.resolved_theme();

        constraints.clamp(Size::new(
            theme.metrics.slider_min_width,
            default_form_control_height(&theme),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let hover_progress = self.hover_animation.value;
        let drag_progress = self.drag_animation.value;
        let focus_progress = self.focus_animation.value;
        let track = self.track_rect(ctx.bounds());
        let active = Rect::new(
            track.x(),
            track.y(),
            track.width() * self.fraction(),
            track.height(),
        );
        let thumb = self.thumb_rect(ctx.bounds());

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            mix_color(
                mix_color(
                    palette.control,
                    palette.control_hover,
                    hover_progress.max(drag_progress),
                ),
                palette.surface_focus,
                focus_progress,
            ),
            mix_color(
                mix_color(palette.border, palette.border_hover, hover_progress),
                palette.border_focus,
                focus_progress,
            ),
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );
        ctx.fill(
            rounded_rect_path(track, track.height() * 0.5),
            palette.control_active,
        );
        ctx.fill(
            rounded_rect_path(active, track.height() * 0.5),
            palette.accent,
        );
        ctx.fill(
            Path::circle(rect_center(thumb), thumb.width() * 0.5),
            mix_color(
                mix_color(palette.accent, palette.accent_hover, hover_progress),
                palette.accent_pressed,
                drag_progress,
            ),
        );
        ctx.stroke(
            Path::circle(rect_center(thumb), (thumb.width() * 0.5) - 0.5),
            palette.accent_border,
            StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Slider, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Range {
            value: self.value,
            min: self.min,
            max: self.max,
        });
        node.numeric_step = Some(self.step.max(0.01));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
            SemanticsAction::SetValue,
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberInputStepperPart {
    Increment,
    Decrement,
}

pub struct NumberInput {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    precision: usize,
    buffer: String,
    hovered: bool,
    hovered_stepper: Option<NumberInputStepperPart>,
    pressed_stepper: Option<NumberInputStepperPart>,
    hover_animation: AnimatedScalar,
    stepper_hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    editing: bool,
    value_reader: Option<Box<dyn Fn() -> f64>>,
    on_change: Option<Box<dyn FnMut(f64)>>,
}

impl NumberInput {
    pub fn new(name: impl Into<String>) -> Self {
        let value = 0.0;
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            value,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            precision: 2,
            buffer: format_number(value, 2),
            hovered: false,
            hovered_stepper: None,
            pressed_stepper: None,
            hover_animation: AnimatedScalar::new(0.0),
            stepper_hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            editing: false,
            value_reader: None,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self.value = clamp_and_snap_value(self.value, self.min, self.max, self.step);
        self.buffer = format_number(self.value, self.precision);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs().max(f64::EPSILON);
        self.value = clamp_and_snap_value(self.value, self.min, self.max, self.step);
        self.buffer = format_number(self.value, self.precision);
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self.buffer = format_number(self.value, self.precision);
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = clamp_and_snap_value(value, self.min, self.max, self.step);
        self.buffer = format_number(self.value, self.precision);
        self.value_reader = None;
        self
    }

    pub fn value_when<F>(mut self, value: F) -> Self
    where
        F: Fn() -> f64 + 'static,
    {
        self.value_reader = Some(Box::new(value));
        self
    }

    pub const fn current_value(&self) -> f64 {
        self.value
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(f64) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn text_style(&self) -> TextStyle {
        numeric_text_style(self.resolved_theme().body_text_style())
    }

    fn sync_external_value(&mut self) {
        if self.editing {
            return;
        }
        let Some(reader) = &self.value_reader else {
            return;
        };
        let next = clamp_and_snap_value(reader(), self.min, self.max, self.step);
        if (next - self.value).abs() > f64::EPSILON {
            self.value = next;
            self.buffer = format_number(self.value, self.precision);
        }
    }

    fn resolved_value(&self) -> f64 {
        if !self.editing
            && let Some(reader) = &self.value_reader
        {
            return clamp_and_snap_value(reader(), self.min, self.max, self.step);
        }

        self.value
    }

    fn display_buffer(&self) -> String {
        if !self.editing && self.value_reader.is_some() {
            format_number(self.resolved_value(), self.precision)
        } else {
            self.buffer.clone()
        }
    }

    fn commit_buffer(&mut self) {
        if let Ok(parsed) = self.buffer.trim().parse::<f64>() {
            let next = clamp_and_snap_value(parsed, self.min, self.max, self.step);
            if (next - self.value).abs() > f64::EPSILON {
                self.value = next;
                if let Some(on_change) = &mut self.on_change {
                    on_change(self.value);
                }
            }
            self.buffer = format_number(self.value, self.precision);
        }
    }

    fn apply_edit_buffer(&mut self) {
        let Ok(parsed) = self.buffer.trim().parse::<f64>() else {
            return;
        };
        if !parsed.is_finite() || parsed < self.min || parsed > self.max {
            return;
        }
        if (parsed - self.value).abs() > f64::EPSILON {
            self.value = parsed;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.value);
            }
        }
    }

    fn nudge(&mut self, delta: f64) {
        let next = clamp_and_snap_value(self.value + delta, self.min, self.max, self.step);
        if (next - self.value).abs() > f64::EPSILON {
            self.value = next;
            self.buffer = format_number(self.value, self.precision);
            if let Some(on_change) = &mut self.on_change {
                on_change(self.value);
            }
        }
    }

    fn set_hover_state(
        &mut self,
        hovered: bool,
        hovered_stepper: Option<NumberInputStepperPart>,
        ctx: &mut EventCtx,
    ) {
        if self.hovered != hovered || self.hovered_stepper != hovered_stepper {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            self.hovered_stepper = hovered_stepper;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            set_hover_animation_target(
                &mut self.stepper_hover_animation,
                hovered_stepper.is_some() as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn set_pressed_stepper(
        &mut self,
        pressed_stepper: Option<NumberInputStepperPart>,
        ctx: &mut EventCtx,
    ) {
        if self.pressed_stepper != pressed_stepper {
            let theme = self.resolved_theme();
            self.pressed_stepper = pressed_stepper;
            set_press_animation_target(
                &mut self.press_animation,
                pressed_stepper.is_some() as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.stepper_hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for NumberInput {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_external_value();
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let theme = self.resolved_theme();
                self.set_hover_state(
                    ctx.bounds().contains(pointer.position),
                    number_input_stepper_part(ctx.bounds(), theme.metrics, pointer.position),
                    ctx,
                );
            }
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Enter) => {
                let theme = self.resolved_theme();
                self.set_hover_state(
                    ctx.bounds().contains(pointer.position),
                    number_input_stepper_part(ctx.bounds(), theme.metrics, pointer.position),
                    ctx,
                );
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hover_state(false, None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                let stepper_part =
                    number_input_stepper_part(ctx.bounds(), theme.metrics, pointer.position);
                self.set_hover_state(ctx.bounds().contains(pointer.position), stepper_part, ctx);
                self.set_pressed_stepper(stepper_part, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                match stepper_part {
                    Some(NumberInputStepperPart::Increment) => self.nudge(self.step),
                    Some(NumberInputStepperPart::Decrement) => self.nudge(-self.step),
                    None => {}
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.set_hover_state(
                    ctx.bounds().contains(pointer.position),
                    number_input_stepper_part(ctx.bounds(), theme.metrics, pointer.position),
                    ctx,
                );
                self.set_pressed_stepper(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_stepper.is_some() {
                    self.set_pressed_stepper(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let next = match &semantics.action {
                    SemanticsActionRequest::Increment => Some(self.value + self.step),
                    SemanticsActionRequest::Decrement => Some(self.value - self.step),
                    SemanticsActionRequest::SetValue(SemanticsValue::Number(value)) => Some(*value),
                    SemanticsActionRequest::SetValue(SemanticsValue::Range { value, .. }) => {
                        Some(*value)
                    }
                    _ => None,
                };
                let Some(next) = next.filter(|value| value.is_finite()) else {
                    return;
                };
                let next = clamp_and_snap_value(next, self.min, self.max, self.step);
                if (next - self.value).abs() > f64::EPSILON {
                    self.value = next;
                    self.buffer = format_number(self.value, self.precision);
                    if let Some(on_change) = &mut self.on_change {
                        on_change(self.value);
                    }
                }
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowUp" => self.nudge(self.step),
                    "ArrowDown" => self.nudge(-self.step),
                    "Enter" => self.commit_buffer(),
                    "Escape" => self.buffer = format_number(self.value, self.precision),
                    "Backspace" => {
                        self.buffer.pop();
                        self.apply_edit_buffer();
                    }
                    _ => {
                        if let Some(text) = keyboard_text(key)
                            && text.chars().all(is_numeric_input_char)
                        {
                            self.buffer.push_str(text);
                            self.apply_edit_buffer();
                        }
                    }
                }
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let previous_hover = self.hover_animation.value;
                let previous_stepper_hover = self.stepper_hover_animation.value;
                let previous_press = self.press_animation.value;
                let previous_focus = self.focus_animation.value;
                let animating = self.advance_animations(*time);
                let changed = self.hover_animation.changed_since(previous_hover)
                    || self
                        .stepper_hover_animation
                        .changed_since(previous_stepper_hover)
                    || self.press_animation.changed_since(previous_press)
                    || self.focus_animation.changed_since(previous_focus);
                if changed {
                    ctx.request_paint();
                }
                if animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_external_value();
        let buffer = self.display_buffer();
        let text_style = self.text_style();
        let measurement = measure_text(ctx, &buffer, &text_style);
        let theme = self.resolved_theme();
        let padding = theme.metrics.text_input_padding;
        let height =
            (measurement.height.max(text_style.line_height) + padding.top + padding.bottom)
                .max(theme.metrics.min_height);
        constraints.clamp(Size::new(
            (measurement.width
                + padding.left
                + padding.right
                + theme.metrics.number_input_stepper_width)
                .max(theme.metrics.button_min_width + 60.0),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let content = number_input_text_rect(ctx.bounds(), metrics);
        let stepper = number_input_stepper_rect(ctx.bounds(), metrics);
        let text_style = self.text_style();
        let buffer = self.display_buffer();
        let hover_progress = self.hover_animation.value * interaction.hover_blend;
        let stepper_hover_progress = self.stepper_hover_animation.value * interaction.hover_blend;
        let press_progress = self.press_animation.value * interaction.pressed_blend;
        let focus_progress = self.focus_animation.value;
        let base_background = mix_color(palette.control, palette.control_hover, hover_progress);

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            mix_color(base_background, palette.surface_focus, focus_progress),
            mix_color(
                mix_color(
                    palette.border,
                    palette.border_hover,
                    self.hover_animation.value,
                ),
                palette.border_focus,
                focus_progress,
            ),
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );

        paint_aligned_text(
            ctx,
            content,
            &buffer,
            &text_style,
            text_style.line_height,
            1.0,
        );
        ctx.stroke(
            line_path(
                Point::new(stepper.x(), ctx.bounds().y() + 6.0),
                Point::new(stepper.x(), ctx.bounds().max_y() - 6.0),
            ),
            palette.border,
            StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
        );
        for (part, rect) in [
            (
                NumberInputStepperPart::Increment,
                Rect::new(
                    stepper.x(),
                    stepper.y(),
                    stepper.width(),
                    stepper.height() * 0.5,
                ),
            ),
            (
                NumberInputStepperPart::Decrement,
                Rect::new(
                    stepper.x(),
                    stepper.y() + (stepper.height() * 0.5),
                    stepper.width(),
                    stepper.height() * 0.5,
                ),
            ),
        ] {
            let hover_amount = if self.hovered_stepper == Some(part) {
                stepper_hover_progress
            } else {
                0.0
            };
            let press_amount = if self.pressed_stepper == Some(part) {
                press_progress
            } else {
                0.0
            };
            if hover_amount > 0.0 || press_amount > 0.0 {
                let fill = mix_color(
                    mix_color(palette.control, palette.control_hover, hover_amount),
                    palette.control_active,
                    press_amount,
                );
                ctx.fill(
                    rounded_rect_path(rect.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    fill,
                );
            }
        }
        let increment_offset = if self.pressed_stepper == Some(NumberInputStepperPart::Increment) {
            Vector::new(0.0, self.press_animation.value * interaction.pressed_offset)
        } else {
            Vector::ZERO
        };
        let decrement_offset = if self.pressed_stepper == Some(NumberInputStepperPart::Decrement) {
            Vector::new(0.0, self.press_animation.value * interaction.pressed_offset)
        } else {
            Vector::ZERO
        };
        draw_icon_glyph(
            ctx,
            IconGlyph::ChevronUp,
            Rect::new(
                stepper.x(),
                stepper.y(),
                stepper.width(),
                stepper.height() * 0.5,
            )
            .translate(increment_offset),
            palette.text,
        );
        draw_icon_glyph(
            ctx,
            IconGlyph::ChevronDown,
            Rect::new(
                stepper.x(),
                stepper.y() + (stepper.height() * 0.5),
                stepper.width(),
                stepper.height() * 0.5,
            )
            .translate(decrement_offset),
            palette.text,
        );

        if ctx.is_focused() {
            let caret_x = content.max_x();
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let caret = Rect::new(
                caret_x.min((content.max_x() - caret_width).max(content.x())),
                content.y(),
                caret_width,
                content.height(),
            );
            ctx.set_ime_composition_rect(caret);
            ctx.fill(rounded_rect_path(caret, caret_width * 0.5), palette.caret);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::SpinBox, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Range {
            value: self.resolved_value(),
            min: self.min,
            max: self.max,
        });
        node.numeric_step = Some(self.step);
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
            SemanticsAction::SetValue,
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if focused {
            self.sync_external_value();
        }
        self.editing = focused;
        if !focused {
            self.commit_buffer();
            self.sync_external_value();
        }
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct TextArea {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    editor: EditorState,
    placeholder: String,
    read_only: bool,
    appearance: FieldAppearance,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    focused: bool,
    dragging_selection: bool,
    hover_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    caret_blink: Blink,
    caret_timer: Option<TimerToken>,
    caret_visible: bool,
    display_layout: Option<PersistentTextLayout>,
    input_layout: Option<PersistentTextLayout>,
    selection_scope: Option<SelectionScope>,
    on_change: Option<Box<dyn FnMut(String)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, String)>>,
    on_submit: Option<Box<dyn FnMut(&str)>>,
    on_focus_change: Option<Box<dyn FnMut(bool)>>,
}

impl TextArea {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            editor: EditorState::new(),
            placeholder: String::new(),
            read_only: false,
            appearance: FieldAppearance::Framed,
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            focused: false,
            dragging_selection: false,
            hover_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            caret_blink: Blink::new(CARET_BLINK_PERIOD_SECONDS),
            caret_timer: None,
            caret_visible: true,
            display_layout: None,
            input_layout: None,
            selection_scope: None,
            on_change: None,
            on_change_with_ctx: None,
            on_submit: None,
            on_focus_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn appearance(mut self, appearance: FieldAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn bare(mut self) -> Self {
        self.appearance = FieldAppearance::Bare;
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width.max(0.0));
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height.max(0.0));
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn selection_scope(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.editor.set_text(value);
        self
    }

    pub fn current_value(&self) -> &str {
        self.editor.document().text()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.editor.set_text(value);
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, String) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    /// Fire `on_submit(current_text)` when the user presses a plain `Enter` (no Shift/Ctrl/Meta
    /// modifier) while focused, *instead* of inserting a newline. `Shift+Enter` (and any modified
    /// Enter) still inserts a newline. When no `on_submit` is set, `Enter` inserts a newline as
    /// before, so this is fully backward-compatible.
    ///
    /// This turns the multi-line `TextArea` into a chat-style composer: Enter to send, Shift+Enter
    /// for a soft line break.
    pub fn on_submit<F>(mut self, on_submit: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_submit = Some(Box::new(on_submit));
        self
    }

    pub fn on_focus_change<F>(mut self, on_focus_change: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_focus_change = Some(Box::new(on_focus_change));
        self
    }

    fn input_text(&self) -> String {
        self.editor.display_text()
    }

    fn display_text(&self) -> String {
        let input = self.input_text();
        if input.is_empty() {
            self.placeholder.clone()
        } else {
            input
        }
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style())
    }

    fn display_text_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        if self.input_text().is_empty() {
            theme.placeholder_text_style()
        } else if self.read_only {
            theme.text_style(theme.palette.text_muted)
        } else {
            self.resolved_text_style()
        }
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.text_input_padding)
    }

    fn resolved_min_size(&self) -> Size {
        let theme = self.resolved_theme();
        Size::new(
            self.min_width.unwrap_or(theme.metrics.text_input_min_width),
            self.min_height
                .unwrap_or(theme.metrics.text_area_min_height),
        )
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn commit_text_change(&mut self, ctx: &mut EventCtx) {
        let value = self.current_value().to_string();
        if let Some(on_change) = &mut self.on_change {
            on_change(value.clone());
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, value);
        }
    }

    fn apply_editor_result(&mut self, ctx: &mut EventCtx, mut result: EditorCommandResult) {
        let copied_to_clipboard = result.clipboard_text.is_some();
        if let Some(text) = result.clipboard_text.take() {
            ctx.set_clipboard_text(text);
        }
        if result.text_changed {
            self.commit_text_change(ctx);
        }
        if result.layout_changed() {
            ctx.request_measure();
            ctx.request_paint();
        } else if result.overlay_changed() {
            ctx.request_paint();
        }
        if result.text_changed || result.selection_changed || result.composition_changed {
            sync_editor_selection_scope(ctx, self.selection_scope.as_ref(), &self.editor);
            ctx.request_semantics();
        }
        if result.handled {
            if self.focused {
                self.reset_caret_blink(ctx);
            }
            if !(copied_to_clipboard && self.selection_scope.is_some()) {
                ctx.set_handled();
            }
        }
    }

    fn execute_editor_command(&mut self, ctx: &mut EventCtx, command: EditorCommand) {
        let result = self.editor.execute(command);
        self.apply_editor_result(ctx, result);
    }

    /// Select the entire document.
    pub fn select_all(&mut self, ctx: &mut EventCtx) {
        self.execute_editor_command(ctx, EditorCommand::SelectAll);
    }

    /// Copy the current selection to the clipboard. No-op when the selection
    /// is collapsed.
    pub fn copy(&mut self, ctx: &mut EventCtx) {
        self.execute_editor_command(ctx, EditorCommand::Copy);
    }

    /// Copy the current selection to the clipboard and delete it. No-op when
    /// read-only or the selection is collapsed.
    pub fn cut(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            return;
        }
        self.execute_editor_command(ctx, EditorCommand::Cut);
    }

    /// Replace the current selection with the clipboard text. No-op when
    /// read-only or the clipboard has no text.
    pub fn paste(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            return;
        }
        let command = paste_command(ctx);
        self.execute_editor_command(ctx, command);
    }

    /// Currently selected document text (empty when the selection is
    /// collapsed).
    pub fn selected_text(&self) -> &str {
        self.editor.selected_text()
    }

    fn apply_text_command(&mut self, ctx: &mut EventCtx, command: TextCommand) {
        match command {
            TextCommand::SelectAll => self.select_all(ctx),
            TextCommand::Copy => self.copy(ctx),
            TextCommand::Cut => self.cut(ctx),
            TextCommand::Paste => self.paste(ctx),
        }
    }

    fn text_offset_at_position(&self, bounds: Rect, position: Point) -> usize {
        let content = inset_rect(bounds, self.resolved_padding());
        self.input_layout
            .as_ref()
            .map(|layout| {
                layout
                    .hit_test_point(Point::new(
                        position.x - content.x(),
                        position.y - content.y(),
                    ))
                    .utf8_offset
            })
            .unwrap_or(self.editor.document().len())
    }

    fn set_caret_from_position(
        &mut self,
        bounds: Rect,
        position: Point,
        extend: bool,
        ctx: &mut EventCtx,
    ) {
        let offset = self.text_offset_at_position(bounds, position);
        let command = if extend {
            EditorCommand::SetSelection {
                anchor: self.editor.selection().anchor.utf8_offset,
                focus: offset,
            }
        } else {
            EditorCommand::MoveTo {
                offset,
                extend: false,
            }
        };
        let result = self.editor.execute(command);
        self.apply_editor_result(ctx, result);
    }

    fn caret_blink_delay(&self) -> f64 {
        let span = if self.caret_visible {
            self.caret_blink.period * self.caret_blink.duty_cycle as f64
        } else {
            self.caret_blink.period * (1.0 - self.caret_blink.duty_cycle as f64)
        };
        span.max(f64::EPSILON)
    }

    fn arm_caret_blink(&mut self, ctx: &mut EventCtx) {
        if let Some(token) = self.caret_timer.take() {
            ctx.cancel_timer(token);
        }
        if self.focused {
            self.caret_timer = Some(ctx.schedule_timer_after(self.caret_blink_delay()));
        }
    }

    fn reset_caret_blink(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
            return;
        }
        self.caret_visible = self.focused;
        self.arm_caret_blink(ctx);
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for TextArea {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
                if self.dragging_selection
                    && ctx.phase() != EventPhase::Capture
                    && pointer.buttons.contains(PointerButton::Primary)
                {
                    let offset = self.text_offset_at_position(ctx.bounds(), pointer.position);
                    let result = self.editor.execute(EditorCommand::SetSelection {
                        anchor: self.editor.selection().anchor.utf8_offset,
                        focus: offset,
                    });
                    self.apply_editor_result(ctx, result);
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.set_hovered(true, ctx);
                if self.focused {
                    self.reset_caret_blink(ctx);
                }
                self.set_caret_from_position(
                    ctx.bounds(),
                    pointer.position,
                    pointer.modifiers.shift,
                    ctx,
                );
                self.dragging_selection = true;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.dragging_selection =>
            {
                self.dragging_selection = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging_selection {
                    self.dragging_selection = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                // Focus on right-click (keeping the selection intact) so
                // follow-up clipboard commands land here. Deliberately not
                // handled: wrapping context menus react to the same press.
                self.set_hovered(true, ctx);
                if !ctx.is_focused() {
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                if let Some(commands) =
                    semantics_editor_commands(ctx, &semantics.action, self.read_only, false)
                {
                    for command in commands {
                        self.execute_editor_command(ctx, command);
                    }
                }
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(ctx, EditorCommand::StartComposition);
                }
            }
            Event::Ime(ImeEvent::CompositionUpdate { text, cursor_range }) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(
                        ctx,
                        EditorCommand::UpdateComposition {
                            text: text.clone(),
                            cursor_range: cursor_range.clone(),
                        },
                    );
                }
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(
                        ctx,
                        EditorCommand::CommitComposition(text.clone()),
                    );
                }
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(ctx, EditorCommand::EndComposition);
                }
            }
            Event::Keyboard(key)
                if !self.read_only
                    && key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && key.key == "Backspace" =>
            {
                self.execute_editor_command(ctx, EditorCommand::DeleteBackward);
            }
            Event::Keyboard(key)
                if !self.read_only
                    && key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && key.key == "Delete" =>
            {
                self.execute_editor_command(ctx, EditorCommand::DeleteForward);
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Enter" =>
            {
                // Chat-composer behavior (only when an `on_submit` is wired): a plain Enter (no
                // Shift/Ctrl/Meta) submits the current text and is consumed, while Shift+Enter (or
                // any modified Enter) inserts a newline as usual. With no `on_submit`, Enter always
                // inserts a newline (backward-compatible).
                let plain_enter =
                    !key.modifiers.shift && !key.modifiers.control && !key.modifiers.meta;
                if self.on_submit.is_some() && plain_enter {
                    let text = self.current_value().to_string();
                    if let Some(on_submit) = &mut self.on_submit {
                        on_submit(&text);
                    }
                    ctx.set_handled();
                } else if !self.read_only {
                    self.execute_editor_command(ctx, EditorCommand::InsertText("\n".to_string()));
                }
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                let command_modifier = key.modifiers.control || key.modifiers.meta;
                let command = match key.key.as_str() {
                    "a" | "A" if command_modifier => EditorCommand::SelectAll,
                    "c" | "C" if command_modifier => EditorCommand::Copy,
                    "x" | "X" if command_modifier && !self.read_only => EditorCommand::Cut,
                    "v" | "V" if command_modifier && !self.read_only => paste_command(ctx),
                    "z" | "Z" if command_modifier && key.modifiers.shift && !self.read_only => {
                        EditorCommand::Redo
                    }
                    "z" | "Z" if command_modifier && !self.read_only => EditorCommand::Undo,
                    "y" | "Y" if command_modifier && !self.read_only => EditorCommand::Redo,
                    "ArrowLeft" if command_modifier => EditorCommand::MoveWordLeft {
                        extend: key.modifiers.shift,
                    },
                    "ArrowRight" if command_modifier => EditorCommand::MoveWordRight {
                        extend: key.modifiers.shift,
                    },
                    "ArrowLeft" => EditorCommand::MoveLeft {
                        extend: key.modifiers.shift,
                    },
                    "ArrowRight" => EditorCommand::MoveRight {
                        extend: key.modifiers.shift,
                    },
                    "ArrowUp" => EditorCommand::MoveUp {
                        extend: key.modifiers.shift,
                    },
                    "ArrowDown" => EditorCommand::MoveDown {
                        extend: key.modifiers.shift,
                    },
                    "Home" => EditorCommand::MoveLineStart {
                        extend: key.modifiers.shift,
                    },
                    "End" => EditorCommand::MoveLineEnd {
                        extend: key.modifiers.shift,
                    },
                    "PageUp" => EditorCommand::PageUp {
                        extend: key.modifiers.shift,
                        lines: 8,
                    },
                    "PageDown" => EditorCommand::PageDown {
                        extend: key.modifiers.shift,
                        lines: 8,
                    },
                    _ if !self.read_only && self.editor.composition().is_none() => {
                        keyboard_text(key)
                            .map(|text| EditorCommand::InsertText(text.to_string()))
                            .unwrap_or(EditorCommand::Noop)
                    }
                    _ => EditorCommand::Noop,
                };
                if !matches!(command, EditorCommand::Noop) {
                    self.execute_editor_command(ctx, command);
                }
            }
            Event::Wake(sui_core::WakeEvent::Timer { token, .. })
                if self.caret_timer == Some(*token) =>
            {
                self.caret_timer = None;
                if self.focused {
                    self.caret_visible = !self.caret_visible;
                    self.arm_caret_blink(ctx);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                let previous_hover = self.hover_animation.value;
                let previous_focus = self.focus_animation.value;
                let animating =
                    self.hover_animation.advance(*time) | self.focus_animation.advance(*time);
                let changed = self.hover_animation.changed_since(previous_hover)
                    || self.focus_animation.changed_since(previous_focus);
                if animating {
                    ctx.request_animation_frame();
                }
                if changed {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(command) = TextCommand::from_command(command) else {
            return;
        };
        if !ctx.is_focused() {
            ctx.request_focus();
        }
        self.apply_text_command(ctx, command);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let min_size = self.resolved_min_size();
        let content_width = if constraints.max.width.is_finite() {
            (constraints.max.width - padding.left - padding.right).max(0.0)
        } else {
            (min_size.width - padding.left - padding.right).max(0.0)
        };
        let display_text = self.display_text();
        let input_text = self.input_text();
        let display_style = self.display_text_style();
        let display_min_height = display_style.line_height.max(1.0);
        let input_min_height = text_style.line_height.max(1.0);
        let display_box = Size::new(content_width.max(1.0), display_min_height);
        let input_box = Size::new(content_width.max(1.0), input_min_height);

        let mut display_layout = ctx
            .layout()
            .shape_text_persistent(
                self.display_layout.as_ref().map(|layout| layout.handle()),
                display_text.clone(),
                display_box,
                display_style.clone(),
            )
            .ok();
        if let Some(required_height) = display_layout
            .as_ref()
            .map(|layout| layout.measurement().height.max(display_min_height).max(1.0))
            .filter(|height| *height > display_box.height + 0.01)
            && let Ok(layout) = ctx.layout().shape_text_persistent(
                self.display_layout.as_ref().map(|layout| layout.handle()),
                display_text,
                Size::new(content_width.max(1.0), required_height),
                display_style.clone(),
            )
        {
            display_layout = Some(layout);
        }

        let mut input_layout = ctx
            .layout()
            .shape_text_persistent(
                self.input_layout.as_ref().map(|layout| layout.handle()),
                input_text.clone(),
                input_box,
                text_style.clone(),
            )
            .ok();
        if let Some(required_height) = input_layout
            .as_ref()
            .map(|layout| layout.measurement().height.max(input_min_height).max(1.0))
            .filter(|height| *height > input_box.height + 0.01)
            && let Ok(layout) = ctx.layout().shape_text_persistent(
                self.input_layout.as_ref().map(|layout| layout.handle()),
                input_text,
                Size::new(content_width.max(1.0), required_height),
                text_style.clone(),
            )
        {
            input_layout = Some(layout);
        }

        let measured_height = display_layout
            .as_ref()
            .map(|layout| layout.measurement().height.max(display_style.line_height))
            .unwrap_or(display_style.line_height);
        self.display_layout = display_layout;
        self.input_layout = input_layout;

        constraints.clamp(Size::new(
            min_size
                .width
                .max(content_width + padding.left + padding.right),
            min_size
                .height
                .max(measured_height + padding.top + padding.bottom),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let padding = self.resolved_padding();
        let content = inset_rect(ctx.bounds(), padding);
        let focus_progress = self.focus_animation.value;

        // Light fields lift from their slightly recessed rest fill to the
        // surface on hover. Focus then moves every scheme toward its soft
        // accent well; dark and Void keep their established resting depth.
        let background = field_background(
            &theme,
            self.read_only,
            self.hover_animation.value,
            focus_progress,
        );
        if self.appearance == FieldAppearance::Framed {
            draw_control_frame(
                ctx,
                ctx.bounds(),
                metrics.corner_radius,
                metrics,
                background,
                mix_color(
                    mix_color(
                        palette.border,
                        palette.border_hover,
                        self.hover_animation.value,
                    ),
                    palette.border_focus,
                    focus_progress,
                ),
                (focus_progress > 0.0).then_some(
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                ),
            );
        }

        if let Some(layout) = &self.display_layout {
            ctx.push_clip_rect(content);
            let input_text = self.input_text();
            let selection = selection_range(&self.editor.display_selection(), input_text.len());
            if !selection.is_empty() {
                for rect in layout.selection_rects(selection) {
                    ctx.fill_rect(
                        rect.translate(content.origin.to_vector()),
                        palette.selection,
                    );
                }
            }
            ctx.draw_persistent_text_layout(content.origin, layout);
            ctx.pop_clip();
        }

        if self.focused && !self.read_only {
            let text_style = self.resolved_text_style();
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let fallback_caret = Rect::new(
                content.x(),
                content.y(),
                caret_width,
                text_style.line_height.max(1.0),
            );
            let caret = self
                .input_layout
                .as_ref()
                .and_then(|layout| {
                    let caret = layout
                        .caret_rect(self.editor.display_selection().focus.utf8_offset)
                        .translate(content.origin.to_vector());
                    rect_is_finite(caret).then_some(caret)
                })
                .unwrap_or(fallback_caret);
            let caret = Rect::new(
                caret
                    .x()
                    .min((content.max_x() - caret_width).max(content.x()))
                    .max(content.x()),
                caret.y(),
                caret_width,
                caret.height().max(text_style.line_height).max(1.0),
            );
            ctx.set_ime_composition_rect(caret);
            if self.caret_visible {
                ctx.fill(rounded_rect_path(caret, caret_width * 0.5), palette.caret);
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        let display_text = self.input_text();
        let display_selection = self.editor.display_selection();
        let selection = selection_range(&display_selection, display_text.len());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(display_text));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.editable_text = Some(EditableTextSemantics {
            caret_offset: display_selection.focus.utf8_offset,
            selection: SemanticsTextRange::new(selection.start, selection.end),
            multiline: true,
            password: false,
            readonly: self.read_only,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        node.actions = if self.read_only {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetSelection,
                SemanticsAction::Copy,
            ]
        } else {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetValue,
                SemanticsAction::SetSelection,
                SemanticsAction::InsertText,
                SemanticsAction::DeleteBackward,
                SemanticsAction::DeleteForward,
                SemanticsAction::Copy,
                SemanticsAction::Cut,
                SemanticsAction::Paste,
                SemanticsAction::Undo,
                SemanticsAction::Redo,
            ]
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.focused = focused;
        if !focused {
            let result = self.editor.execute(EditorCommand::ClearComposition);
            if result.layout_changed() {
                ctx.request_measure();
            }
        }
        if focused {
            self.reset_caret_blink(ctx);
        } else {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
        }
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        if let Some(on_focus_change) = &mut self.on_focus_change {
            on_focus_change(focused);
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone)]
struct SelectMenuPresentationState {
    theme: DefaultTheme,
    options: Vec<String>,
    selected: Option<usize>,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    placement: SelectMenuPlacement,
    menu_bounds: Rect,
    reveal: AnimatedScalar,
}

impl SelectMenuPresentationState {
    fn new() -> Self {
        Self {
            theme: DefaultTheme::default(),
            options: Vec::new(),
            selected: None,
            hovered: None,
            hover_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            placement: SelectMenuPlacement::Below,
            menu_bounds: Rect::ZERO,
            reveal: AnimatedScalar::new(0.0),
        }
    }

    fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    fn row_height(&self) -> f32 {
        default_form_control_height(&self.theme)
    }

    fn row_rect(&self, index: usize, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y() + (index as f32 * self.row_height()),
            bounds.width(),
            self.row_height(),
        )
    }

    fn set_hovered(&mut self, hovered: Option<usize>, animate: bool, ctx: &mut EventCtx) -> bool {
        if self.hovered == hovered && self.hover_visual == hovered {
            return false;
        }

        self.hovered = hovered;
        match hovered {
            Some(index) => {
                if self.hover_visual != Some(index) {
                    self.hover_visual = Some(index);
                    self.hover_animation = AnimatedScalar::new(0.0);
                }
                if animate {
                    set_hover_animation_target(&mut self.hover_animation, 1.0, &self.theme, ctx);
                } else {
                    self.hover_animation = AnimatedScalar::new(1.0);
                }
            }
            None => {
                if animate {
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &self.theme, ctx);
                } else {
                    self.hover_animation = AnimatedScalar::new(0.0);
                    self.hover_visual = None;
                }
            }
        }
        true
    }

    fn sync_hovered_without_animation(&mut self, hovered: Option<usize>) -> bool {
        if self.hovered == hovered && self.hover_visual == hovered {
            return false;
        }
        self.hovered = hovered;
        self.hover_visual = hovered;
        self.hover_animation = AnimatedScalar::new(hovered.is_some() as u8 as f32);
        true
    }

    fn advance_hover(&mut self, time: f64) -> (bool, bool) {
        let previous = self.hover_animation.value;
        let active = self.hover_animation.advance(time);
        let changed = self.hover_animation.changed_since(previous);
        if self.hovered.is_none() && !self.hover_animation.is_presented() {
            self.hover_visual = None;
        }
        (changed, active)
    }

    fn hover_progress_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        let direction = match self.placement {
            SelectMenuPlacement::Below => -1.0,
            SelectMenuPlacement::Above => 1.0,
        };
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value) * direction,
            ),
        }
    }
}

struct SelectMenuSurface {
    state: Rc<RefCell<SelectMenuPresentationState>>,
}

impl SelectMenuSurface {
    fn new(state: Rc<RefCell<SelectMenuPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for SelectMenuSurface {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Move | PointerEventKind::Enter
                ) =>
            {
                let mut state = self.state.borrow_mut();
                let bounds = state.menu_bounds;
                let hovered = bounds.contains(pointer.position).then(|| {
                    state.options.iter().enumerate().find_map(|(index, _)| {
                        state
                            .row_rect(index, bounds)
                            .contains(pointer.position)
                            .then_some(index)
                    })
                });
                let changed = state.set_hovered(hovered.flatten(), true, ctx);
                drop(state);
                if changed {
                    ctx.request_paint();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                let mut state = self.state.borrow_mut();
                let changed = state.set_hovered(None, true, ctx);
                drop(state);
                if changed {
                    ctx.request_paint();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let mut state = self.state.borrow_mut();
                let (changed, active) = state.advance_hover(*time);
                drop(state);
                if changed {
                    ctx.request_paint();
                }
                if active {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.menu_bounds.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let menu = ctx.bounds();
        let theme = state.theme;
        let metrics = theme.metrics;
        let palette = theme.palette;
        let menu_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(ctx, menu, [menu_radius; 4], &theme.shadows.box_shadow.md);
        draw_control_shape(
            ctx,
            menu,
            menu_radius,
            physical_pixels(ctx, metrics.border_width),
            palette.surface_raised,
            palette.border,
        );
        ctx.push_clip_rect(menu);
        for (index, option) in state.options.iter().enumerate() {
            let row = state.row_rect(index, menu);
            let selected = state.selected == Some(index);
            let hover_progress = state.hover_progress_for(index);
            let text_style = theme.body_text_style();
            if hover_progress > AnimatedScalar::EPSILON || selected {
                let background = if selected {
                    mix_color(
                        palette.selection,
                        palette.control_hover,
                        hover_progress * theme.interaction.hover_blend,
                    )
                } else {
                    mix_color(
                        palette.surface_raised,
                        palette.control_hover,
                        hover_progress,
                    )
                };
                ctx.fill(
                    rounded_rect_path(row.inflate(-4.0, -4.0), metrics.corner_radius - 2.0),
                    background,
                );
            }
            let text_slot = horizontal_text_inset_rect(row, metrics.text_input_padding);
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot,
                option,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                ..StackSurfaceOptions::default()
            })
    }
}

pub struct Select {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    options: Vec<String>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    placeholder: String,
    expanded: bool,
    hovered_option: Option<usize>,
    hovered_header: bool,
    pressed_header: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    menu_surface: SingleChild,
    menu_state: Rc<RefCell<SelectMenuPresentationState>>,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String)>>,
}

impl Select {
    pub fn new(name: impl Into<String>) -> Self {
        let menu_state = Rc::new(RefCell::new(SelectMenuPresentationState::new()));
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            options: Vec::new(),
            selected: None,
            selected_reader: None,
            placeholder: String::new(),
            expanded: false,
            hovered_option: None,
            hovered_header: false,
            pressed_header: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            menu_surface: SingleChild::new(SelectMenuSurface::new(Rc::clone(&menu_state))),
            menu_state,
            on_change: None,
            on_change_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn option(mut self, option: impl Into<String>) -> Self {
        self.options.push(option.into());
        self
    }

    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.options.extend(options.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self.selected_reader = None;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        if expanded {
            self.hovered_option = self.current_selected_index().or(Some(0));
        } else {
            self.hovered_option = None;
        }
        self.menu_state.borrow_mut().reveal = AnimatedScalar::new(expanded as u8 as f32);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn current_value(&self) -> Option<&str> {
        self.current_selected_index()
            .and_then(|index| self.options.get(index).map(String::as_str))
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, String) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn header_height(&self) -> f32 {
        default_form_control_height(&self.resolved_theme())
    }

    fn current_label(&self) -> String {
        self.current_value()
            .map(str::to_string)
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn current_selected_index(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.options.len())
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let height = self.header_height().min(bounds.height()).max(0.0);
        Rect::new(
            bounds.x(),
            bounds.y() + ((bounds.height() - height) * 0.5).max(0.0),
            bounds.width(),
            height,
        )
    }

    fn menu_height(&self) -> f32 {
        (self.options.len() as f32 * self.header_height())
            .min(self.resolved_theme().metrics.select_menu_max_height)
    }

    fn menu_layout(&self, bounds: Rect, viewport: Size) -> (SelectMenuPlacement, Rect) {
        let theme = self.resolved_theme();
        let viewport = if viewport.width.is_finite()
            && viewport.height.is_finite()
            && viewport.width > 0.0
            && viewport.height > 0.0
        {
            Rect::from_origin_size(Point::ZERO, viewport)
        } else {
            Rect::new(0.0, 0.0, f32::MAX / 4.0, f32::MAX / 4.0)
        };
        let result = place_overlay(
            &OverlayPlacementRequest::new(
                self.header_rect(bounds),
                Size::new(bounds.width(), self.menu_height()),
                viewport,
                OverlayPlacement::BOTTOM_START,
            )
            .fallbacks([OverlayPlacement::TOP_START])
            .gap(theme.metrics.select_menu_gap)
            .margin(theme.metrics.select_menu_edge_padding),
        );
        let placement = if result.placement == OverlayPlacement::TOP_START {
            SelectMenuPlacement::Above
        } else {
            SelectMenuPlacement::Below
        };
        (placement, result.bounds)
    }

    fn menu_placement(&self, bounds: Rect, viewport: Size) -> SelectMenuPlacement {
        self.menu_layout(bounds, viewport).0
    }

    fn menu_rect(&self, bounds: Rect, viewport: Size) -> Rect {
        self.menu_layout(bounds, viewport).1
    }

    fn option_rect(&self, bounds: Rect, viewport: Size, index: usize) -> Rect {
        let menu = self.menu_rect(bounds, viewport);
        Rect::new(
            menu.x(),
            menu.y() + (index as f32 * self.header_height()),
            menu.width(),
            self.header_height(),
        )
    }

    fn option_at(&self, bounds: Rect, viewport: Size, position: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }

        let menu = self.menu_rect(bounds, viewport);
        if !menu.contains(position) {
            return None;
        }

        self.options.iter().enumerate().find_map(|(index, _)| {
            self.option_rect(bounds, viewport, index)
                .contains(position)
                .then_some(index)
        })
    }

    fn select_index(&mut self, ctx: &mut EventCtx, index: usize) {
        if self.options.is_empty() {
            return;
        }
        let index = index.min(self.options.len().saturating_sub(1));
        self.selected = Some(index);
        let value = self.options[index].clone();
        if let Some(on_change) = &mut self.on_change {
            on_change(index, value.clone());
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, index, value);
        }
        self.refresh_menu_interaction_state(ctx);
    }

    fn sync_menu_state(&self, bounds: Rect, viewport: Size) {
        let mut state = self.menu_state.borrow_mut();
        state.theme = self.resolved_theme();
        state.options = self.options.clone();
        state.selected = self.current_selected_index();
        if state.hovered != self.hovered_option {
            state.sync_hovered_without_animation(self.hovered_option);
        }
        state.placement = self.menu_placement(bounds, viewport);
        state.menu_bounds = self.menu_rect(bounds, viewport);
    }

    fn refresh_menu_interaction_state(&self, ctx: &mut EventCtx) {
        let selected = self.current_selected_index();
        let hovered = self.hovered_option;
        let surface_id = self.menu_surface.child().id();
        let mut state = self.menu_state.borrow_mut();
        let selected_changed = state.selected != selected;
        state.selected = selected;
        let hover_changed = state.set_hovered(hovered, true, ctx);
        let changed = selected_changed || hover_changed;
        let presented = state.is_presented();
        drop(state);

        if changed && presented {
            request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
        }
    }

    fn set_expanded(&mut self, ctx: &mut EventCtx, expanded: bool) {
        if self.expanded == expanded {
            return;
        }

        self.expanded = expanded;
        self.hovered_option = if expanded {
            self.current_selected_index().or(Some(0))
        } else {
            None
        };

        let surface_id = self.menu_surface.child().id();
        let theme = self.resolved_theme();
        let mut state = self.menu_state.borrow_mut();
        state.theme = theme;
        let was_presented = state.is_presented();
        state.sync_hovered_without_animation(self.hovered_option);
        let should_animate = if expanded {
            let motion = theme.motion;
            state.reveal.set_target(
                1.0,
                ctx.current_time(),
                motion.entrance_duration(),
                motion.entrance_easing(),
            )
        } else {
            state.reveal = AnimatedScalar::new(0.0);
            false
        };
        let is_presented = state.is_presented();
        drop(state);

        if expanded || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_hover_state(
        &mut self,
        hovered_header: bool,
        hovered_option: Option<usize>,
        ctx: &mut EventCtx,
    ) {
        if self.hovered_header != hovered_header || self.hovered_option != hovered_option {
            let theme = self.resolved_theme();
            self.hovered_header = hovered_header;
            self.hovered_option = hovered_option;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered_header as u8 as f32,
                &theme,
                ctx,
            );
            let surface_id = self.menu_surface.child().id();
            let mut state = self.menu_state.borrow_mut();
            state.theme = theme;
            state.set_hovered(hovered_option, true, ctx);
            let presented = state.is_presented();
            drop(state);
            if presented {
                request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
            }
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn set_pressed_header(&mut self, pressed: bool, ctx: &mut EventCtx) {
        if self.pressed_header != pressed {
            let theme = self.resolved_theme();
            self.pressed_header = pressed;
            set_press_animation_target(
                &mut self.press_animation,
                pressed as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_header_animations(&mut self, time: f64) -> (bool, bool) {
        let previous_hover = self.hover_animation.value;
        let previous_press = self.press_animation.value;
        let previous_focus = self.focus_animation.value;
        let animating = self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time);
        let changed = self.hover_animation.changed_since(previous_hover)
            || self.press_animation.changed_since(previous_press)
            || self.focus_animation.changed_since(previous_focus);
        (changed, animating)
    }
}

impl Widget for Select {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some() && self.expanded {
            self.set_expanded(ctx, false);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hover_state(
                    self.header_rect(ctx.bounds()).contains(pointer.position),
                    self.option_at(ctx.bounds(), ctx.dpi().viewport, pointer.position),
                    ctx,
                );
            }
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Enter) => {
                self.set_hover_state(
                    self.header_rect(ctx.bounds()).contains(pointer.position),
                    self.option_at(ctx.bounds(), ctx.dpi().viewport, pointer.position),
                    ctx,
                );
            }
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Leave) => {
                self.set_hover_state(
                    false,
                    self.option_at(ctx.bounds(), ctx.dpi().viewport, pointer.position),
                    ctx,
                );
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered_header = self.header_rect(ctx.bounds()).contains(pointer.position);
                let hovered_option =
                    self.option_at(ctx.bounds(), ctx.dpi().viewport, pointer.position);
                self.set_hover_state(hovered_header, hovered_option, ctx);
                self.set_pressed_header(hovered_header, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered_header = self.header_rect(ctx.bounds()).contains(pointer.position);
                let hovered_option =
                    self.option_at(ctx.bounds(), ctx.dpi().viewport, pointer.position);
                let was_pressed_header = self.pressed_header;

                if was_pressed_header && hovered_header {
                    self.set_expanded(ctx, !self.expanded);
                } else if let Some(index) = hovered_option {
                    self.select_index(ctx, index);
                    self.set_expanded(ctx, false);
                } else {
                    self.set_expanded(ctx, false);
                }

                self.set_pressed_header(false, ctx);
                self.set_hover_state(
                    hovered_header,
                    if self.expanded {
                        self.hovered_option
                    } else {
                        None
                    },
                    ctx,
                );
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_header {
                    self.set_pressed_header(false, ctx);
                    self.set_hover_state(false, None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if self.options.is_empty() {
                    return;
                }

                match key.key.as_str() {
                    "Enter" | " " => {
                        if self.expanded {
                            if let Some(index) = self
                                .hovered_option
                                .or_else(|| self.current_selected_index())
                            {
                                self.select_index(ctx, index);
                            }
                            self.set_expanded(ctx, false);
                        } else {
                            self.set_expanded(ctx, true);
                        }
                    }
                    "Escape" => self.set_expanded(ctx, false),
                    "ArrowDown" => {
                        if self.expanded {
                            let next = self
                                .hovered_option
                                .unwrap_or_else(|| self.current_selected_index().unwrap_or(0))
                                .saturating_add(1)
                                .min(self.options.len() - 1);
                            self.set_hover_state(self.hovered_header, Some(next), ctx);
                        } else {
                            let next = self
                                .current_selected_index()
                                .unwrap_or(0)
                                .saturating_add(1)
                                .min(self.options.len() - 1);
                            self.select_index(ctx, next);
                        }
                    }
                    "ArrowUp" => {
                        if self.expanded {
                            let next = self
                                .hovered_option
                                .unwrap_or_else(|| self.current_selected_index().unwrap_or(0))
                                .saturating_sub(1);
                            self.set_hover_state(self.hovered_header, Some(next), ctx);
                        } else {
                            let next = self.current_selected_index().unwrap_or(0).saturating_sub(1);
                            self.select_index(ctx, next);
                        }
                    }
                    "Home" => self.select_index(ctx, 0),
                    "End" => self.select_index(ctx, self.options.len() - 1),
                    _ => {}
                }

                self.refresh_menu_interaction_state(ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let (header_changed, header_animating) = self.advance_header_animations(*time);
                let surface_id = self.menu_surface.child().id();
                let mut state = self.menu_state.borrow_mut();
                let was_presented = state.is_presented();
                let previous = state.reveal.value;
                let (hover_changed, hover_animating) = state.advance_hover(*time);
                let animating = state.reveal.advance(*time);
                let changed = state.reveal.changed_since(previous);
                let is_presented = state.is_presented();
                drop(state);

                if changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                }
                if hover_changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
                }
                if header_changed {
                    ctx.request_paint();
                }
                if animating || header_animating || hover_animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let padding = theme.metrics.text_input_padding;
        let text_style = theme.body_text_style();
        let widest_option = self
            .options
            .iter()
            .map(|label| measure_text(ctx, label, &text_style).width)
            .fold(0.0, f32::max);
        let placeholder_width =
            measure_text(ctx, &self.placeholder, &theme.placeholder_text_style()).width;
        let widest = widest_option.max(placeholder_width);
        let width = (widest + padding.left + padding.right + SELECT_CHEVRON_SLOT_WIDTH)
            .max(theme.metrics.button_min_width + SELECT_CHEVRON_SLOT_WIDTH + padding.right);
        let height = self.header_height();
        let presented = self.menu_state.borrow().is_presented();
        if presented {
            let menu_size = Size::new(width, self.menu_height());
            {
                let mut state = self.menu_state.borrow_mut();
                state.theme = theme;
                state.options = self.options.clone();
                state.selected = self.current_selected_index();
                if state.hovered != self.hovered_option {
                    state.sync_hovered_without_animation(self.hovered_option);
                }
                state.menu_bounds = Rect::from_origin_size(Point::ZERO, menu_size);
            }
            self.menu_surface
                .measure(ctx, Constraints::tight(menu_size));
        }

        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_menu_state(bounds, ctx.dpi().viewport);
        let state = self.menu_state.borrow();
        let surface_bounds = if state.is_presented() {
            state.menu_bounds
        } else {
            Rect::from_origin_size(bounds.origin, Size::ZERO)
        };
        drop(state);
        self.menu_surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let header = self.header_rect(ctx.bounds());
        let label = self.current_label();
        let placeholder = self.current_value().is_none();
        let hover_progress = self.hover_animation.value * interaction.hover_blend;
        let press_progress = self.press_animation.value * interaction.pressed_blend;
        let focus_progress = self.focus_animation.value;
        let content_offset =
            Vector::new(0.0, self.press_animation.value * interaction.pressed_offset);
        let text_style = if placeholder {
            theme.placeholder_text_style()
        } else {
            theme.body_text_style()
        };
        let text_slot = Rect::new(
            header.x() + metrics.text_input_padding.left,
            header.y(),
            (header.width()
                - metrics.text_input_padding.left
                - metrics.text_input_padding.right
                - SELECT_CHEVRON_SLOT_WIDTH)
                .max(0.0),
            header.height(),
        );
        // Mesh selects are dressed fields: the closed control sits on the
        // field token; hover/press keep the well and animate the border.
        draw_control_frame(
            ctx,
            header,
            metrics.corner_radius,
            metrics,
            mix_color(
                mix_color(palette.field, palette.control_active, press_progress * 0.5),
                palette.surface_focus,
                focus_progress,
            ),
            mix_color(
                mix_color(
                    palette.border,
                    palette.border_hover,
                    self.hover_animation.value.max(hover_progress),
                ),
                palette.border_focus,
                focus_progress,
            ),
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );
        ctx.push_clip_rect(text_slot);
        paint_single_line_aligned_text(
            ctx,
            text_slot.translate(content_offset),
            &label,
            &text_style,
            text_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        draw_icon_glyph(
            ctx,
            if self.expanded {
                IconGlyph::ChevronUp
            } else {
                IconGlyph::ChevronDown
            },
            select_chevron_icon_rect(header).translate(content_offset),
            palette.text,
        );

        if self.menu_state.borrow().is_presented() {
            self.menu_surface.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ComboBox, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.current_label()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered_header || self.menu_state.borrow().hovered.is_some();
        node.state.expanded = Some(self.expanded);
        node.popup = Some(SemanticsPopupKind::ListBox);
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
            SemanticsAction::SetValue,
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (self.expanded || self.menu_state.borrow().is_presented()).then_some(
            OverlayOptions::new(OverlayKind::Menu)
                .dismiss(if self.expanded {
                    OverlayDismissPolicy::TRANSIENT
                } else {
                    OverlayDismissPolicy::NONE
                })
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.expanded {
            self.set_expanded(ctx, false);
        }
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.menu_state.borrow().is_presented() {
            self.menu_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.menu_state.borrow().is_presented() {
            self.menu_surface.visit_children_mut(visitor);
        }
    }
}

pub type Divider = Separator;
pub type SpinBox = NumberInput;
pub type MultilineTextInput = TextArea;
pub type ComboBox = Select;

const PASSWORD_MASK: &str = "•";

pub struct TextInput {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    editor: EditorState,
    password: bool,
    placeholder: String,
    leading_icon: Option<IconGlyph>,
    read_only: bool,
    appearance: FieldAppearance,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    focused: bool,
    dragging_selection: bool,
    hover_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    caret_blink: Blink,
    caret_timer: Option<TimerToken>,
    caret_visible: bool,
    visible_measurement: Option<TextMeasurement>,
    input_measurement: Option<TextMeasurement>,
    display_layout: Option<PersistentTextLayout>,
    input_layout: Option<PersistentTextLayout>,
    selection_scope: Option<SelectionScope>,
    on_change: Option<Box<dyn FnMut(String)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, String)>>,
    on_focus_change: Option<Box<dyn FnMut(bool)>>,
}

impl TextInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            editor: EditorState::new(),
            password: false,
            placeholder: String::new(),
            leading_icon: None,
            read_only: false,
            appearance: FieldAppearance::Framed,
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            focused: false,
            dragging_selection: false,
            hover_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            caret_blink: Blink::new(CARET_BLINK_PERIOD_SECONDS),
            caret_timer: None,
            caret_visible: true,
            visible_measurement: None,
            input_measurement: None,
            display_layout: None,
            input_layout: None,
            selection_scope: None,
            on_change: None,
            on_change_with_ctx: None,
            on_focus_change: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn appearance(mut self, appearance: FieldAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn bare(mut self) -> Self {
        self.appearance = FieldAppearance::Bare;
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width.max(0.0));
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height.max(0.0));
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn leading_icon(mut self, icon: IconGlyph) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn selection_scope(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.editor.set_text(single_line_text(value.into()));
        self
    }

    fn password(mut self) -> Self {
        self.password = true;
        self
    }

    pub fn current_value(&self) -> &str {
        self.editor.document().text()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.editor.set_text(single_line_text(value.into()));
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, String) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn on_focus_change<F>(mut self, on_focus_change: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_focus_change = Some(Box::new(on_focus_change));
        self
    }

    fn input_text(&self) -> String {
        self.editor.display_text()
    }

    fn rendered_input_text(&self) -> String {
        let input = self.input_text();
        if self.password {
            PASSWORD_MASK.repeat(input.graphemes(true).count())
        } else {
            input
        }
    }

    fn rendered_offset(&self, input: &str, editor_offset: usize) -> usize {
        if !self.password {
            return editor_offset.min(input.len());
        }

        input
            .grapheme_indices(true)
            .take_while(|(offset, _)| *offset < editor_offset.min(input.len()))
            .count()
            * PASSWORD_MASK.len()
    }

    fn editor_offset(&self, input: &str, rendered_offset: usize) -> usize {
        if !self.password {
            return rendered_offset.min(input.len());
        }

        let grapheme_index = rendered_offset / PASSWORD_MASK.len();
        input
            .grapheme_indices(true)
            .nth(grapheme_index)
            .map(|(offset, _)| offset)
            .unwrap_or(input.len())
    }

    fn rendered_selection_range(&self, input: &str) -> Range<usize> {
        let selection = selection_range(&self.editor.display_selection(), input.len());
        self.rendered_offset(input, selection.start)..self.rendered_offset(input, selection.end)
    }

    fn display_caret_offset(&self) -> usize {
        let input = self.input_text();
        self.rendered_offset(&input, self.editor.display_selection().focus.utf8_offset)
    }

    fn visible_text(&self) -> String {
        let input = self.rendered_input_text();
        if input.is_empty() {
            self.placeholder.clone()
        } else {
            input
        }
    }

    fn display_text_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        if self.input_text().is_empty() {
            theme.placeholder_text_style()
        } else if self.read_only {
            theme.text_style(theme.palette.text_muted)
        } else {
            self.resolved_text_style()
        }
    }

    fn commit_text_change(&mut self, ctx: &mut EventCtx) {
        let value = self.current_value().to_string();
        if let Some(on_change) = &mut self.on_change {
            on_change(value.clone());
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, value);
        }
    }

    fn apply_editor_result(&mut self, ctx: &mut EventCtx, mut result: EditorCommandResult) {
        let copied_to_clipboard = result.clipboard_text.is_some();
        if let Some(text) = result.clipboard_text.take() {
            ctx.set_clipboard_text(text);
        }
        if result.text_changed {
            self.commit_text_change(ctx);
        }
        if result.layout_changed() {
            ctx.request_measure();
            ctx.request_paint();
        } else if result.overlay_changed() {
            ctx.request_paint();
        }
        if result.text_changed || result.selection_changed || result.composition_changed {
            sync_editor_selection_scope(ctx, self.selection_scope.as_ref(), &self.editor);
            ctx.request_semantics();
        }
        if result.handled {
            if self.focused {
                self.reset_caret_blink(ctx);
            }
            if !(copied_to_clipboard && self.selection_scope.is_some()) {
                ctx.set_handled();
            }
        }
    }

    fn execute_editor_command(&mut self, ctx: &mut EventCtx, command: EditorCommand) {
        let result = self.editor.execute(command);
        self.apply_editor_result(ctx, result);
    }

    /// Select the entire document.
    pub fn select_all(&mut self, ctx: &mut EventCtx) {
        self.execute_editor_command(ctx, EditorCommand::SelectAll);
    }

    /// Copy the current selection to the clipboard. No-op when the selection
    /// is collapsed.
    pub fn copy(&mut self, ctx: &mut EventCtx) {
        self.execute_editor_command(ctx, EditorCommand::Copy);
    }

    /// Copy the current selection to the clipboard and delete it. No-op when
    /// read-only or the selection is collapsed.
    pub fn cut(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            return;
        }
        self.execute_editor_command(ctx, EditorCommand::Cut);
    }

    /// Replace the current selection with the clipboard text (coerced to a
    /// single line). No-op when read-only or the clipboard has no text.
    pub fn paste(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            return;
        }
        let command = match paste_command(ctx) {
            EditorCommand::Paste(text) => EditorCommand::Paste(single_line_text(text)),
            command => command,
        };
        self.execute_editor_command(ctx, command);
    }

    /// Currently selected document text (empty when the selection is
    /// collapsed).
    pub fn selected_text(&self) -> &str {
        self.editor.selected_text()
    }

    fn apply_text_command(&mut self, ctx: &mut EventCtx, command: TextCommand) {
        match command {
            TextCommand::SelectAll => self.select_all(ctx),
            TextCommand::Copy => self.copy(ctx),
            TextCommand::Cut => self.cut(ctx),
            TextCommand::Paste => self.paste(ctx),
        }
    }

    fn text_offset_at_position(&self, bounds: Rect, position: Point) -> usize {
        let content = self.text_content_rect(bounds);
        let rendered_offset = self
            .input_layout
            .as_ref()
            .map(|layout| {
                layout
                    .hit_test_point(Point::new(
                        position.x - content.x(),
                        position.y - content.y(),
                    ))
                    .utf8_offset
            })
            .unwrap_or(self.rendered_input_text().len());
        self.editor_offset(&self.input_text(), rendered_offset)
    }

    fn set_caret_from_position(
        &mut self,
        bounds: Rect,
        position: Point,
        extend: bool,
        ctx: &mut EventCtx,
    ) {
        let offset = self.text_offset_at_position(bounds, position);
        let command = if extend {
            EditorCommand::SetSelection {
                anchor: self.editor.selection().anchor.utf8_offset,
                focus: offset,
            }
        } else {
            EditorCommand::MoveTo {
                offset,
                extend: false,
            }
        };
        let result = self.editor.execute(command);
        self.apply_editor_result(ctx, result);
        self.reset_caret_blink(ctx);
    }

    fn caret_blink_delay(&self) -> f64 {
        let span = if self.caret_visible {
            self.caret_blink.period * self.caret_blink.duty_cycle as f64
        } else {
            self.caret_blink.period * (1.0 - self.caret_blink.duty_cycle as f64)
        };
        span.max(f64::EPSILON)
    }

    fn arm_caret_blink(&mut self, ctx: &mut EventCtx) {
        if let Some(token) = self.caret_timer.take() {
            ctx.cancel_timer(token);
        }
        if self.focused {
            self.caret_timer = Some(ctx.schedule_timer_after(self.caret_blink_delay()));
        }
    }

    fn reset_caret_blink(&mut self, ctx: &mut EventCtx) {
        if self.read_only {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
            return;
        }
        self.caret_visible = self.focused;
        self.arm_caret_blink(ctx);
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = self.resolved_theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.text_input_padding)
    }

    fn leading_icon_advance(&self) -> f32 {
        if self.leading_icon.is_some() {
            24.0
        } else {
            0.0
        }
    }

    fn text_content_rect(&self, bounds: Rect) -> Rect {
        let content = inset_rect(bounds, self.resolved_padding());
        let leading = self.leading_icon_advance();
        Rect::new(
            (content.x() + leading).min(content.max_x()),
            content.y(),
            (content.width() - leading).max(0.0),
            content.height(),
        )
    }

    fn resolved_min_size(&self) -> Size {
        let theme = self.resolved_theme();
        Size::new(
            self.min_width.unwrap_or(theme.metrics.text_input_min_width),
            self.min_height.unwrap_or(theme.metrics.min_height),
        )
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn single_line_layout_rect(
        &self,
        ctx: &PaintCtx,
        content: Rect,
        layout: &PersistentTextLayout,
        line_height: f32,
    ) -> Rect {
        aligned_text_rect_for_layout(ctx, content, layout.layout(), line_height, 0.0)
    }

    fn single_line_text_rect(
        &self,
        ctx: &PaintCtx,
        content: Rect,
        text: &str,
        style: &TextStyle,
        line_height: f32,
    ) -> Rect {
        aligned_text_rect_for_text(ctx, content, text, style, line_height, 0.0)
    }
}

impl Widget for TextInput {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
                if self.dragging_selection
                    && ctx.phase() != EventPhase::Capture
                    && pointer.buttons.contains(PointerButton::Primary)
                {
                    let offset = self.text_offset_at_position(ctx.bounds(), pointer.position);
                    let result = self.editor.execute(EditorCommand::SetSelection {
                        anchor: self.editor.selection().anchor.utf8_offset,
                        focus: offset,
                    });
                    self.apply_editor_result(ctx, result);
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.set_hovered(true, ctx);
                self.set_caret_from_position(
                    ctx.bounds(),
                    pointer.position,
                    pointer.modifiers.shift,
                    ctx,
                );
                self.dragging_selection = true;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.dragging_selection =>
            {
                self.dragging_selection = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging_selection {
                    self.dragging_selection = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                // Focus on right-click (keeping the selection intact) so
                // follow-up clipboard commands land here. Deliberately not
                // handled: wrapping context menus react to the same press.
                self.set_hovered(true, ctx);
                if !ctx.is_focused() {
                    ctx.request_focus();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                if let Some(commands) =
                    semantics_editor_commands(ctx, &semantics.action, self.read_only, true)
                {
                    for command in commands {
                        self.execute_editor_command(ctx, command);
                    }
                }
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(ctx, EditorCommand::StartComposition);
                }
            }
            Event::Ime(ImeEvent::CompositionUpdate { text, cursor_range }) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(
                        ctx,
                        EditorCommand::UpdateComposition {
                            text: single_line_text(text.clone()),
                            cursor_range: cursor_range.clone(),
                        },
                    );
                }
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(
                        ctx,
                        EditorCommand::CommitComposition(single_line_text(text.clone())),
                    );
                }
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                if !self.read_only {
                    self.execute_editor_command(ctx, EditorCommand::EndComposition);
                }
            }
            Event::Keyboard(key)
                if !self.read_only
                    && key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && key.key == "Backspace" =>
            {
                self.execute_editor_command(ctx, EditorCommand::DeleteBackward);
            }
            Event::Keyboard(key)
                if !self.read_only
                    && key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && key.key == "Delete" =>
            {
                self.execute_editor_command(ctx, EditorCommand::DeleteForward);
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "ArrowLeft" =>
            {
                self.execute_editor_command(
                    ctx,
                    if key.modifiers.control || key.modifiers.meta {
                        EditorCommand::MoveWordLeft {
                            extend: key.modifiers.shift,
                        }
                    } else {
                        EditorCommand::MoveLeft {
                            extend: key.modifiers.shift,
                        }
                    },
                );
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && key.key == "ArrowRight" =>
            {
                self.execute_editor_command(
                    ctx,
                    if key.modifiers.control || key.modifiers.meta {
                        EditorCommand::MoveWordRight {
                            extend: key.modifiers.shift,
                        }
                    } else {
                        EditorCommand::MoveRight {
                            extend: key.modifiers.shift,
                        }
                    },
                );
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Home" =>
            {
                self.execute_editor_command(
                    ctx,
                    EditorCommand::MoveLineStart {
                        extend: key.modifiers.shift,
                    },
                );
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "End" =>
            {
                self.execute_editor_command(
                    ctx,
                    EditorCommand::MoveLineEnd {
                        extend: key.modifiers.shift,
                    },
                );
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                let command_modifier = key.modifiers.control || key.modifiers.meta;
                let command = match key.key.as_str() {
                    "a" | "A" if command_modifier => EditorCommand::SelectAll,
                    "c" | "C" if command_modifier => EditorCommand::Copy,
                    "x" | "X" if command_modifier && !self.read_only => EditorCommand::Cut,
                    "v" | "V" if command_modifier && !self.read_only => match paste_command(ctx) {
                        EditorCommand::Paste(text) => EditorCommand::Paste(single_line_text(text)),
                        command => command,
                    },
                    "z" | "Z" if command_modifier && key.modifiers.shift && !self.read_only => {
                        EditorCommand::Redo
                    }
                    "z" | "Z" if command_modifier && !self.read_only => EditorCommand::Undo,
                    "y" | "Y" if command_modifier && !self.read_only => EditorCommand::Redo,
                    _ if !self.read_only && self.editor.composition().is_none() => {
                        keyboard_text(key)
                            .map(|text| EditorCommand::InsertText(single_line_text(text)))
                            .unwrap_or(EditorCommand::Noop)
                    }
                    _ => EditorCommand::Noop,
                };
                if !matches!(command, EditorCommand::Noop) {
                    self.execute_editor_command(ctx, command);
                }
            }
            Event::Wake(sui_core::WakeEvent::Timer { token, .. })
                if self.caret_timer == Some(*token) =>
            {
                self.caret_timer = None;
                if self.focused {
                    self.caret_visible = !self.caret_visible;
                    self.arm_caret_blink(ctx);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                let previous_hover = self.hover_animation.value;
                let previous_focus = self.focus_animation.value;
                let animating =
                    self.hover_animation.advance(*time) | self.focus_animation.advance(*time);
                let changed = self.hover_animation.changed_since(previous_hover)
                    || self.focus_animation.changed_since(previous_focus);
                if animating {
                    ctx.request_animation_frame();
                }
                if changed {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(command) = TextCommand::from_command(command) else {
            return;
        };
        if !ctx.is_focused() {
            ctx.request_focus();
        }
        self.apply_text_command(ctx, command);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let min_size = self.resolved_min_size();
        let visible_text = self.visible_text();
        let input_text = self.rendered_input_text();
        let display_style = self.display_text_style();
        let measured_visible = measure_text(ctx, &visible_text, &display_style);
        let measured_input = if input_text.is_empty() {
            TextMeasurement {
                width: 0.0,
                height: measured_visible.height,
                bounds: Rect::new(0.0, 0.0, 0.0, measured_visible.height),
                ascent: measured_visible.ascent,
                descent: measured_visible.descent,
                cap_height: measured_visible.cap_height,
            }
        } else {
            measure_text(ctx, &input_text, &text_style)
        };
        let display_line_height = display_style
            .line_height
            .max(measured_visible.height)
            .max(1.0);
        let input_line_height = text_style.line_height.max(measured_input.height).max(1.0);
        let display_line_box = Size::new(f32::INFINITY, display_line_height);
        let input_line_box = Size::new(f32::INFINITY, input_line_height);
        let display_layout = ctx
            .layout()
            .shape_text_persistent(
                self.display_layout.as_ref().map(|layout| layout.handle()),
                visible_text.clone(),
                display_line_box,
                display_style.clone(),
            )
            .ok();
        let input_layout = ctx
            .layout()
            .shape_text_persistent(
                self.input_layout.as_ref().map(|layout| layout.handle()),
                input_text.clone(),
                input_line_box,
                text_style.clone(),
            )
            .ok();

        let visible_measurement = display_layout
            .as_ref()
            .map(|layout| layout.measurement())
            .unwrap_or(measured_visible);
        let input_measurement = input_layout
            .as_ref()
            .map(|layout| layout.measurement())
            .unwrap_or(measured_input);

        self.visible_measurement = Some(visible_measurement);
        self.input_measurement = Some(input_measurement);
        self.display_layout = display_layout;
        self.input_layout = input_layout;

        let width = (visible_measurement.width
            + self.leading_icon_advance()
            + padding.left
            + padding.right)
            .max(min_size.width);
        let height = (visible_measurement.height.max(display_style.line_height)
            + padding.top
            + padding.bottom)
            .max(min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let focus_progress = self.focus_animation.value;
        // Light fields lift from their slightly recessed rest fill to the
        // surface on hover. Focus then moves every scheme toward its soft
        // accent well; dark and Void keep their established resting depth.
        let background = field_background(
            &theme,
            self.read_only,
            self.hover_animation.value,
            focus_progress,
        );
        let border = mix_color(
            mix_color(
                palette.border,
                palette.border_hover,
                self.hover_animation.value,
            ),
            palette.border_focus,
            focus_progress,
        );
        let full_content_rect = inset_rect(ctx.bounds(), padding);
        let content_rect = self.text_content_rect(ctx.bounds());
        let display_text = self.visible_text();
        let placeholder = self.input_text().is_empty();

        if self.appearance == FieldAppearance::Framed {
            draw_control_frame(
                ctx,
                ctx.bounds(),
                metrics.corner_radius,
                metrics,
                background,
                border,
                (focus_progress > 0.0).then_some(
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                ),
            );
        }
        if let Some(icon) = self.leading_icon {
            let icon_color = if self.read_only || placeholder {
                palette.text_muted
            } else {
                palette.text
            };
            let icon_side = 15.0;
            let icon_rect = Rect::new(
                full_content_rect.x() + 1.0,
                full_content_rect.y() + (full_content_rect.height() - icon_side) * 0.5,
                icon_side,
                icon_side,
            );
            draw_icon_glyph(ctx, icon, icon_rect, icon_color);
        }
        ctx.push_clip_rect(content_rect);
        if let Some(layout) = &self.display_layout {
            let layout_bounds = layout.measurement().bounds;
            let layout_rect =
                self.single_line_layout_rect(ctx, content_rect, layout, layout.style().line_height);
            let layout_origin = Point::new(layout_rect.x() - layout_bounds.x(), layout_rect.y());
            if !placeholder {
                let input = self.input_text();
                let selection = self.rendered_selection_range(&input);
                if !selection.is_empty() {
                    for rect in layout.selection_rects(selection) {
                        ctx.fill_rect(rect.translate(layout_origin.to_vector()), palette.selection);
                    }
                }
            }
            ctx.draw_persistent_text_layout(layout_origin, layout);
        } else {
            let display_style = if placeholder {
                theme.placeholder_text_style()
            } else if self.read_only {
                theme.text_style(palette.text_muted)
            } else {
                text_style.clone()
            };
            paint_aligned_text(
                ctx,
                content_rect,
                &display_text,
                &display_style,
                display_style.line_height,
                0.0,
            );
        }
        ctx.pop_clip();

        if self.focused && !self.read_only {
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let input_text = self.rendered_input_text();
            let input_text_rect = self
                .input_layout
                .as_ref()
                .map(|layout| {
                    self.single_line_layout_rect(ctx, content_rect, layout, text_style.line_height)
                })
                .unwrap_or_else(|| {
                    self.single_line_text_rect(
                        ctx,
                        content_rect,
                        &input_text,
                        &text_style,
                        text_style.line_height,
                    )
                });
            let caret_rect = self
                .input_layout
                .as_ref()
                .map(|layout| {
                    layout
                        .caret_rect(self.display_caret_offset())
                        .translate(input_text_rect.origin.to_vector())
                })
                .unwrap_or(Rect::new(
                    input_text_rect.x()
                        + self
                            .input_measurement
                            .map(|measurement| measurement.width)
                            .unwrap_or(0.0),
                    input_text_rect.y(),
                    caret_width,
                    input_text_rect.height().max(text_style.line_height),
                ));
            let caret_rect = Rect::new(
                caret_rect
                    .x()
                    .min((content_rect.max_x() - caret_width).max(content_rect.x()))
                    .max(content_rect.x()),
                caret_rect.y(),
                caret_width,
                caret_rect.height().max(text_style.line_height),
            );
            ctx.set_ime_composition_rect(caret_rect);
            if self.caret_visible {
                ctx.fill(
                    rounded_rect_path(caret_rect, caret_width * 0.5),
                    palette.caret,
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        let display_text = self.input_text();
        let display_selection = self.editor.display_selection();
        let selection = selection_range(&display_selection, display_text.len());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(display_text));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.editable_text = Some(EditableTextSemantics {
            caret_offset: display_selection.focus.utf8_offset,
            selection: SemanticsTextRange::new(selection.start, selection.end),
            multiline: false,
            password: self.password,
            readonly: self.read_only,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        node.actions = if self.read_only {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetSelection,
                SemanticsAction::Copy,
            ]
        } else {
            vec![
                SemanticsAction::Focus,
                SemanticsAction::SetValue,
                SemanticsAction::SetSelection,
                SemanticsAction::InsertText,
                SemanticsAction::DeleteBackward,
                SemanticsAction::DeleteForward,
                SemanticsAction::Copy,
                SemanticsAction::Cut,
                SemanticsAction::Paste,
                SemanticsAction::Undo,
                SemanticsAction::Redo,
            ]
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.focused = focused;
        if !focused {
            let result = self.editor.execute(EditorCommand::ClearComposition);
            if result.layout_changed() {
                ctx.request_measure();
            }
        }
        if focused {
            self.reset_caret_blink(ctx);
        } else {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
        }
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        if let Some(on_focus_change) = &mut self.on_focus_change {
            on_focus_change(focused);
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

/// A single-line text input that masks its visible value while retaining the
/// same selection, clipboard, IME, and change-callback behavior as
/// [`TextInput`].
pub struct PasswordInput {
    inner: TextInput,
}

impl PasswordInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: TextInput::new(name).password(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.inner = self.inner.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.inner = self.inner.theme_when(theme);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.inner = self.inner.placeholder(placeholder);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.inner = self.inner.value(value);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.inner = self.inner.min_width(width);
        self
    }

    pub fn read_only(mut self) -> Self {
        self.inner = self.inner.read_only();
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.inner = self.inner.selectable(selection_scope);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.inner = self.inner.on_change(on_change);
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, String) + 'static,
    {
        self.inner = self.inner.on_change_with_ctx(on_change);
        self
    }

    pub fn current_value(&self) -> &str {
        self.inner.current_value()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.inner.set_value(value);
    }

    pub fn selected_text(&self) -> &str {
        self.inner.selected_text()
    }

    pub fn select_all(&mut self, ctx: &mut EventCtx) {
        self.inner.select_all(ctx);
    }

    pub fn copy(&mut self, ctx: &mut EventCtx) {
        self.inner.copy(ctx);
    }

    pub fn cut(&mut self, ctx: &mut EventCtx) {
        self.inner.cut(ctx);
    }

    pub fn paste(&mut self, ctx: &mut EventCtx) {
        self.inner.paste(ctx);
    }
}

impl Widget for PasswordInput {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

/// A lightweight single-line local date/time field. Values remain strings so
/// applications can choose their own parsing, timezone, and validation rules;
/// the suggested format is `YYYY-MM-DD HH:MM`.
pub struct DateTimeInput {
    inner: TextInput,
}

impl DateTimeInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: TextInput::new(name).placeholder("YYYY-MM-DD HH:MM"),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.inner = self.inner.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.inner = self.inner.theme_when(theme);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.inner = self.inner.placeholder(placeholder);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.inner = self.inner.value(value);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.inner = self.inner.min_width(width);
        self
    }

    pub fn read_only(mut self) -> Self {
        self.inner = self.inner.read_only();
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.inner = self.inner.selectable(selection_scope);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.inner = self.inner.on_change(on_change);
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, String) + 'static,
    {
        self.inner = self.inner.on_change_with_ctx(on_change);
        self
    }

    pub fn current_value(&self) -> &str {
        self.inner.current_value()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.inner.set_value(value);
    }

    pub fn selected_text(&self) -> &str {
        self.inner.selected_text()
    }

    pub fn select_all(&mut self, ctx: &mut EventCtx) {
        self.inner.select_all(ctx);
    }

    pub fn copy(&mut self, ctx: &mut EventCtx) {
        self.inner.copy(ctx);
    }

    pub fn cut(&mut self, ctx: &mut EventCtx) {
        self.inner.cut(ctx);
    }

    pub fn paste(&mut self, ctx: &mut EventCtx) {
        self.inner.paste(ctx);
    }
}

impl Widget for DateTimeInput {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.layout()
        .measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}

fn numeric_text_style(mut style: TextStyle) -> TextStyle {
    style.features.enable(FontFeature::TABULAR_FIGURES);
    style
}

fn single_line_text(text: impl Into<String>) -> String {
    text.into()
        .chars()
        .filter(|ch| *ch != '\r' && *ch != '\n')
        .collect()
}

fn semantics_editor_commands(
    ctx: &EventCtx,
    action: &SemanticsActionRequest,
    read_only: bool,
    single_line: bool,
) -> Option<Vec<EditorCommand>> {
    let normalize = |text: String| {
        if single_line {
            single_line_text(text)
        } else {
            text
        }
    };

    match action {
        SemanticsActionRequest::SetValue(SemanticsValue::Text(text)) if !read_only => Some(vec![
            EditorCommand::SelectAll,
            EditorCommand::InsertText(normalize(text.clone())),
        ]),
        SemanticsActionRequest::SetSelection(selection) => {
            Some(vec![EditorCommand::SetSelection {
                anchor: selection.start,
                focus: selection.end,
            }])
        }
        SemanticsActionRequest::InsertText(text) if !read_only => {
            Some(vec![EditorCommand::InsertText(normalize(text.clone()))])
        }
        SemanticsActionRequest::DeleteBackward if !read_only => {
            Some(vec![EditorCommand::DeleteBackward])
        }
        SemanticsActionRequest::DeleteForward if !read_only => {
            Some(vec![EditorCommand::DeleteForward])
        }
        SemanticsActionRequest::Copy => Some(vec![EditorCommand::Copy]),
        SemanticsActionRequest::Cut if !read_only => Some(vec![EditorCommand::Cut]),
        SemanticsActionRequest::Paste if !read_only => {
            let command = match paste_command(ctx) {
                EditorCommand::Paste(text) => EditorCommand::Paste(normalize(text)),
                command => command,
            };
            Some(vec![command])
        }
        SemanticsActionRequest::Undo if !read_only => Some(vec![EditorCommand::Undo]),
        SemanticsActionRequest::Redo if !read_only => Some(vec![EditorCommand::Redo]),
        _ => None,
    }
}

fn keyboard_text(event: &sui_core::KeyboardEvent) -> Option<&str> {
    if event.state != KeyState::Pressed
        || event.is_composing
        || event.modifiers.control
        || event.modifiers.alt
        || event.modifiers.meta
    {
        return None;
    }

    if let Some(text) = event
        .text
        .as_deref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    {
        return Some(text);
    }

    let key = event.key.as_str();
    (key.chars().count() == 1 && !key.chars().any(char::is_control)).then_some(key)
}

fn center_square(bounds: Rect, side: f32) -> Rect {
    let side = side.min(bounds.width()).min(bounds.height()).max(0.0);
    Rect::new(
        bounds.x() + ((bounds.width() - side) * 0.5),
        bounds.y() + ((bounds.height() - side) * 0.5),
        side,
        side,
    )
}

fn rect_center(rect: Rect) -> Point {
    Point::new(
        rect.x() + (rect.width() * 0.5),
        rect.y() + (rect.height() * 0.5),
    )
}

fn switch_track_rect(bounds: Rect, padding: Insets, metrics: ControlMetrics) -> Rect {
    Rect::new(
        bounds.x() + padding.left,
        bounds.y() + ((bounds.height() - metrics.switch_track_height) * 0.5),
        metrics.switch_track_width,
        metrics.switch_track_height,
    )
}

fn switch_label_rect(bounds: Rect, padding: Insets, metrics: ControlMetrics, gap: f32) -> Rect {
    let x = bounds.x() + padding.left + metrics.switch_track_width + gap;
    Rect::new(
        x,
        bounds.y(),
        (bounds.width() - (x - bounds.x()) - padding.right).max(0.0),
        bounds.height(),
    )
}

fn horizontal_text_inset_rect(bounds: Rect, padding: Insets) -> Rect {
    Rect::new(
        bounds.x() + padding.left,
        bounds.y(),
        (bounds.width() - padding.left - padding.right).max(0.0),
        bounds.height(),
    )
}

fn select_chevron_icon_rect(header: Rect) -> Rect {
    let x = header.max_x() - SELECT_CHEVRON_SLOT_WIDTH
        + ((SELECT_CHEVRON_SLOT_WIDTH - SELECT_CHEVRON_ICON_SIZE).max(0.0) * 0.5);
    Rect::new(x, header.y(), SELECT_CHEVRON_ICON_SIZE, header.height())
}

fn number_input_stepper_rect(bounds: Rect, metrics: ControlMetrics) -> Rect {
    Rect::new(
        bounds.max_x() - metrics.number_input_stepper_width,
        bounds.y(),
        metrics.number_input_stepper_width,
        bounds.height(),
    )
}

fn number_input_stepper_part(
    bounds: Rect,
    metrics: ControlMetrics,
    position: Point,
) -> Option<NumberInputStepperPart> {
    let stepper = number_input_stepper_rect(bounds, metrics);
    if !stepper.contains(position) {
        return None;
    }
    if position.y < stepper.y() + (stepper.height() * 0.5) {
        Some(NumberInputStepperPart::Increment)
    } else {
        Some(NumberInputStepperPart::Decrement)
    }
}

fn number_input_text_rect(bounds: Rect, metrics: ControlMetrics) -> Rect {
    let padding = metrics.text_input_padding;
    Rect::new(
        bounds.x() + padding.left,
        bounds.y(),
        (bounds.width() - padding.left - padding.right - metrics.number_input_stepper_width)
            .max(0.0),
        bounds.height(),
    )
}

fn clamp_and_snap_value(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let clamped = value.clamp(min, max);
    if !step.is_finite() || step <= f64::EPSILON {
        return clamped;
    }

    let snapped = (clamped / step).round() * step;
    snapped.clamp(min, max)
}

fn format_number(value: f64, precision: usize) -> String {
    let mut text = format!("{value:.precision$}");
    if precision > 0 && text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" { "0".to_string() } else { text }
}

fn is_numeric_input_char(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+')
}

/// Draw an [`IconGlyph`] tinted `color`, centered and fit within `bounds`. Exposed for bespoke
/// painters (application chrome) that draw an icon mark without composing an [`Icon`] widget; the
/// glyph is painted directly as native SUI path geometry.
pub fn draw_glyph(ctx: &mut PaintCtx, glyph: IconGlyph, bounds: Rect, color: Color) {
    draw_icon_glyph(ctx, glyph, bounds, color);
}

pub(crate) fn draw_icon_glyph(ctx: &mut PaintCtx, glyph: IconGlyph, bounds: Rect, color: Color) {
    glyph.lucide_icon().paint(ctx, bounds, color);
}

fn line_path(start: Point, end: Point) -> Path {
    let mut builder = PathBuilder::new();
    builder.move_to(start).line_to(end);
    builder.build()
}

pub(crate) fn apply_hdr_policy_cap(color: Color, peak_lift: f32) -> Color {
    let cap = if peak_lift.is_finite() {
        peak_lift.max(0.0)
    } else {
        return color;
    };

    Color {
        red: color.red.clamp(0.0, cap),
        green: color.green.clamp(0.0, cap),
        blue: color.blue.clamp(0.0, cap),
        ..color
    }
}

pub(crate) fn cap_resolved_hdr_style(style: ResolvedHdrStyle) -> ResolvedHdrStyle {
    ResolvedHdrStyle {
        color: apply_hdr_policy_cap(style.color, style.peak_lift),
        effect: style.effect.map(|effect| ResolvedEffectStyle {
            color: apply_hdr_policy_cap(effect.color, style.peak_lift),
            ..effect
        }),
        ..style
    }
}

fn draw_control_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
) {
    draw_control_shape(
        ctx,
        bounds,
        radius,
        physical_pixels(ctx, metrics.border_width),
        background,
        border,
    );

    draw_control_focus_ring(ctx, bounds, radius, metrics, focus_ring);
}

fn draw_choice_control_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    appearance: ChoiceAppearance,
    visuals: ChoiceFrameVisuals,
    focus_ring: Option<Color>,
) {
    if appearance == ChoiceAppearance::Framed {
        draw_control_frame(
            ctx,
            bounds,
            radius,
            metrics,
            visuals.background,
            visuals.border,
            focus_ring,
        );
        return;
    }

    if visuals.background.alpha > f32::EPSILON {
        ctx.fill(rounded_rect_path(bounds, radius), visuals.background);
    }
    draw_control_focus_ring(ctx, bounds, radius, metrics, focus_ring);
}

fn draw_control_focus_ring(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    focus_ring: Option<Color>,
) {
    if let Some(focus_ring) = focus_ring {
        let focus_ring_outset = physical_pixels(ctx, metrics.focus_ring_outset);
        ctx.stroke(
            rounded_rect_path(
                bounds.inflate(focus_ring_outset, focus_ring_outset),
                radius + focus_ring_outset,
            ),
            focus_ring,
            StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
        );
    }
}

fn draw_control_shape(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    border_width: f32,
    background: Color,
    border: Color,
) {
    let fill_shape = rounded_rect_path(bounds, radius);
    ctx.fill(fill_shape, background);

    if border_width > 0.0 {
        let inset = border_width * 0.5;
        let stroke_shape =
            rounded_rect_path(bounds.inflate(-inset, -inset), (radius - inset).max(0.0));
        ctx.stroke(stroke_shape, border, StrokeStyle::new(border_width));
    }
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

fn checkmark_path(rect: Rect) -> Path {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(
            rect.x() + (rect.width() * 0.18),
            rect.y() + (rect.height() * 0.54),
        ))
        .line_to(Point::new(
            rect.x() + (rect.width() * 0.42),
            rect.y() + (rect.height() * 0.76),
        ))
        .line_to(Point::new(
            rect.x() + (rect.width() * 0.82),
            rect.y() + (rect.height() * 0.28),
        ));
    builder.build()
}

fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

fn choice_control_height(
    content_height: f32,
    padding: Insets,
    baseline_height: f32,
    has_explicit_padding: bool,
) -> f32 {
    let padding_height = if has_explicit_padding {
        padding.top + padding.bottom
    } else {
        0.0
    };
    (content_height + padding_height).max(baseline_height)
}

fn default_form_control_height(theme: &DefaultTheme) -> f32 {
    let style = theme.body_text_style();
    let padding = theme.metrics.text_input_padding;
    (style.line_height + padding.top + padding.bottom).max(theme.metrics.min_height)
}

fn choice_control_layout_padding(padding: Insets, has_explicit_padding: bool) -> Insets {
    if has_explicit_padding {
        padding
    } else {
        Insets {
            top: 0.0,
            bottom: 0.0,
            ..padding
        }
    }
}

fn indicator_rect(bounds: Rect, padding: Insets, indicator_size: f32) -> Rect {
    let x = bounds.x() + padding.left;
    let content = inset_rect(bounds, padding);
    let y = content.y() + ((content.height() - indicator_size) * 0.5);
    Rect::new(x, y, indicator_size, indicator_size)
}

fn checkbox_label_rect(bounds: Rect, padding: Insets, indicator_size: f32, gap: f32) -> Rect {
    let x = bounds.x() + padding.left + indicator_size + gap;
    let width = (bounds.width() - padding.left - padding.right - indicator_size - gap).max(0.0);
    let content = inset_rect(bounds, padding);
    Rect::new(x, content.y(), width, content.height())
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    ctx.dpi().physical_pixels_to_logical(value)
}

fn rect_is_finite(rect: Rect) -> bool {
    rect.x().is_finite()
        && rect.y().is_finite()
        && rect.width().is_finite()
        && rect.height().is_finite()
}

fn request_selection_change(ctx: &mut EventCtx, change: SelectionChange) {
    for owner in change.affected_owners() {
        let widget_id = WidgetId::new(owner.get());
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Paint,
        ));
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Semantics,
        ));
    }
}

fn sync_editor_selection_scope(
    ctx: &mut EventCtx,
    selection_scope: Option<&SelectionScope>,
    editor: &EditorState,
) {
    let Some(scope) = selection_scope else {
        return;
    };
    let owner = SelectionOwnerId::from(ctx.widget_id());
    let range = editor.selection_range();
    let selected = editor.selected_text().to_string();
    let change = scope.replace_text(owner, owner, range, editor.document().len(), selected);
    request_selection_change(ctx, change);
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Button, ButtonAppearance, CARET_BLINK_PERIOD_SECONDS, Checkbox, ChoiceAppearance,
        DateTimeInput, DefaultTheme, FieldAppearance, Icon, IconButton, IconButtonPaint, IconGlyph,
        Label, Link, NumberInput, PasswordInput, RadioButton, RadioGroup, Select, Separator,
        Slider, Switch, TextArea, TextInput, paint_icon_button, rect_is_finite,
    };
    use crate::{
        HdrThemeMode, SemanticColorToken, SemanticTone, WidgetLuminanceRole, resolve_luminance_role,
    };
    use crate::{
        containers::{SizedBox, Stack},
        selection::SelectionScope,
        text_command::{TEXT_COMMAND, TextCommand},
    };
    use sui_core::{
        Color, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
        PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect, Result, SemanticsAction,
        SemanticsActionRequest, SemanticsRole, SemanticsTextRange, SemanticsValue, Size, Vector,
        WidgetId, WindowEvent,
    };
    use sui_layout::{Constraints, Padding as TestPadding};
    use sui_reactive::Signal;
    use sui_render_wgpu::{RgbaImage, WgpuRenderer};
    use sui_runtime::{
        Application, CommandDelivery, CommandTarget, MeasureCtx, PaintCtx, RenderOutput, Runtime,
        Widget, WindowBuilder, WindowRenderOptions, clear_window_render_options,
        set_window_render_options,
    };
    use sui_scene::{
        Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor, SceneLayerUpdateKind,
    };
    use sui_text::{FontFeature, FontRegistry, TextStyle, TextSystem};

    fn hover_duration() -> f64 {
        DefaultTheme::default().motion.hover_duration()
    }

    fn press_duration() -> f64 {
        DefaultTheme::default().motion.press_duration()
    }

    fn toggle_duration() -> f64 {
        DefaultTheme::default().motion.toggle_duration()
    }

    fn focus_duration() -> f64 {
        DefaultTheme::default().motion.focus_duration()
    }

    fn entrance_duration() -> f64 {
        DefaultTheme::default().motion.entrance_duration()
    }

    fn slow_normal_motion_theme() -> DefaultTheme {
        let mut theme = DefaultTheme::default();
        theme.motion.duration_fast = 0.0;
        theme.motion.duration_normal = 0.6;
        theme
    }

    fn slow_toggle_theme() -> DefaultTheme {
        slow_normal_motion_theme()
    }

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Controls").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn render<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        runtime.render(window_id).unwrap()
    }

    fn render_isolated<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Unused")
                    .root(Label::new("Unused")),
            )
            .window(WindowBuilder::new().title("Controls").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[1];
        runtime.render(window_id).unwrap()
    }

    fn render_rgba<W>(root: W, feathering_enabled: bool) -> (RenderOutput, RgbaImage)
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        let output = runtime.render(window_id).unwrap();
        let mut renderer = WgpuRenderer::default();
        if feathering_enabled {
            renderer.set_feather_width(1.0);
            renderer.set_feathering_enabled(true);
        }
        renderer.render(&output.frame).unwrap();
        let image = renderer.capture_last_frame_rgba(window_id).unwrap();
        (output, image)
    }

    fn dark_pixel_count(image: &RgbaImage, rect: Rect, max_channel: u8) -> usize {
        let min_x = rect.x().floor().max(0.0) as u32;
        let min_y = rect.y().floor().max(0.0) as u32;
        let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
        let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
        let pixels = image.pixels();
        let width = image.width() as usize;

        let mut count = 0usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let index = ((y as usize * width) + x as usize) * 4;
                let red = pixels[index];
                let green = pixels[index + 1];
                let blue = pixels[index + 2];
                let alpha = pixels[index + 3];
                if alpha != 0 && red <= max_channel && green <= max_channel && blue <= max_channel {
                    count += 1;
                }
            }
        }

        count
    }

    fn bright_pixel_count(image: &RgbaImage, rect: Rect, min_channel: u8) -> usize {
        let min_x = rect.x().floor().max(0.0) as u32;
        let min_y = rect.y().floor().max(0.0) as u32;
        let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
        let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
        let pixels = image.pixels();
        let width = image.width() as usize;

        let mut count = 0usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let index = ((y as usize * width) + x as usize) * 4;
                let red = pixels[index];
                let green = pixels[index + 1];
                let blue = pixels[index + 2];
                let alpha = pixels[index + 3];
                if alpha > 200 && red >= min_channel && green >= min_channel && blue >= min_channel
                {
                    count += 1;
                }
            }
        }

        count
    }

    fn first_text_run(output: &RenderOutput) -> sui_text::TextRun {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::DrawText(text) => Some(text.clone()),
                SceneCommand::DrawShapedText(text) => text
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .map(|layout| {
                        let mut style = layout.style().clone();
                        if let Some(color) = text.color_override {
                            style.color = color;
                        }
                        sui_text::TextRun {
                            rect: shaped_text_run_rect(text.origin, layout),
                            text: layout.text().to_string(),
                            style,
                        }
                    }),
                _ => None,
            })
            .expect("text draw command present")
    }

    fn shaped_text_run_rect(origin: Point, layout: &sui_text::TextLayout) -> Rect {
        let measurement = layout.measurement();
        let bounds = measurement.bounds;
        let width = if bounds.width().is_finite() && bounds.width() > 0.0 {
            bounds.width()
        } else {
            measurement.width
        };
        Rect::new(
            origin.x + bounds.x(),
            origin.y + ((layout.box_size().height - measurement.height).max(0.0) * 0.5),
            width,
            layout.style().line_height.max(measurement.height),
        )
    }

    fn first_shaped_text(output: &RenderOutput) -> &sui_text::ShapedText {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::DrawShapedText(text) => Some(text),
                _ => None,
            })
            .expect("shaped text draw command present")
    }

    fn solid_fill_colors(output: &RenderOutput) -> Vec<Color> {
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

    fn solid_stroke_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect {
                    brush: Brush::Solid(color),
                    ..
                }
                | SceneCommand::StrokePath {
                    brush: Brush::Solid(color),
                    ..
                } => colors.push(*color),
                _ => {}
            });
        colors
    }

    fn non_lucide_stroke_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect {
                    brush: Brush::Solid(color),
                    ..
                } => colors.push(*color),
                SceneCommand::StrokePath {
                    brush: Brush::Solid(color),
                    stroke,
                    ..
                } if stroke.cap != sui_scene::StrokeCap::Round
                    || stroke.join != sui_scene::StrokeJoin::Round =>
                {
                    colors.push(*color);
                }
                _ => {}
            });
        colors
    }

    fn solid_stroke_path_bounds(output: &RenderOutput, expected_color: Color) -> Vec<Rect> {
        let mut bounds = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::StrokePath {
                path,
                brush: Brush::Solid(color),
                ..
            } = command
                && *color == expected_color
            {
                bounds.push(path.bounds());
            }
        });
        bounds
    }

    fn assert_rect_approx_eq(actual: Rect, expected: Rect) {
        const TOLERANCE: f32 = 0.01;
        assert!(
            (actual.x() - expected.x()).abs() <= TOLERANCE
                && (actual.y() - expected.y()).abs() <= TOLERANCE
                && (actual.width() - expected.width()).abs() <= TOLERANCE
                && (actual.height() - expected.height()).abs() <= TOLERANCE,
            "rect mismatch: actual={actual:?}, expected={expected:?}"
        );
    }

    fn lucide_strokes(output: &RenderOutput) -> Vec<(Rect, Color, sui_scene::StrokeStyle)> {
        let mut strokes = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::StrokePath {
                path,
                brush: Brush::Solid(color),
                stroke,
            } = command
                && stroke.cap == sui_scene::StrokeCap::Round
                && stroke.join == sui_scene::StrokeJoin::Round
            {
                strokes.push((path.bounds(), *color, *stroke));
            }
        });
        strokes
    }

    fn first_lucide_icon_rect(output: &RenderOutput) -> Rect {
        let (ink_bounds, _, stroke) = lucide_strokes(output)
            .into_iter()
            .next()
            .expect("Lucide icon should paint as a native stroked path");
        let side = stroke.width * 12.0;
        Rect::new(
            ink_bounds.x() + (ink_bounds.width() - side) * 0.5,
            ink_bounds.y() + (ink_bounds.height() - side) * 0.5,
            side,
            side,
        )
    }

    fn assert_color_approx_eq(actual: Color, expected: Color) {
        const CHANNEL_TOLERANCE: f32 = 1.0 / 255.0;
        assert_eq!(actual.space, expected.space);
        assert!(
            (actual.red - expected.red).abs() <= CHANNEL_TOLERANCE
                && (actual.green - expected.green).abs() <= CHANNEL_TOLERANCE
                && (actual.blue - expected.blue).abs() <= CHANNEL_TOLERANCE
                && (actual.alpha - expected.alpha).abs() <= CHANNEL_TOLERANCE,
            "color {actual:?} did not match {expected:?} within one channel step"
        );
    }

    fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            found = match command {
                SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
                SceneCommand::DrawShapedText(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .map(|layout| {
                        let mut style = layout.style().clone();
                        if let Some(color) = run.color_override {
                            style.color = color;
                        }
                        sui_text::TextRun {
                            rect: shaped_text_run_rect(run.origin, layout),
                            text: layout.text().to_string(),
                            style,
                        }
                    }),
                SceneCommand::DrawShapedTextWindow(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .map(|layout| {
                        let mut style = layout.style().clone();
                        if let Some(color) = run.color_override {
                            style.color = color;
                        }
                        sui_text::TextRun {
                            rect: run.translated_bounds(),
                            text: layout.text().to_string(),
                            style,
                        }
                    }),
                _ => None,
            };
        });
        found.expect("text draw command present")
    }

    fn draw_clip_rect_for(output: &RenderOutput, text: &str) -> Rect {
        let mut stack = Vec::new();
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            match command {
                SceneCommand::PushClip { rect } => stack.push(*rect),
                SceneCommand::PopClip => {
                    stack.pop();
                }
                SceneCommand::DrawText(run) if run.text == text => {
                    found = stack.last().copied();
                }
                SceneCommand::DrawShapedText(run)
                    if run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .is_some_and(|layout| layout.text() == text) =>
                {
                    found = stack.last().copied();
                }
                SceneCommand::DrawShapedTextWindow(run)
                    if run
                        .resolve(output.frame.text_layout_registry.as_ref())
                        .is_some_and(|layout| layout.text() == text) =>
                {
                    found = stack.last().copied();
                }
                _ => {}
            }
        });
        found.expect("text draw command should have an active clip")
    }

    fn shaped_text_layout_for(output: &RenderOutput, text: &str) -> sui_text::TextLayout {
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            found = match command {
                SceneCommand::DrawShapedText(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .cloned(),
                SceneCommand::DrawShapedTextWindow(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .cloned(),
                _ => None,
            };
        });
        found.expect("shaped text layout present")
    }

    fn visual_center(measurement: sui_text::TextMeasurement, optical_centering: bool) -> f32 {
        let top = if optical_centering {
            -measurement.cap_height.unwrap_or(measurement.ascent)
        } else {
            -measurement.ascent
        };
        let bottom = if optical_centering {
            measurement.descent * 0.5
        } else {
            measurement.descent
        };

        (top + bottom) * 0.5
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        visual_center(measurement, true)
    }

    fn text_run_layout(run: &sui_text::TextRun) -> sui_text::TextLayout {
        TextSystem::new()
            .shape_text(
                run.text.clone(),
                Size::new(f32::INFINITY, run.rect.height().max(1.0)),
                run.style.clone(),
                &FontRegistry::new(),
            )
            .expect("text run should shape")
    }

    fn text_run_visual_center(run: &sui_text::TextRun) -> f32 {
        let layout = text_run_layout(run);
        let line = layout
            .lines()
            .first()
            .expect("text run should contain a line");
        run.rect.y() + line.baseline + optical_visual_center(layout.measurement())
    }

    fn assert_tall_body_text_centered(
        output: &RenderOutput,
        text: &str,
        theme: DefaultTheme,
        expected_center_y: f32,
    ) {
        let run = text_run_for(output, text);
        let layout = shaped_text_layout_for(output, text);

        assert_eq!(run.style.font_size, theme.typography.body_font_size);
        assert_eq!(run.style.line_height, theme.typography.body_line_height);
        assert!(
            (text_run_visual_center(&run) - expected_center_y).abs() < 0.75,
            "{text} visual center should match {expected_center_y}; rect={:?}, measurement={:?}",
            run.rect,
            layout.measurement()
        );
    }

    fn layer_descriptor_for(
        output: &RenderOutput,
        owner: WidgetId,
    ) -> Option<SceneLayerDescriptor> {
        let mut descriptor = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.widget_id() == owner {
                descriptor = Some(layer.descriptor.clone());
            }
        });
        descriptor
    }

    fn overlay_layer_descriptor(output: &RenderOutput) -> Option<SceneLayerDescriptor> {
        let mut descriptor = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                descriptor = Some(layer.descriptor.clone());
            }
        });
        descriptor
    }

    fn overlay_layer_owner(output: &RenderOutput) -> Option<WidgetId> {
        let mut owner = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                owner = Some(layer.widget_id());
            }
        });
        owner
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }

        Event::Pointer(PointerEvent {
            pointer_id: 1,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Primary),
            buttons,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn secondary_pointer_down(position: Point) -> Event {
        let mut buttons = PointerButtons::NONE;
        buttons.insert(PointerButton::Secondary);
        Event::Pointer(PointerEvent {
            pointer_id: 2,
            kind: PointerEventKind::Down,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Secondary),
            buttons,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn command_key(key: &str) -> Event {
        let mut event = KeyboardEvent::new(key, KeyState::Pressed);
        event.modifiers.control = true;
        Event::Keyboard(event)
    }

    fn key_without_text(key: &str) -> Event {
        let mut event = KeyboardEvent::new(key, KeyState::Pressed);
        event.text = None;
        Event::Keyboard(event)
    }

    fn handle_ready_events(runtime: &mut Runtime) -> Result<usize> {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (ready_window, event) in ready {
            runtime.handle_event(ready_window, event)?;
        }
        Ok(count)
    }

    #[test]
    fn label_paints_text_and_exposes_text_semantics() {
        let output = render(Label::new("Hello SUI").color(Color::rgba(0.8, 0.9, 1.0, 1.0)));

        assert!(output.frame.viewport.height >= 16.0);
        assert!(matches!(
            output.frame.scene.commands()[0],
            SceneCommand::DrawShapedText(_)
        ));
        assert_eq!(output.semantics[0].role, SemanticsRole::Text);
        assert_eq!(output.semantics[0].name.as_deref(), Some("Hello SUI"));
    }

    #[test]
    fn icon_color_when_uses_external_color() -> Result<()> {
        let color = Rc::new(RefCell::new(Color::rgba(0.2, 0.5, 0.9, 1.0)));
        let reader = Rc::clone(&color);
        let (mut runtime, window_id) = build_runtime(
            Icon::new(IconGlyph::Sparkles)
                .size(24.0)
                .label("Agent")
                .color_when(move || *reader.borrow()),
        );

        let output = runtime.render(window_id)?;
        assert!(
            lucide_strokes(&output)
                .iter()
                .any(|(_, stroke_color, _)| *stroke_color == Color::rgba(0.2, 0.5, 0.9, 1.0))
        );

        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Image
                    && node.name.as_deref() == Some("Agent"))
        );
        Ok(())
    }

    #[test]
    fn selectable_label_syncs_selected_text_to_scope() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) =
            build_runtime(Label::new("Hello SUI").selectable(selection.clone()));
        let output = runtime.render(window_id)?;
        let label = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Hello SUI")
            })
            .expect("selectable label semantics should exist");
        let center = Point::new(
            label.bounds.x() + 4.0,
            label.bounds.y() + label.bounds.height() * 0.5,
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, center, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, center, false),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;

        assert_eq!(selection.selected_text().as_deref(), Some("Hello SUI"));
        Ok(())
    }

    #[test]
    fn selectable_label_copies_with_hotkey_command_and_semantics() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(Label::new("Hello SUI").selectable(selection));
        let output = runtime.render(window_id)?;
        let label = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Text && node.name.as_deref() == Some("Hello SUI")
            })
            .expect("selectable label semantics should exist");
        let label_id = label.id;
        let center = Point::new(
            label.bounds.x() + 4.0,
            label.bounds.y() + label.bounds.height() * 0.5,
        );

        runtime.handle_event(window_id, secondary_pointer_down(center))?;
        let focused = runtime.render(window_id)?;
        let label = focused
            .semantics
            .iter()
            .find(|node| node.id == label_id)
            .expect("selectable label semantics should remain present");
        assert!(
            label.state.focused,
            "right click should focus selectable text"
        );
        assert!(label.actions.contains(&SemanticsAction::Copy));

        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("c"))?;
        assert_eq!(runtime.clipboard().text().as_deref(), Some("Hello SUI"));

        runtime.clipboard().set_text("");
        runtime.handle_command(
            CommandTarget::FocusedWidget(window_id),
            CommandDelivery::Directed,
            TEXT_COMMAND,
            TextCommand::Copy,
        );
        assert_eq!(runtime.clipboard().text().as_deref(), Some("Hello SUI"));

        runtime.clipboard().set_text("");
        assert!(runtime.handle_semantics_action(
            window_id,
            label_id,
            SemanticsActionRequest::Copy,
        )?);
        assert_eq!(runtime.clipboard().text().as_deref(), Some("Hello SUI"));
        Ok(())
    }

    #[test]
    fn selectable_labels_sharing_scope_replace_previous_selection() -> Result<()> {
        let selection = SelectionScope::new();
        let root = Stack::vertical()
            .spacing(4.0)
            .with_child(Label::new("First").selectable(selection.clone()))
            .with_child(Label::new("Second").selectable(selection.clone()));
        let (mut runtime, window_id) = build_runtime(root);
        let output = runtime.render(window_id)?;
        let first = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("First"))
            .expect("first label semantics should exist")
            .bounds;
        let second = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Second"))
            .expect("second label semantics should exist")
            .bounds;

        let first_center = Point::new(first.x() + 2.0, first.y() + first.height() * 0.5);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, first_center, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, first_center, false),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        assert_eq!(selection.selected_text().as_deref(), Some("First"));

        let second_center = Point::new(second.x() + 2.0, second.y() + second.height() * 0.5);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, second_center, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, second_center, false),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;

        assert_eq!(selection.selected_text().as_deref(), Some("Second"));
        Ok(())
    }

    #[test]
    fn label_dynamic_text_updates_named_semantic_value() -> Result<()> {
        let text = Rc::new(RefCell::new("Zoom 25%".to_string()));
        let text_reader = Rc::clone(&text);
        let (mut runtime, window_id) = build_runtime(
            Label::dynamic("Zoom --", move || text_reader.borrow().clone())
                .semantic_name("Zoom level"),
        );

        let output = runtime.render(window_id)?;
        assert_eq!(output.semantics[0].role, SemanticsRole::Text);
        assert_eq!(output.semantics[0].name.as_deref(), Some("Zoom level"));
        assert_eq!(
            output.semantics[0].value,
            Some(SemanticsValue::Text("Zoom 25%".to_string()))
        );

        *text.borrow_mut() = "Zoom 50%".to_string();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 80.0))),
        )?;
        let output = runtime.render(window_id)?;
        assert_eq!(
            output.semantics[0].value,
            Some(SemanticsValue::Text("Zoom 50%".to_string()))
        );
        Ok(())
    }

    #[test]
    fn label_observable_text_invalidates_without_window_refresh() -> Result<()> {
        let text = Signal::named("zoom_label", "Zoom 25%".to_string());
        let (mut runtime, window_id) = build_runtime(
            Label::new("Zoom --")
                .text_from(text.clone())
                .semantic_name("Zoom level"),
        );

        let output = runtime.render(window_id)?;
        assert_eq!(
            output.semantics[0].value,
            Some(SemanticsValue::Text("Zoom 25%".to_string()))
        );

        assert!(text.set("Zoom 50%".to_string()));
        let output = runtime.render(window_id)?;
        assert_eq!(
            output.semantics[0].value,
            Some(SemanticsValue::Text("Zoom 50%".to_string()))
        );
        assert!(
            output
                .diagnostics
                .reactive_invalidations
                .iter()
                .any(|sample| sample.source_name == "zoom_label")
        );
        Ok(())
    }

    #[test]
    fn link_exposes_link_semantics_and_activates_on_click() -> Result<()> {
        let opened = Rc::new(RefCell::new(None::<String>));
        let opened_for_link = Rc::clone(&opened);
        let (mut runtime, window_id) = build_runtime(
            Link::new("Open login", "https://example.test/device").on_open(move |url| {
                *opened_for_link.borrow_mut() = Some(url.to_string());
            }),
        );

        let output = runtime.render(window_id)?;
        let link = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Link)
            .expect("link semantics should be present");
        assert_eq!(link.name.as_deref(), Some("Open login"));
        assert_eq!(
            link.value,
            Some(SemanticsValue::Text(
                "https://example.test/device".to_string()
            ))
        );
        assert!(link.actions.contains(&SemanticsAction::Focus));
        assert!(link.actions.contains(&SemanticsAction::Activate));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;
        assert_eq!(
            opened.borrow().as_deref(),
            Some("https://example.test/device")
        );
        Ok(())
    }

    #[test]
    fn link_with_empty_url_collapses_out_of_semantics() {
        let output = render(
            SizedBox::new()
                .width(160.0)
                .height(24.0)
                .with_child(Link::new("Open login", "")),
        );

        assert!(
            output
                .semantics
                .iter()
                .all(|node| node.role != SemanticsRole::Link)
        );
    }

    #[test]
    fn label_measures_wrapped_height_when_width_is_constrained() {
        let output = render(SizedBox::new().width(96.0).with_child(Label::new(
            "This label should wrap onto multiple lines when its layout width is constrained.",
        )));

        assert!(output.frame.viewport.height > DefaultTheme::default().typography.body_line_height);
    }

    #[test]
    fn label_measures_explicit_multiline_text_height() {
        let text = "First line\nSecond line";
        let output = render(Label::new(text));
        let layout = shaped_text_layout_for(&output, text);
        let label = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(text))
            .expect("label semantics should exist");

        assert_eq!(layout.lines().len(), 2);
        assert!(label.bounds.height() >= layout.measurement().height - 0.01);
        assert!(label.bounds.height() > DefaultTheme::default().typography.body_line_height);
    }

    #[test]
    fn label_centers_explicit_multiline_text_as_a_block() {
        let text = "First line\nSecond line";
        let output = render(
            SizedBox::new()
                .width(180.0)
                .height(96.0)
                .with_child(Label::new(text)),
        );
        let shaped = first_shaped_text(&output);
        let layout = shaped_text_layout_for(&output, text);
        let expected_origin_y =
            (output.frame.viewport.height - layout.measurement().height).max(0.0) * 0.5;

        assert_eq!(layout.lines().len(), 2);
        assert!(
            (shaped.origin.y - expected_origin_y).abs() < 0.75,
            "multiline label origin should block-center at {expected_origin_y}, got {}",
            shaped.origin.y
        );
    }

    #[test]
    fn label_visual_center_matches_tall_allocation_center() {
        let output = render(
            SizedBox::new()
                .width(160.0)
                .height(48.0)
                .with_child(Label::new("Body")),
        );
        let text = text_run_for(&output, "Body");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("label text should shape");
        let line = layout
            .lines()
            .first()
            .expect("label text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let allocation_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - allocation_center).abs() < 0.75);
    }

    #[test]
    fn label_preserves_tall_measurement_in_compact_line_box() {
        let mut style = DefaultTheme::default().body_text_style();
        style.font_size = 30.0;
        style.line_height = 10.0;

        let output = render(
            SizedBox::new()
                .width(160.0)
                .height(48.0)
                .with_child(Label::new("Body").style(style.clone())),
        );
        let text = text_run_for(&output, "Body");
        let layout = shaped_text_layout_for(&output, "Body");
        let allocation_center = output.frame.viewport.height * 0.5;

        assert_eq!(text.style.font_size, style.font_size);
        assert_eq!(text.style.line_height, style.line_height);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text_run_visual_center(&text) - allocation_center).abs() < 0.75);
    }

    #[test]
    fn label_window_option_keeps_geometric_label_centered() {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(160.0)
                .height(48.0)
                .with_child(Label::new("Body")),
        );
        set_window_render_options(
            window_id,
            WindowRenderOptions::new(true, 1.0).with_optical_vertical_text_alignment_enabled(false),
        );
        let output = runtime.render(window_id).unwrap();
        clear_window_render_options(window_id);
        let text = text_run_for(&output, "Body");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("label text should shape");
        let line = layout
            .lines()
            .first()
            .expect("label text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + visual_center(layout.measurement(), false);
        let allocation_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - allocation_center).abs() < 0.75);
    }

    #[test]
    fn button_activates_on_primary_pointer_click() -> Result<()> {
        let activations = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&activations);
        let (mut runtime, window_id) = build_runtime(Button::new("Save").on_press(move || {
            *on_press.borrow_mut() += 1;
        }));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        assert_eq!(*activations.borrow(), 1);

        let output = runtime.render(window_id)?;
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .unwrap();
        assert_eq!(button.name.as_deref(), Some("Save"));
        Ok(())
    }

    #[test]
    fn button_releases_primary_press_on_unlabelled_pointer_up() -> Result<()> {
        let activations = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&activations);
        let (mut runtime, window_id) = build_runtime(Button::new("Save").on_press(move || {
            *on_press.borrow_mut() += 1;
        }));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        let mut up = primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false);
        if let Event::Pointer(pointer) = &mut up {
            pointer.button = None;
        }
        runtime.handle_event(window_id, up)?;

        assert_eq!(*activations.borrow(), 1);
        Ok(())
    }

    #[test]
    fn disabled_button_exposes_semantics_and_ignores_activation() -> Result<()> {
        let activations = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&activations);
        let (mut runtime, window_id) = build_runtime(
            Button::new("Save")
                .enabled(false)
                .on_press(move || *on_press.borrow_mut() += 1),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        assert_eq!(*activations.borrow(), 0);
        let output = runtime.render(window_id)?;
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("button semantics should be present");
        assert!(button.state.disabled);
        assert!(button.actions.is_empty());
        Ok(())
    }

    #[test]
    fn button_semantic_name_and_description_override_visible_label() {
        let output = render(
            Button::new("Cancel")
                .semantic_name("Cancel report export")
                .description("Stop the active report export task"),
        );
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("button semantics should be present");
        assert_eq!(button.name.as_deref(), Some("Cancel report export"));
        assert_eq!(
            button.description.as_deref(),
            Some("Stop the active report export task")
        );
        let text = text_run_for(&output, "Cancel");
        assert_eq!(text.text, "Cancel");
    }

    #[test]
    fn icon_button_description_is_exposed_to_semantics() {
        let output = render(
            IconButton::new(IconGlyph::Close, "Close activity")
                .description("Hide the runtime activity panel"),
        );
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("icon button semantics should be present");
        assert_eq!(button.name.as_deref(), Some("Close activity"));
        assert_eq!(
            button.description.as_deref(),
            Some("Hide the runtime activity panel")
        );
    }

    #[test]
    fn button_appearance_and_tone_resolve_without_theme_remapping() {
        let theme = DefaultTheme::default();
        let danger = theme.semantic_tone_color(SemanticTone::Danger);
        let outline = Button::new("Delete")
            .theme(theme)
            .appearance(ButtonAppearance::Outline)
            .tone(SemanticTone::Danger)
            .resolved_visuals(false);

        assert_eq!(outline.background, Color::TRANSPARENT);
        assert_eq!(outline.border, danger.with_alpha(0.72));
        assert_eq!(outline.label_color, danger);

        let tonal = Button::new("Retry")
            .theme(theme)
            .appearance(ButtonAppearance::Tonal)
            .tone(SemanticTone::Warning)
            .resolved_visuals(false);
        let (soft_fill, soft_text) = theme.semantic_tone_soft_colors(SemanticTone::Warning);
        assert_eq!(tonal.background, soft_fill);
        assert_eq!(tonal.label_color, soft_text);
    }

    #[test]
    fn button_defaults_are_quiet_and_explicit_action_helpers_are_filled() {
        let theme = DefaultTheme::default();
        assert_eq!(ButtonAppearance::default(), ButtonAppearance::Tonal);

        let ordinary = Button::new("More options").theme(theme);
        assert_eq!(ordinary.appearance, ButtonAppearance::Tonal);
        assert_eq!(ordinary.tone, SemanticTone::Neutral);
        let (neutral_fill, neutral_text) = theme.semantic_tone_soft_colors(SemanticTone::Neutral);
        let ordinary_visuals = ordinary.resolved_visuals(false);
        assert_eq!(ordinary_visuals.background, neutral_fill);
        assert_eq!(ordinary_visuals.label_color, neutral_text);

        let primary = Button::primary("Save").theme(theme);
        assert_eq!(primary.appearance, ButtonAppearance::Filled);
        assert_eq!(primary.tone, SemanticTone::Accent);
        assert_eq!(
            primary.resolved_visuals(false).background,
            theme.palette.accent
        );

        let danger = Button::danger("Delete").theme(theme);
        assert_eq!(danger.appearance, ButtonAppearance::Filled);
        assert_eq!(danger.tone, SemanticTone::Danger);
        assert_eq!(
            danger.resolved_visuals(false).background,
            theme.semantic_tone_color(SemanticTone::Danger)
        );

        let overridden = Button::new("Retry")
            .primary_action()
            .appearance(ButtonAppearance::Ghost)
            .tone(SemanticTone::Warning);
        assert_eq!(overridden.appearance, ButtonAppearance::Ghost);
        assert_eq!(overridden.tone, SemanticTone::Warning);
    }

    #[test]
    fn choice_controls_are_plain_by_default_and_framed_on_request() {
        let theme = DefaultTheme::default();
        let checkbox = Checkbox::new("Visible");
        let switch = Switch::new("Wifi");
        let radio = RadioButton::new("Automatic");
        assert_eq!(checkbox.appearance, ChoiceAppearance::Plain);
        assert_eq!(switch.appearance, ChoiceAppearance::Plain);
        assert_eq!(radio.appearance, ChoiceAppearance::Plain);

        for output in [render(checkbox), render(switch), render(radio)] {
            assert!(
                !solid_fill_colors(&output).contains(&theme.palette.control),
                "plain choice rows should not paint the permanent control fill"
            );
        }

        for output in [
            render(Checkbox::new("Visible").framed()),
            render(Switch::new("Wifi").appearance(ChoiceAppearance::Framed)),
            render(RadioButton::new("Automatic").framed()),
        ] {
            assert!(
                solid_fill_colors(&output).contains(&theme.palette.control),
                "framed choice rows should preserve the control fill"
            );
        }
    }

    #[test]
    fn plain_choice_row_reveals_soft_hover_wash() -> Result<()> {
        let theme = DefaultTheme::default();
        let expected_wash = super::choice_frame_visuals(
            &theme,
            ChoiceAppearance::Plain,
            theme.palette.control,
            theme.palette.border,
            theme.interaction.hover_blend,
            0.0,
            0.0,
        )
        .background;
        let (mut runtime, window_id) = build_runtime(Checkbox::new("Visible"));

        let rest = runtime.render(window_id)?;
        assert!(!solid_fill_colors(&rest).contains(&expected_wash));
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(10.0, 10.0), false),
        )?;
        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);

        let hovered = runtime.render(window_id)?;
        assert!(solid_fill_colors(&hovered).contains(&expected_wash));
        Ok(())
    }

    #[test]
    fn icon_button_appearance_and_tone_use_semantic_tokens() {
        let theme = DefaultTheme::default();
        let output = render(IconButtonPaintFixture {
            theme,
            style: IconButtonPaint::new()
                .appearance(ButtonAppearance::Ghost)
                .tone(SemanticTone::Danger)
                .hovered(true),
        });
        let (soft_fill, _) = theme.semantic_tone_soft_colors(SemanticTone::Danger);
        let expected =
            super::mix_color(Color::TRANSPARENT, soft_fill, theme.interaction.hover_blend);
        assert_color_approx_eq(solid_fill_colors(&output)[0], expected);
        assert!(
            non_lucide_stroke_colors(&output)
                .iter()
                .all(|color| color.alpha <= f32::EPSILON)
        );
    }

    #[test]
    fn bare_text_editors_leave_chrome_to_their_container() {
        let framed = render(TextArea::new("Notes").value("Body"));
        let bare = render(
            TextArea::new("Notes")
                .appearance(FieldAppearance::Bare)
                .value("Body"),
        );
        assert!(!solid_fill_colors(&framed).is_empty());
        assert!(!solid_stroke_colors(&framed).is_empty());
        assert!(solid_fill_colors(&bare).is_empty());
        assert!(solid_stroke_colors(&bare).is_empty());

        let bare_input = render(TextInput::new("Search").bare().value("query"));
        assert!(solid_fill_colors(&bare_input).is_empty());
        assert!(solid_stroke_colors(&bare_input).is_empty());
    }

    #[test]
    fn disabled_button_label_uses_disabled_muted_text() {
        let theme = DefaultTheme::default();
        let output = render(Button::new("Save").enabled(false).theme(theme));
        let text = text_run_for(&output, "Save");

        assert_eq!(
            text.style.color,
            theme
                .palette
                .text_muted
                .with_alpha(theme.interaction.disabled_content_opacity)
        );
    }

    #[test]
    fn button_cached_label_uses_visual_color_without_changing_layout_metrics() {
        let text_color = Color::rgba(0.18, 0.42, 0.91, 1.0);
        let output = render(Button::new("Apply").text_style(TextStyle {
            font_size: 17.0,
            line_height: 29.0,
            color: text_color,
            ..TextStyle::default()
        }));

        let shaped = first_shaped_text(&output);
        let layout = shaped
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("button label layout should resolve");

        assert_eq!(layout.style().font_size, 17.0);
        assert_eq!(layout.style().line_height, 29.0);
        assert_eq!(layout.style().color, text_color);
        assert_eq!(shaped.color_override, Some(text_color));
    }

    #[test]
    fn button_with_icon_keeps_label_semantics_and_paints_icon() {
        let plain = render(Button::new("Export").min_width(96.0));
        let with_icon = render_isolated(
            Button::new("Export")
                .icon(IconGlyph::Download)
                .min_width(96.0),
        );

        let button = with_icon
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("button semantics should exist");
        assert_eq!(button.name.as_deref(), Some("Export"));
        assert!(
            with_icon.frame.scene.commands().len() > plain.frame.scene.commands().len(),
            "icon button should add visible icon ink"
        );
        let icon_rect = first_lucide_icon_rect(&with_icon);
        let text = text_run_for(&with_icon, "Export");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("button label should shape");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let label_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let icon_center = icon_rect.y() + (icon_rect.height() * 0.5);

        assert!((label_visual_center - icon_center).abs() < 0.75);
    }

    #[test]
    fn button_with_icon_preserves_tall_label_measurement_and_icon_centering() {
        let text_style = TextStyle {
            font_size: 28.0,
            line_height: 12.0,
            color: Color::rgba(0.95, 0.98, 1.0, 1.0),
            ..TextStyle::default()
        };
        let output = render_isolated(
            Button::new("Export")
                .icon(IconGlyph::Download)
                .text_style(text_style.clone())
                .min_width(220.0)
                .min_height(64.0),
        );
        let icon_rect = first_lucide_icon_rect(&output);
        let text = text_run_for(&output, "Export");
        let layout = shaped_text_layout_for(&output, "Export");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let label_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let icon_center = icon_rect.y() + (icon_rect.height() * 0.5);
        let control_center = output.frame.viewport.height * 0.5;

        assert_eq!(text.style.font_size, text_style.font_size);
        assert_eq!(text.style.line_height, text_style.line_height);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((label_visual_center - icon_center).abs() < 0.75);
        assert!((label_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn disabled_icon_button_exposes_semantics_and_ignores_activation() -> Result<()> {
        let activations = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&activations);
        let (mut runtime, window_id) = build_runtime(
            IconButton::new(IconGlyph::Add, "Add")
                .enabled(false)
                .on_press(move || *on_press.borrow_mut() += 1),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        assert_eq!(*activations.borrow(), 0);
        let output = runtime.render(window_id)?;
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("icon button semantics should be present");
        assert!(button.state.disabled);
        assert!(button.actions.is_empty());
        Ok(())
    }

    #[test]
    fn density_modes_resize_core_controls() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        assert!(
            render(Button::new("Density").theme(compact))
                .frame
                .viewport
                .height
                < render(Button::new("Density").theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(IconButton::new(IconGlyph::Search, "Search").theme(compact))
                .frame
                .viewport
                .width
                < render(IconButton::new(IconGlyph::Search, "Search").theme(touch))
                    .frame
                    .viewport
                    .width
        );
        assert!(
            render(TextInput::new("Name").theme(compact))
                .frame
                .viewport
                .height
                < render(TextInput::new("Name").theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(TextArea::new("Notes").theme(compact))
                .frame
                .viewport
                .height
                < render(TextArea::new("Notes").theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(Checkbox::new("Visible").theme(compact))
                .frame
                .viewport
                .height
                < render(Checkbox::new("Visible").theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(Switch::new("Enabled").theme(compact))
                .frame
                .viewport
                .height
                < render(Switch::new("Enabled").theme(touch))
                    .frame
                    .viewport
                    .height
        );
        assert!(
            render(Slider::new("Opacity").theme(compact))
                .frame
                .viewport
                .height
                < render(Slider::new("Opacity").theme(touch))
                    .frame
                    .viewport
                    .height
        );
    }

    #[test]
    fn button_hover_animation_advances_over_multiple_frames() -> Result<()> {
        let theme = DefaultTheme::default();
        let rest_background = super::semantic_button_visuals(
            &theme,
            ButtonAppearance::Tonal,
            SemanticTone::Neutral,
            true,
            0.0,
            0.0,
        )
        .background;
        let settled_background = super::semantic_button_visuals(
            &theme,
            ButtonAppearance::Tonal,
            SemanticTone::Neutral,
            true,
            1.0,
            0.0,
        )
        .background;
        let (mut runtime, window_id) = build_runtime(Button::new("Hover"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let mid_background = solid_fill_colors(&mid)[0];
        assert_ne!(mid_background, rest_background);
        assert_ne!(mid_background, settled_background);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let end_background = solid_fill_colors(&end)[0];
        assert_eq!(end_background, settled_background);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn button_press_changes_color_without_moving_content() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.interaction.pressed_offset = 7.0;
        let (mut runtime, window_id) =
            build_runtime(Button::new("Press").icon(IconGlyph::Brush).theme(theme));
        let rest = runtime.render(window_id)?;
        let rest_background = solid_fill_colors(&rest)[0];
        let rest_label = text_run_for(&rest, "Press").rect;
        let rest_icon = first_lucide_icon_rect(&rest);
        let point = Point::new(12.0, 12.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )?;
        runtime.tick(press_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);

        let pressed = runtime.render(window_id)?;
        assert_ne!(solid_fill_colors(&pressed)[0], rest_background);
        assert_eq!(text_run_for(&pressed, "Press").rect, rest_label);
        assert_eq!(first_lucide_icon_rect(&pressed), rest_icon);
        Ok(())
    }

    #[test]
    fn switch_thumb_animation_tracks_progress_and_completion() -> Result<()> {
        let theme = slow_toggle_theme();
        let toggle_time = theme.motion.toggle_duration();
        let (mut runtime, window_id) = build_runtime(Switch::new("Wifi").theme(theme));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(toggle_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(toggle_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);

        let output = runtime.render(window_id)?;
        let switch = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Switch)
            .unwrap();
        assert_eq!(switch.state.checked, Some(sui_core::ToggleState::Checked));
        Ok(())
    }

    #[test]
    fn switch_track_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_time = hover_duration();
        let press_time = press_duration();
        let (mut runtime, window_id) = build_runtime(Switch::new("Wifi").on(true));

        let _ = runtime.render(window_id)?;
        let point = Point::new(12.0, 12.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, point, false),
        )?;

        runtime.tick(hover_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        let mid_hover_track = solid_fill_colors(&mid_hover)[1];
        let settled_hover_track = super::mix_color(
            theme.palette.accent,
            theme.palette.accent_hover,
            theme.interaction.hover_blend,
        );
        assert_ne!(mid_hover_track, theme.palette.accent);
        assert_ne!(mid_hover_track, settled_hover_track);

        runtime.tick(hover_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let hover = runtime.render(window_id)?;
        assert_eq!(solid_fill_colors(&hover)[1], settled_hover_track);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )?;

        runtime.tick(hover_time + (press_time * 0.5));
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        let mid_press_track = solid_fill_colors(&mid_press)[1];
        let settled_press_track = super::mix_color(
            settled_hover_track,
            theme.palette.accent_pressed,
            theme.interaction.pressed_blend,
        );
        assert_ne!(mid_press_track, settled_hover_track);
        assert_ne!(mid_press_track, settled_press_track);

        runtime.tick(hover_time + press_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let press = runtime.render(window_id)?;
        assert_eq!(solid_fill_colors(&press)[1], settled_press_track);
        Ok(())
    }

    #[test]
    fn slider_thumb_hover_animation_requests_followup_frames_until_complete() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(Slider::new("Gain"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(32.0, 16.0), false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn select_header_hover_animation_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(Select::new("Mode").placeholder("Choose mode").options([
                "Automatic",
                "Linear",
                "Gamma",
            ]));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        // Mesh selects are dressed fields: the well stays on the field token
        // while hover animates the border toward border_hover.
        assert!(solid_fill_colors(&mid).contains(&theme.palette.field));
        let mid_strokes = solid_stroke_colors(&mid);
        assert!(!mid_strokes.contains(&theme.palette.border));
        assert!(!mid_strokes.contains(&theme.palette.border_hover));
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        assert!(solid_fill_colors(&end).contains(&theme.palette.field));
        assert!(solid_stroke_colors(&end).contains(&theme.palette.border_hover));
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn expanded_select_option_hover_animation_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Automatic", "Linear", "Gamma"])
                .selected(2),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let entrance_time = entrance_duration();
        let hover_time = hover_duration();
        runtime.tick(entrance_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let expanded = runtime.render(window_id)?;
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let menu = overlay_layer_descriptor(&expanded).expect("select menu overlay present");
        let menu_owner = overlay_layer_owner(&expanded).expect("select menu overlay owner");
        let option_point = Point::new(
            menu.bounds.x() + 20.0,
            menu.bounds.y() + (select.bounds.height() * 0.5),
        );
        let menu_node = runtime
            .widget_graph(window_id)?
            .nodes
            .into_iter()
            .find(|node| node.id == menu_owner)
            .expect("menu surface in widget graph");
        assert!(
            menu_node.geometry.input_bounds.contains(option_point),
            "option point {option_point:?} should hit menu input bounds {:?}",
            menu_node.geometry.input_bounds
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, option_point, false),
        )?;

        runtime.tick(entrance_time + (hover_time * 0.5));
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid = runtime.render(window_id)?;
        let mid_fills = solid_fill_colors(&mid);
        assert!(
            !mid_fills.contains(&theme.palette.control_hover),
            "expanded select option hover should not snap directly to the settled hover token"
        );
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        // Allow a tiny margin because opening the managed overlay can schedule
        // an independent focus-frame at the same timestamp as the menu reveal.
        runtime.tick(entrance_time + hover_time + 0.001);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let settled = runtime.render(window_id)?;
        let settled_fills = solid_fill_colors(&settled);
        assert!(
            settled_fills.contains(&theme.palette.control_hover),
            "expanded select option hover should settle to the theme hover token; fills={settled_fills:?}, expected={:?}",
            theme.palette.control_hover
        );
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn number_input_stepper_press_animation_uses_theme_motion() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(180.0)
                .with_child(NumberInput::new("Gamma").value(1.0)),
        );

        let initial = runtime.render(window_id)?;
        let spin = initial
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("number input semantics present")
            .bounds;
        let stepper_point = Point::new(spin.max_x() - 8.0, spin.y() + (spin.height() * 0.25));
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, stepper_point, false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, stepper_point, true),
        )?;
        let press_mid_time = (hover_duration() * 0.5) + (press_duration() * 0.5);
        runtime.tick(press_mid_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, stepper_point, false),
        )?;
        runtime.tick(press_mid_time + focus_duration() + press_duration() + 0.01);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);

        let output = runtime.render(window_id)?;
        let spin = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("number input semantics present after stepper press");
        assert_eq!(
            spin.value,
            Some(SemanticsValue::Range {
                value: 2.0,
                min: f64::NEG_INFINITY,
                max: f64::INFINITY,
            })
        );
        assert_eq!(spin.numeric_step, Some(1.0));
        Ok(())
    }

    #[test]
    fn text_input_hover_animation_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(TextInput::new("Name").placeholder("Type a name"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        // The light field well lifts toward the card surface while the border
        // strengthens, without snapping either transition.
        let mid_background = solid_fill_colors(&mid)[0];
        assert_ne!(mid_background, theme.palette.field);
        assert_ne!(mid_background, theme.palette.surface);
        let mid_strokes = solid_stroke_colors(&mid);
        assert!(!mid_strokes.contains(&theme.palette.border));
        assert!(!mid_strokes.contains(&theme.palette.border_hover));
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        assert_eq!(
            solid_fill_colors(&end)[0],
            super::mix_color(
                theme.palette.field,
                theme.palette.surface,
                theme.interaction.hover_blend,
            )
        );
        assert!(solid_stroke_colors(&end).contains(&theme.palette.border_hover));
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn text_area_hover_animation_uses_theme_motion() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(TextArea::new("Notes").placeholder("Write notes"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(hover_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    struct IconButtonPaintFixture {
        theme: DefaultTheme,
        style: IconButtonPaint,
    }

    impl Widget for IconButtonPaintFixture {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(28.0, 28.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            paint_icon_button(ctx, &self.theme, ctx.bounds(), IconGlyph::Close, self.style);
        }
    }

    #[test]
    fn icon_button_paint_matches_widget_visual_states() {
        let theme = DefaultTheme::default();
        let output = render(IconButtonPaintFixture {
            theme,
            style: IconButtonPaint::new()
                .hovered(true)
                .selected(true)
                .icon_size(16.0),
        });

        let selected_base = super::mix_color(
            theme.palette.control,
            theme.palette.accent,
            theme.interaction.selected_blend,
        );
        let selected_hover = super::mix_color(selected_base, theme.palette.accent_hover, 0.18);
        assert_color_approx_eq(
            solid_fill_colors(&output)[0],
            super::mix_color(selected_base, selected_hover, theme.interaction.hover_blend),
        );
        assert!(!lucide_strokes(&output).is_empty());
    }

    #[test]
    fn icon_button_press_changes_color_without_moving_icon() {
        let mut theme = DefaultTheme::default();
        theme.interaction.pressed_offset = 7.0;
        let rest = render(IconButtonPaintFixture {
            theme,
            style: IconButtonPaint::new().icon_size(16.0),
        });
        let pressed = render(IconButtonPaintFixture {
            theme,
            style: IconButtonPaint::new().pressed(true).icon_size(16.0),
        });

        assert_ne!(solid_fill_colors(&pressed)[0], solid_fill_colors(&rest)[0]);
        assert_eq!(
            first_lucide_icon_rect(&pressed),
            first_lucide_icon_rect(&rest)
        );
    }

    #[test]
    fn icon_button_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_time = hover_duration();
        let press_time = press_duration();
        let (mut runtime, window_id) = build_runtime(IconButton::new(IconGlyph::Brush, "Brush"));

        let _ = runtime.render(window_id)?;
        let point = Point::new(12.0, 12.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, point, false),
        )?;

        runtime.tick(hover_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        let mid_hover_background = solid_fill_colors(&mid_hover)[0];
        let settled_hover_background = super::mix_color(
            theme.palette.control,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        assert_ne!(mid_hover_background, theme.palette.control);
        assert_ne!(mid_hover_background, settled_hover_background);

        runtime.tick(hover_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let hover = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&hover)[0], settled_hover_background);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )?;

        runtime.tick(hover_time + (press_time * 0.5));
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        let mid_press_background = solid_fill_colors(&mid_press)[0];
        let settled_press_background = super::mix_color(
            settled_hover_background,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        assert_ne!(mid_press_background, settled_hover_background);
        assert_ne!(mid_press_background, settled_press_background);

        runtime.tick(hover_time + press_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let press = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&press)[0], settled_press_background);
        Ok(())
    }

    #[test]
    fn icon_button_pressed_animation_decays_after_release() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(super::IconButton::new(super::IconGlyph::Add, "Add"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(press_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let mid_background = solid_fill_colors(&mid)[0];
        assert_ne!(mid_background, theme.palette.control_active);
        assert_ne!(mid_background, theme.palette.control_hover);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(hover_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let end_fills = solid_fill_colors(&end);
        assert_ne!(end_fills, solid_fill_colors(&mid));
        assert!(!end_fills.contains(&theme.palette.control_active));
        if runtime.next_wakeup_time(window_id)?.is_some() {
            runtime.tick(focus_duration());
            assert_eq!(handle_ready_events(&mut runtime)?, 1);
        }
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn icon_button_selected_state_is_exposed_to_semantics() {
        let output =
            render(super::IconButton::new(super::IconGlyph::Check, "Brush tool").selected(true));
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("icon button semantics should exist");

        assert_eq!(button.name.as_deref(), Some("Brush tool"));
        assert!(button.state.selected);
    }

    #[test]
    fn editor_icon_glyphs_paint_visible_ink() {
        for glyph in [
            IconGlyph::Undo,
            IconGlyph::Redo,
            IconGlyph::Brush,
            IconGlyph::Eraser,
            IconGlyph::PaintBucket,
            IconGlyph::Hand,
            IconGlyph::Lock,
            IconGlyph::Unlock,
            IconGlyph::Trash,
            IconGlyph::Download,
            IconGlyph::FitView,
            IconGlyph::ActualSize,
            IconGlyph::AudioLines,
            IconGlyph::Mic,
            IconGlyph::MicOff,
            IconGlyph::Camera,
            IconGlyph::CameraOff,
            IconGlyph::Video,
            IconGlyph::VideoOff,
            IconGlyph::Phone,
            IconGlyph::PhoneOff,
            IconGlyph::Monitor,
            IconGlyph::ScreenShare,
        ] {
            let output = render(IconButton::new(glyph, "Editor command"));
            assert!(
                output.frame.scene.commands().len() > 2,
                "{glyph:?} should paint more than the button frame"
            );
        }
    }

    #[test]
    fn icon_button_paints_lucide_native_path() {
        let glyph = IconGlyph::Brush;
        let handle = glyph.lucide_icon().handle();
        let output = render(IconButton::new(glyph, "Brush tool"));

        assert!(!output.frame.image_registry.contains(handle));
        assert!(
            !lucide_strokes(&output).is_empty(),
            "{glyph:?} should paint native Lucide path geometry"
        );
        assert!(
            !output.frame.scene.commands().iter().any(|command| matches!(
                command,
                SceneCommand::DrawImage { source, .. } if source.image == handle
            )),
            "{glyph:?} should bypass the raster image path"
        );
    }

    #[test]
    fn checkbox_check_indicator_animation_progresses_deterministically() -> Result<()> {
        let theme = slow_toggle_theme();
        let toggle_time = theme.motion.toggle_duration();
        let (mut runtime, window_id) = build_runtime(Checkbox::new("Subscribe").theme(theme));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 10.0), false),
        )?;

        runtime.tick(toggle_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let fills = solid_fill_colors(&mid);
        assert!(!fills.is_empty());
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(toggle_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let checkbox = end
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::CheckBox)
            .unwrap();
        assert_eq!(checkbox.state.checked, Some(sui_core::ToggleState::Checked));
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn checkbox_focus_border_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(Checkbox::new("Subscribe"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;

        runtime.tick(focus_duration() * 0.5);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid_focus = runtime.render(window_id)?;
        assert!(
            !solid_stroke_colors(&mid_focus).contains(&theme.palette.border_focus),
            "checkbox focus border should not snap to the settled focus border color"
        );

        runtime.tick(focus_duration());
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let settled_focus = runtime.render(window_id)?;
        assert!(
            solid_stroke_colors(&settled_focus).contains(&theme.palette.border_focus),
            "checkbox focus border should settle to the theme focus border color"
        );
        Ok(())
    }

    #[test]
    fn focused_control_ring_path_sits_outside_control_bounds() -> Result<()> {
        let theme = DefaultTheme::default();
        assert_eq!(theme.metrics.focus_ring_outset, 2.0);
        let (mut runtime, window_id) = build_runtime(Button::new("Save").theme(theme));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(16.0, 16.0), true),
        )?;
        runtime.tick(focus_duration());
        assert!(handle_ready_events(&mut runtime)? >= 1);

        let output = runtime.render(window_id)?;
        let button = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some("Save"))
            .expect("focused button semantics present");
        let focus_bounds = solid_stroke_path_bounds(&output, theme.palette.focus_ring);

        assert!(
            focus_bounds.len() == 1,
            "expected one focused control ring, got {focus_bounds:?}"
        );
        assert_rect_approx_eq(
            focus_bounds[0],
            button.bounds.inflate(
                theme.metrics.focus_ring_outset,
                theme.metrics.focus_ring_outset,
            ),
        );
        Ok(())
    }

    #[test]
    fn checkbox_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_time = hover_duration();
        let press_time = press_duration();
        let (mut runtime, window_id) = build_runtime(Checkbox::new("Subscribe").framed());

        let _ = runtime.render(window_id)?;
        let point = Point::new(10.0, 10.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, point, false),
        )?;

        runtime.tick(hover_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        let mid_hover_background = solid_fill_colors(&mid_hover)[0];
        let settled_hover_background = super::mix_color(
            theme.palette.control,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        assert_ne!(mid_hover_background, theme.palette.control);
        assert_ne!(mid_hover_background, settled_hover_background);

        runtime.tick(hover_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let hover = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&hover)[0], settled_hover_background);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )?;

        runtime.tick(hover_time + (press_time * 0.5));
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        let mid_press_background = solid_fill_colors(&mid_press)[0];
        let settled_press_background = super::mix_color(
            settled_hover_background,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        assert_ne!(mid_press_background, settled_hover_background);
        assert_ne!(mid_press_background, settled_press_background);

        runtime.tick(hover_time + press_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let press = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&press)[0], settled_press_background);
        Ok(())
    }

    #[test]
    fn radio_button_selection_animation_uses_theme_motion() -> Result<()> {
        let theme = slow_toggle_theme();
        let toggle_time = theme.motion.toggle_duration();
        let (mut runtime, window_id) = build_runtime(RadioButton::new("Manual").theme(theme));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 10.0), false),
        )?;

        runtime.tick(toggle_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(toggle_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let radio = end
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioButton)
            .unwrap();
        assert!(radio.state.selected);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn radio_button_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_time = hover_duration();
        let press_time = press_duration();
        let (mut runtime, window_id) = build_runtime(RadioButton::new("Manual").selected(true));

        let _ = runtime.render(window_id)?;
        let point = Point::new(10.0, 10.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, point, false),
        )?;

        runtime.tick(hover_time * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        let mid_hover_indicator = solid_fill_colors(&mid_hover)[1];
        let settled_hover_indicator = super::mix_color(
            theme.palette.accent,
            theme.palette.accent_hover,
            theme.interaction.hover_blend,
        );
        assert_ne!(mid_hover_indicator, theme.palette.accent);
        assert_ne!(mid_hover_indicator, settled_hover_indicator);

        runtime.tick(hover_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let hover = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&hover)[1], settled_hover_indicator);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, point, true),
        )?;

        runtime.tick(hover_time + (press_time * 0.5));
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        let mid_press_indicator = solid_fill_colors(&mid_press)[1];
        let settled_press_indicator = super::mix_color(
            settled_hover_indicator,
            theme.palette.accent_pressed,
            theme.interaction.pressed_blend,
        );
        assert_ne!(mid_press_indicator, settled_hover_indicator);
        assert_ne!(mid_press_indicator, settled_press_indicator);

        runtime.tick(hover_time + press_time);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let press = runtime.render(window_id)?;
        assert_color_approx_eq(solid_fill_colors(&press)[1], settled_press_indicator);
        Ok(())
    }

    #[test]
    fn radio_group_hover_press_and_selection_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_time = hover_duration();
        let press_time = press_duration();
        let toggle_time = toggle_duration();
        let (mut runtime, window_id) =
            build_runtime(RadioGroup::new("Mode").options(["Manual", "Automatic"]));

        let _ = runtime.render(window_id)?;
        let row_point = Point::new(10.0, 10.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, row_point, false),
        )?;

        runtime.tick(hover_time * 0.5);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid_hover = runtime.render(window_id)?;
        let mid_hover_background = solid_fill_colors(&mid_hover)[0];
        let settled_hover_background = super::mix_color(
            theme.palette.control,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        assert_ne!(mid_hover_background, theme.palette.control);
        assert_ne!(mid_hover_background, settled_hover_background);

        runtime.tick(hover_time);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let hover = runtime.render(window_id)?;
        assert_eq!(solid_fill_colors(&hover)[0], settled_hover_background);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, row_point, true),
        )?;
        runtime.tick(hover_time + (press_time * 0.5));
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid_press = runtime.render(window_id)?;
        let mid_press_background = solid_fill_colors(&mid_press)[0];
        let settled_press_background = super::mix_color(
            settled_hover_background,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        assert_ne!(mid_press_background, settled_hover_background);
        assert_ne!(mid_press_background, settled_press_background);

        runtime.tick(hover_time + press_time);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let press = runtime.render(window_id)?;
        assert_eq!(solid_fill_colors(&press)[0], settled_press_background);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, row_point, false),
        )?;
        let selection_start = hover_time + press_time;
        runtime.tick(selection_start + (toggle_time * 0.5));
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid_selection = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_selection).contains(&theme.palette.accent_text),
            "radio group selection dot should not snap directly to the settled selected color"
        );

        runtime.tick(selection_start + toggle_time);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let selected = runtime.render(window_id)?;
        assert!(
            solid_fill_colors(&selected).contains(&theme.palette.accent_text),
            "radio group selection dot should settle to the theme selected text color"
        );
        Ok(())
    }

    #[test]
    fn radio_group_focus_ring_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(RadioGroup::new("Mode").options(["Manual", "Automatic"]));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;

        runtime.tick(focus_duration() * 0.5);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid_focus = runtime.render(window_id)?;
        assert!(
            !solid_stroke_colors(&mid_focus).contains(&theme.palette.focus_ring),
            "radio group focus ring should not snap to the settled focus color"
        );

        runtime.tick(focus_duration());
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let settled_focus = runtime.render(window_id)?;
        assert!(
            solid_stroke_colors(&settled_focus).contains(&theme.palette.focus_ring),
            "radio group focus ring should settle to the theme focus color"
        );
        Ok(())
    }

    #[test]
    fn checkbox_toggles_and_updates_semantics() -> Result<()> {
        let states = Rc::new(RefCell::new(Vec::new()));
        let on_toggle = Rc::clone(&states);
        let (mut runtime, window_id) =
            build_runtime(Checkbox::new("Subscribe").on_toggle(move |checked| {
                on_toggle.borrow_mut().push(checked);
            }));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 10.0), false),
        )?;

        assert_eq!(states.borrow().as_slice(), &[true]);

        let output = runtime.render(window_id)?;
        let checkbox = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::CheckBox)
            .unwrap();
        assert_eq!(checkbox.state.checked, Some(sui_core::ToggleState::Checked));
        Ok(())
    }

    #[test]
    fn checkbox_indicator_and_label_respect_asymmetric_padding() {
        let theme = DefaultTheme::default();
        let padding = TestPadding {
            left: 8.0,
            top: 4.0,
            right: 8.0,
            bottom: 22.0,
        };
        let output = render(Checkbox::new("Visible").padding(padding));
        let checkbox = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::CheckBox)
            .expect("checkbox semantics should exist");
        let content_center = checkbox.bounds.y()
            + padding.top
            + ((checkbox.bounds.height() - padding.top - padding.bottom) * 0.5);
        let text = text_run_for(&output, "Visible");
        let mut indicator_bounds = None;
        output.frame.scene.visit_commands(&mut |command| {
            if indicator_bounds.is_some() {
                return;
            }
            if let SceneCommand::FillPath { path, .. } = command {
                let bounds = path.bounds();
                if (bounds.width() - theme.metrics.checkbox_indicator_size).abs() < 0.75
                    && (bounds.height() - theme.metrics.checkbox_indicator_size).abs() < 0.75
                {
                    indicator_bounds = Some(bounds);
                }
            }
        });
        let indicator = indicator_bounds.expect("checkbox indicator should paint");

        assert!((text_run_visual_center(&text) - content_center).abs() < 0.75);
        assert!((super::rect_center(indicator).y - content_center).abs() < 0.75);
    }

    #[test]
    fn text_input_caret_blink_toggles_visibility_as_time_advances() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(TextInput::new("Name").placeholder("Type a name"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        let focused = runtime.render(window_id)?;
        let caret_color = theme.palette.caret;
        let focused_caret_count = solid_fill_colors(&focused)
            .into_iter()
            .filter(|color| *color == caret_color)
            .count();
        assert!(focused.ime_composition_rect.is_some());
        assert!(focused_caret_count > 0);

        runtime.tick(CARET_BLINK_PERIOD_SECONDS * 0.75);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let blinked = runtime.render(window_id)?;
        let blinked_caret_count = solid_fill_colors(&blinked)
            .into_iter()
            .filter(|color| *color == caret_color)
            .count();
        assert!(blinked.ime_composition_rect.is_some());
        assert_eq!(blinked_caret_count, 0);
        Ok(())
    }

    #[test]
    fn text_input_selection_scope_tracks_keyboard_selection() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("Ada Lovelace")
                .selectable(selection.clone()),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;

        assert_eq!(selection.selected_text().as_deref(), Some("Ada Lovelace"));
        Ok(())
    }

    #[test]
    fn text_input_paints_keyboard_selection_and_copies_it() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(TextInput::new("Name").value("Ada Lovelace"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        let selected = runtime.render(window_id)?;

        assert!(
            solid_fill_colors(&selected).contains(&theme.palette.selection),
            "TextInput should paint its active selection before the text"
        );

        runtime.handle_event(window_id, command_key("c"))?;
        assert_eq!(runtime.clipboard().text().as_deref(), Some("Ada Lovelace"));
        Ok(())
    }

    #[test]
    fn password_input_masks_display_but_edits_and_copies_actual_value() -> Result<()> {
        let value = Rc::new(RefCell::new(String::new()));
        let captured = Rc::clone(&value);
        let (mut runtime, window_id) = build_runtime(
            PasswordInput::new("Password")
                .value("sëcret")
                .on_change(move |text| *captured.borrow_mut() = text),
        );

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("password input semantics present");
        let editable = input
            .editable_text
            .as_ref()
            .expect("password input should expose editable semantics");

        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("sëcret".to_string()))
        );
        assert!(editable.password);
        assert_eq!(text_run_for(&output, "••••••").text, "••••••");

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("c"))?;
        assert_eq!(runtime.clipboard().text().as_deref(), Some("sëcret"));

        runtime.clipboard().set_text("new secret");
        runtime.handle_event(window_id, command_key("v"))?;
        assert_eq!(value.borrow().as_str(), "new secret");
        Ok(())
    }

    #[test]
    fn datetime_input_edits_and_pastes_a_single_line_value() -> Result<()> {
        let value = Rc::new(RefCell::new(String::new()));
        let captured = Rc::clone(&value);
        let (mut runtime, window_id) = build_runtime(
            DateTimeInput::new("Scheduled for")
                .value("2026-07-15 09:30")
                .on_change(move |text| *captured.borrow_mut() = text),
        );

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("date/time input semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("2026-07-15 09:30".to_string()))
        );
        assert!(!input.editable_text.as_ref().unwrap().password);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.clipboard().set_text("2026-08-01\n14:45");
        runtime.handle_event(window_id, command_key("v"))?;

        assert_eq!(value.borrow().as_str(), "2026-08-0114:45");
        Ok(())
    }

    #[test]
    fn text_area_selection_scope_tracks_keyboard_selection() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes")
                .value("first line\nsecond line")
                .selectable(selection.clone()),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;

        assert_eq!(
            selection.selected_text().as_deref(),
            Some("first line\nsecond line")
        );
        Ok(())
    }

    #[test]
    fn read_only_text_area_paints_selection_and_copies_it() -> Result<()> {
        let theme = DefaultTheme::default();
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Connection details")
                .value("node = local\naddress = 127.0.0.1:21353")
                .read_only()
                .selectable(selection.clone()),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        let selected = runtime.render(window_id)?;

        assert_eq!(
            selection.selected_text().as_deref(),
            Some("node = local\naddress = 127.0.0.1:21353")
        );
        assert!(
            solid_fill_colors(&selected).contains(&theme.palette.selection),
            "read-only TextArea should paint the active selection before its text"
        );

        runtime.handle_event(window_id, command_key("c"))?;
        assert_eq!(
            runtime.clipboard().text().as_deref(),
            Some("node = local\naddress = 127.0.0.1:21353")
        );
        Ok(())
    }

    #[test]
    fn text_input_copy_and_paste_use_runtime_clipboard() -> Result<()> {
        let value = Rc::new(RefCell::new(String::new()));
        let captured = Rc::clone(&value);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("Ada Lovelace")
                .on_change(move |text| *captured.borrow_mut() = text),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("c"))?;

        assert_eq!(runtime.clipboard().text().as_deref(), Some("Ada Lovelace"));

        runtime.clipboard().set_text("Grace\nHopper");
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("v"))?;

        // Pasted text is coerced to a single line.
        assert_eq!(value.borrow().as_str(), "GraceHopper");
        Ok(())
    }

    #[test]
    fn text_area_paste_with_empty_clipboard_preserves_selection() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").value("alpha"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("v"))?;
        runtime.handle_event(window_id, command_key("c"))?;

        // An empty clipboard paste must not delete the selection, so the
        // follow-up copy still captures the full document.
        assert_eq!(runtime.clipboard().text().as_deref(), Some("alpha"));
        Ok(())
    }

    #[test]
    fn text_area_mouse_drag_selects_text() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes")
                .value("alpha beta gamma")
                .selectable(selection.clone()),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(360.0, 8.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(360.0, 8.0), false),
        )?;

        let selected = selection.selected_text().unwrap_or_default();
        assert!(
            selected.starts_with("alpha"),
            "drag from the line start should select leading text, got {selected:?}"
        );
        Ok(())
    }

    #[test]
    fn text_input_mouse_drag_selects_text() -> Result<()> {
        let selection = SelectionScope::new();
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("hello world")
                .selectable(selection.clone()),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(360.0, 8.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(360.0, 8.0), false),
        )?;

        let selected = selection.selected_text().unwrap_or_default();
        assert!(
            selected.starts_with("hello"),
            "drag from the field start should select leading text, got {selected:?}"
        );
        Ok(())
    }

    #[test]
    fn typed_text_commands_drive_clipboard_actions() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").value("alpha"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )?;
        runtime.handle_command(
            CommandTarget::FocusedWidget(window_id),
            CommandDelivery::Directed,
            TEXT_COMMAND,
            TextCommand::SelectAll,
        );
        runtime.handle_command(
            CommandTarget::FocusedWidget(window_id),
            CommandDelivery::Directed,
            TEXT_COMMAND,
            TextCommand::Copy,
        );
        assert_eq!(runtime.clipboard().text().as_deref(), Some("alpha"));

        runtime.clipboard().set_text("beta");
        for command in [
            TextCommand::SelectAll,
            TextCommand::Paste,
            TextCommand::SelectAll,
            TextCommand::Copy,
        ] {
            runtime.handle_command(
                CommandTarget::FocusedWidget(window_id),
                CommandDelivery::Directed,
                TEXT_COMMAND,
                command,
            );
        }
        assert_eq!(runtime.clipboard().text().as_deref(), Some("beta"));
        Ok(())
    }

    #[test]
    fn text_input_caret_uses_theme_palette_color() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.palette.caret = Color::rgba(0.02, 0.18, 0.72, 1.0);
        // A sentinel accent_text distinct from the (white) field well, so the
        // background fill cannot mask an accent_text-colored caret.
        theme.palette.accent_text = Color::rgba(0.9, 0.05, 0.85, 1.0);
        let caret_color = theme.palette.caret;
        let accent_text = theme.palette.accent_text;
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .theme(theme)
                .value("Visible caret on white"),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(80.0, 16.0), true),
        )?;
        let output = runtime.render(window_id)?;
        let fill_colors = solid_fill_colors(&output);

        assert!(fill_colors.contains(&caret_color));
        assert!(!fill_colors.contains(&accent_text));
        Ok(())
    }

    #[test]
    fn text_input_text_visual_center_matches_tall_control_center() {
        let output = render(
            TextInput::new("Name")
                .value("Ada")
                .min_width(180.0)
                .min_height(52.0),
        );
        let text = text_run_for(&output, "Ada");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("text input value should shape");
        let line = layout
            .lines()
            .first()
            .expect("text input value should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn text_input_value_preserves_tall_measurement_and_centering() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 30.0;
        theme.typography.body_line_height = 10.0;
        let output = render(
            TextInput::new("Name")
                .theme(theme)
                .value("Ada")
                .min_width(180.0)
                .min_height(56.0),
        );

        assert_tall_body_text_centered(&output, "Ada", theme, output.frame.viewport.height * 0.5);
    }

    #[test]
    fn text_input_placeholder_visual_center_matches_tall_control_center() {
        let theme = DefaultTheme::default();
        let output = render(
            TextInput::new("Name")
                .placeholder("Type a name")
                .min_width(180.0)
                .min_height(52.0),
        );
        let text = text_run_for(&output, "Type a name");
        let control_center = output.frame.viewport.height * 0.5;

        assert_eq!(text.style.color, theme.placeholder_text_style().color);
        assert!((text_run_visual_center(&text) - control_center).abs() < 0.75);
    }

    #[test]
    fn text_input_leading_icon_offsets_placeholder_and_keeps_editing() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Search")
                .placeholder("Search conversations")
                .leading_icon(IconGlyph::Search)
                .min_width(220.0)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let output = runtime.render(window_id)?;
        let icon_rect = first_lucide_icon_rect(&output);
        let placeholder = text_run_for(&output, "Search conversations");
        assert!(
            placeholder.rect.x() >= icon_rect.max_x() + 4.0,
            "placeholder should start after the leading icon"
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(8.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "repo".to_string(),
            }),
        )?;

        assert_eq!(changes.borrow().as_slice(), &["repo".to_string()]);
        Ok(())
    }

    #[test]
    fn text_input_placeholder_preserves_tall_measurement_and_centering() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 30.0;
        theme.typography.body_line_height = 10.0;
        let output = render(
            TextInput::new("Name")
                .theme(theme)
                .placeholder("Type a name")
                .min_width(180.0)
                .min_height(56.0),
        );
        let text = text_run_for(&output, "Type a name");

        assert_eq!(text.style.color, theme.placeholder_text_style().color);
        assert_tall_body_text_centered(
            &output,
            "Type a name",
            theme,
            output.frame.viewport.height * 0.5,
        );
    }

    #[test]
    fn text_input_accepts_printable_key_without_text_payload() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name").on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(window_id, key_without_text("h"))?;

        assert_eq!(changes.borrow().last().map(String::as_str), Some("h"));
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text input semantics present");
        assert_eq!(input.value, Some(SemanticsValue::Text("h".to_string())));
        Ok(())
    }

    #[test]
    fn text_input_on_change_with_ctx_receives_text() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .on_change_with_ctx(move |_, value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(window_id, key_without_text("h"))?;

        assert_eq!(changes.borrow().as_slice(), &["h".to_string()]);
        Ok(())
    }

    #[test]
    fn text_input_read_only_uses_muted_text_and_blocks_mutation() -> Result<()> {
        let theme = DefaultTheme::default();
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("Locked")
                .min_height(52.0)
                .read_only()
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(40.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "!".to_string(),
            }),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Backspace", KeyState::Pressed)),
        )?;
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text input semantics present");
        let editable = input
            .editable_text
            .as_ref()
            .expect("text input should expose editable semantics");

        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("Locked".to_string()))
        );
        assert!(editable.readonly);
        assert!(input.actions.contains(&SemanticsAction::Copy));
        assert!(!input.actions.contains(&SemanticsAction::InsertText));
        assert!(!input.actions.contains(&SemanticsAction::SetValue));
        assert!(changes.borrow().is_empty());
        let text = text_run_for(&output, "Locked");
        assert_eq!(text.style.color, theme.palette.text_muted);
        assert!(
            (text_run_visual_center(&text) - (input.bounds.y() + input.bounds.height() * 0.5))
                .abs()
                < 0.75
        );
        assert!(!solid_fill_colors(&output).contains(&theme.palette.caret));
        assert!(output.ime_composition_rect.is_none());
        Ok(())
    }

    #[test]
    fn text_input_focus_animation_settles_into_blink_timer_without_frame_spin() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(TextInput::new("Name").placeholder("Type a name"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(100.0, 16.0), true),
        )?;
        let _ = runtime.render(window_id)?;

        let settled_at = focus_duration() + 0.01;
        runtime.tick(settled_at);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let next = runtime
            .next_wakeup_time(window_id)?
            .expect("caret blink timer should remain armed after focus settles");
        assert!(next >= (CARET_BLINK_PERIOD_SECONDS * 0.5) - 1e-6);
        assert!(next - settled_at > 0.25);

        Ok(())
    }

    #[test]
    fn text_input_click_while_focused_restores_hidden_caret() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(TextInput::new("Name").placeholder("Type a name"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        let _ = runtime.render(window_id)?;

        runtime.tick(CARET_BLINK_PERIOD_SECONDS * 0.75);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let hidden = runtime.render(window_id)?;
        let caret_color = theme.palette.caret;
        assert_eq!(
            solid_fill_colors(&hidden)
                .into_iter()
                .filter(|color| *color == caret_color)
                .count(),
            0
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        let restored = runtime.render(window_id)?;
        assert!(
            solid_fill_colors(&restored)
                .into_iter()
                .filter(|color| *color == caret_color)
                .count()
                > 0
        );

        Ok(())
    }

    #[test]
    fn text_area_click_while_focused_restores_hidden_caret() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(TextArea::new("Notes").placeholder("Type notes"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        let _ = runtime.render(window_id)?;

        runtime.tick(CARET_BLINK_PERIOD_SECONDS * 0.75);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let hidden = runtime.render(window_id)?;
        let caret_color = theme.palette.caret;
        assert_eq!(
            solid_fill_colors(&hidden)
                .into_iter()
                .filter(|color| *color == caret_color)
                .count(),
            0
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        let restored = runtime.render(window_id)?;
        assert!(
            solid_fill_colors(&restored)
                .into_iter()
                .filter(|color| *color == caret_color)
                .count()
                > 0
        );

        Ok(())
    }

    #[test]
    fn text_area_read_only_exposes_readonly_semantics_and_blocks_mutation() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) =
            build_runtime(TextArea::new("Notes").value("Pinned\nNotes").read_only());

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Backspace", KeyState::Pressed)),
        )?;
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        let editable = input
            .editable_text
            .as_ref()
            .expect("text area should expose editable semantics");

        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("Pinned\nNotes".to_string()))
        );
        assert!(editable.readonly);
        assert!(editable.multiline);
        assert!(input.actions.contains(&SemanticsAction::Copy));
        assert!(!input.actions.contains(&SemanticsAction::InsertText));
        assert!(!input.actions.contains(&SemanticsAction::DeleteBackward));
        assert_eq!(
            text_run_for(&output, "Pinned\nNotes").style.color,
            theme.palette.text_muted
        );
        assert!(!solid_fill_colors(&output).contains(&theme.palette.caret));
        assert!(output.ime_composition_rect.is_none());
        Ok(())
    }

    #[test]
    fn text_area_placeholder_uses_placeholder_style_and_top_line_slot() {
        let theme = DefaultTheme::default();
        let output = render(
            TextArea::new("Notes")
                .placeholder("Write notes")
                .min_width(260.0)
                .min_height(96.0),
        );
        let text = text_run_for(&output, "Write notes");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("text area placeholder should shape");

        assert_eq!(text.style.color, theme.placeholder_text_style().color);
        assert!((text.rect.y() - theme.metrics.text_input_padding.top).abs() < 0.75);
        assert!(
            (layout.box_size().height - text.style.line_height).abs() < 0.75,
            "placeholder line box should use the placeholder line height"
        );
    }

    #[test]
    fn text_area_placeholder_preserves_tall_measurement_in_top_line_slot() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 30.0;
        theme.typography.body_line_height = 10.0;
        let output = render(
            TextArea::new("Notes")
                .theme(theme)
                .placeholder("Write notes")
                .min_width(260.0)
                .min_height(96.0),
        );
        let text = text_run_for(&output, "Write notes");
        let layout = shaped_text_layout_for(&output, "Write notes");

        assert_eq!(text.style.color, theme.placeholder_text_style().color);
        assert_eq!(text.style.font_size, theme.typography.body_font_size);
        assert_eq!(text.style.line_height, theme.typography.body_line_height);
        assert!((text.rect.y() - theme.metrics.text_input_padding.top).abs() < 0.75);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
    }

    #[test]
    fn text_area_read_only_value_preserves_tall_measurement_and_muted_text() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 30.0;
        theme.typography.body_line_height = 10.0;
        let output = render(
            TextArea::new("Notes")
                .theme(theme)
                .value("Pinned notes")
                .read_only()
                .min_width(260.0)
                .min_height(96.0),
        );
        let text = text_run_for(&output, "Pinned notes");
        let layout = shaped_text_layout_for(&output, "Pinned notes");

        assert_eq!(text.style.color, theme.palette.text_muted);
        assert_eq!(text.style.font_size, theme.typography.body_font_size);
        assert_eq!(text.style.line_height, theme.typography.body_line_height);
        assert!((text.rect.y() - theme.metrics.text_input_padding.top).abs() < 0.75);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
    }

    #[test]
    fn text_area_shapes_multiline_value_with_finite_positions() {
        let notes = "Pinned notes for inspector workflows.\nSupports multiline editing.";
        let output = render(
            SizedBox::new().width(420.0).with_child(
                TextArea::new("Notes")
                    .min_height(150.0)
                    .value(notes)
                    .placeholder("Write notes"),
            ),
        );
        let layout = shaped_text_layout_for(&output, notes);

        assert!(layout.box_size().height.is_finite());
        assert!(layout.lines().iter().all(|line| rect_is_finite(line.rect)));
        assert!(
            layout
                .glyphs()
                .iter()
                .all(|glyph| glyph.origin_x.is_finite() && glyph.origin_y.is_finite())
        );
    }

    #[test]
    fn text_area_focus_ring_animation_progresses_without_losing_ime_rect() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(TextArea::new("Notes").placeholder("Type notes"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        let initial = runtime.render(window_id)?;
        assert!(initial.ime_composition_rect.is_some());

        runtime.tick(focus_duration() * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        assert!(mid.ime_composition_rect.is_some());
        assert_ne!(solid_fill_colors(&initial), solid_fill_colors(&mid));

        Ok(())
    }

    #[test]
    fn text_input_commits_ime_text_and_supports_backspace() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .placeholder("Type a name")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "Ada".to_string(),
            }),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent {
                key: "Backspace".to_string(),
                code: "Backspace".to_string(),
                text: None,
                state: KeyState::Pressed,
                modifiers: Modifiers::NONE,
                repeat: false,
                is_composing: false,
            }),
        )?;

        assert_eq!(
            changes.borrow().as_slice(),
            &["Ada".to_string(), "Ad".to_string()]
        );

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .unwrap();
        assert_eq!(input.name.as_deref(), Some("Name"));
        assert_eq!(
            input.value,
            Some(sui_core::SemanticsValue::Text("Ad".to_string()))
        );
        assert!(output.ime_composition_rect.is_some());
        Ok(())
    }

    #[test]
    fn text_input_edits_at_caret_with_keyboard_navigation() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("Ada")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(100.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowLeft", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "m".to_string(),
            }),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Backspace", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowLeft", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Delete", KeyState::Pressed)),
        )?;

        assert_eq!(
            changes.borrow().as_slice(),
            &["Adma".to_string(), "Ada".to_string(), "Aa".to_string()]
        );

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .unwrap();
        assert_eq!(
            input.value,
            Some(sui_core::SemanticsValue::Text("Aa".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_input_uses_shared_editor_commands_and_editable_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("hello world")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(100.0, 16.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(window_id, command_key("x"))?;
        runtime.handle_event(window_id, command_key("v"))?;
        runtime.handle_event(window_id, command_key("z"))?;
        runtime.handle_event(window_id, command_key("y"))?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "\n!".to_string(),
            }),
        )?;

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .unwrap();
        assert_eq!(
            input.value,
            Some(sui_core::SemanticsValue::Text("hello world!".to_string()))
        );
        let editable = input
            .editable_text
            .as_ref()
            .expect("text input should expose editable semantics");
        assert!(!editable.multiline);
        assert_eq!(editable.caret_offset, "hello world!".len());
        assert_eq!(
            editable.selection,
            SemanticsTextRange::new("hello world!".len(), "hello world!".len())
        );
        assert_eq!(
            changes.borrow().last().map(String::as_str),
            Some("hello world!")
        );
        Ok(())
    }

    #[test]
    fn text_input_click_positions_caret_for_insertion() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .value("Ada")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(1.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "Lady ".to_string(),
            }),
        )?;

        assert_eq!(changes.borrow().as_slice(), &["Lady Ada".to_string()]);

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .unwrap();
        assert_eq!(
            input.value,
            Some(sui_core::SemanticsValue::Text("Lady Ada".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_input_ignores_process_key_without_text() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextInput::new("Name")
                .placeholder("Type a name")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent {
                key: "Process".to_string(),
                code: "KeyA".to_string(),
                text: None,
                state: KeyState::Pressed,
                modifiers: Modifiers::NONE,
                repeat: false,
                is_composing: false,
            }),
        )?;

        assert!(changes.borrow().is_empty());

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .unwrap();
        assert_eq!(
            input.value,
            Some(sui_core::SemanticsValue::Text(String::new()))
        );
        Ok(())
    }

    #[test]
    fn button_obeys_minimum_size() {
        let output = render(Button::new("Go").min_width(140.0).min_height(40.0));
        assert_eq!(output.frame.viewport, Size::new(140.0, 40.0));
    }

    #[test]
    fn button_preserves_sdr_palette_when_hdr_mode_disabled() {
        let mut theme = DefaultTheme::default();
        theme.hdr.mode = HdrThemeMode::Disabled;
        theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.palette.accent)
            .with_hdr(Color::linear_display_p3(1.35, 0.28, 0.22, 1.0));

        let visuals = Button::primary("Go").theme(theme).resolved_visuals(true);
        let fills = solid_fill_colors(&render(Button::primary("Go").theme(theme)));

        assert_eq!(visuals.background, theme.palette.accent);
        assert_eq!(visuals.border, theme.palette.accent_border_focus);
        assert_eq!(visuals.focus_ring, Some(theme.palette.focus_ring));
        assert_eq!(visuals.label_color, theme.palette.accent_text);
        assert_eq!(visuals.label_peak_lift, theme.hdr.luminance.reference_white);
        assert!(visuals.chrome_style.is_none());
        assert_eq!(fills.first().copied(), Some(theme.palette.accent));
        assert_ne!(
            fills.first().copied(),
            theme.hdr.color_roles.accent.hdr,
            "disabled mode should paint the SDR accent, not the HDR token"
        );
    }

    #[test]
    fn button_can_resolve_constrained_hdr_accent_style() {
        let mut theme = DefaultTheme::default();
        theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
        theme.hdr.luminance.semantic_accent = 1.18;
        theme.hdr.policy.max_large_area_lift = 1.22;
        theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.palette.accent)
            .with_hdr(Color::linear_display_p3(1.28, 0.42, 0.30, 1.0));

        let visuals = Button::primary("Go").theme(theme).resolved_visuals(true);
        let chrome_style = visuals.chrome_style.expect("hdr accent style present");

        assert_eq!(visuals.background, chrome_style.color);
        assert!(visuals.border.red <= theme.hdr.policy.max_large_area_lift);
        assert_ne!(visuals.background, theme.palette.accent);
        assert_eq!(chrome_style.peak_lift, 1.18);
        assert!((chrome_style.color.red - chrome_style.peak_lift).abs() < f32::EPSILON);
        assert!(visuals.focus_ring.is_some());
    }

    #[test]
    fn button_hdr_style_keeps_label_at_reference_white() {
        let mut theme = DefaultTheme::default();
        theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
        theme.hdr.luminance.semantic_accent = 1.2;
        theme.hdr.policy.max_large_area_lift = 1.25;
        theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.palette.accent)
            .with_hdr(Color::linear_display_p3(1.20, 0.36, 0.30, 1.0));

        let visuals = Button::primary("Go").theme(theme).resolved_visuals(false);

        assert_eq!(visuals.label_color, theme.palette.accent_text);
        assert_eq!(visuals.label_peak_lift, theme.hdr.luminance.reference_white);
        assert!(visuals.label_peak_lift <= theme.hdr.policy.max_large_area_lift);
    }

    #[test]
    fn button_centers_label_within_available_content_width() {
        let theme = DefaultTheme::default();
        let optical = render(Button::new("Go").min_width(140.0));
        let optical_label = first_text_run(&optical).rect;

        assert!(optical_label.x() > theme.metrics.button_padding.left);
        assert!(optical_label.max_y() <= optical.frame.viewport.height);
    }

    #[test]
    fn button_optically_centers_label_ink_with_side_bearings() {
        let mut style = DefaultTheme::default().button_text_style();
        style.font_size = 48.0;
        style.line_height = 56.0;
        let candidates = ["j", "T.", "f)", "(f", "AV", "To", "WA", "1"];
        let (label, offset) = candidates
            .iter()
            .find_map(|candidate| {
                let measurement = TextSystem::new()
                    .measure_text(candidate.to_string(), style.clone(), &FontRegistry::new())
                    .ok()?;
                let offset = measurement.bounds.x() + (measurement.bounds.width() * 0.5)
                    - (measurement.width * 0.5);
                (offset.abs() > 0.75).then_some((*candidate, offset))
            })
            .expect("test font should expose a label with asymmetric side bearings");
        let output = render(Button::new(label).text_style(style).min_width(220.0));
        let text = first_shaped_text(&output);
        let ink_bounds = text.translated_bounds();
        let ink_center = ink_bounds.x() + (ink_bounds.width() * 0.5);
        let control_center = output.frame.viewport.width * 0.5;

        assert!(offset.abs() > 0.75);
        assert!((ink_center - control_center).abs() < 0.75);
    }

    #[test]
    fn button_window_option_keeps_button_label_centered() {
        let (mut runtime, window_id) = build_runtime(Button::new("Go").min_width(140.0));
        set_window_render_options(
            window_id,
            WindowRenderOptions::new(true, 1.0).with_optical_vertical_text_alignment_enabled(false),
        );
        let geometric = runtime.render(window_id).unwrap();
        clear_window_render_options(window_id);
        let text = first_shaped_text(&geometric);
        let layout = text
            .resolve(geometric.frame.text_layout_registry.as_ref())
            .expect("button label layout should resolve");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let actual_visual_center =
            text.origin.y + line.baseline + visual_center(layout.measurement(), false);
        let control_center = geometric.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn button_label_visual_center_matches_control_center() {
        let output = render(Button::new("Go").min_width(140.0));
        let text = first_shaped_text(&output);
        let layout = text
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("button label layout should resolve");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let actual_visual_center =
            text.origin.y + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn button_label_visual_center_respects_asymmetric_padding() {
        let padding = TestPadding {
            left: 12.0,
            top: 4.0,
            right: 12.0,
            bottom: 20.0,
        };
        let output = render(
            Button::new("Go")
                .padding(padding)
                .min_width(140.0)
                .min_height(64.0),
        );
        let text = first_shaped_text(&output);
        let layout = text
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("button label layout should resolve");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let actual_visual_center =
            text.origin.y + line.baseline + optical_visual_center(layout.measurement());
        let content_center =
            padding.top + ((output.frame.viewport.height - padding.top - padding.bottom) * 0.5);

        assert!((actual_visual_center - content_center).abs() < 0.75);
    }

    #[test]
    fn button_persistent_label_visual_center_matches_control_center() {
        let output = render(Button::new("Apply").min_width(140.0));
        let text = first_shaped_text(&output);
        let layout = text
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("button label layout should resolve");
        let line = layout
            .lines()
            .first()
            .expect("button label should contain one line");
        let actual_visual_center =
            text.origin.y + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn switch_label_visual_center_matches_control_center() {
        let output = render(Switch::new("Airplane mode"));
        let text = first_text_run(&output);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("switch label should shape");
        let line = layout
            .lines()
            .first()
            .expect("switch label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn switch_label_visual_center_ignores_asymmetric_padding() {
        let output = render(Switch::new("Wifi").padding(TestPadding {
            left: 8.0,
            top: 0.0,
            right: 8.0,
            bottom: 18.0,
        }));
        let text = first_text_run(&output);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("switch label should shape");
        let line = layout
            .lines()
            .first()
            .expect("switch label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let track_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - track_center).abs() < 0.75);
    }

    #[test]
    fn switch_thumb_uses_foreground_in_dark_theme_variants() {
        let light = DefaultTheme::default();
        assert_eq!(
            Switch::new("Wifi")
                .theme(light)
                .resolved_visuals(false)
                .thumb_color,
            light.palette.accent_text
        );

        for theme in [DefaultTheme::dark(), DefaultTheme::high_contrast()] {
            for on in [false, true] {
                assert_eq!(
                    Switch::new("Wifi")
                        .on(on)
                        .theme(theme)
                        .resolved_visuals(false)
                        .thumb_color,
                    theme.palette.text
                );
            }

            let fills = solid_fill_colors(&render(Switch::new("Wifi").theme(theme)));
            assert!(fills.contains(&theme.palette.text));
        }
    }

    #[test]
    fn switch_on_state_can_use_emissive_indicator_role() {
        let mut theme = DefaultTheme::default();
        theme.hdr.mode = HdrThemeMode::ConstrainedHdr;
        theme.hdr.luminance.emissive_indicator = 1.3;
        theme.hdr.policy.max_constrained_lift = 1.35;
        theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.palette.accent)
            .with_hdr(Color::linear_display_p3(1.30, 0.48, 0.32, 1.0));

        let visuals = Switch::new("Wifi")
            .on(true)
            .theme(theme)
            .resolved_visuals(false);
        let indicator_style = visuals
            .indicator_style
            .expect("emissive indicator style present");

        assert_eq!(visuals.track_color, indicator_style.color);
        assert_eq!(
            indicator_style.peak_lift,
            resolve_luminance_role(&theme.hdr, WidgetLuminanceRole::EmissiveIndicator)
        );
        assert_eq!(visuals.label_peak_lift, theme.hdr.luminance.reference_white);
    }

    #[test]
    fn switch_label_readability_preserved_when_hdr_mode_disabled() {
        let mut theme = DefaultTheme::default();
        theme.hdr.mode = HdrThemeMode::Disabled;
        theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.palette.accent)
            .with_hdr(Color::linear_display_p3(1.34, 0.40, 0.30, 1.0));

        let visuals = Switch::new("Wifi")
            .on(true)
            .theme(theme)
            .resolved_visuals(true);

        assert_eq!(visuals.label_color, theme.palette.text);
        assert_eq!(visuals.label_peak_lift, theme.hdr.luminance.reference_white);
        assert!(visuals.indicator_style.is_none());
    }

    #[test]
    fn switch_constrained_hdr_does_not_overshoot_full_hdr_limits() {
        let mut constrained = DefaultTheme::default();
        constrained.hdr.mode = HdrThemeMode::ConstrainedHdr;
        constrained.hdr.luminance.emissive_indicator = 2.5;
        constrained.hdr.policy.max_constrained_lift = 1.3;
        constrained.hdr.policy.max_emissive_lift = 2.1;
        constrained.hdr.color_roles.accent =
            SemanticColorToken::from_sdr(constrained.palette.accent)
                .with_hdr(Color::linear_display_p3(2.5, 0.48, 0.32, 1.0));

        let mut full = constrained;
        full.hdr.mode = HdrThemeMode::FullHdr;

        let constrained_visuals = Switch::new("Wifi")
            .on(true)
            .theme(constrained)
            .resolved_visuals(false);
        let full_visuals = Switch::new("Wifi")
            .on(true)
            .theme(full)
            .resolved_visuals(false);
        let constrained_track =
            solid_fill_colors(&render(Switch::new("Wifi").on(true).theme(constrained)));
        let full_track = solid_fill_colors(&render(Switch::new("Wifi").on(true).theme(full)));

        let constrained_peak = constrained_visuals
            .indicator_style
            .expect("constrained indicator style")
            .peak_lift;
        let full_peak = full_visuals
            .indicator_style
            .expect("full indicator style")
            .peak_lift;

        assert_eq!(constrained_peak, 1.3);
        assert_eq!(full_peak, 2.1);
        assert!(constrained_peak < full_peak);
        assert!(constrained_track.contains(&constrained_visuals.track_color));
        assert!(full_track.contains(&full_visuals.track_color));
        assert!((constrained_visuals.track_color.red - constrained_peak).abs() < f32::EPSILON);
        assert!((full_visuals.track_color.red - full_peak).abs() < f32::EPSILON);
    }

    #[test]
    fn radio_button_label_visual_center_matches_control_center() {
        let output = render(RadioButton::new("Option A"));
        let text = first_text_run(&output);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("radio button label should shape");
        let line = layout
            .lines()
            .first()
            .expect("radio button label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn radio_group_first_label_visual_center_matches_row_center() {
        let output = render(RadioGroup::new("Choices").options(["Alpha", "Beta"]));
        let text = text_run_for(&output, "Alpha");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("radio group label should shape");
        let line = layout
            .lines()
            .first()
            .expect("radio group label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let theme = DefaultTheme::default();
        let row_center = super::default_form_control_height(&theme) * 0.5;

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    #[test]
    fn toggle_and_radio_labels_preserve_tall_measurements_and_control_centering() {
        let mut theme = DefaultTheme::default();
        theme.text.sm.size = 28.0;
        theme.text.sm.line_height = 10.0;
        theme.sync_derived_fields();
        theme.metrics.min_height = 56.0;

        let checkbox = render(Checkbox::new("Accept").theme(theme));
        assert_tall_body_text_centered(
            &checkbox,
            "Accept",
            theme,
            checkbox.frame.viewport.height * 0.5,
        );

        let switch = render(Switch::new("Wifi").theme(theme));
        assert_tall_body_text_centered(&switch, "Wifi", theme, switch.frame.viewport.height * 0.5);

        let radio_button = render(RadioButton::new("Option A").theme(theme));
        assert_tall_body_text_centered(
            &radio_button,
            "Option A",
            theme,
            radio_button.frame.viewport.height * 0.5,
        );

        let radio_group = render(
            RadioGroup::new("Choices")
                .theme(theme)
                .options(["Alpha", "Beta"]),
        );
        assert_tall_body_text_centered(
            &radio_group,
            "Alpha",
            theme,
            theme.metrics.min_height * 0.5,
        );
    }

    #[test]
    fn controls_default_to_native_form_height() {
        let theme = DefaultTheme::default();
        let expected = super::default_form_control_height(&theme);

        macro_rules! assert_default_height {
            ($widget:expr, $name:literal) => {{
                let height = render($widget).frame.viewport.height;
                assert!(
                    (height - expected).abs() < 0.01,
                    "expected {} height to match native form height {}, got {}",
                    $name,
                    expected,
                    height
                );
            }};
        }

        assert_default_height!(Button::new("Go"), "button");
        assert_default_height!(Checkbox::new("Subscribe"), "checkbox");
        assert_default_height!(Switch::new("Enabled"), "switch");
        assert_default_height!(RadioButton::new("Manual"), "radio button");
        assert_default_height!(
            RadioGroup::new("Choices").options(["Alpha"]),
            "single-row radio group"
        );
        assert_default_height!(Slider::new("Opacity"), "slider");
        assert_default_height!(NumberInput::new("Size"), "number input");
        assert_default_height!(Select::new("Blend mode").options(["Normal"]), "select");
        assert_default_height!(TextInput::new("Name"), "text input");
    }

    #[test]
    fn button_theme_is_public_and_changes_metrics_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.metrics.button_min_width = 156.0;
        theme.metrics.min_height = 52.0;
        theme.typography.body_font_size = 16.0;
        theme.typography.body_line_height = 24.0;
        theme.palette.accent_text = Color::rgba(0.10, 0.12, 0.15, 1.0);

        let output = render(Button::primary("Theme").theme(theme));

        assert_eq!(output.frame.viewport, Size::new(156.0, 52.0));
        let label = first_text_run(&output);
        assert_eq!(label.style.font_size, 16.0);
        assert_eq!(label.style.line_height, 24.0);
        assert_eq!(label.style.color, theme.palette.accent_text);
    }

    #[test]
    fn separator_theme_when_reads_current_theme() {
        let mut theme = DefaultTheme::dark();
        theme.metrics.separator_thickness = 3.0;

        let separator = Separator::vertical().theme_when(move || theme);

        assert_eq!(
            separator.resolved_theme().colors.scheme,
            theme.colors.scheme
        );
        assert_eq!(separator.resolved_thickness(), 3.0);
    }

    #[test]
    fn label_theme_uses_default_widget_typography() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 15.0;
        theme.typography.body_line_height = 22.0;
        theme.palette.text = Color::rgba(0.78, 0.82, 0.90, 1.0);

        let output = render(Label::new("Body").theme(theme));
        let label = first_text_run(&output);

        assert_eq!(label.style.font_size, 15.0);
        assert_eq!(label.style.line_height, 22.0);
        assert_eq!(label.style.color, theme.palette.text);
    }

    #[test]
    fn button_scales_border_width_for_hidpi() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(Button::new("HiDPI"));

        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: 2.0,
                raw_dpi: Some(192.0),
                suggested_size: None,
            }),
        )?;

        let output = runtime.render(window_id)?;
        let stroke = output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::StrokePath { stroke, .. } => Some(*stroke),
                _ => None,
            })
            .expect("button border stroke command present");

        assert_eq!(stroke.width, 0.5);
        Ok(())
    }

    #[test]
    fn text_input_scales_caret_width_for_hidpi() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(TextInput::new("Name").value("Ada"));

        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: 2.0,
                raw_dpi: Some(192.0),
                suggested_size: None,
            }),
        )?;
        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;

        let output = runtime.render(window_id)?;

        assert_eq!(
            output
                .ime_composition_rect
                .expect("focused text input caret")
                .width(),
            1.0
        );
        Ok(())
    }

    #[test]
    fn switch_toggles_and_reports_switch_semantics() -> Result<()> {
        let states = Rc::new(RefCell::new(Vec::new()));
        let on_toggle = Rc::clone(&states);
        let (mut runtime, window_id) =
            build_runtime(Switch::new("Airplane mode").on_toggle(move |checked| {
                on_toggle.borrow_mut().push(checked);
            }));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        assert_eq!(states.borrow().as_slice(), &[true]);

        let output = runtime.render(window_id)?;
        let switch = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Switch)
            .expect("switch semantics present");
        assert_eq!(switch.state.checked, Some(sui_core::ToggleState::Checked));
        Ok(())
    }

    #[test]
    fn radio_group_applies_typed_semantic_text_value_through_selection_callback() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            RadioGroup::new("Mode")
                .options(["Alpha", "Beta", "Gamma"])
                .selected(0)
                .on_change(move |index, value| on_change.borrow_mut().push((index, value))),
        );
        let group_id = runtime
            .render(window_id)?
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioGroup)
            .expect("radio group semantics present")
            .id;

        assert!(runtime.handle_semantics_action(
            window_id,
            group_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Text("Beta".to_string())),
        )?);
        assert!(!runtime.handle_semantics_action(
            window_id,
            group_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Text("Missing".to_string())),
        )?);
        assert_eq!(changes.borrow().as_slice(), &[(1, "Beta".to_string())]);

        let group = runtime
            .render(window_id)?
            .semantics
            .into_iter()
            .find(|node| node.id == group_id)
            .expect("radio group semantics remain present");
        assert_eq!(group.value, Some(SemanticsValue::Text("Beta".to_string())));
        Ok(())
    }

    #[test]
    fn slider_accepts_keyboard_adjustment_and_reports_range_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            Slider::new("Opacity")
                .range(0.0, 1.0)
                .step(0.25)
                .value(0.0)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
        )?;

        assert!(
            changes
                .borrow()
                .last()
                .is_some_and(|value| (*value - 0.25).abs() < 1e-6)
        );

        let output = runtime.render(window_id)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present");
        assert_eq!(
            slider.value,
            Some(SemanticsValue::Range {
                value: 0.25,
                min: 0.0,
                max: 1.0,
            })
        );
        assert_eq!(slider.numeric_step, Some(0.25));
        Ok(())
    }

    #[test]
    fn slider_applies_typed_semantic_numeric_actions_through_step_and_callbacks() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            Slider::new("Opacity")
                .range(0.0, 1.0)
                .step(0.25)
                .value(0.25)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );
        let slider_id = runtime
            .render(window_id)?
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present")
            .id;

        assert!(runtime.handle_semantics_action(
            window_id,
            slider_id,
            SemanticsActionRequest::Increment,
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            slider_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Number(0.88)),
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            slider_id,
            SemanticsActionRequest::Decrement,
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            slider_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Range {
                value: 0.24,
                min: -100.0,
                max: 100.0,
            }),
        )?);
        assert!(!runtime.handle_semantics_action(
            window_id,
            slider_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Text("invalid".to_string())),
        )?);

        assert_eq!(changes.borrow().as_slice(), &[0.5, 1.0, 0.75, 0.25]);
        let slider = runtime
            .render(window_id)?
            .semantics
            .into_iter()
            .find(|node| node.id == slider_id)
            .expect("slider semantics remain present");
        assert_eq!(
            slider.value,
            Some(SemanticsValue::Range {
                value: 0.25,
                min: 0.0,
                max: 1.0,
            })
        );
        assert_eq!(slider.numeric_step, Some(0.25));
        Ok(())
    }

    #[test]
    fn slider_value_when_syncs_external_value() -> Result<()> {
        let value = Rc::new(RefCell::new(0.25));
        let value_reader = Rc::clone(&value);
        let (mut runtime, window_id) = build_runtime(
            Slider::new("Opacity")
                .range(0.0, 1.0)
                .step(0.01)
                .value_when(move || *value_reader.borrow()),
        );

        let output = runtime.render(window_id)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present");
        assert_eq!(
            slider.value,
            Some(SemanticsValue::Range {
                value: 0.25,
                min: 0.0,
                max: 1.0,
            })
        );
        assert_eq!(slider.numeric_step, Some(0.01));

        *value.borrow_mut() = 0.75;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(200.0, 32.0))),
        )?;
        let output = runtime.render(window_id)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present after external update");
        assert_eq!(
            slider.value,
            Some(SemanticsValue::Range {
                value: 0.75,
                min: 0.0,
                max: 1.0,
            })
        );
        assert_eq!(slider.numeric_step, Some(0.01));
        Ok(())
    }

    #[test]
    fn slider_on_change_with_ctx_receives_value() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(200.0).height(32.0).with_child(
                Slider::new("Opacity")
                    .range(0.0, 1.0)
                    .step(0.01)
                    .on_change_with_ctx(move |ctx, value| {
                        on_change.borrow_mut().push(value);
                        ctx.request_semantics();
                    }),
            ),
        );
        let output = runtime.render(window_id)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present");
        let position = Point::new(
            slider.bounds.x() + (slider.bounds.width() * 0.5),
            slider.bounds.y() + (slider.bounds.height() * 0.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert!(
            changes
                .borrow()
                .last()
                .is_some_and(|value| (*value - 0.5).abs() < 1e-6)
        );
        Ok(())
    }

    #[test]
    fn slider_clears_hover_state_after_pointer_moves_off_control() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            Slider::new("Opacity").range(0.0, 1.0).step(0.25).value(0.5),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(20.0, 20.0), false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(4.0, 4.0), false),
        )?;

        let output = runtime.render(window_id)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Slider)
            .expect("slider semantics present");
        assert!(!slider.state.hovered);
        Ok(())
    }

    #[test]
    fn number_input_nudges_value_and_exposes_numeric_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            NumberInput::new("Count")
                .range(0.0, 10.0)
                .step(2.0)
                .value(4.0)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowUp", KeyState::Pressed)),
        )?;

        assert_eq!(changes.borrow().as_slice(), &[6.0]);

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("spinbox semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Range {
                value: 6.0,
                min: 0.0,
                max: 10.0,
            })
        );
        assert_eq!(input.numeric_step, Some(2.0));
        Ok(())
    }

    #[test]
    fn number_input_applies_typed_semantic_numeric_actions_through_step_and_callbacks() -> Result<()>
    {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            NumberInput::new("Count")
                .range(0.0, 10.0)
                .step(2.0)
                .precision(0)
                .value(4.0)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );
        let input_id = runtime
            .render(window_id)?
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("number input semantics present")
            .id;

        assert!(runtime.handle_semantics_action(
            window_id,
            input_id,
            SemanticsActionRequest::Increment,
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            input_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Range {
                value: 9.0,
                min: -100.0,
                max: 100.0,
            }),
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            input_id,
            SemanticsActionRequest::Decrement,
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            input_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Number(3.1)),
        )?);

        assert_eq!(changes.borrow().as_slice(), &[6.0, 10.0, 8.0, 4.0]);
        let input = runtime
            .render(window_id)?
            .semantics
            .into_iter()
            .find(|node| node.id == input_id)
            .expect("number input semantics remain present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Range {
                value: 4.0,
                min: 0.0,
                max: 10.0,
            })
        );
        assert_eq!(input.numeric_step, Some(2.0));
        Ok(())
    }

    #[test]
    fn number_input_preserves_raw_text_while_typing() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            NumberInput::new("Count")
                .range(0.0, 10.0)
                .precision(2)
                .value(0.0)
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Backspace", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("2", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new(".", KeyState::Pressed)),
        )?;

        assert_eq!(changes.borrow().as_slice(), &[2.0]);

        let output = runtime.render(window_id)?;
        let run = text_run_for(&output, "2.");
        assert_eq!(run.text, "2.");

        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("spinbox semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Range {
                value: 2.0,
                min: 0.0,
                max: 10.0,
            })
        );
        assert_eq!(input.numeric_step, Some(1.0));
        Ok(())
    }

    #[test]
    fn number_input_clears_hover_state_after_pointer_moves_off_control() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            NumberInput::new("Count")
                .range(0.0, 10.0)
                .step(1.0)
                .value(4.0),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(20.0, 20.0), false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(4.0, 4.0), false),
        )?;

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("spinbox semantics present");
        assert!(!input.state.hovered);
        Ok(())
    }

    #[test]
    fn number_input_retains_stepper_ink_when_feathering_is_enabled() {
        let root = crate::Padding::all(
            12.0,
            NumberInput::new("Count")
                .range(0.0, 20.0)
                .step(1.0)
                .value(12.0),
        );

        let (feathered_output, feathered_image) = render_rgba(root, true);
        let number_input_bounds = feathered_output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .map(|node| node.bounds)
            .expect("number input semantics present");

        let (_, hard_image) = render_rgba(
            crate::Padding::all(
                12.0,
                NumberInput::new("Count")
                    .range(0.0, 20.0)
                    .step(1.0)
                    .value(12.0),
            ),
            false,
        );

        let stepper_crop = Rect::new(
            number_input_bounds.max_x() - 32.0,
            number_input_bounds.y(),
            32.0,
            number_input_bounds.height(),
        );
        let feathered_ink = dark_pixel_count(&feathered_image, stepper_crop, 224);
        let hard_ink = dark_pixel_count(&hard_image, stepper_crop, 224);

        assert!(
            feathered_ink * 3 >= hard_ink * 2,
            "feathered number-input stepper lost too much dark ink (feathered={feathered_ink}, hard={hard_ink}, crop={stepper_crop:?})"
        );
    }

    #[test]
    fn number_input_value_text_visual_center_matches_control_center() {
        let output = render(NumberInput::new("Count").value(12.0));
        let text = text_run_for(&output, "12");
        let layout = text_run_layout(&text);
        let line = layout
            .lines()
            .first()
            .expect("number input text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn number_input_value_uses_tabular_figures_and_end_alignment() {
        let theme = DefaultTheme::default();
        let output = render(
            SizedBox::new()
                .width(180.0)
                .with_child(NumberInput::new("Count").precision(0).value(12.0)),
        );
        let text = text_run_for(&output, "12");
        let spinbox = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("number input semantics present");
        let content_right = spinbox.bounds.max_x()
            - theme.metrics.number_input_stepper_width
            - theme.metrics.text_input_padding.right;

        assert!(
            text.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((text.rect.max_x() - content_right).abs() < 1.0);
    }

    #[test]
    fn number_input_value_preserves_tall_measurement_and_end_alignment() {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.min_height = 64.0;
        let metrics = theme.metrics;
        let output = render_isolated(
            SizedBox::new().width(220.0).height(64.0).with_child(
                NumberInput::new("Count")
                    .theme(theme)
                    .precision(0)
                    .value(12.0),
            ),
        );
        let text = text_run_for(&output, "12");
        let layout = text_run_layout(&text);
        let line = layout
            .lines()
            .first()
            .expect("number input text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let spinbox = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("number input semantics present");
        let content_right = spinbox.bounds.max_x()
            - metrics.number_input_stepper_width
            - metrics.text_input_padding.right;

        assert_eq!(text.style.font_size, 28.0);
        assert_eq!(text.style.line_height, 12.0);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text.rect.max_x() - content_right).abs() < 1.0);
        let control_center = spinbox.bounds.y() + (spinbox.bounds.height() * 0.5);
        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn number_input_value_when_syncs_unfocused_external_value() {
        let value = Rc::new(RefCell::new(12.0));
        let value_reader = Rc::clone(&value);
        let (mut runtime, window_id) = build_runtime(
            NumberInput::new("Count")
                .range(0.0, 96.0)
                .precision(0)
                .value_when(move || *value_reader.borrow()),
        );

        let output = runtime.render(window_id).unwrap();
        let count = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Count")
            })
            .expect("number input semantics should exist");
        assert_eq!(
            count.value,
            Some(SemanticsValue::Range {
                value: 12.0,
                min: 0.0,
                max: 96.0,
            })
        );
        assert_eq!(count.numeric_step, Some(1.0));

        *value.borrow_mut() = 36.0;
        let position = Point::new(
            count.bounds.x() + (count.bounds.width() * 0.5),
            count.bounds.y() + (count.bounds.height() * 0.5),
        );
        let mut move_event = PointerEvent::new(PointerEventKind::Move, position);
        move_event.pointer_id = 1;
        runtime
            .handle_event(window_id, Event::Pointer(move_event))
            .unwrap();
        let output = runtime.render(window_id).unwrap();
        let count = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Count")
            })
            .expect("number input semantics should still exist");
        assert_eq!(
            count.value,
            Some(SemanticsValue::Range {
                value: 36.0,
                min: 0.0,
                max: 96.0,
            })
        );
        assert_eq!(count.numeric_step, Some(1.0));
        text_run_for(&output, "36");
    }

    #[test]
    fn text_area_supports_multiline_input() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes").on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "Line 1".to_string(),
            }),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "Line 2".to_string(),
            }),
        )?;

        assert_eq!(
            changes.borrow().last().map(String::as_str),
            Some("Line 1\nLine 2")
        );

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("Line 1\nLine 2".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_inputs_apply_typed_semantic_value_selection_and_edit_actions() -> Result<()> {
        let input_changes = Rc::new(RefCell::new(Vec::new()));
        let on_input_change = Rc::clone(&input_changes);
        let (mut input_runtime, input_window_id) = build_runtime(
            TextInput::new("Name")
                .value("alpha")
                .on_change(move |value| on_input_change.borrow_mut().push(value)),
        );
        let input_id = input_runtime
            .render(input_window_id)?
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text input semantics present")
            .id;

        assert!(input_runtime.handle_semantics_action(
            input_window_id,
            input_id,
            SemanticsActionRequest::SetSelection(SemanticsTextRange::new(0, 5)),
        )?);
        assert!(input_runtime.handle_semantics_action(
            input_window_id,
            input_id,
            SemanticsActionRequest::InsertText("Ada\nLovelace".to_string()),
        )?);
        let input = input_runtime
            .render(input_window_id)?
            .semantics
            .into_iter()
            .find(|node| node.id == input_id)
            .expect("text input semantics remain present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("AdaLovelace".to_string()))
        );
        assert_eq!(
            input_changes.borrow().as_slice(),
            &["AdaLovelace".to_string()]
        );

        let area_changes = Rc::new(RefCell::new(Vec::new()));
        let on_area_change = Rc::clone(&area_changes);
        let (mut area_runtime, area_window_id) = build_runtime(
            TextArea::new("Notes")
                .value("old")
                .on_change(move |value| on_area_change.borrow_mut().push(value)),
        );
        let area_id = area_runtime
            .render(area_window_id)?
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present")
            .id;
        assert!(area_runtime.handle_semantics_action(
            area_window_id,
            area_id,
            SemanticsActionRequest::SetValue(SemanticsValue::Text("Line 1\nLine 2".to_string())),
        )?);
        let area = area_runtime
            .render(area_window_id)?
            .semantics
            .into_iter()
            .find(|node| node.id == area_id)
            .expect("text area semantics remain present");
        assert_eq!(
            area.value,
            Some(SemanticsValue::Text("Line 1\nLine 2".to_string()))
        );
        assert_eq!(
            area_changes.borrow().as_slice(),
            &["Line 1\nLine 2".to_string()]
        );
        Ok(())
    }

    #[test]
    fn text_area_accepts_printable_key_without_text_payload() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes").on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(window_id, key_without_text("h"))?;

        assert_eq!(changes.borrow().last().map(String::as_str), Some("h"));
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        assert_eq!(input.value, Some(SemanticsValue::Text("h".to_string())));
        Ok(())
    }

    #[test]
    fn text_area_on_change_with_ctx_receives_text() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes")
                .on_change_with_ctx(move |_, value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "Line 1".to_string(),
            }),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;

        assert_eq!(
            changes.borrow().as_slice(),
            &["Line 1".to_string(), "Line 1\n".to_string()]
        );
        Ok(())
    }

    #[test]
    fn text_area_on_submit_fires_on_plain_enter_and_shift_enter_inserts_newline() -> Result<()> {
        let submits = Rc::new(RefCell::new(Vec::new()));
        let on_submit = Rc::clone(&submits);
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Composer")
                .value("hello")
                .on_submit(move |text| on_submit.borrow_mut().push(text.to_string())),
        );

        let _ = runtime.render(window_id)?;
        // Focus the composer.
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;

        // Shift+Enter inserts a newline rather than submitting.
        let mut shift_enter = KeyboardEvent::new("Enter", KeyState::Pressed);
        shift_enter.modifiers.shift = true;
        runtime.handle_event(window_id, Event::Keyboard(shift_enter))?;
        assert!(
            submits.borrow().is_empty(),
            "Shift+Enter must not submit; it inserts a newline"
        );

        // The Shift+Enter inserted exactly one newline into "hello" (the caret position depends on
        // the click hit-test, so assert on the newline count, not its placement).
        let after_shift = {
            let output = runtime.render(window_id)?;
            let input = output
                .semantics
                .iter()
                .find(|node| node.role == SemanticsRole::TextInput)
                .expect("text area semantics present");
            match input.value.clone() {
                Some(SemanticsValue::Text(text)) => text,
                other => panic!("unexpected semantics value: {other:?}"),
            }
        };
        assert_eq!(
            after_shift.matches('\n').count(),
            1,
            "Shift+Enter inserts exactly one newline"
        );

        // A plain Enter fires on_submit once with the current text (and does NOT insert another
        // newline — it is consumed).
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;
        assert_eq!(
            submits.borrow().as_slice(),
            std::slice::from_ref(&after_shift),
            "plain Enter submits the current text exactly once"
        );

        // The submit consumed the Enter, so the value is unchanged (no extra newline).
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text(after_shift)),
            "plain Enter does not append a second newline"
        );
        Ok(())
    }

    #[test]
    fn text_area_without_on_submit_inserts_newline_on_enter() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").value("a"));
        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;
        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("a\n".to_string())),
            "with no on_submit, Enter inserts a newline (backward-compatible)"
        );
        Ok(())
    }

    #[test]
    fn text_area_uses_shared_editor_commands_and_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            TextArea::new("Notes")
                .value("alpha\nbeta")
                .on_change(move |value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(18.0, 18.0), true),
        )?;
        runtime.handle_event(window_id, command_key("a"))?;
        runtime.handle_event(
            window_id,
            Event::Ime(ImeEvent::CompositionCommit {
                text: "gamma".to_string(),
            }),
        )?;
        runtime.handle_event(window_id, command_key("z"))?;
        runtime.handle_event(window_id, command_key("y"))?;

        let output = runtime.render(window_id)?;
        let input = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::TextInput)
            .expect("text area semantics present");
        assert_eq!(input.value, Some(SemanticsValue::Text("gamma".to_string())));
        let editable = input
            .editable_text
            .as_ref()
            .expect("text area should expose editable semantics");
        assert!(editable.multiline);
        assert_eq!(editable.caret_offset, "gamma".len());
        assert_eq!(
            editable.selection,
            SemanticsTextRange::new("gamma".len(), "gamma".len())
        );
        assert_eq!(changes.borrow().last().map(String::as_str), Some("gamma"));
        Ok(())
    }

    #[test]
    fn select_can_choose_option_from_keyboard() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Draft", "Final", "Review"])
                .on_change(move |_, value| on_change.borrow_mut().push(value)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
        )?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Enter", KeyState::Pressed)),
        )?;

        assert_eq!(changes.borrow().as_slice(), &["Final".to_string()]);

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Final".to_string()))
        );
        Ok(())
    }

    #[test]
    fn select_selected_when_reads_external_selection() -> Result<()> {
        let selected = Rc::new(RefCell::new(Some(1usize)));
        let selected_reader = Rc::clone(&selected);
        let (mut runtime, window_id) = build_runtime(
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Draft", "Final", "Review"])
                .selected_when(move || *selected_reader.borrow()),
        );

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Final".to_string()))
        );

        *selected.borrow_mut() = Some(2);
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 80.0))),
        )?;
        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after external selection changes");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Review".to_string()))
        );

        *selected.borrow_mut() = None;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 80.0))),
        )?;
        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after external selection clears");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Choose mode".to_string()))
        );
        Ok(())
    }

    #[test]
    fn radio_group_selected_when_reads_external_selection() -> Result<()> {
        let selected = Rc::new(RefCell::new(Some(1usize)));
        let selected_reader = Rc::clone(&selected);
        let (mut runtime, window_id) = build_runtime(
            RadioGroup::new("Mode")
                .options(["Manual", "Automatic", "Scheduled"])
                .selected_when(move || *selected_reader.borrow()),
        );

        let output = runtime.render(window_id)?;
        let group = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioGroup)
            .expect("radio group semantics present");
        assert_eq!(
            group.value,
            Some(SemanticsValue::Text("Automatic".to_string()))
        );

        *selected.borrow_mut() = Some(2);
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 120.0))),
        )?;
        let output = runtime.render(window_id)?;
        let group = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioGroup)
            .expect("radio group semantics present after external selection changes");
        assert_eq!(
            group.value,
            Some(SemanticsValue::Text("Scheduled".to_string()))
        );

        *selected.borrow_mut() = None;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 120.0))),
        )?;
        let output = runtime.render(window_id)?;
        let group = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::RadioGroup)
            .expect("radio group semantics present after external selection clears");
        assert_eq!(group.value, None);
        Ok(())
    }

    #[test]
    fn select_clears_hover_state_after_pointer_moves_off_control() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Draft", "Final", "Review"]),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(20.0, 20.0), false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(4.0, 4.0), false),
        )?;

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        assert!(!select.state.hovered);
        Ok(())
    }

    #[test]
    fn expanded_select_menu_uses_overlay_surface_layer_metadata() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Draft", "Final", "Review"]),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        let descriptor =
            overlay_layer_descriptor(&output).expect("select menu overlay layer present");

        assert_eq!(select.state.expanded, Some(true));
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
        assert!(
            layer_descriptor_for(&output, select.id).is_none(),
            "the combobox trigger should not own the floating menu layer"
        );
        Ok(())
    }

    #[test]
    fn expanded_select_menu_entrance_uses_theme_motion_layer_properties() -> Result<()> {
        let theme = slow_normal_motion_theme();
        let duration = theme.motion.entrance_duration();
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            Select::new("Mode")
                .theme(theme)
                .placeholder("Choose mode")
                .options(["Draft", "Final", "Review"]),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let start = runtime.render(window_id)?;
        let start_descriptor =
            overlay_layer_descriptor(&start).expect("select menu overlay layer present");
        let menu_owner = overlay_layer_owner(&start).expect("select menu overlay owner present");
        assert_eq!(start_descriptor.properties.opacity, 0.0);
        assert!(start_descriptor.properties.translation.y < 0.0);

        runtime.tick(duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let mid_descriptor =
            overlay_layer_descriptor(&mid).expect("select menu overlay layer still present");
        assert!(mid_descriptor.properties.opacity > 0.0);
        assert!(mid_descriptor.properties.opacity < 1.0);
        assert!(mid_descriptor.properties.translation.y < 0.0);
        assert!(
            mid_descriptor.properties.translation.y.abs()
                < start_descriptor.properties.translation.y.abs()
        );
        assert!(
            mid.frame.layer_updates.iter().any(|update| {
                update.owner == menu_owner
                    && matches!(
                        update.kind,
                        SceneLayerUpdateKind::Transform | SceneLayerUpdateKind::Effect
                    )
            }),
            "select menu entrance should update retained layer properties"
        );
        assert!(
            !mid.frame.layer_updates.iter().any(|update| {
                update.owner == menu_owner && update.kind == SceneLayerUpdateKind::Content
            }),
            "select menu entrance should not repaint option content"
        );
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled = runtime.render(window_id)?;
        let settled_descriptor =
            overlay_layer_descriptor(&settled).expect("select menu overlay layer still present");
        assert_eq!(settled_descriptor.properties.opacity, 1.0);
        assert_eq!(settled_descriptor.properties.translation.y, 0.0);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn expanded_select_does_not_reflow_following_widgets() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            crate::Stack::vertical()
                .spacing(10.0)
                .with_child(Select::new("Mode").placeholder("Choose mode").options([
                    "Automatic",
                    "Linear",
                    "Gamma",
                ]))
                .with_child(NumberInput::new("Gamma").value(1.4)),
        ));

        let before = runtime.render(window_id)?;
        let spin_before = before
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("spin box semantics present before expand")
            .bounds;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let after = runtime.render(window_id)?;
        let spin_after = after
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::SpinBox)
            .expect("spin box semantics present after expand")
            .bounds;
        let select = after
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let descriptor =
            overlay_layer_descriptor(&after).expect("select menu overlay layer present");

        assert_eq!(spin_before.y(), spin_after.y());
        assert!(descriptor.paint_bounds.max_y() > select.bounds.max_y());
        Ok(())
    }

    #[test]
    fn expanded_select_accepts_pointer_selection_in_floating_menu() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            crate::Stack::vertical()
                .spacing(10.0)
                .with_child(
                    Select::new("Mode")
                        .placeholder("Choose mode")
                        .options(["Automatic", "Linear", "Gamma"])
                        .on_change(move |_, value| on_change.borrow_mut().push(value)),
                )
                .with_child(NumberInput::new("Gamma").value(1.4)),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let expanded = runtime.render(window_id)?;
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let menu = overlay_layer_descriptor(&expanded).expect("select menu overlay present");
        let option_point = Point::new(
            menu.bounds.x() + 20.0,
            menu.bounds.y() + (select.bounds.height() * 1.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, option_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, option_point, false),
        )?;

        assert_eq!(changes.borrow().as_slice(), &["Linear".to_string()]);

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after pointer selection");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Linear".to_string()))
        );
        Ok(())
    }

    #[test]
    fn expanded_select_flips_above_when_below_space_is_constrained() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().size(Size::new(220.0, 180.0)).with_child(
                crate::Stack::vertical()
                    .with_child(SizedBox::new().height(128.0))
                    .with_child(
                        Select::new("Mode")
                            .placeholder("Choose mode")
                            .options(["Automatic", "Linear", "Gamma"])
                            .on_change(move |_, value| on_change.borrow_mut().push(value)),
                    ),
            ),
        );

        let initial = runtime.render(window_id)?;
        let select_bounds = initial
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present before expand")
            .bounds;
        let header_point = Point::new(
            select_bounds.x() + 20.0,
            select_bounds.y() + (select_bounds.height() * 0.5),
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, header_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, header_point, false),
        )?;
        runtime.tick(entrance_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);

        let expanded = runtime.render(window_id)?;
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let descriptor =
            overlay_layer_descriptor(&expanded).expect("select menu overlay layer present");

        assert!(descriptor.paint_bounds.y() < select.bounds.y());

        let option_point = Point::new(
            select.bounds.x() + 20.0,
            select.bounds.y() - super::SELECT_MENU_GAP - (select.bounds.height() * 1.5),
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, option_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, option_point, false),
        )?;

        assert_eq!(changes.borrow().as_slice(), &["Linear".to_string()]);
        Ok(())
    }

    #[test]
    fn expanded_select_popover_paints_outside_layout_bounds() -> Result<()> {
        let root = crate::Background::new(
            Brush::Solid(Color::srgba(0.04, 0.045, 0.055, 1.0)),
            SizedBox::new().size(Size::new(220.0, 180.0)).with_child(
                crate::Stack::vertical()
                    .with_child(SizedBox::new().height(128.0))
                    .with_child(Select::new("Mode").placeholder("Choose mode").options([
                        "Automatic",
                        "Linear",
                        "Gamma",
                    ])),
            ),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let mut renderer = WgpuRenderer::default().with_feathering_enabled(false);

        let initial = runtime.render(window_id)?;
        renderer.render(&initial.frame)?;
        let select_bounds = initial
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present before expand")
            .bounds;
        let header_point = Point::new(
            select_bounds.x() + 20.0,
            select_bounds.y() + (select_bounds.height() * 0.5),
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, header_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, header_point, false),
        )?;
        runtime.tick(entrance_duration());
        assert_eq!(handle_ready_events(&mut runtime)?, 1);

        let expanded = runtime.render(window_id)?;
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let descriptor =
            overlay_layer_descriptor(&expanded).expect("select menu overlay layer present");
        assert!(descriptor.paint_bounds.y() < select.bounds.y());

        renderer.render(&expanded.frame)?;
        let image = renderer.capture_last_frame_rgba(window_id)?;
        let menu_probe = Rect::new(
            select.bounds.x() + 4.0,
            select.bounds.y() - super::SELECT_MENU_GAP - (select.bounds.height() * 3.0) + 4.0,
            select.bounds.width() - 8.0,
            (select.bounds.height() * 3.0) - 8.0,
        );
        let bright_pixels = bright_pixel_count(&image, menu_probe, 160);

        assert!(
            bright_pixels > 200,
            "expanded select menu should paint outside layout bounds; bright_pixels={bright_pixels}, menu_probe={menu_probe:?}, select_bounds={:?}, paint_bounds={:?}",
            select.bounds,
            descriptor.paint_bounds
        );
        Ok(())
    }

    #[test]
    fn select_header_text_visual_center_matches_control_center() {
        let output = render(Select::new("Mode").placeholder("Choose mode").options([
            "Automatic",
            "Linear",
            "Gamma",
        ]));
        let text = text_run_for(&output, "Choose mode");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("select header text should shape");
        let line = layout
            .lines()
            .first()
            .expect("select header text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let control_center = output.frame.viewport.height * 0.5;

        assert!((actual_visual_center - control_center).abs() < 0.75);
    }

    #[test]
    fn select_chevron_icon_centers_in_reserved_slot() {
        let output = render(
            SizedBox::new().width(220.0).with_child(
                Select::new("Mode")
                    .options(["Automatic", "Linear"])
                    .selected(0),
            ),
        );
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        let slot = Rect::new(
            select.bounds.max_x() - super::SELECT_CHEVRON_SLOT_WIDTH,
            select.bounds.y(),
            super::SELECT_CHEVRON_SLOT_WIDTH,
            select.bounds.height(),
        );
        let chevron = lucide_strokes(&output)
            .into_iter()
            .map(|(bounds, _, stroke)| {
                let side = stroke.width * 12.0;
                Rect::new(
                    bounds.x() + (bounds.width() - side) * 0.5,
                    bounds.y() + (bounds.height() - side) * 0.5,
                    side,
                    side,
                )
            })
            .find(|rect| slot.contains(super::rect_center(*rect)))
            .expect("select chevron should paint as a native Lucide path");

        assert!((super::rect_center(chevron).x - super::rect_center(slot).x).abs() < 0.75);
        assert!((super::rect_center(chevron).y - super::rect_center(slot).y).abs() < 0.75);
        assert!((chevron.width() - super::SELECT_CHEVRON_ICON_SIZE).abs() < 0.75);
        assert!((chevron.height() - super::SELECT_CHEVRON_ICON_SIZE).abs() < 0.75);
    }

    #[test]
    fn select_header_placeholder_clips_before_chevron_slot() {
        let theme = DefaultTheme::default();
        let placeholder = "Choose an extremely detailed rendering pipeline preset";
        let output =
            render(SizedBox::new().width(180.0).with_child(
                Select::new("Mode").placeholder(placeholder).options([
                    "Automatic",
                    "Linear",
                    "Gamma",
                ]),
            ));
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        let text = text_run_for(&output, placeholder);
        let clip = draw_clip_rect_for(&output, placeholder);
        let expected_clip_max_x = select.bounds.max_x()
            - theme.metrics.text_input_padding.right
            - super::SELECT_CHEVRON_SLOT_WIDTH;

        assert_eq!(text.style.color, theme.placeholder_text_style().color);
        assert!((clip.max_x() - expected_clip_max_x).abs() < 0.75);
        assert!(clip.max_x() <= select.bounds.max_x() - super::SELECT_CHEVRON_SLOT_WIDTH + 0.75);
        assert!(
            (text_run_visual_center(&text) - (select.bounds.y() + select.bounds.height() * 0.5))
                .abs()
                < 0.75
        );
    }

    #[test]
    fn select_header_and_options_preserve_tall_measurement_centering() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.text.sm.size = 28.0;
        theme.text.sm.line_height = 10.0;
        theme.sync_derived_fields();
        theme.metrics.min_height = 52.0;
        let placeholder = "Choose mode";
        let option = "Automatic";
        let (mut runtime, window_id) = build_runtime(
            Select::new("Mode")
                .theme(theme)
                .placeholder(placeholder)
                .options([option, "Linear", "Gamma"]),
        );

        let collapsed = runtime.render(window_id)?;
        let select = collapsed
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        let placeholder_text = text_run_for(&collapsed, placeholder);
        let placeholder_layout = shaped_text_layout_for(&collapsed, placeholder);
        let placeholder_clip = draw_clip_rect_for(&collapsed, placeholder);
        let expected_clip_max_x = select.bounds.max_x()
            - theme.metrics.text_input_padding.right
            - super::SELECT_CHEVRON_SLOT_WIDTH;

        assert_eq!(
            placeholder_text.style.font_size,
            theme.typography.body_font_size
        );
        assert_eq!(
            placeholder_text.style.line_height,
            theme.typography.body_line_height
        );
        assert_eq!(
            placeholder_text.style.color,
            theme.placeholder_text_style().color
        );
        assert!((placeholder_clip.max_x() - expected_clip_max_x).abs() < 0.75);
        assert!(
            (text_run_visual_center(&placeholder_text) - super::rect_center(select.bounds).y).abs()
                < 0.75,
            "select placeholder should visually center in the header; rect={:?}, bounds={:?}, measurement={:?}",
            placeholder_text.rect,
            select.bounds,
            placeholder_layout.measurement()
        );

        let header_point = super::rect_center(select.bounds);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, header_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, header_point, false),
        )?;

        let expanded = runtime.render(window_id)?;
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let option_text = text_run_for(&expanded, option);
        let option_layout = shaped_text_layout_for(&expanded, option);
        let option_clip = draw_clip_rect_for(&expanded, option);
        let menu = overlay_layer_descriptor(&expanded).expect("select menu overlay present");
        let row = Rect::new(
            menu.bounds.x(),
            menu.bounds.y(),
            menu.bounds.width(),
            select.bounds.height(),
        );
        let expected_option_clip =
            super::horizontal_text_inset_rect(row, theme.metrics.text_input_padding);

        assert_eq!(option_text.style.font_size, theme.typography.body_font_size);
        assert_eq!(
            option_text.style.line_height,
            theme.typography.body_line_height
        );
        assert!((option_clip.x() - expected_option_clip.x()).abs() < 0.75);
        assert!((option_clip.max_x() - expected_option_clip.max_x()).abs() < 0.75);
        assert!(
            (text_run_visual_center(&option_text) - super::rect_center(row).y).abs() < 0.75,
            "select option should visually center in its row; rect={:?}, row={:?}, measurement={:?}",
            option_text.rect,
            row,
            option_layout.measurement()
        );
        Ok(())
    }

    #[test]
    fn expanded_select_option_text_visual_center_matches_row_center() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(Select::new("Mode").placeholder("Choose mode").options([
                "Automatic",
                "Linear",
                "Gamma",
            ]));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let output = runtime.render(window_id)?;
        let select = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present");
        let text = text_run_for(&output, "Automatic");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("select menu option text should shape");
        let line = layout
            .lines()
            .first()
            .expect("select menu option text should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let menu = overlay_layer_descriptor(&output).expect("select menu overlay present");
        let row_center = menu.bounds.y() + (select.bounds.height() * 0.5);

        assert!((actual_visual_center - row_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn closed_select_does_not_block_immediate_clicks_before_next_render() -> Result<()> {
        let presses = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&presses);
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            crate::Stack::vertical()
                .spacing(4.0)
                .with_child(Select::new("Mode").placeholder("Choose mode").options([
                    "Automatic",
                    "Linear",
                    "Gamma",
                    "Display P3",
                    "HDR",
                ]))
                .with_child(Button::new("Apply").on_press(move || {
                    *on_press.borrow_mut() += 1;
                })),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let expanded = runtime.render(window_id)?;
        let button = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("button semantics present after expand")
            .bounds;
        let descriptor =
            overlay_layer_descriptor(&expanded).expect("select menu overlay layer present");

        assert!(descriptor.paint_bounds.intersection(button).is_some());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let button_center = Point::new(
            button.x() + (button.width() * 0.5),
            button.y() + (button.height() * 0.5),
        );
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, button_center, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, button_center, false),
        )?;

        assert_eq!(*presses.borrow(), 1);
        Ok(())
    }

    #[test]
    fn outside_click_closes_select_without_blocking_following_interactions() -> Result<()> {
        let presses = Rc::new(RefCell::new(0usize));
        let on_press = Rc::clone(&presses);
        let (mut runtime, window_id) = build_runtime(crate::Padding::all(
            12.0,
            crate::Stack::vertical()
                .spacing(4.0)
                .with_child(Select::new("Mode").placeholder("Choose mode").options([
                    "Automatic",
                    "Linear",
                    "Gamma",
                    "Display P3",
                    "HDR",
                ]))
                .with_child(Button::new("Apply").on_press(move || {
                    *on_press.borrow_mut() += 1;
                })),
        ));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(20.0, 20.0), false),
        )?;

        let expanded = runtime.render(window_id)?;
        let button = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button)
            .expect("button semantics present after expand")
            .bounds;
        let outside_point = Point::new(
            button.x() + (button.width() * 0.5),
            button.y() + (button.height() * 0.5),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, outside_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, outside_point, false),
        )?;

        assert_eq!(*presses.borrow(), 0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, outside_point, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, outside_point, false),
        )?;

        assert_eq!(*presses.borrow(), 1);
        Ok(())
    }

    #[test]
    fn select_retains_chevron_ink_when_feathering_is_enabled() {
        let root = crate::Padding::all(
            12.0,
            Select::new("Mode")
                .placeholder("Choose mode")
                .options(["Normal", "Multiply", "Screen"])
                .selected(0),
        );

        let (feathered_output, feathered_image) = render_rgba(root, true);
        let select_bounds = feathered_output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .map(|node| node.bounds)
            .expect("select semantics present");

        let (_, hard_image) = render_rgba(
            crate::Padding::all(
                12.0,
                Select::new("Mode")
                    .placeholder("Choose mode")
                    .options(["Normal", "Multiply", "Screen"])
                    .selected(0),
            ),
            false,
        );

        let chevron_crop = Rect::new(
            select_bounds.max_x() - 30.0,
            select_bounds.y(),
            30.0,
            select_bounds.height(),
        );
        let feathered_ink = dark_pixel_count(&feathered_image, chevron_crop, 224);
        let hard_ink = dark_pixel_count(&hard_image, chevron_crop, 224);

        assert!(
            feathered_ink * 3 >= hard_ink * 2,
            "feathered select chevron lost too much dark ink (feathered={feathered_ink}, hard={hard_ink}, crop={chevron_crop:?})"
        );
    }
}
