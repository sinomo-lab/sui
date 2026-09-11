use crate::ControlMetrics;
use crate::DefaultTheme;
use crate::IconGlyph;
use crate::SemanticTone;
use crate::composites::indicators::{
    draw_control_frame, inset_rect, measure_text, mix_color, physical_pixels, rounded_rect_path,
    text_token_style,
};
use crate::composites::popups::AnimatedScalar;
use crate::composites::surfaces::SurfaceElevation;
use crate::controls::draw_icon_glyph;
use crate::paint_theme_shadow;
use crate::text_align::paint_aligned_text;
use crate::text_align::paint_single_line_aligned_text;
use sui_core::Color;
use sui_core::Event;
use sui_core::KeyState;
use sui_core::PathBuilder;
use sui_core::Point;
use sui_core::PointerButton;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsAction;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::SemanticsValue;
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
use sui_runtime::WidgetChildren;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::StrokeStyle;
use sui_text::FontWeight;
use sui_text::TextMeasurement;
use sui_text::TextStyle;

pub struct ActionCard {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) icon: Option<IconGlyph>,
    pub(super) tone: SemanticTone,
    pub(super) accent: Option<Color>,
    pub(super) padding: Option<Insets>,
    pub(super) min_width: Option<f32>,
    pub(super) min_height: Option<f32>,
    pub(super) hovered: bool,
    pub(super) pressed: bool,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) title_measurement: Option<TextMeasurement>,
    pub(super) description_measurement: Option<TextMeasurement>,
    pub(super) enabled: bool,
    pub(super) enabled_reader: Option<Box<dyn Fn() -> bool>>,
    pub(super) on_press: Option<Box<dyn FnMut()>>,
    pub(super) on_press_with_ctx: Option<Box<dyn FnMut(&mut EventCtx)>>,
}

impl ActionCard {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            description: description.into(),
            icon: None,
            tone: SemanticTone::Accent,
            accent: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            title_measurement: None,
            description_measurement: None,
            enabled: true,
            enabled_reader: None,
            on_press: None,
            on_press_with_ctx: None,
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

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    pub fn accent(mut self, accent: Color) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
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

    pub(super) fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.action_card_padding)
    }

    pub(super) fn resolved_min_width(&self, metrics: ControlMetrics) -> f32 {
        self.min_width.unwrap_or(metrics.action_card_min_width)
    }

    pub(super) fn resolved_min_height(&self, metrics: ControlMetrics) -> f32 {
        self.min_height.unwrap_or(metrics.action_card_min_height)
    }

    pub(super) fn activate(&mut self, ctx: &mut EventCtx) {
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

    pub(super) fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        set_action_card_hover_animation_target(
            &mut self.hover_animation,
            hovered as u8 as f32,
            &theme,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    pub(super) fn clear_transient_state_for_hidden_bounds(&mut self, ctx: &mut ArrangeCtx) {
        if !self.hovered && !self.pressed && !self.focus_animation.is_presented() {
            return;
        }

        self.hovered = false;
        self.pressed = false;
        self.hover_animation = AnimatedScalar::new(0.0);
        self.press_animation = AnimatedScalar::new(0.0);
        self.focus_animation = AnimatedScalar::new(0.0);
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        TextStyle {
            weight: FontWeight::SEMIBOLD,
            ..text_token_style(&theme, theme.text.base, theme.palette.text)
        }
    }

    pub(super) fn resolved_description_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        text_token_style(&theme, theme.text.sm, theme.palette.text_muted)
    }

    pub(super) fn content_rect(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        inset_rect(bounds, self.resolved_padding(metrics))
    }

    pub(super) fn text_bounds(&self, bounds: Rect, metrics: ControlMetrics) -> Rect {
        let content = self.content_rect(bounds, metrics);
        let icon_extent = self
            .icon
            .map(|_| metrics.action_card_icon_box_size + metrics.action_card_icon_gap)
            .unwrap_or(0.0);
        let trailing = metrics.action_card_trailing_gap;
        Rect::new(
            content.x() + icon_extent,
            content.y(),
            (content.width() - icon_extent - trailing).max(0.0),
            content.height(),
        )
    }
}

impl Widget for ActionCard {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_enabled() {
            if self.hovered || self.pressed {
                let theme = self.resolved_theme();
                self.hovered = false;
                self.pressed = false;
                set_action_card_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                set_action_card_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.request_paint();
                ctx.request_semantics();
            }
            return;
        }

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
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                self.pressed = true;
                self.hovered = true;
                set_action_card_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_action_card_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
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
                set_action_card_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_action_card_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
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
                    set_action_card_hover_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        &theme,
                        ctx,
                    );
                    set_action_card_press_animation_target(
                        &mut self.press_animation,
                        0.0,
                        &theme,
                        ctx,
                    );
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
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
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
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title = measure_text(ctx, &self.title, &title_style);
        let description = measure_text(ctx, &self.description, &description_style);
        self.title_measurement = Some(title);
        self.description_measurement = Some(description);

        let icon_extent = self
            .icon
            .map(|_| metrics.action_card_icon_box_size + metrics.action_card_icon_gap)
            .unwrap_or(0.0);
        let text_width = title.width.max(description.width).min(320.0);
        let natural = Size::new(
            self.resolved_min_width(metrics).max(
                padding.left
                    + icon_extent
                    + text_width
                    + metrics.action_card_trailing_gap
                    + padding.right,
            ),
            self.resolved_min_height(metrics).max(
                padding.top
                    + title.height.max(title_style.line_height)
                    + metrics.action_card_text_gap
                    + description.height.max(description_style.line_height)
                    + padding.bottom,
            ),
        );
        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            self.clear_transient_state_for_hidden_bounds(ctx);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let enabled = self.is_enabled();
        let hover = if enabled {
            self.hover_animation.value
        } else {
            0.0
        };
        let press = if enabled {
            self.press_animation.value
        } else {
            0.0
        };
        let accent = self
            .accent
            .unwrap_or_else(|| theme.semantic_tone_color(self.tone));
        let mut background = mix_color(palette.control, palette.control_hover, hover);
        background = mix_color(background, palette.control_active, press * 0.55);
        if !enabled {
            background = mix_color(background, palette.surface, 0.68).with_alpha(0.82);
        }
        let border = if !enabled {
            palette.border.with_alpha(0.55)
        } else if ctx.is_focused() {
            palette.border_focus
        } else {
            mix_color(palette.border, palette.border_hover, hover)
        };

        // Elevation shadow behind the raised card surface, drawn before the
        // fill so the soft shadow is not clipped.
        if enabled {
            paint_theme_shadow(
                ctx,
                ctx.bounds(),
                [metrics.corner_radius; 4],
                &theme.shadows.box_shadow.md,
            );
        }

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            background,
            border,
            (self.focus_animation.value > AnimatedScalar::EPSILON && enabled).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
            ),
        );

        let bounds = ctx.bounds();
        let content = self.content_rect(bounds, metrics);
        let accent_inset = metrics.action_card_accent_inset.min(bounds.height() * 0.5);
        let accent_height = (bounds.height() - accent_inset * 2.0).max(0.0);
        let accent_rail = Rect::new(
            bounds.x(),
            bounds.y() + accent_inset,
            metrics.action_card_accent_width,
            accent_height,
        );
        ctx.fill(
            rounded_rect_path(accent_rail, metrics.action_card_accent_width * 0.5),
            accent.with_alpha(0.78),
        );

        if let Some(icon) = self.icon {
            let icon_box_size = metrics
                .action_card_icon_box_size
                .min(content.width())
                .min(content.height())
                .max(0.0);
            let icon_box = Rect::new(
                content.x(),
                content.y() + ((content.height() - icon_box_size) * 0.5),
                icon_box_size,
                icon_box_size,
            );
            ctx.fill(
                rounded_rect_path(icon_box, metrics.corner_radius),
                mix_color(background, accent, 0.14),
            );
            ctx.stroke(
                rounded_rect_path(icon_box, metrics.corner_radius),
                accent.with_alpha(if enabled { 0.42 } else { 0.22 }),
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
            let icon_size = metrics
                .action_card_icon_size
                .min(icon_box.width())
                .min(icon_box.height())
                .max(0.0);
            let icon_rect = Rect::new(
                icon_box.x() + ((icon_box.width() - icon_size) * 0.5),
                icon_box.y() + ((icon_box.height() - icon_size) * 0.5),
                icon_size,
                icon_size,
            );
            draw_icon_glyph(
                ctx,
                icon,
                icon_rect,
                if enabled {
                    accent
                } else {
                    palette.text.with_alpha(0.34)
                },
            );
        }

        let text_bounds = self.text_bounds(bounds, metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title_height = title_style.line_height.max(
            self.title_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(title_style.line_height),
        );
        let description_min_height = description_style.line_height.max(
            self.description_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(description_style.line_height),
        );
        let description_height =
            (text_bounds.height() - title_height - metrics.action_card_text_gap)
                .max(description_min_height)
                .min((description_style.line_height * 2.0).max(description_min_height));
        let text_block_height = title_height + metrics.action_card_text_gap + description_height;
        let text_y = text_bounds.y() + ((text_bounds.height() - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(text_bounds.x(), text_y, text_bounds.width(), title_height);
        let description_slot = Rect::new(
            text_bounds.x(),
            title_slot.max_y() + metrics.action_card_text_gap,
            text_bounds.width(),
            description_height,
        );
        let title_paint_style = TextStyle {
            color: if enabled {
                palette.text
            } else {
                palette.text.with_alpha(0.45)
            },
            ..title_style
        };
        let description_paint_style = TextStyle {
            color: if enabled {
                palette.placeholder
            } else {
                palette.placeholder.with_alpha(0.45)
            },
            ..description_style
        };
        let description_layout = {
            let mut layout_style = description_paint_style.clone();
            layout_style.color = Color::WHITE;
            ctx.shape_text(
                self.description.clone(),
                Size::new(
                    description_slot.width().max(1.0),
                    description_slot.height().max(1.0),
                ),
                layout_style,
            )
            .ok()
        };
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_paint_style,
            title_paint_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        ctx.push_clip_rect(description_slot);
        if let Some(layout) = description_layout.filter(|layout| layout.lines().len() > 1) {
            let measurement = layout.measurement();
            let width = measurement.width.min(description_slot.width()).max(0.0);
            let height = description_paint_style
                .line_height
                .max(measurement.height)
                .min(description_slot.height());
            let description_rect = Rect::new(
                description_slot.x(),
                description_slot.y() + ((description_slot.height() - height).max(0.0) * 0.5),
                width,
                height,
            );
            ctx.draw_text_layout_with_color(
                description_rect.origin,
                &layout,
                description_paint_style.color,
            );
        } else {
            paint_aligned_text(
                ctx,
                description_slot,
                &self.description,
                &description_paint_style,
                description_paint_style.line_height,
                0.0,
            );
        }
        ctx.pop_clip();

        let chevron_size = metrics
            .action_card_chevron_size
            .min(content.width())
            .min(content.height())
            .max(0.0);
        let chevron = Rect::new(
            content.max_x() - chevron_size,
            content.y() + ((content.height() - chevron_size) * 0.5),
            chevron_size,
            chevron_size,
        );
        draw_icon_glyph(
            ctx,
            IconGlyph::ChevronRight,
            chevron,
            if enabled {
                mix_color(palette.placeholder, accent, hover * 0.45).with_alpha(0.74)
            } else {
                palette.placeholder.with_alpha(0.32)
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.title.clone());
        node.description = Some(self.description.clone());
        node.value = Some(SemanticsValue::Text(self.description.clone()));
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

pub(super) fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) -> bool {
    animation.set_target_event(target, duration, easing, ctx)
}

pub(super) fn set_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    )
}

pub(super) fn set_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
        ctx,
    )
}

pub(super) fn set_focus_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.focus_duration(),
        theme.motion.focus_easing(),
        ctx,
    )
}

pub(super) fn set_action_card_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) {
    set_animation_target(animation, target, duration, easing, ctx);
}

pub(super) fn set_action_card_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_action_card_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    );
}

pub(super) fn set_action_card_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    set_action_card_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
        ctx,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRowLayout {
    Stacked,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PropertyRowDefaults {
    Property,
    Form,
}

pub struct PropertyRow {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) label: String,
    pub(super) defaults: PropertyRowDefaults,
    pub(super) layout: PropertyRowLayout,
    pub(super) label_width: Option<f32>,
    pub(super) control_width: Option<f32>,
    pub(super) auto_control_width: bool,
    pub(super) gap: Option<f32>,
    pub(super) label_style: Option<TextStyle>,
    pub(super) child: SingleChild,
    pub(super) label_measurement: Option<TextMeasurement>,
}

impl PropertyRow {
    pub fn new<W>(label: impl Into<String>, control: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            defaults: PropertyRowDefaults::Property,
            layout: PropertyRowLayout::Stacked,
            label_width: None,
            control_width: None,
            auto_control_width: true,
            gap: None,
            label_style: None,
            child: SingleChild::new(control),
            label_measurement: None,
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

    pub fn layout(mut self, layout: PropertyRowLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn stacked(self) -> Self {
        self.layout(PropertyRowLayout::Stacked)
    }

    pub fn inline(self) -> Self {
        self.layout(PropertyRowLayout::Inline)
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.label_width = Some(width.max(0.0));
        self
    }

    pub fn control_width(mut self, width: f32) -> Self {
        self.control_width = Some(width.max(0.0));
        self.auto_control_width = false;
        self
    }

    pub fn auto_control_width(mut self) -> Self {
        self.control_width = None;
        self.auto_control_width = true;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.label_style = Some(style);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    pub(super) fn resolved_label_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.label_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.sm, theme.palette.text_muted))
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn with_form_defaults(mut self) -> Self {
        self.defaults = PropertyRowDefaults::Form;
        self.auto_control_width = true;
        self
    }

    pub(super) fn resolved_label_width(&self, metrics: ControlMetrics) -> f32 {
        self.label_width.unwrap_or(match self.defaults {
            PropertyRowDefaults::Property => metrics.property_row_label_width,
            PropertyRowDefaults::Form => metrics.form_row_label_width,
        })
    }

    pub(super) fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(match self.defaults {
            PropertyRowDefaults::Form => metrics.form_row_gap,
            PropertyRowDefaults::Property => match self.layout {
                PropertyRowLayout::Stacked => metrics.property_row_stacked_gap,
                PropertyRowLayout::Inline => metrics.property_row_inline_gap,
            },
        })
    }

    pub(super) fn resolved_control_width(&self, metrics: ControlMetrics) -> Option<f32> {
        if self.auto_control_width {
            None
        } else {
            self.control_width.or_else(|| {
                matches!(self.defaults, PropertyRowDefaults::Form)
                    .then_some(metrics.form_row_control_width)
            })
        }
    }

    pub(super) fn label_height(&self, style: &TextStyle) -> f32 {
        self.label_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    pub(super) fn child_constraints(
        &self,
        constraints: Constraints,
        label_extent: f32,
        metrics: ControlMetrics,
    ) -> Constraints {
        let max_width = constraints.max.width;
        let gap = self.resolved_gap(metrics);
        let available = match self.layout {
            PropertyRowLayout::Stacked => max_width,
            PropertyRowLayout::Inline => {
                if max_width.is_finite() {
                    (max_width - label_extent - gap).max(0.0)
                } else {
                    f32::INFINITY
                }
            }
        };
        let width = self
            .resolved_control_width(metrics)
            .map(|width| width.min(available).max(0.0));

        match width {
            Some(width) => Constraints::new(
                Size::new(width, 0.0),
                Size::new(width, constraints.max.height),
            ),
            None => Constraints::new(Size::ZERO, Size::new(available, constraints.max.height)),
        }
    }

    pub(super) fn child_width_for_bounds(
        &self,
        bounds: Rect,
        label_extent: f32,
        metrics: ControlMetrics,
    ) -> f32 {
        let gap = self.resolved_gap(metrics);
        let available = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => (bounds.width() - label_extent - gap).max(0.0),
        };
        self.resolved_control_width(metrics)
            .unwrap_or(available)
            .min(available)
            .max(0.0)
    }
}

impl Widget for PropertyRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let label_style = self.resolved_label_style();
        let label_measurement = measure_text(ctx, &self.label, &label_style);
        self.label_measurement = Some(label_measurement);
        let label_height = self.label_height(&label_style);
        let label_extent = match self.layout {
            PropertyRowLayout::Stacked => label_measurement.width,
            PropertyRowLayout::Inline => self
                .resolved_label_width(metrics)
                .max(label_measurement.width),
        };
        let child_size = self.child.measure(
            ctx,
            self.child_constraints(constraints, label_extent, metrics),
        );
        let natural = match self.layout {
            PropertyRowLayout::Stacked => Size::new(
                label_measurement.width.max(child_size.width),
                label_height + gap + child_size.height,
            ),
            PropertyRowLayout::Inline => Size::new(
                label_extent + gap + child_size.width,
                label_height.max(child_size.height),
            ),
        };

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let label_width = match self.layout {
            PropertyRowLayout::Stacked => bounds.width(),
            PropertyRowLayout::Inline => self
                .resolved_label_width(metrics)
                .min(bounds.width())
                .max(0.0),
        };
        let child_measured = self.child.child().measured_size();
        let child_width = self.child_width_for_bounds(bounds, label_width, metrics);
        let child_height = child_measured.height.min(bounds.height()).max(0.0);

        let child_bounds = match self.layout {
            PropertyRowLayout::Stacked => Rect::new(
                bounds.x(),
                bounds.y() + label_height + gap,
                child_width,
                child_height.min((bounds.height() - label_height - gap).max(0.0)),
            ),
            PropertyRowLayout::Inline => Rect::new(
                bounds.x() + label_width + gap,
                bounds.y() + ((bounds.height() - child_height) * 0.5).max(0.0),
                child_width,
                child_height,
            ),
        };
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let bounds = ctx.bounds();
        let label_rect = match self.layout {
            PropertyRowLayout::Stacked => {
                Rect::new(bounds.x(), bounds.y(), bounds.width(), label_height)
            }
            PropertyRowLayout::Inline => Rect::new(
                bounds.x(),
                bounds.y() + ((bounds.height() - label_height) * 0.5).max(0.0),
                self.resolved_label_width(metrics)
                    .min(bounds.width())
                    .max(0.0),
                label_height,
            ),
        };
        ctx.push_clip_rect(label_rect);
        paint_single_line_aligned_text(
            ctx,
            label_rect,
            &self.label,
            &label_style,
            label_height,
            0.0,
        );
        ctx.pop_clip();
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let mut row = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        row.name = Some(self.label.clone());
        ctx.push(row);

        let label_style = self.resolved_label_style();
        let label_height = self.label_height(&label_style);
        let label_bounds = match self.layout {
            PropertyRowLayout::Stacked => Rect::new(
                ctx.bounds().x(),
                ctx.bounds().y(),
                ctx.bounds().width(),
                label_height,
            ),
            PropertyRowLayout::Inline => Rect::new(
                ctx.bounds().x(),
                ctx.bounds().y() + ((ctx.bounds().height() - label_height) * 0.5).max(0.0),
                self.resolved_label_width(metrics)
                    .min(ctx.bounds().width())
                    .max(0.0),
                label_height,
            ),
        };
        let mut label = SemanticsNode::new(
            property_row_label_id(ctx.widget_id()),
            SemanticsRole::Text,
            label_bounds,
        );
        label.parent = Some(ctx.widget_id());
        label.name = Some(self.label.clone());
        label.value = Some(SemanticsValue::Text(self.label.clone()));
        ctx.push(label);

        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub(super) fn property_row_label_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 1_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(271).wrapping_add(1) & LOW_MASK))
}

pub struct SectionLabel {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) label: String,
    pub(super) semantic_name: Option<String>,
    pub(super) color: Option<Color>,
}

impl SectionLabel {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            semantic_name: None,
            color: None,
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

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn display_text(&self) -> String {
        self.label.to_uppercase()
    }

    pub(super) fn text_style(&self, theme: &DefaultTheme) -> TextStyle {
        section_label_text_style(theme, self.color)
    }
}

impl Widget for SectionLabel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let style = self.text_style(&theme);
        let text = self.display_text();
        let measured = measure_text(ctx, &text, &style);
        constraints.clamp(Size::new(measured.width, style.line_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let style = self.text_style(&theme);
        let text = self.display_text();
        ctx.push_clip_rect(ctx.bounds());
        paint_single_line_aligned_text(ctx, ctx.bounds(), &text, &style, style.line_height, 0.0);
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        let name = self
            .semantic_name
            .clone()
            .unwrap_or_else(|| self.label.clone());
        node.name = Some(name.clone());
        node.value = Some(SemanticsValue::Text(name));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SectionLabelPaint {
    pub(super) color: Option<Color>,
}

impl SectionLabelPaint {
    pub const fn new() -> Self {
        Self { color: None }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

pub fn paint_section_label(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    paint: SectionLabelPaint,
) {
    let style = section_label_text_style(theme, paint.color);
    let text = label.to_uppercase();
    ctx.push_clip_rect(rect);
    paint_single_line_aligned_text(ctx, rect, &text, &style, style.line_height, 0.0);
    ctx.pop_clip();
}

pub fn paint_section_label_detail(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    detail: &str,
    paint: SectionLabelPaint,
) {
    let style = section_label_text_style(theme, paint.color);
    let text = if detail.trim().is_empty() {
        label.to_uppercase()
    } else {
        format!("{} · {detail}", label.to_uppercase())
    };
    ctx.push_clip_rect(rect);
    paint_single_line_aligned_text(ctx, rect, &text, &style, style.line_height, 0.0);
    ctx.pop_clip();
}

pub(super) fn section_label_text_style(theme: &DefaultTheme, color: Option<Color>) -> TextStyle {
    let mut style = text_token_style(
        theme,
        theme.text.xs,
        color.unwrap_or(theme.surfaces.text_faint),
    );
    style.weight = FontWeight::SEMIBOLD;
    style
}

pub struct DetailRow {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) label: String,
    pub(super) label_reader: Option<Box<dyn Fn() -> String>>,
    pub(super) value: String,
    pub(super) value_reader: Option<Box<dyn Fn() -> String>>,
    pub(super) max_value_lines: Option<usize>,
}

impl DetailRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            value: value.into(),
            value_reader: None,
            max_value_lines: None,
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

    pub fn label_when<F>(mut self, label: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.label_reader = Some(Box::new(label));
        self
    }

    pub fn value_when<F>(mut self, value: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.value_reader = Some(Box::new(value));
        self
    }

    pub fn max_value_lines(mut self, max_lines: usize) -> Self {
        self.max_value_lines = Some(max_lines.max(1));
        self
    }

    pub(super) fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    pub(super) fn value(&self) -> String {
        self.value_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.value.clone())
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for DetailRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let label_style = detail_row_label_style(&theme);
        let value_style = detail_row_value_style(&theme);
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.max(0.0)
        } else {
            let label = measure_text(ctx, &self.label().to_uppercase(), &label_style);
            let value = measure_text(ctx, &self.value(), &value_style);
            label.width.max(value.width)
        };
        let value = self.value();
        let lines = wrap_detail_row_value(&value, width, self.max_value_lines, |text| {
            measure_text(ctx, text, &value_style).width
        });
        constraints.clamp(Size::new(width, detail_row_height(&theme, lines.len())))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        paint_detail_row_at(
            ctx,
            &theme,
            Point::new(ctx.bounds().x(), ctx.bounds().y()),
            ctx.bounds().width(),
            &self.label(),
            &self.value(),
            self.max_value_lines,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.label());
        node.value = Some(SemanticsValue::Text(self.value()));
        ctx.push(node);
    }
}

pub fn paint_detail_row_at(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    origin: Point,
    width: f32,
    label: &str,
    value: &str,
    max_value_lines: Option<usize>,
) -> f32 {
    let width = width.max(0.0);
    let label_style = detail_row_label_style(theme);
    let value_style = detail_row_value_style(theme);
    let lines = wrap_detail_row_value(value, width, max_value_lines, |text| {
        ctx.measure_text(text.to_string(), value_style.clone())
            .map(|measurement| measurement.width)
            .unwrap_or(0.0)
    });
    let height = detail_row_height(theme, lines.len());
    let clip = Rect::new(origin.x, origin.y, width, height);

    ctx.push_clip_rect(clip);
    paint_aligned_text(
        ctx,
        Rect::new(origin.x, origin.y, width, label_style.line_height),
        &label.to_uppercase(),
        &label_style,
        label_style.line_height,
        0.0,
    );

    let mut y = origin.y + label_style.line_height + detail_row_label_value_gap(theme);
    for line in lines {
        paint_aligned_text(
            ctx,
            Rect::new(origin.x, y, width, value_style.line_height),
            &line,
            &value_style,
            value_style.line_height,
            0.0,
        );
        y += value_style.line_height;
    }
    ctx.pop_clip();
    height
}

pub fn detail_row_height_for_value(
    ctx: &PaintCtx,
    theme: &DefaultTheme,
    width: f32,
    value: &str,
    max_value_lines: Option<usize>,
) -> f32 {
    let width = width.max(0.0);
    let value_style = detail_row_value_style(theme);
    let lines = wrap_detail_row_value(value, width, max_value_lines, |text| {
        ctx.measure_text(text.to_string(), value_style.clone())
            .map(|measurement| measurement.width)
            .unwrap_or(0.0)
    });
    detail_row_height(theme, lines.len())
}

pub(super) fn detail_row_label_style(theme: &DefaultTheme) -> TextStyle {
    let mut style = text_token_style(theme, theme.text.xs, theme.palette.text_muted);
    style.weight = FontWeight::SEMIBOLD;
    style
}

pub(super) fn detail_row_value_style(theme: &DefaultTheme) -> TextStyle {
    text_token_style(theme, theme.text.sm, theme.palette.text)
}

pub(super) fn detail_row_label_value_gap(theme: &DefaultTheme) -> f32 {
    (theme.metrics.icon_label_gap * 0.35).max(2.0)
}

pub(super) fn detail_row_bottom_gap(theme: &DefaultTheme) -> f32 {
    theme.metrics.property_row_stacked_gap.max(6.0)
}

pub(super) fn detail_row_height(theme: &DefaultTheme, value_lines: usize) -> f32 {
    let label_style = detail_row_label_style(theme);
    let value_style = detail_row_value_style(theme);
    label_style.line_height
        + detail_row_label_value_gap(theme)
        + value_style.line_height * value_lines.max(1) as f32
        + detail_row_bottom_gap(theme)
}

pub(super) fn wrap_detail_row_value<F>(
    value: &str,
    width: f32,
    max_lines: Option<usize>,
    mut measure: F,
) -> Vec<String>
where
    F: FnMut(&str) -> f32,
{
    let max_lines = max_lines.unwrap_or(usize::MAX).max(1);
    let width = width.max(1.0);
    let mut lines = Vec::new();

    for paragraph in value.split('\n') {
        if lines.len() >= max_lines {
            break;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || measure(&candidate) <= width {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                if lines.len() >= max_lines {
                    break;
                }
                current = word.to_string();
            }
        }
        if lines.len() < max_lines {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub struct FormRow {
    pub(super) row: PropertyRow,
}

impl FormRow {
    pub fn new<W>(label: impl Into<String>, control: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            row: PropertyRow::new(label, control)
                .inline()
                .with_form_defaults(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.row = self.row.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.row = self.row.theme_when(theme);
        self
    }

    pub fn stacked(mut self) -> Self {
        self.row = self.row.stacked();
        self
    }

    pub fn inline(mut self) -> Self {
        self.row = self.row.inline();
        self
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.row = self.row.label_width(width);
        self
    }

    pub fn control_width(mut self, width: f32) -> Self {
        self.row = self.row.control_width(width);
        self
    }

    pub fn auto_control_width(mut self) -> Self {
        self.row = self.row.auto_control_width();
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.row = self.row.gap(gap);
        self
    }

    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.row = self.row.label_style(style);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.row.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.row.child_mut()
    }
}

impl Widget for FormRow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.row.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.row.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.row.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.row.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.row.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.row.visit_children_mut(visitor);
    }
}

pub struct FieldGroup {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) children: WidgetChildren,
    pub(super) spacing: Option<f32>,
    pub(super) padding: Insets,
    pub(super) max_width: Option<f32>,
    pub(super) fill_width: bool,
}

impl FieldGroup {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            children: WidgetChildren::new(),
            spacing: None,
            padding: Insets::ZERO,
            max_width: None,
            fill_width: false,
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

    pub fn with_child<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.children.push(child);
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width.max(0.0));
        self
    }

    pub fn auto_width(mut self) -> Self {
        self.max_width = None;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn children(&self) -> &[sui_runtime::WidgetPod] {
        self.children.as_slice()
    }

    pub fn children_mut(&mut self) -> &mut [sui_runtime::WidgetPod] {
        self.children.as_mut_slice()
    }

    pub(super) fn content_max_width(&self, constraints: Constraints) -> f32 {
        let available = if constraints.max.width.is_finite() {
            (constraints.max.width - self.padding.left - self.padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        self.max_width
            .map(|width| width.min(available))
            .unwrap_or(available)
    }

    pub(super) fn content_rect(&self, bounds: Rect) -> Rect {
        let inset = inset_rect(bounds, self.padding);
        let width = self
            .max_width
            .map(|max_width| max_width.min(inset.width()))
            .unwrap_or(inset.width())
            .max(0.0);
        Rect::new(inset.x(), inset.y(), width, inset.height())
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_spacing(&self) -> f32 {
        self.spacing
            .unwrap_or_else(|| self.resolved_theme().metrics.field_group_spacing)
    }
}

impl Default for FieldGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for FieldGroup {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let spacing = self.resolved_spacing();
        let content_max_width = self.content_max_width(constraints);
        let mut y: f32 = 0.0;
        let mut width: f32 = 0.0;
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                y += spacing;
            }
            let child_size = child.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(content_max_width, constraints.max.height),
                ),
            );
            y += child_size.height;
            width = width.max(child_size.width);
        }

        if self.fill_width && content_max_width.is_finite() {
            width = content_max_width;
        }

        constraints.clamp(Size::new(
            width + self.padding.left + self.padding.right,
            y + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let spacing = self.resolved_spacing();
        let content = self.content_rect(bounds);
        let mut y = content.y();
        for (index, child) in self.children.as_mut_slice().iter_mut().enumerate() {
            if index > 0 {
                y += spacing;
            }
            let measured = child.measured_size();
            let width = if self.fill_width {
                content.width()
            } else {
                measured.width.min(content.width())
            };
            child.arrange(ctx, Rect::new(content.x(), y, width, measured.height));
            y += measured.height;
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

pub struct FormSection {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) title_style: Option<TextStyle>,
    pub(super) description_style: Option<TextStyle>,
    pub(super) header_action: Option<SingleChild>,
    pub(super) child: SingleChild,
    pub(super) padding: Option<Insets>,
    pub(super) body_gap: Option<f32>,
    pub(super) header_gap: Option<f32>,
    pub(super) description_gap: Option<f32>,
    pub(super) max_width: Option<f32>,
    pub(super) auto_width: bool,
    pub(super) radius: Option<f32>,
    pub(super) elevation: SurfaceElevation,
    pub(super) fill_width: bool,
    pub(super) title_measurement: Option<TextMeasurement>,
    pub(super) description_measurement: Option<TextMeasurement>,
}

impl FormSection {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            description: None,
            title_style: None,
            description_style: None,
            header_action: None,
            child: SingleChild::new(child),
            padding: None,
            body_gap: None,
            header_gap: None,
            description_gap: None,
            max_width: None,
            auto_width: false,
            radius: None,
            elevation: SurfaceElevation::Small,
            fill_width: false,
            title_measurement: None,
            description_measurement: None,
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

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn title_style(mut self, style: TextStyle) -> Self {
        self.title_style = Some(style);
        self
    }

    pub fn description_style(mut self, style: TextStyle) -> Self {
        self.description_style = Some(style);
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn body_gap(mut self, gap: f32) -> Self {
        self.body_gap = Some(gap.max(0.0));
        self
    }

    pub fn header_gap(mut self, gap: f32) -> Self {
        self.header_gap = Some(gap.max(0.0));
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width.max(0.0));
        self.auto_width = false;
        self
    }

    pub fn auto_width(mut self) -> Self {
        self.max_width = None;
        self.auto_width = true;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius.max(0.0));
        self
    }

    pub fn elevation(mut self, elevation: SurfaceElevation) -> Self {
        self.elevation = elevation;
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.form_section_padding)
    }

    pub(super) fn resolved_body_gap(&self, metrics: ControlMetrics) -> f32 {
        self.body_gap.unwrap_or(metrics.form_section_body_gap)
    }

    pub(super) fn resolved_header_gap(&self, metrics: ControlMetrics) -> f32 {
        self.header_gap.unwrap_or(metrics.form_section_header_gap)
    }

    pub(super) fn resolved_description_gap(&self, metrics: ControlMetrics) -> f32 {
        self.description_gap
            .unwrap_or(metrics.form_section_description_gap)
    }

    pub(super) fn resolved_max_width(&self, metrics: ControlMetrics) -> Option<f32> {
        if self.auto_width {
            None
        } else {
            Some(self.max_width.unwrap_or(metrics.form_section_max_width))
        }
    }

    pub(super) fn resolved_radius(&self, metrics: ControlMetrics) -> f32 {
        self.radius.unwrap_or(metrics.form_section_radius).max(0.0)
    }

    pub(super) fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.title_style.clone().unwrap_or_else(|| TextStyle {
            weight: FontWeight::SEMIBOLD,
            ..text_token_style(&theme, theme.text.sm, theme.surfaces.text)
        })
    }

    pub(super) fn resolved_description_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.description_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.xs, theme.surfaces.text_muted))
    }

    pub(super) fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    pub(super) fn description_height(&self, style: &TextStyle) -> f32 {
        if self.description.is_some() {
            self.description_measurement
                .map(|measurement| measurement.height)
                .unwrap_or(style.line_height)
                .max(style.line_height)
        } else {
            0.0
        }
    }

    pub(super) fn text_block_height(
        &self,
        title_style: &TextStyle,
        description_style: &TextStyle,
    ) -> f32 {
        let title = self.title_height(title_style);
        let description = self.description_height(description_style);
        if description > 0.0 {
            let metrics = self.resolved_theme().metrics;
            title + self.resolved_description_gap(metrics) + description
        } else {
            title
        }
    }

    pub(super) fn content_max_width(&self, available_width: f32) -> f32 {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let available = if available_width.is_finite() {
            (available_width - padding.left - padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        self.resolved_max_width(metrics)
            .map(|width| width.min(available))
            .unwrap_or(available)
    }

    pub(super) fn card_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let width = if self.fill_width {
            bounds.width()
        } else {
            self.resolved_max_width(metrics)
                .map(|max_width| (max_width + padding.left + padding.right).min(bounds.width()))
                .unwrap_or(bounds.width())
        }
        .max(0.0);
        let x = if self.fill_width || width >= bounds.width() {
            bounds.x()
        } else {
            bounds.x() + ((bounds.width() - width) * 0.5)
        };
        Rect::new(x, bounds.y(), width, bounds.height())
    }

    pub(super) fn content_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        inset_rect(self.card_rect(bounds), self.resolved_padding(theme.metrics))
    }

    pub(super) fn header_height(
        &self,
        title_style: &TextStyle,
        description_style: &TextStyle,
    ) -> f32 {
        let text_height = self.text_block_height(title_style, description_style);
        let action_height = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().height)
            .unwrap_or(0.0);
        text_height.max(action_height)
    }
}

impl Widget for FormSection {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let body_gap = self.resolved_body_gap(metrics);
        let header_gap = self.resolved_header_gap(metrics);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title);
        let description = self
            .description
            .as_ref()
            .map(|description| measure_text(ctx, description, &description_style));
        self.description_measurement = description;

        let content_max_width = self.content_max_width(constraints.max.width);
        let action_size = self
            .header_action
            .as_mut()
            .map(|action| {
                action.measure(
                    ctx,
                    Constraints::new(
                        Size::ZERO,
                        Size::new(content_max_width, constraints.max.height),
                    ),
                )
            })
            .unwrap_or(Size::ZERO);
        let action_extent = if self.header_action.is_some() {
            action_size.width + header_gap
        } else {
            0.0
        };
        let text_width = title.width.max(
            description
                .map(|measurement| measurement.width)
                .unwrap_or(0.0),
        );
        let header_width = (text_width + action_extent).min(content_max_width);
        let child_size = self.child.measure(
            ctx,
            Constraints::new(
                Size::ZERO,
                Size::new(content_max_width, constraints.max.height),
            ),
        );
        let content_width = header_width.max(child_size.width).min(content_max_width);
        let header_height = self.header_height(&title_style, &description_style);

        let mut width = content_width + padding.left + padding.right;
        if self.fill_width && constraints.max.width.is_finite() {
            width = constraints.max.width;
        }
        let height = padding.top + header_height + body_gap + child_size.height + padding.bottom;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let body_gap = self.resolved_body_gap(metrics);
        let content = self.content_rect(bounds);
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let header_height = self.header_height(&title_style, &description_style);

        if let Some(action) = &mut self.header_action {
            let action_size = action.child().measured_size();
            action.arrange(
                ctx,
                Rect::new(
                    content.max_x() - action_size.width,
                    content.y() + ((header_height - action_size.height) * 0.5).max(0.0),
                    action_size.width,
                    action_size.height,
                ),
            );
        }

        let child_size = self.child.child().measured_size();
        self.child.arrange(
            ctx,
            Rect::new(
                content.x(),
                content.y() + header_height + body_gap,
                child_size.width.min(content.width()),
                child_size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let card = self.card_rect(ctx.bounds());
        let radius = self
            .resolved_radius(metrics)
            .min(card.width().min(card.height()) * 0.5);
        let shadow = match self.elevation {
            SurfaceElevation::None => None,
            SurfaceElevation::Small => Some(&theme.shadows.box_shadow.sm),
            SurfaceElevation::Medium => Some(&theme.shadows.box_shadow.md),
            SurfaceElevation::Large => Some(&theme.shadows.box_shadow.lg),
        };
        if let Some(shadow) = shadow {
            paint_theme_shadow(ctx, card, [radius; 4], shadow);
        }

        let background = theme.surfaces.panel;
        let border = theme.surfaces.border;
        let shape = rounded_rect_path(card, radius);
        ctx.fill(shape.clone(), background);
        ctx.stroke(
            shape,
            border,
            StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width.max(1.0))),
        );

        let content = inset_rect(card, self.resolved_padding(metrics));
        let title_style = self.resolved_title_style();
        let description_style = self.resolved_description_style();
        let title_height = self.title_height(&title_style);
        let description_height = self.description_height(&description_style);
        let header_gap = self.resolved_header_gap(metrics);
        let description_gap = self.resolved_description_gap(metrics);
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + header_gap)
            .unwrap_or(0.0)
            .min(content.width());
        let text_width = (content.width() - action_width).max(0.0);
        let text_block_height = self.text_block_height(&title_style, &description_style);
        let header_height = self.header_height(&title_style, &description_style);
        let text_y = content.y() + ((header_height - text_block_height) * 0.5).max(0.0);
        let title_slot = Rect::new(content.x(), text_y, text_width, title_height);
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        if let Some(description) = &self.description {
            let description_slot = Rect::new(
                content.x(),
                title_slot.max_y() + description_gap,
                text_width,
                description_height,
            );
            ctx.push_clip_rect(description_slot);
            paint_aligned_text(
                ctx,
                description_slot,
                description,
                &description_style,
                description_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }

        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let card = self.card_rect(ctx.bounds());
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::GenericContainer, card);
        node.name = Some(self.title.clone());
        node.description = self.description.clone();
        ctx.push(node);
        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.header_action {
            action.visit_children(visitor);
        }
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.header_action {
            action.visit_children_mut(visitor);
        }
        self.child.visit_children_mut(visitor);
    }
}

pub struct PanelSection {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) title: String,
    pub(super) gap: Option<f32>,
    pub(super) action_gap: Option<f32>,
    pub(super) title_style: Option<TextStyle>,
    pub(super) header_action: Option<SingleChild>,
    pub(super) child: SingleChild,
    pub(super) title_measurement: Option<TextMeasurement>,
    pub(super) collapsible: bool,
    pub(super) expanded: bool,
    pub(super) hovered_header: bool,
    pub(super) pressed_header: bool,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
}

impl PanelSection {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            title: title.into(),
            gap: None,
            action_gap: None,
            title_style: None,
            header_action: None,
            child: SingleChild::new(child),
            title_measurement: None,
            collapsible: false,
            expanded: true,
            hovered_header: false,
            pressed_header: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
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

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn title_style(mut self, style: TextStyle) -> Self {
        self.title_style = Some(style);
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    pub fn action_gap(mut self, gap: f32) -> Self {
        self.action_gap = Some(gap.max(0.0));
        self
    }

    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn collapsed(mut self) -> Self {
        self.expanded = false;
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    pub(super) fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        self.title_style
            .clone()
            .unwrap_or_else(|| text_token_style(&theme, theme.text.xs, theme.palette.text_muted))
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(metrics.panel_section_gap)
    }

    pub(super) fn resolved_action_gap(&self, metrics: ControlMetrics) -> f32 {
        self.action_gap.unwrap_or(metrics.panel_section_action_gap)
    }

    pub(super) fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    pub(super) fn header_height(&self, title_style: &TextStyle) -> f32 {
        let action_height = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().height)
            .unwrap_or(0.0);
        self.title_height(title_style).max(action_height)
    }

    pub(super) fn is_expanded(&self) -> bool {
        !self.collapsible || self.expanded
    }

    pub(super) fn disclosure_width(&self, metrics: ControlMetrics) -> f32 {
        if self.collapsible {
            metrics.panel_section_disclosure_size
        } else {
            0.0
        }
    }

    pub(super) fn title_rect(&self, bounds: Rect, header_height: f32, title_height: f32) -> Rect {
        let metrics = self.resolved_theme().metrics;
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.resolved_action_gap(metrics))
            .unwrap_or(0.0)
            .min(bounds.width());
        let disclosure_width = self.disclosure_width(metrics);
        Rect::new(
            bounds.x() + disclosure_width,
            bounds.y() + ((header_height - title_height) * 0.5).max(0.0),
            (bounds.width() - action_width - disclosure_width).max(0.0),
            title_height,
        )
    }

    pub(super) fn header_rect(&self, bounds: Rect) -> Rect {
        let title_style = self.resolved_title_style();
        let header_height = self.header_height(&title_style);
        Rect::new(bounds.x(), bounds.y(), bounds.width(), header_height)
    }

    pub(super) fn header_hit_rect(&self, bounds: Rect) -> Rect {
        let metrics = self.resolved_theme().metrics;
        let header = self.header_rect(bounds);
        let action_width = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size().width + self.resolved_action_gap(metrics))
            .unwrap_or(0.0)
            .min(header.width());
        Rect::new(
            header.x(),
            header.y(),
            (header.width() - action_width).max(0.0),
            header.height(),
        )
    }

    pub(super) fn toggle(&mut self, ctx: &mut EventCtx) {
        if !self.collapsible {
            return;
        }

        self.expanded = !self.expanded;
        self.set_pressed_header(false, ctx);
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn set_hovered_header(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered_header == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered_header = hovered;
        set_hover_animation_target(&mut self.hover_animation, hovered as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn set_pressed_header(&mut self, pressed: bool, ctx: &mut EventCtx) {
        if self.pressed_header == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed_header = pressed;
        set_press_animation_target(&mut self.press_animation, pressed as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for PanelSection {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.collapsible {
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self
                    .header_hit_rect(ctx.bounds())
                    .contains(pointer.position);
                self.set_hovered_header(hovered, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .header_hit_rect(ctx.bounds())
                        .contains(pointer.position) =>
            {
                self.set_hovered_header(true, ctx);
                self.set_pressed_header(true, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pressed_header =>
            {
                let hovered = self
                    .header_hit_rect(ctx.bounds())
                    .contains(pointer.position);
                if hovered {
                    self.toggle(ctx);
                }
                self.set_hovered_header(hovered, ctx);
                self.set_pressed_header(false, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered_header(false, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_header || self.hovered_header {
                    self.set_hovered_header(false, ctx);
                    self.set_pressed_header(false, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "Enter" | " " => {
                        self.toggle(ctx);
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
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
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let action_gap = self.resolved_action_gap(metrics);
        let title_style = self.resolved_title_style();
        let title_measurement = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title_measurement);
        let action_size = self
            .header_action
            .as_mut()
            .map(|action| {
                action.measure(
                    ctx,
                    Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY)),
                )
            })
            .unwrap_or(Size::ZERO);
        let header_height = self.title_height(&title_style).max(action_size.height);
        let child_size = if self.is_expanded() {
            self.child.measure(ctx, constraints)
        } else {
            Size::ZERO
        };
        let header_width = if self.header_action.is_some() {
            self.disclosure_width(metrics)
                + title_measurement.width
                + action_gap
                + action_size.width
        } else {
            self.disclosure_width(metrics) + title_measurement.width
        };
        let natural = Size::new(
            header_width.max(child_size.width),
            header_height
                + if self.is_expanded() && child_size.height > 0.0 {
                    gap + child_size.height
                } else {
                    0.0
                },
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let gap = self.resolved_gap(metrics);
        let title_style = self.resolved_title_style();
        let header_height = self.header_height(&title_style);
        if let Some(action) = &mut self.header_action {
            let action_size = action.child().measured_size();
            action.arrange(
                ctx,
                Rect::new(
                    bounds.max_x() - action_size.width.min(bounds.width()),
                    bounds.y() + ((header_height - action_size.height) * 0.5).max(0.0),
                    action_size.width.min(bounds.width()).max(0.0),
                    action_size.height,
                ),
            );
        }
        let child_size = if self.is_expanded() {
            self.child.child().measured_size()
        } else {
            Size::ZERO
        };
        let child_height = if self.is_expanded() {
            child_size
                .height
                .min((bounds.height() - header_height - gap).max(0.0))
        } else {
            0.0
        };
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + header_height + gap,
                bounds.width().min(child_size.width).max(0.0),
                child_height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header_height = self.header_height(&title_style);
        let title_slot = self.title_rect(ctx.bounds(), header_height, title_height);
        if self.collapsible {
            let header_hit = self.header_hit_rect(ctx.bounds());
            let hover_amount = self.hover_animation.value;
            let press_amount = self.press_animation.value;
            let focus_amount = self.focus_animation.value;
            if focus_amount > AnimatedScalar::EPSILON {
                let outset = physical_pixels(ctx, metrics.focus_ring_outset);
                ctx.stroke(
                    rounded_rect_path(
                        header_hit.inflate(outset, outset),
                        metrics.indicator_corner_radius + outset,
                    ),
                    theme
                        .palette
                        .focus_ring
                        .with_alpha(theme.palette.focus_ring.alpha * focus_amount),
                    StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
                );
            }
            let hover_alpha = (theme.interaction.hover_blend * 0.07 * hover_amount).min(0.08);
            let press_alpha = (theme.interaction.selected_blend * 0.48 * press_amount).min(0.14);
            let header_fill = if press_alpha > 0.0 {
                theme.palette.text.with_alpha(press_alpha)
            } else if hover_alpha > 0.0 {
                theme.palette.text.with_alpha(hover_alpha)
            } else {
                theme.palette.surface.with_alpha(0.001)
            };
            ctx.fill(
                rounded_rect_path(header_hit, metrics.indicator_corner_radius),
                header_fill,
            );
            paint_panel_section_disclosure(
                ctx,
                self.header_rect(ctx.bounds()),
                self.expanded,
                hover_amount,
                press_amount,
                &theme,
                metrics.panel_section_disclosure_size,
            );
        }
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        if self.is_expanded() {
            self.child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut section = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        section.name = Some(self.title.clone());
        section.state.focused = ctx.is_focused();
        section.state.hovered = self.hovered_header;
        if self.collapsible {
            section.state.expanded = Some(self.expanded);
            section.actions = vec![
                SemanticsAction::Focus,
                SemanticsAction::Expand,
                SemanticsAction::Collapse,
            ];
        }
        ctx.push(section);

        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header_height = self.header_height(&title_style);
        let mut title = SemanticsNode::new(
            panel_section_title_id(ctx.widget_id()),
            SemanticsRole::Text,
            self.title_rect(ctx.bounds(), header_height, title_height),
        );
        title.parent = Some(ctx.widget_id());
        title.name = Some(self.title.clone());
        title.value = Some(SemanticsValue::Text(self.title.clone()));
        ctx.push(title);

        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        if self.is_expanded() {
            self.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.header_action {
            action.visit_children(visitor);
        }
        if self.is_expanded() {
            self.child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.header_action {
            action.visit_children_mut(visitor);
        }
        if self.is_expanded() {
            self.child.visit_children_mut(visitor);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.collapsible
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if self.collapsible {
            let theme = self.resolved_theme();
            set_focus_animation_target(
                &mut self.focus_animation,
                focused as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

pub(super) fn panel_section_title_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 3_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(431).wrapping_add(7) & LOW_MASK))
}

pub(super) fn paint_panel_section_disclosure(
    ctx: &mut PaintCtx,
    header: Rect,
    expanded: bool,
    hover_amount: f32,
    press_amount: f32,
    theme: &DefaultTheme,
    disclosure_size: f32,
) {
    let palette = theme.palette;
    let center = Point::new(
        header.x() + disclosure_size * 0.5,
        header.y() + header.height() * 0.5,
    );
    let half = disclosure_size * 0.25;
    let tip = disclosure_size * 0.22;
    let base_color = palette.text.with_alpha(0.68);
    let hover_color = mix_color(base_color, palette.text, hover_amount);
    let color = mix_color(hover_color, palette.text, press_amount);
    let mut builder = PathBuilder::new();
    if expanded {
        builder
            .move_to(Point::new(center.x - half, center.y - tip * 0.55))
            .line_to(Point::new(center.x + half, center.y - tip * 0.55))
            .line_to(Point::new(center.x, center.y + tip));
    } else {
        builder
            .move_to(Point::new(center.x - tip * 0.55, center.y - half))
            .line_to(Point::new(center.x + tip, center.y))
            .line_to(Point::new(center.x - tip * 0.55, center.y + half));
    }
    ctx.fill(builder.build(), color);
}

pub struct DockPanel {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: Option<String>,
    pub(super) title: String,
    pub(super) header_height: Option<f32>,
    pub(super) padding: Option<Insets>,
    pub(super) background: Option<Color>,
    pub(super) header_background: Option<Color>,
    pub(super) child: SingleChild,
    pub(super) title_measurement: Option<TextMeasurement>,
}

impl DockPanel {
    pub fn new<W>(title: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            title: title.into(),
            header_height: None,
            padding: None,
            background: None,
            header_background: None,
            child: SingleChild::new(child),
            title_measurement: None,
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

    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = Some(height.max(0.0));
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn header_background(mut self, color: Color) -> Self {
        self.header_background = Some(color);
        self
    }

    pub fn child(&self) -> &sui_runtime::WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.child.child_mut()
    }

    pub(super) fn resolved_title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        text_token_style(&theme, theme.text.sm, theme.palette.text)
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_header_height(&self, metrics: ControlMetrics) -> f32 {
        self.header_height
            .unwrap_or(metrics.dock_panel_header_height)
    }

    pub(super) fn resolved_padding(&self, metrics: ControlMetrics) -> Insets {
        self.padding.unwrap_or(metrics.dock_panel_padding)
    }

    pub(super) fn title_height(&self, style: &TextStyle) -> f32 {
        self.title_measurement
            .map(|measurement| measurement.height)
            .unwrap_or(style.line_height)
            .max(style.line_height)
    }

    pub(super) fn header_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            self.resolved_header_height(theme.metrics),
        )
    }

    pub(super) fn content_rect(&self, bounds: Rect) -> Rect {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let header_height = self.resolved_header_height(metrics);
        inset_rect(
            Rect::new(
                bounds.x(),
                bounds.y() + header_height,
                bounds.width(),
                (bounds.height() - header_height).max(0.0),
            ),
            self.resolved_padding(metrics),
        )
    }

    pub(super) fn child_constraints(&self, constraints: Constraints) -> Constraints {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let header_height = self.resolved_header_height(metrics);
        let width = if constraints.max.width.is_finite() {
            (constraints.max.width - padding.left - padding.right).max(0.0)
        } else {
            f32::INFINITY
        };
        let height = if constraints.max.height.is_finite() {
            (constraints.max.height - header_height - padding.top - padding.bottom).max(0.0)
        } else {
            f32::INFINITY
        };
        Constraints::new(Size::ZERO, Size::new(width, height))
    }
}

impl Widget for DockPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let header_height = self.resolved_header_height(metrics);
        let title_style = self.resolved_title_style();
        let title_measurement = measure_text(ctx, &self.title, &title_style);
        self.title_measurement = Some(title_measurement);
        let child_size = self.child.measure(ctx, self.child_constraints(constraints));
        let natural = Size::new(
            (title_measurement.width + padding.left + padding.right)
                .max(child_size.width + padding.left + padding.right),
            header_height + padding.top + child_size.height + padding.bottom,
        );

        constraints.clamp(natural)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let padding = self.resolved_padding(metrics);
        let bounds = ctx.bounds();
        let header = self.header_rect(bounds);
        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let title_slot = Rect::new(
            header.x() + padding.left,
            header.y() + ((header.height() - title_height) * 0.5).max(0.0),
            (header.width() - padding.left - padding.right).max(0.0),
            title_height,
        );
        let divider_height = physical_pixels(ctx, 1.0);

        ctx.fill_rect(bounds, self.background.unwrap_or(palette.surface));
        ctx.fill_rect(
            header,
            self.header_background
                .unwrap_or_else(|| palette.surface_raised.with_alpha(0.72)),
        );
        ctx.fill_rect(
            Rect::new(
                header.x(),
                header.max_y() - divider_height,
                header.width(),
                divider_height,
            ),
            palette.border,
        );
        ctx.push_clip_rect(title_slot);
        paint_aligned_text(
            ctx,
            title_slot,
            &self.title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();

        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let padding = self.resolved_padding(theme.metrics);
        let mut panel = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        panel.name = Some(self.name.clone().unwrap_or_else(|| self.title.clone()));
        ctx.push(panel);

        let title_style = self.resolved_title_style();
        let title_height = self.title_height(&title_style);
        let header = self.header_rect(ctx.bounds());
        let mut title = SemanticsNode::new(
            dock_panel_title_id(ctx.widget_id()),
            SemanticsRole::Text,
            Rect::new(
                header.x() + padding.left,
                header.y() + ((header.height() - title_height) * 0.5).max(0.0),
                (header.width() - padding.left - padding.right).max(0.0),
                title_height,
            ),
        );
        title.parent = Some(ctx.widget_id());
        title.name = Some(self.title.clone());
        title.value = Some(SemanticsValue::Text(self.title.clone()));
        ctx.push(title);

        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

pub(super) fn dock_panel_title_id(parent: WidgetId) -> WidgetId {
    const TAG: u64 = 5_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(TAG | (parent.get().wrapping_mul(467).wrapping_add(11) & LOW_MASK))
}
