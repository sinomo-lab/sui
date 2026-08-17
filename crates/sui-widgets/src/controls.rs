use crate::{
    Blink, ControlMetrics, DefaultTheme, HdrThemeMode, Interpolate, MotionScalar,
    ResolvedEffectStyle, ResolvedHdrStyle, SemanticTone, ThemeColorScheme, WidgetColorRole,
    WidgetLuminanceRole, WidgetMaterialRole,
    editable_text::{
        EditableTextController, EditableTextLineMode, keyboard_text, paste_command,
        single_line_text,
    },
    editor::{EditorCommand, EditorCommandResult, selection_range},
    overlay::{OverlayPlacement, OverlayPlacementRequest, place_overlay},
    paint_theme_shadow, resolve_luminance_role, resolve_widget_hdr_style,
    selection::{SelectionChange, SelectionClipboardBehavior, SelectionOwnerId, SelectionScope},
    text_align::{
        HorizontalTextAlignmentMode, aligned_text_rect_for_layout,
        aligned_text_rect_for_layout_with_mode, aligned_text_rect_for_text, paint_aligned_text,
        paint_single_line_aligned_text,
    },
    text_command::TextCommand,
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

mod text_display;
pub use text_display::{Label, Link};

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
        ctx.push_clip_rect(label_slot);
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
        ctx.pop_clip();
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

    fn resolved_value(&self) -> f64 {
        if self.dragging {
            return self.value;
        }

        self.value_reader
            .as_ref()
            .map(|reader| clamp_and_snap_value(reader(), self.min, self.max, self.step))
            .unwrap_or(self.value)
    }

    fn fraction_for(&self, value: f64) -> f32 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            return 0.0;
        }

        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
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

    fn thumb_rect_for(&self, bounds: Rect, value: f64) -> Rect {
        let track = self.track_rect(bounds);
        let theme = self.resolved_theme();
        let thumb = theme.metrics.slider_thumb_size;
        Rect::new(
            track.x() + (track.width() * self.fraction_for(value)) - (thumb * 0.5),
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
        let value = self.resolved_value();
        let track = self.track_rect(ctx.bounds());
        let active = Rect::new(
            track.x(),
            track.y(),
            track.width() * self.fraction_for(value),
            track.height(),
        );
        let thumb = self.thumb_rect_for(ctx.bounds(), value);

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
            value: self.resolved_value(),
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
    editor: EditableTextController,
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
            editor: EditableTextController::new(),
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
        self.editor.selectable(selection_scope);
        self
    }

    pub fn selection_scope(mut self, selection_scope: SelectionScope) -> Self {
        self.editor.selection_scope(selection_scope);
        self
    }

    pub fn clipboard_behavior(mut self, behavior: SelectionClipboardBehavior) -> Self {
        self.editor.clipboard_behavior(behavior);
        self
    }

    pub fn copy_to_clipboard(self, enabled: bool) -> Self {
        self.clipboard_behavior(if enabled {
            SelectionClipboardBehavior::WidgetManaged
        } else {
            SelectionClipboardBehavior::AppManaged
        })
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
        let handled = result.handled;
        self.editor.apply_result_common(ctx, &mut result);
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
            ctx.request_semantics();
        }
        if handled {
            if self.focused {
                self.reset_caret_blink(ctx);
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
        let command = paste_command(ctx, EditableTextLineMode::MultiLine);
        self.execute_editor_command(ctx, command);
    }

    /// Currently selected document text (empty when the selection is
    /// collapsed).
    pub fn selected_text(&self) -> &str {
        self.editor.selected_text()
    }

    fn apply_text_command(&mut self, ctx: &mut EventCtx, command: TextCommand) {
        if let Some(command) = self.editor.text_command(
            ctx,
            command,
            self.read_only,
            EditableTextLineMode::MultiLine,
        ) {
            self.execute_editor_command(ctx, command);
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
                    let anchor = self.editor.selection().anchor.utf8_offset;
                    let result = self.editor.execute(EditorCommand::SetSelection {
                        anchor,
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
                if let Some(commands) = self.editor.semantics_commands(
                    ctx,
                    &semantics.action,
                    self.read_only,
                    EditableTextLineMode::MultiLine,
                ) {
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
                } else if let Some(command) = self.editor.keyboard_command(
                    ctx,
                    key,
                    self.read_only,
                    EditableTextLineMode::MultiLine,
                ) {
                    self.execute_editor_command(ctx, command);
                }
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                if let Some(command) = self.editor.keyboard_command(
                    ctx,
                    key,
                    self.read_only,
                    EditableTextLineMode::MultiLine,
                ) {
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
        node.actions = self.editor.semantic_actions(self.read_only);
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
    editor: EditableTextController,
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
            editor: EditableTextController::new(),
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
        self.editor.selectable(selection_scope);
        self
    }

    pub fn selection_scope(mut self, selection_scope: SelectionScope) -> Self {
        self.editor.selection_scope(selection_scope);
        self
    }

    pub fn clipboard_behavior(mut self, behavior: SelectionClipboardBehavior) -> Self {
        self.editor.clipboard_behavior(behavior);
        self
    }

    pub fn copy_to_clipboard(self, enabled: bool) -> Self {
        self.clipboard_behavior(if enabled {
            SelectionClipboardBehavior::WidgetManaged
        } else {
            SelectionClipboardBehavior::AppManaged
        })
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
        let handled = result.handled;
        self.editor.apply_result_common(ctx, &mut result);
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
            ctx.request_semantics();
        }
        if handled {
            if self.focused {
                self.reset_caret_blink(ctx);
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
        let command = paste_command(ctx, EditableTextLineMode::SingleLine);
        self.execute_editor_command(ctx, command);
    }

    /// Currently selected document text (empty when the selection is
    /// collapsed).
    pub fn selected_text(&self) -> &str {
        self.editor.selected_text()
    }

    fn apply_text_command(&mut self, ctx: &mut EventCtx, command: TextCommand) {
        if let Some(command) = self.editor.text_command(
            ctx,
            command,
            self.read_only,
            EditableTextLineMode::SingleLine,
        ) {
            self.execute_editor_command(ctx, command);
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
                    let anchor = self.editor.selection().anchor.utf8_offset;
                    let result = self.editor.execute(EditorCommand::SetSelection {
                        anchor,
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
                if let Some(commands) = self.editor.semantics_commands(
                    ctx,
                    &semantics.action,
                    self.read_only,
                    EditableTextLineMode::SingleLine,
                ) {
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
            Event::Keyboard(key) if key.state == KeyState::Pressed && ctx.is_focused() => {
                if let Some(command) = self.editor.keyboard_command(
                    ctx,
                    key,
                    self.read_only,
                    EditableTextLineMode::SingleLine,
                ) {
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
        node.actions = self.editor.semantic_actions(self.read_only);
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

    pub fn clipboard_behavior(mut self, behavior: SelectionClipboardBehavior) -> Self {
        self.inner = self.inner.clipboard_behavior(behavior);
        self
    }

    pub fn copy_to_clipboard(mut self, enabled: bool) -> Self {
        self.inner = self.inner.copy_to_clipboard(enabled);
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

    pub fn clipboard_behavior(mut self, behavior: SelectionClipboardBehavior) -> Self {
        self.inner = self.inner.clipboard_behavior(behavior);
        self
    }

    pub fn copy_to_clipboard(mut self, enabled: bool) -> Self {
        self.inner = self.inner.copy_to_clipboard(enabled);
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

#[cfg(test)]
mod tests;
