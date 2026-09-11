use crate::DefaultTheme;
use crate::SemanticTone;
use crate::composites::forms::set_hover_animation_target;
use crate::composites::indicators::{inset_rect, mix_color, physical_pixels, rounded_rect_path};
use crate::composites::popups::AnimatedScalar;
use crate::paint_theme_shadow;
use sui_core::Color;
use sui_core::Event;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::Size;
use sui_core::WakeEvent;
use sui_core::WidgetId;
use sui_layout::Constraints;
use sui_layout::Padding as Insets;
use sui_runtime::ArrangeCtx;
use sui_runtime::EventCtx;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::SingleChild;
use sui_runtime::Widget;
use sui_runtime::WidgetPod;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::StrokeStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Window,
    Sidebar,
    Panel,
    Titlebar,
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceBorder {
    None,
    All,
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceElevation {
    None,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SurfaceAppearance {
    /// The surface token associated with [`SurfaceRole`].
    #[default]
    Standard,
    /// The shared raised-surface token, useful for elevated chips and cards.
    Raised,
    /// A low-emphasis semantic wash.
    Soft,
    /// A solid semantic fill.
    Filled,
}

pub struct Surface {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: Option<String>,
    pub(super) role: SurfaceRole,
    pub(super) appearance: SurfaceAppearance,
    pub(super) tone: SemanticTone,
    pub(super) border: SurfaceBorder,
    pub(super) elevation: SurfaceElevation,
    pub(super) radius: f32,
    pub(super) padding: Insets,
    pub(super) fill_width: bool,
    pub(super) fill_height: bool,
    pub(super) child: SingleChild,
}

impl Surface {
    pub fn new<W>(role: SurfaceRole, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            role,
            appearance: SurfaceAppearance::Standard,
            tone: SemanticTone::Neutral,
            border: SurfaceBorder::None,
            elevation: SurfaceElevation::None,
            radius: 0.0,
            padding: Insets::ZERO,
            fill_width: false,
            fill_height: false,
            child: SingleChild::new(child),
        }
    }

    pub fn window<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Window, child)
    }

    pub fn sidebar<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Sidebar, child).border(SurfaceBorder::Right)
    }

    pub fn panel<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Panel, child)
            .border(SurfaceBorder::All)
            .radius(8.0)
    }

    pub fn titlebar<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Titlebar, child).border(SurfaceBorder::Bottom)
    }

    pub fn field<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::new(SurfaceRole::Field, child)
            .border(SurfaceBorder::All)
            .radius(6.0)
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

    pub fn border(mut self, border: SurfaceBorder) -> Self {
        self.border = border;
        self
    }

    pub fn appearance(mut self, appearance: SurfaceAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn elevation(mut self, elevation: SurfaceElevation) -> Self {
        self.elevation = elevation;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn fill(mut self) -> Self {
        self.fill_width = true;
        self.fill_height = true;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.fill_height = true;
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn background_for_role(theme: &DefaultTheme, role: SurfaceRole) -> Color {
        match role {
            SurfaceRole::Window => theme.surfaces.window,
            SurfaceRole::Sidebar => theme.surfaces.sidebar,
            SurfaceRole::Panel => theme.surfaces.panel,
            SurfaceRole::Titlebar => theme.surfaces.titlebar,
            SurfaceRole::Field => theme.surfaces.field,
        }
    }

    pub(super) fn resolved_colors(&self, theme: &DefaultTheme) -> (Color, Color) {
        match self.appearance {
            SurfaceAppearance::Standard => (
                Self::background_for_role(theme, self.role),
                theme.surfaces.border,
            ),
            SurfaceAppearance::Raised => (theme.palette.surface_raised, theme.surfaces.border),
            SurfaceAppearance::Soft => {
                let (fill, _) = theme.semantic_tone_soft_colors(self.tone);
                let border = if self.tone == SemanticTone::Neutral {
                    theme.surfaces.border
                } else {
                    theme.semantic_tone_color(self.tone).with_alpha(0.36)
                };
                (fill, border)
            }
            SurfaceAppearance::Filled => {
                let (fill, _) = theme.semantic_tone_colors(self.tone);
                (fill, fill)
            }
        }
    }

    pub(super) fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }
}

impl Widget for Surface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_child = Size::new(
            if constraints.max.width.is_finite() {
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
            } else {
                f32::INFINITY
            },
            if constraints.max.height.is_finite() {
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0)
            } else {
                f32::INFINITY
            },
        );
        // `arrange` always gives the child the complete content rect. Keep the
        // measurement constraints consistent with that contract so
        // width-dependent children (notably wrapping text in a flex item) are
        // not measured narrowly and then stretched without being remeasured.
        // A filling surface makes the corresponding content axis tight even
        // when its own parent supplied loose constraints.
        let min_child = Size::new(
            if self.fill_width && max_child.width.is_finite() {
                max_child.width
            } else {
                (constraints.min.width - self.padding.left - self.padding.right)
                    .max(0.0)
                    .min(max_child.width)
            },
            if self.fill_height && max_child.height.is_finite() {
                max_child.height
            } else {
                (constraints.min.height - self.padding.top - self.padding.bottom)
                    .max(0.0)
                    .min(max_child.height)
            },
        );
        let child_size = self
            .child
            .measure(ctx, Constraints::new(min_child, max_child));
        let mut size = Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        );
        if self.fill_width && constraints.max.width.is_finite() {
            size.width = constraints.max.width;
        }
        if self.fill_height && constraints.max.height.is_finite() {
            size.height = constraints.max.height;
        }
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let radius = self.radius.min(bounds.width().min(bounds.height()) * 0.5);

        let shadow = match self.elevation {
            SurfaceElevation::None => None,
            SurfaceElevation::Small => Some(&theme.shadows.box_shadow.sm),
            SurfaceElevation::Medium => Some(&theme.shadows.box_shadow.md),
            SurfaceElevation::Large => Some(&theme.shadows.box_shadow.lg),
        };
        if let Some(shadow) = shadow {
            paint_theme_shadow(ctx, bounds, [radius; 4], shadow);
        }

        let (background, border) = self.resolved_colors(&theme);
        if radius > 0.0 {
            ctx.fill(rounded_rect_path(bounds, radius), background);
        } else {
            ctx.fill_rect(bounds, background);
        }

        let stroke_width = physical_pixels(ctx, theme.metrics.border_width.max(1.0));
        match self.border {
            SurfaceBorder::None => {}
            SurfaceBorder::All => {
                ctx.stroke(
                    rounded_rect_path(bounds, radius),
                    border,
                    StrokeStyle::new(stroke_width),
                );
            }
            SurfaceBorder::Top => ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), bounds.width(), stroke_width),
                border,
            ),
            SurfaceBorder::Right => ctx.fill_rect(
                Rect::new(
                    bounds.max_x() - stroke_width,
                    bounds.y(),
                    stroke_width,
                    bounds.height(),
                ),
                border,
            ),
            SurfaceBorder::Bottom => ctx.fill_rect(
                Rect::new(
                    bounds.x(),
                    bounds.max_y() - stroke_width,
                    bounds.width(),
                    stroke_width,
                ),
                border,
            ),
            SurfaceBorder::Left => ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), stroke_width, bounds.height()),
                border,
            ),
        }

        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(name) = &self.name {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(name.clone());
            ctx.push(node);
        }
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

/// A reusable field frame for compound editors such as search/composer rows.
///
/// The child owns editing, focus, and semantics. The frame owns only the
/// standard field surface and border, which avoids applications repainting
/// control chrome around otherwise stock SUI editors. Use [`Self::focused_when`]
/// when the wrapped editor publishes its focus state.
pub struct FramedField {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) padding: Insets,
    pub(super) min_height: Option<f32>,
    pub(super) fill_width: bool,
    pub(super) focused: Option<bool>,
    pub(super) focused_reader: Option<Box<dyn Fn() -> bool>>,
    pub(super) invalid: bool,
    pub(super) invalid_reader: Option<Box<dyn Fn() -> bool>>,
    pub(super) hovered: bool,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) child: SingleChild,
}

impl FramedField {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            description: None,
            padding: Insets::ZERO,
            min_height: None,
            fill_width: false,
            focused: None,
            focused_reader: None,
            invalid: false,
            invalid_reader: None,
            hovered: false,
            hover_animation: AnimatedScalar::new(0.0),
            child: SingleChild::new(child),
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = Some(min_height.max(0.0));
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self.focused_reader = None;
        self
    }

    pub fn focused_when<F>(mut self, focused: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.focused_reader = Some(Box::new(focused));
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self.invalid_reader = None;
        self
    }

    pub fn invalid_when<F>(mut self, invalid: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.invalid_reader = Some(Box::new(invalid));
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn child_contains(&self, target: WidgetId) -> bool {
        if self.child.child().id() == target {
            return true;
        }
        struct Finder {
            target: WidgetId,
            found: bool,
        }
        impl WidgetPodVisitor for Finder {
            fn visit(&mut self, child: &WidgetPod) {
                if self.found {
                    return;
                }
                if child.id() == self.target {
                    self.found = true;
                } else {
                    child.visit_children(self);
                }
            }
        }
        let mut finder = Finder {
            target,
            found: false,
        };
        self.child.child().visit_children(&mut finder);
        finder.found
    }

    pub(super) fn is_focused(&self, focused_widget_id: Option<WidgetId>) -> bool {
        if let Some(focused) = &self.focused_reader {
            return focused();
        }
        if let Some(focused) = self.focused {
            return focused;
        }
        focused_widget_id
            .map(|focused| self.child_contains(focused))
            .unwrap_or(false)
    }

    pub(super) fn is_invalid(&self) -> bool {
        self.invalid_reader
            .as_ref()
            .map(|invalid| invalid())
            .unwrap_or(self.invalid)
    }

    pub(super) fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.padding)
    }

    pub(super) fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        let theme = self.resolved_theme();
        set_hover_animation_target(&mut self.hover_animation, hovered as u8 as f32, &theme, ctx);
        ctx.request_paint();
    }
}

impl Widget for FramedField {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let previous = self.hover_animation.value;
                if self.hover_animation.advance(*time) {
                    ctx.request_animation_frame();
                }
                if self.hover_animation.changed_since(previous) {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_child = Size::new(
            if constraints.max.width.is_finite() {
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
            } else {
                f32::INFINITY
            },
            if constraints.max.height.is_finite() {
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0)
            } else {
                f32::INFINITY
            },
        );
        let child = self
            .child
            .measure(ctx, Constraints::new(Size::ZERO, max_child));
        let theme = self.resolved_theme();
        let mut size = Size::new(
            child.width + self.padding.left + self.padding.right,
            (child.height + self.padding.top + self.padding.bottom)
                .max(self.min_height.unwrap_or(theme.metrics.min_height)),
        );
        if self.fill_width && constraints.max.width.is_finite() {
            size.width = constraints.max.width;
        }
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let radius = theme
            .metrics
            .corner_radius
            .min(bounds.width().min(bounds.height()) * 0.5);
        let invalid = self.is_invalid();
        let focused = self.is_focused(ctx.focused_widget_id());
        let interaction_border = mix_color(
            theme.palette.border,
            theme.palette.border_hover,
            self.hover_animation.value,
        );
        let border = if invalid {
            theme.semantic_tone_color(SemanticTone::Danger)
        } else if focused {
            theme.palette.border_focus
        } else {
            interaction_border
        };
        let background = mix_color(
            theme.surfaces.field,
            theme.palette.surface_focus,
            focused as u8 as f32,
        );
        ctx.fill(rounded_rect_path(bounds, radius), background);
        ctx.stroke(
            rounded_rect_path(bounds, radius),
            border,
            StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width.max(1.0))),
        );
        if focused {
            let outset = physical_pixels(ctx, theme.metrics.focus_ring_outset);
            ctx.stroke(
                rounded_rect_path(bounds.inflate(outset, outset), radius + outset),
                if invalid {
                    theme.semantic_tone_color(SemanticTone::Danger)
                } else {
                    theme.palette.focus_ring
                },
                StrokeStyle::new(physical_pixels(ctx, theme.metrics.focus_ring_width)),
            );
        }
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.name.is_some() || self.description.is_some() {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = self.name.clone();
            node.description = self.description.clone();
            node.state.focused = self.is_focused(ctx.focused_widget_id());
            node.state.hovered = self.hovered;
            ctx.push(node);
        }
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}
