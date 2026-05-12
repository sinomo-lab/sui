use crate::{
    Blink, ControlMetrics, DefaultTheme, Easing, HdrThemeMode, Interpolate, ResolvedEffectStyle,
    ResolvedHdrStyle, Transition, WidgetColorRole, WidgetLuminanceRole, WidgetMaterialRole,
    resolve_luminance_role, resolve_widget_hdr_style,
};
use sui_core::{
    Color, Event, ImeEvent, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind,
    Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size, TimerToken,
    ToggleState,
};
use sui_layout::{Axis, Constraints, Padding as Insets};
use sui_runtime::{
    EventCtx, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx, SemanticsCtx,
    StackSurfaceOptions, Widget, window_render_options,
};
use sui_scene::{LayerCompositionMode, StrokeStyle};
use sui_text::{PersistentTextLayout, TextMeasurement, TextStyle};

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
    MoreHorizontal,
    MoreVertical,
    Search,
}

pub struct Separator {
    theme: Box<DefaultTheme>,
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
            .unwrap_or(self.theme.metrics.separator_thickness)
            .max(1.0)
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
            self.theme.palette.border,
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
    label: Option<String>,
}

impl Icon {
    pub fn new(glyph: IconGlyph) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            glyph,
            size: None,
            color: None,
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
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
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
            self.color.unwrap_or(self.theme.palette.text),
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

const HOVER_ANIMATION_SECONDS: f64 = 1.0 / 8.0;
const PRESS_ANIMATION_SECONDS: f64 = 1.0 / 12.0;
const TOGGLE_ANIMATION_SECONDS: f64 = 1.0 / 6.0;
const FOCUS_ANIMATION_SECONDS: f64 = 1.0 / 7.0;
const CARET_BLINK_PERIOD_SECONDS: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimatedScalar {
    value: f32,
    target: f32,
    transition: Option<Transition<f32>>,
}

impl AnimatedScalar {
    const fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            transition: None,
        }
    }

    fn current(&self, time: f64) -> f32 {
        self.transition
            .map(|transition| transition.sample(time))
            .unwrap_or(self.value)
    }

    fn set_target(&mut self, target: f32, time: f64, duration: f64) -> bool {
        let target = target.clamp(0.0, 1.0);
        let current = self.current(time);
        if (current - target).abs() < 1e-4 {
            self.value = target;
            self.target = target;
            self.transition = None;
            return false;
        }

        self.value = current;
        self.target = target;
        self.transition = Some(Transition::new(
            current,
            target,
            time,
            duration,
            Easing::EaseInOut,
        ));
        true
    }

    fn advance(&mut self, time: f64) -> bool {
        let Some(transition) = self.transition else {
            return false;
        };

        self.value = transition.sample(time);
        if transition.is_complete(time) {
            self.value = self.target;
            self.transition = None;
            return false;
        }

        true
    }
}

fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    ctx: &mut EventCtx,
) {
    if animation.set_target(target, ctx.current_time(), duration) {
        ctx.request_animation_frame();
    }
}

fn mix_color(from: Color, to: Color, t: f32) -> Color {
    Color::interpolate(from, to, t)
}

pub struct IconButton {
    theme: Box<DefaultTheme>,
    icon: IconGlyph,
    label: String,
    size: Option<f32>,
    icon_size: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    on_press: Option<Box<dyn FnMut()>>,
}

impl IconButton {
    pub fn new(icon: IconGlyph, label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            icon,
            label: label.into(),
            size: None,
            icon_size: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            on_press: None,
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

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = Some(icon_size.max(0.0));
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    fn resolved_size(&self) -> f32 {
        self.size
            .unwrap_or(self.theme.metrics.icon_button_size)
            .max(self.theme.metrics.min_height)
    }

    fn resolved_icon_size(&self) -> f32 {
        self.icon_size.unwrap_or(self.theme.metrics.icon_size)
    }

    fn activate(&mut self) {
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            set_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                HOVER_ANIMATION_SECONDS,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time) | self.press_animation.advance(time)
    }
}

impl Widget for IconButton {
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
                self.pressed = true;
                self.hovered = true;
                set_animation_target(&mut self.hover_animation, 1.0, HOVER_ANIMATION_SECONDS, ctx);
                set_animation_target(&mut self.press_animation, 1.0, PRESS_ANIMATION_SECONDS, ctx);
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
                set_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_animation_target(&mut self.press_animation, 0.0, PRESS_ANIMATION_SECONDS, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate();
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    set_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_animation_target(
                        &mut self.press_animation,
                        0.0,
                        PRESS_ANIMATION_SECONDS,
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
                self.activate();
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
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let hover_progress = self.hover_animation.value;
        let press_progress = self.press_animation.value;
        let background = mix_color(
            mix_color(palette.surface, palette.surface_hover, hover_progress),
            palette.surface_pressed,
            press_progress,
        );
        let border = if ctx.is_focused() {
            palette.border_focus
        } else {
            mix_color(palette.border, palette.border_hover, hover_progress)
        };

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            background,
            border,
            ctx.is_focused().then_some(palette.focus_ring),
        );
        draw_icon_glyph(
            ctx,
            self.icon,
            center_square(ctx.bounds(), self.resolved_icon_size()),
            palette.text,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.label.clone());
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
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

pub struct Label {
    text: String,
    style: TextStyle,
    measurement: Option<TextMeasurement>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: DefaultTheme::default().body_text_style(),
            measurement: None,
        }
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
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
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
}

impl Widget for Label {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let natural_measurement = measure_text(ctx, &self.text, &self.style);
        let (measured_width, measurement) = if constraints.max.width.is_finite()
            && natural_measurement.width > constraints.max.width
        {
            let wrapped_measurement = ctx
                .layout()
                .shape_text(
                    self.text.clone(),
                    Size::new(constraints.max.width.max(1.0), f32::INFINITY),
                    self.style.clone(),
                )
                .map(|layout| layout.measurement())
                .unwrap_or(natural_measurement);
            (constraints.max.width.max(0.0), wrapped_measurement)
        } else {
            (natural_measurement.width, natural_measurement)
        };
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measured_width,
            measurement.height.max(self.style.line_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.draw_text(ctx.bounds(), self.text.clone(), self.style.clone());
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.text.clone());
        ctx.push(node);
    }
}

pub struct Button {
    theme: Box<DefaultTheme>,
    label: String,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    label_measurement: Option<TextMeasurement>,
    label_layout: Option<PersistentTextLayout>,
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
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            label: label.into(),
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            label_measurement: None,
            label_layout: None,
            on_press: None,
            on_press_with_ctx: None,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
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
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
        if let Some(on_press) = &mut self.on_press_with_ctx {
            on_press(ctx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            set_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                HOVER_ANIMATION_SECONDS,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time) | self.press_animation.advance(time)
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.theme.button_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding.unwrap_or(self.theme.metrics.button_padding)
    }

    fn resolved_min_size(&self) -> Size {
        Size::new(
            self.min_width
                .unwrap_or(self.theme.metrics.button_min_width),
            self.min_height.unwrap_or(self.theme.metrics.min_height),
        )
    }

    fn resolved_visuals(&self, focused: bool) -> ButtonVisuals {
        let palette = self.theme.palette;
        let background = if self.pressed {
            palette.accent_pressed
        } else if self.hovered {
            palette.accent_hover
        } else {
            palette.accent
        };
        let border = if focused {
            palette.accent_border_focus
        } else if self.hovered {
            palette.accent_border_hover
        } else {
            palette.accent_border
        };
        let label_peak_lift =
            resolve_luminance_role(&self.theme.hdr, WidgetLuminanceRole::Standard);
        let label_color = apply_hdr_policy_cap(self.resolved_text_style().color, label_peak_lift);

        if matches!(self.theme.hdr.mode, HdrThemeMode::Disabled) {
            return ButtonVisuals {
                background,
                border,
                focus_ring: focused.then_some(palette.focus_ring),
                label_color,
                label_peak_lift,
                chrome_style: None,
            };
        }

        let chrome_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::SemanticAccent,
            WidgetMaterialRole::Flat,
            None,
        ));
        let focus_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::Focused,
            WidgetMaterialRole::Flat,
            None,
        ));

        ButtonVisuals {
            background: if self.pressed {
                palette.accent_pressed
            } else if self.hovered {
                palette.accent_hover
            } else {
                chrome_style.color
            },
            border: if focused {
                focus_style.color
            } else if self.hovered {
                palette.accent_border_hover
            } else {
                palette.accent_border
            },
            focus_ring: focused.then_some(focus_style.color.with_alpha(palette.focus_ring.alpha)),
            label_color,
            label_peak_lift,
            chrome_style: Some(chrome_style),
        }
    }
}

impl Widget for Button {
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
                self.pressed = true;
                self.hovered = true;
                set_animation_target(&mut self.hover_animation, 1.0, HOVER_ANIMATION_SECONDS, ctx);
                set_animation_target(&mut self.press_animation, 1.0, PRESS_ANIMATION_SECONDS, ctx);
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
                set_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_animation_target(&mut self.press_animation, 0.0, PRESS_ANIMATION_SECONDS, ctx);
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
                    set_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_animation_target(
                        &mut self.press_animation,
                        0.0,
                        PRESS_ANIMATION_SECONDS,
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
        let label_layout = ctx
            .layout()
            .shape_text_persistent(
                self.label_layout.as_ref().map(|layout| layout.handle()),
                self.label.clone(),
                Size::new(f32::INFINITY, text_style.line_height.max(1.0)),
                text_style.clone(),
            )
            .ok();
        let measurement = label_layout
            .as_ref()
            .map(|layout| layout.measurement())
            .unwrap_or_else(|| measure_text(ctx, &self.label, &text_style));
        self.label_measurement = Some(measurement);
        self.label_layout = label_layout;

        let width = (measurement.width + padding.left + padding.right).max(min_size.width);
        let height =
            (measurement.height.max(text_style.line_height) + padding.top + padding.bottom)
                .max(min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let visuals = self.resolved_visuals(ctx.is_focused());
        let hover_progress = self.hover_animation.value;
        let press_progress = self.press_animation.value;
        let background = mix_color(
            mix_color(palette.accent, palette.accent_hover, hover_progress),
            palette.accent_pressed,
            press_progress,
        );
        let border = if ctx.is_focused() {
            palette.accent_border_focus
        } else {
            mix_color(
                palette.accent_border,
                palette.accent_border_hover,
                hover_progress,
            )
        };

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            background,
            border,
            visuals.focus_ring,
        );
        let label_rect = centered_text_rect(
            ctx,
            ctx.bounds(),
            padding,
            self.label_measurement,
            text_style.line_height,
        );
        if let Some(layout) = &self.label_layout {
            let layout_bounds = layout.measurement().bounds;
            ctx.push_clip_rect(label_rect);
            ctx.draw_persistent_text_layout(
                Point::new(label_rect.x() - layout_bounds.x(), label_rect.y()),
                layout,
            );
            ctx.pop_clip();
        } else {
            ctx.draw_text(
                label_rect,
                self.label.clone(),
                TextStyle {
                    color: visuals.label_color,
                    ..text_style
                },
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.label.clone());
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
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

pub struct Checkbox {
    theme: Box<DefaultTheme>,
    label: String,
    checked: bool,
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

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            label: label.into(),
            checked: false,
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

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
            self.hovered = hovered;
            set_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                HOVER_ANIMATION_SECONDS,
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
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding.unwrap_or(self.theme.metrics.checkbox_padding)
    }

    fn resolved_indicator_size(&self) -> f32 {
        self.indicator_size
            .unwrap_or(self.theme.metrics.checkbox_indicator_size)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap.unwrap_or(self.theme.metrics.checkbox_gap)
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
                self.pressed = true;
                self.hovered = true;
                set_animation_target(&mut self.hover_animation, 1.0, HOVER_ANIMATION_SECONDS, ctx);
                set_animation_target(&mut self.press_animation, 1.0, PRESS_ANIMATION_SECONDS, ctx);
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
                let toggle = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_animation_target(&mut self.press_animation, 0.0, PRESS_ANIMATION_SECONDS, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if toggle {
                    self.toggle();
                    set_animation_target(
                        &mut self.toggle_animation,
                        self.checked as u8 as f32,
                        TOGGLE_ANIMATION_SECONDS,
                        ctx,
                    );
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    set_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_animation_target(
                        &mut self.press_animation,
                        0.0,
                        PRESS_ANIMATION_SECONDS,
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
                self.toggle();
                set_animation_target(
                    &mut self.toggle_animation,
                    self.checked as u8 as f32,
                    TOGGLE_ANIMATION_SECONDS,
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
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let measurement = measure_text(ctx, &self.label, &text_style);
        self.label_measurement = Some(measurement);

        let width = padding.left + indicator_size + gap + measurement.width + padding.right;
        let height = (indicator_size.max(measurement.height.max(text_style.line_height))
            + padding.top
            + padding.bottom)
            .max(self.theme.metrics.min_height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let hover_progress = self.hover_animation.value;
        let press_progress = self.press_animation.value;
        let toggle_progress = self.toggle_animation.value;
        let focus_progress = self.focus_animation.value;
        let background = mix_color(
            mix_color(palette.surface, palette.surface_hover, hover_progress),
            palette.surface_pressed,
            press_progress,
        );
        let border = if ctx.is_focused() {
            palette.border_focus
        } else {
            mix_color(palette.border, palette.border_hover, hover_progress)
        };
        let indicator = indicator_rect(ctx.bounds(), padding, indicator_size);
        let label_rect = checkbox_label_rect(ctx.bounds(), padding, indicator_size, gap);

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

        let indicator_background = mix_color(
            mix_color(
                palette.surface_pressed,
                palette.surface_focus,
                hover_progress,
            ),
            mix_color(
                mix_color(palette.accent, palette.accent_hover, hover_progress),
                palette.accent_pressed,
                press_progress,
            ),
            toggle_progress,
        );
        let indicator_border = mix_color(
            border,
            palette.accent_border_focus,
            toggle_progress.max(focus_progress),
        );

        draw_control_shape(
            ctx,
            indicator,
            metrics.indicator_corner_radius,
            metrics.border_width,
            indicator_background,
            indicator_border,
        );
        if toggle_progress > 0.0 {
            let check_color = palette.accent_text.with_alpha(toggle_progress);
            ctx.stroke(
                checkmark_path(indicator.inflate(-4.0, -4.0)),
                check_color,
                StrokeStyle::new(physical_pixels(ctx, 2.0)),
            );
        }
        ctx.draw_text(
            vertically_centered_text_rect(
                ctx,
                label_rect,
                self.label_measurement,
                text_style.line_height,
            ),
            self.label.clone(),
            text_style,
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
        set_animation_target(
            &mut self.focus_animation,
            focused as u8 as f32,
            FOCUS_ANIMATION_SECONDS,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Switch {
    theme: Box<DefaultTheme>,
    label: String,
    on: bool,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    gap: Option<f32>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    toggle_animation: AnimatedScalar,
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
            label: label.into(),
            on: false,
            text_style: None,
            padding: None,
            gap: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            toggle_animation: AnimatedScalar::new(0.0),
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

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        self.toggle_animation = AnimatedScalar::new(on as u8 as f32);
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding.unwrap_or(self.theme.metrics.checkbox_padding)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap.unwrap_or(self.theme.metrics.checkbox_gap)
    }

    fn toggle(&mut self) {
        self.on = !self.on;
        if let Some(on_toggle) = &mut self.on_toggle {
            on_toggle(self.on);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            set_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                HOVER_ANIMATION_SECONDS,
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
    }

    fn resolved_visuals_for_state(&self, on: bool, focused: bool) -> SwitchVisuals {
        let palette = self.theme.palette;
        let frame_background = if self.pressed {
            palette.surface_pressed
        } else if self.hovered {
            palette.surface_hover
        } else if focused {
            palette.surface_focus
        } else {
            palette.surface
        };
        let frame_border = if focused {
            palette.border_focus
        } else if self.hovered {
            palette.border_hover
        } else {
            palette.border
        };
        let baseline_track_color = if on {
            if self.pressed {
                palette.accent_pressed
            } else if self.hovered {
                palette.accent_hover
            } else {
                palette.accent
            }
        } else if self.hovered {
            palette.surface_pressed
        } else {
            palette.surface_focus
        };
        let baseline_track_border = if on {
            palette.accent_border
        } else if self.hovered {
            palette.border_hover
        } else {
            palette.border
        };
        let label_peak_lift =
            resolve_luminance_role(&self.theme.hdr, WidgetLuminanceRole::Standard);
        let label_color = apply_hdr_policy_cap(self.resolved_text_style().color, label_peak_lift);

        if matches!(self.theme.hdr.mode, HdrThemeMode::Disabled) || !on {
            return SwitchVisuals {
                frame_background,
                frame_border,
                track_color: baseline_track_color,
                track_border: baseline_track_border,
                thumb_color: palette.accent_text,
                label_color,
                label_peak_lift,
                indicator_style: None,
            };
        }

        let indicator_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::EmissiveIndicator,
            WidgetMaterialRole::Flat,
            None,
        ));

        SwitchVisuals {
            frame_background,
            frame_border,
            track_color: if self.pressed {
                palette.accent_pressed
            } else if self.hovered {
                palette.accent_hover
            } else {
                indicator_style.color
            },
            track_border: if focused {
                indicator_style.color
            } else {
                palette.accent_border
            },
            thumb_color: palette.accent_text,
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
                self.pressed = true;
                self.hovered = true;
                set_animation_target(&mut self.hover_animation, 1.0, HOVER_ANIMATION_SECONDS, ctx);
                set_animation_target(&mut self.press_animation, 1.0, PRESS_ANIMATION_SECONDS, ctx);
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
                let toggle = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_animation_target(&mut self.press_animation, 0.0, PRESS_ANIMATION_SECONDS, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if toggle {
                    self.toggle();
                    set_animation_target(
                        &mut self.toggle_animation,
                        self.on as u8 as f32,
                        TOGGLE_ANIMATION_SECONDS,
                        ctx,
                    );
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    set_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_animation_target(
                        &mut self.press_animation,
                        0.0,
                        PRESS_ANIMATION_SECONDS,
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
                self.toggle();
                set_animation_target(
                    &mut self.toggle_animation,
                    self.on as u8 as f32,
                    TOGGLE_ANIMATION_SECONDS,
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
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let gap = self.resolved_gap();
        let measurement = measure_text(ctx, &self.label, &text_style);
        self.label_measurement = Some(measurement);
        let track_width = self.theme.metrics.switch_track_width;
        let track_height = self.theme.metrics.switch_track_height;

        constraints.clamp(Size::new(
            padding.left + track_width + gap + measurement.width + padding.right,
            (track_height.max(measurement.height.max(text_style.line_height))
                + padding.top
                + padding.bottom)
                .max(self.theme.metrics.min_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let metrics = self.theme.metrics;
        let palette = self.theme.palette;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let gap = self.resolved_gap();
        let track = switch_track_rect(ctx.bounds(), padding, metrics);
        let label_rect = switch_label_rect(ctx.bounds(), padding, metrics, gap);
        let visuals = self.resolved_visuals(ctx.is_focused());
        let off_visuals = self.resolved_visuals_for_state(false, ctx.is_focused());
        let on_visuals = self.resolved_visuals_for_state(true, ctx.is_focused());
        let hover_progress = self.hover_animation.value;
        let press_progress = self.press_animation.value;
        let toggle_progress = self.toggle_animation.value;

        let frame_background = mix_color(
            mix_color(palette.surface, palette.surface_hover, hover_progress),
            palette.surface_pressed,
            press_progress,
        );
        let frame_border = if ctx.is_focused() {
            palette.border_focus
        } else {
            mix_color(palette.border, palette.border_hover, hover_progress)
        };

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            frame_background,
            frame_border,
            ctx.is_focused().then_some(self.theme.palette.focus_ring),
        );

        let thumb_size = (track.height() - 4.0).max(0.0);
        let thumb_x_off = track.x() + 2.0;
        let thumb_x_on = track.max_x() - thumb_size - 2.0;
        let thumb = Rect::new(
            f32::interpolate(thumb_x_off, thumb_x_on, toggle_progress),
            track.y() + 2.0,
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
        ctx.draw_text(
            vertically_centered_text_rect(
                ctx,
                label_rect,
                self.label_measurement,
                text_style.line_height,
            ),
            self.label.clone(),
            TextStyle {
                color: visuals.label_color,
                ..text_style
            },
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct RadioButton {
    theme: Box<DefaultTheme>,
    label: String,
    selected: bool,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    indicator_size: Option<f32>,
    gap: Option<f32>,
    hovered: bool,
    pressed: bool,
    label_measurement: Option<TextMeasurement>,
    on_select: Option<Box<dyn FnMut()>>,
}

impl RadioButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            label: label.into(),
            selected: false,
            text_style: None,
            padding: None,
            indicator_size: None,
            gap: None,
            hovered: false,
            pressed: false,
            label_measurement: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding.unwrap_or(self.theme.metrics.checkbox_padding)
    }

    fn resolved_indicator_size(&self) -> f32 {
        self.indicator_size
            .unwrap_or(self.theme.metrics.checkbox_indicator_size)
    }

    fn resolved_gap(&self) -> f32 {
        self.gap.unwrap_or(self.theme.metrics.checkbox_gap)
    }

    fn activate(&mut self) {
        self.selected = true;
        if let Some(on_select) = &mut self.on_select {
            on_select();
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
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
                    self.activate();
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
                self.activate();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
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

        constraints.clamp(Size::new(
            padding.left + indicator_size + gap + measurement.width + padding.right,
            (indicator_size.max(measurement.height.max(text_style.line_height))
                + padding.top
                + padding.bottom)
                .max(self.theme.metrics.min_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let indicator_size = self.resolved_indicator_size();
        let gap = self.resolved_gap();
        let indicator = indicator_rect(ctx.bounds(), padding, indicator_size);
        let label_rect = checkbox_label_rect(ctx.bounds(), padding, indicator_size, gap);

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            if self.pressed {
                palette.surface_pressed
            } else if self.hovered {
                palette.surface_hover
            } else if ctx.is_focused() {
                palette.surface_focus
            } else {
                palette.surface
            },
            if ctx.is_focused() {
                palette.border_focus
            } else if self.hovered {
                palette.border_hover
            } else {
                palette.border
            },
            ctx.is_focused().then_some(palette.focus_ring),
        );

        ctx.fill(
            Path::circle(rect_center(indicator), indicator.width() * 0.5),
            if self.selected {
                palette.accent
            } else if self.hovered {
                palette.surface_focus
            } else {
                palette.surface_pressed
            },
        );
        ctx.stroke(
            Path::circle(rect_center(indicator), (indicator.width() * 0.5) - 0.5),
            if self.selected {
                palette.accent_border
            } else if ctx.is_focused() {
                palette.border_focus
            } else if self.hovered {
                palette.border_hover
            } else {
                palette.border
            },
            StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
        );
        if self.selected {
            ctx.fill(
                Path::circle(rect_center(indicator), indicator.width() * 0.22),
                palette.accent_text,
            );
        }
        ctx.draw_text(
            vertically_centered_text_rect(
                ctx,
                label_rect,
                self.label_measurement,
                text_style.line_height,
            ),
            self.label.clone(),
            text_style,
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct RadioGroup {
    theme: Box<DefaultTheme>,
    name: String,
    options: Vec<String>,
    selected: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    label_measurements: Vec<TextMeasurement>,
    spacing: f32,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl RadioGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            options: Vec::new(),
            selected: None,
            hovered: None,
            pressed: None,
            label_measurements: Vec::new(),
            spacing: 6.0,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn row_height(&self) -> f32 {
        self.theme.metrics.min_height
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

    fn select(&mut self, index: usize) {
        self.selected = Some(index.min(self.options.len().saturating_sub(1)));
        if let Some(on_change) = &mut self.on_change {
            if let Some(selected) = self.selected {
                on_change(selected, self.options[selected].clone());
            }
        }
    }
}

impl Widget for RadioGroup {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.option_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.hovered = self.option_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
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
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                if let Some((index, _)) = activate {
                    self.select(index);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
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

                let current = self.selected.unwrap_or(0).min(self.options.len() - 1);
                let next = match key.key.as_str() {
                    "ArrowUp" | "ArrowLeft" => Some(current.saturating_sub(1)),
                    "ArrowDown" | "ArrowRight" => Some((current + 1).min(self.options.len() - 1)),
                    "Home" => Some(0),
                    "End" => Some(self.options.len() - 1),
                    "Enter" | " " => Some(current),
                    _ => None,
                };

                if let Some(next) = next {
                    self.hovered = Some(next);
                    self.select(next);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.theme.body_text_style();
        let padding = self.theme.metrics.checkbox_padding;
        let indicator = self.theme.metrics.checkbox_indicator_size;
        let gap = self.theme.metrics.checkbox_gap;
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

        constraints.clamp(Size::new(
            width.max(self.theme.metrics.button_min_width),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for (index, option) in self.options.iter().enumerate() {
            let row = self.row_rect(ctx.bounds(), index);
            let indicator = indicator_rect(
                row,
                self.theme.metrics.checkbox_padding,
                self.theme.metrics.checkbox_indicator_size,
            );
            let label_rect = checkbox_label_rect(
                row,
                self.theme.metrics.checkbox_padding,
                self.theme.metrics.checkbox_indicator_size,
                self.theme.metrics.checkbox_gap,
            );
            let hovered = self.hovered == Some(index);
            let selected = self.selected == Some(index);
            let background = if self.pressed == Some(index) {
                self.theme.palette.surface_pressed
            } else if hovered {
                self.theme.palette.surface_hover
            } else {
                self.theme.palette.surface
            };

            draw_control_shape(
                ctx,
                row,
                self.theme.metrics.corner_radius,
                physical_pixels(ctx, self.theme.metrics.border_width),
                background,
                if selected {
                    self.theme.palette.accent_border
                } else if hovered {
                    self.theme.palette.border_hover
                } else {
                    self.theme.palette.border
                },
            );
            ctx.fill(
                Path::circle(rect_center(indicator), indicator.width() * 0.5),
                if selected {
                    self.theme.palette.accent
                } else {
                    self.theme.palette.surface_pressed
                },
            );
            ctx.stroke(
                Path::circle(rect_center(indicator), (indicator.width() * 0.5) - 0.5),
                if selected {
                    self.theme.palette.accent_border
                } else {
                    self.theme.palette.border
                },
                StrokeStyle::new(physical_pixels(ctx, self.theme.metrics.border_width)),
            );
            if selected {
                ctx.fill(
                    Path::circle(rect_center(indicator), indicator.width() * 0.22),
                    self.theme.palette.accent_text,
                );
            }
            let text_style = self.theme.body_text_style();
            ctx.draw_text(
                vertically_centered_text_rect(
                    ctx,
                    label_rect,
                    self.label_measurements.get(index).copied(),
                    text_style.line_height,
                ),
                option.clone(),
                text_style,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::RadioGroup, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .selected
            .and_then(|index| self.options.get(index).cloned())
            .map(SemanticsValue::Text);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_semantics();
    }
}

pub struct Slider {
    theme: Box<DefaultTheme>,
    name: String,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    hovered: bool,
    dragging: bool,
    hover_animation: AnimatedScalar,
    drag_animation: AnimatedScalar,
    on_change: Option<Box<dyn FnMut(f64)>>,
}

impl Slider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            min: 0.0,
            max: 1.0,
            step: 0.01,
            value: 0.0,
            hovered: false,
            dragging: false,
            hover_animation: AnimatedScalar::new(0.0),
            drag_animation: AnimatedScalar::new(0.0),
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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

    fn fraction(&self) -> f32 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            return 0.0;
        }

        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
    }

    fn track_rect(&self, bounds: Rect) -> Rect {
        let padding = self.theme.metrics.text_input_padding;
        let height = self.theme.metrics.slider_track_height.max(1.0);
        Rect::new(
            bounds.x() + padding.left,
            bounds.y() + ((bounds.height() - height) * 0.5),
            (bounds.width() - padding.left - padding.right).max(0.0),
            height,
        )
    }

    fn thumb_rect(&self, bounds: Rect) -> Rect {
        let track = self.track_rect(bounds);
        let thumb = self.theme.metrics.slider_thumb_size;
        Rect::new(
            track.x() + (track.width() * self.fraction()) - (thumb * 0.5),
            bounds.y() + ((bounds.height() - thumb) * 0.5),
            thumb,
            thumb,
        )
    }

    fn set_from_position(&mut self, bounds: Rect, position: Point) {
        let track = self.track_rect(bounds);
        if track.width() <= 0.0 {
            return;
        }

        let fraction = ((position.x - track.x()) / track.width()).clamp(0.0, 1.0);
        let raw = self.min + ((self.max - self.min) * f64::from(fraction));
        let next = clamp_and_snap_value(raw, self.min, self.max, self.step);
        if (next - self.value).abs() > f64::EPSILON {
            self.value = next;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.value);
            }
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            set_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                HOVER_ANIMATION_SECONDS,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time) | self.drag_animation.advance(time)
    }
}

impl Widget for Slider {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                self.set_hovered(hovered, ctx);
                if self.dragging {
                    let previous = self.value;
                    self.set_from_position(ctx.bounds(), pointer.position);
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
                self.dragging = true;
                self.hovered = true;
                set_animation_target(&mut self.hover_animation, 1.0, HOVER_ANIMATION_SECONDS, ctx);
                set_animation_target(&mut self.drag_animation, 1.0, PRESS_ANIMATION_SECONDS, ctx);
                self.set_from_position(ctx.bounds(), pointer.position);
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
                self.dragging = false;
                self.hovered = ctx.bounds().contains(pointer.position);
                set_animation_target(
                    &mut self.hover_animation,
                    self.hovered as u8 as f32,
                    HOVER_ANIMATION_SECONDS,
                    ctx,
                );
                set_animation_target(&mut self.drag_animation, 0.0, PRESS_ANIMATION_SECONDS, ctx);
                self.set_from_position(ctx.bounds(), pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.dragging {
                    self.dragging = false;
                    self.hovered = false;
                    set_animation_target(
                        &mut self.hover_animation,
                        0.0,
                        HOVER_ANIMATION_SECONDS,
                        ctx,
                    );
                    set_animation_target(
                        &mut self.drag_animation,
                        0.0,
                        PRESS_ANIMATION_SECONDS,
                        ctx,
                    );
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
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
                        if let Some(on_change) = &mut self.on_change {
                            on_change(self.value);
                        }
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
        constraints.clamp(Size::new(
            self.theme.metrics.slider_min_width,
            self.theme.metrics.min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let hover_progress = self.hover_animation.value;
        let drag_progress = self.drag_animation.value;
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
                palette.surface,
                palette.surface_hover,
                hover_progress.max(drag_progress),
            ),
            if ctx.is_focused() {
                palette.border_focus
            } else {
                mix_color(palette.border, palette.border_hover, hover_progress)
            },
            ctx.is_focused().then_some(palette.focus_ring),
        );
        ctx.fill(
            rounded_rect_path(track, track.height() * 0.5),
            palette.surface_pressed,
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct NumberInput {
    theme: Box<DefaultTheme>,
    name: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    precision: usize,
    buffer: String,
    hovered: bool,
    on_change: Option<Box<dyn FnMut(f64)>>,
}

impl NumberInput {
    pub fn new(name: impl Into<String>) -> Self {
        let value = 0.0;
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            value,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            precision: 2,
            buffer: format_number(value, 2),
            hovered: false,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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

    fn text_style(&self) -> TextStyle {
        self.theme.body_text_style()
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

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for NumberInput {
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
                self.hovered = true;
                ctx.request_focus();
                if number_input_stepper_rect(ctx.bounds(), self.theme.metrics)
                    .contains(pointer.position)
                {
                    if pointer.position.y < ctx.bounds().y() + (ctx.bounds().height() * 0.5) {
                        self.nudge(self.step);
                    } else {
                        self.nudge(-self.step);
                    }
                }
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
                        if let Some(text) = keyboard_text(key) {
                            if text.chars().all(is_numeric_input_char) {
                                self.buffer.push_str(text);
                                self.apply_edit_buffer();
                            }
                        }
                    }
                }
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let measurement = measure_text(ctx, &self.buffer, &self.text_style());
        let padding = self.theme.metrics.text_input_padding;
        constraints.clamp(Size::new(
            (measurement.width
                + padding.left
                + padding.right
                + self.theme.metrics.number_input_stepper_width)
                .max(self.theme.metrics.button_min_width + 60.0),
            self.theme.metrics.min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let content = number_input_text_rect(ctx.bounds(), metrics);
        let stepper = number_input_stepper_rect(ctx.bounds(), metrics);
        let text_style = self.text_style();
        let measurement = paint_text_measurement(ctx, &self.buffer, &text_style);

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            if ctx.is_focused() {
                palette.surface_focus
            } else if self.hovered {
                palette.surface_hover
            } else {
                palette.surface
            },
            if ctx.is_focused() {
                palette.border_focus
            } else if self.hovered {
                palette.border_hover
            } else {
                palette.border
            },
            ctx.is_focused().then_some(palette.focus_ring),
        );

        ctx.draw_text(
            vertically_centered_text_rect(ctx, content, Some(measurement), text_style.line_height),
            self.buffer.clone(),
            text_style,
        );
        ctx.stroke(
            line_path(
                Point::new(stepper.x(), ctx.bounds().y() + 6.0),
                Point::new(stepper.x(), ctx.bounds().max_y() - 6.0),
            ),
            palette.border,
            StrokeStyle::new(physical_pixels(ctx, metrics.border_width)),
        );
        draw_icon_glyph(
            ctx,
            IconGlyph::ChevronUp,
            Rect::new(
                stepper.x(),
                stepper.y(),
                stepper.width(),
                stepper.height() * 0.5,
            ),
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
            ),
            palette.text,
        );

        if ctx.is_focused() {
            let caret_x = content.x()
                + measure_text_width_estimate(&self.buffer, self.text_style().font_size.max(1.0));
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let caret = Rect::new(
                caret_x.min((content.max_x() - caret_width).max(content.x())),
                content.y(),
                caret_width,
                content.height(),
            );
            ctx.set_ime_composition_rect(caret);
            ctx.fill(
                rounded_rect_path(caret, caret_width * 0.5),
                palette.accent_text,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::SpinBox, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Number(self.value));
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
        if !focused {
            self.commit_buffer();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct TextArea {
    theme: Box<DefaultTheme>,
    name: String,
    value: String,
    placeholder: String,
    composition: String,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    focused: bool,
    focus_animation: AnimatedScalar,
    caret_blink: Blink,
    caret_timer: Option<TimerToken>,
    caret_visible: bool,
    display_layout: Option<PersistentTextLayout>,
    input_layout: Option<PersistentTextLayout>,
    on_change: Option<Box<dyn FnMut(String)>>,
}

impl TextArea {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            value: String::new(),
            placeholder: String::new(),
            composition: String::new(),
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            focused: false,
            focus_animation: AnimatedScalar::new(0.0),
            caret_blink: Blink::new(CARET_BLINK_PERIOD_SECONDS),
            caret_timer: None,
            caret_visible: true,
            display_layout: None,
            input_layout: None,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.composition.clear();
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn input_text(&self) -> String {
        let mut text = self.value.clone();
        text.push_str(&self.composition);
        text
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
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.theme.metrics.text_input_padding)
    }

    fn resolved_min_size(&self) -> Size {
        Size::new(
            self.min_width
                .unwrap_or(self.theme.metrics.text_input_min_width),
            self.min_height
                .unwrap_or(self.theme.metrics.text_area_min_height),
        )
    }

    fn commit_text_change(&mut self) {
        if let Some(on_change) = &mut self.on_change {
            on_change(self.value.clone());
        }
    }

    fn insert_text(&mut self, text: &str, ctx: &mut EventCtx) {
        if text.is_empty() {
            return;
        }

        self.value.push_str(text);
        self.composition.clear();
        self.commit_text_change();
        if self.focused {
            self.reset_caret_blink(ctx);
        }
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
        ctx.set_handled();
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
        self.caret_visible = self.focused;
        self.arm_caret_blink(ctx);
    }
}

impl Widget for TextArea {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = ctx.bounds().contains(pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.hovered = true;
                if self.focused {
                    self.reset_caret_blink(ctx);
                }
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                self.composition.clear();
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionUpdate { text }) if ctx.is_focused() => {
                self.composition = text.clone();
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                self.insert_text(text, ctx);
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                self.composition.clear();
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Backspace" =>
            {
                if !self.composition.is_empty() {
                    self.composition.clear();
                } else if self.value.pop().is_some() {
                    self.commit_text_change();
                }
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Enter" =>
            {
                self.insert_text("\n", ctx);
            }
            Event::Keyboard(key) if ctx.is_focused() && self.composition.is_empty() => {
                if let Some(text) = keyboard_text(key) {
                    self.insert_text(text, ctx);
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
                let previous_focus = self.focus_animation.value;
                let animating = self.focus_animation.advance(*time);
                let focus_changed = (self.focus_animation.value - previous_focus).abs() > 1e-4;
                if animating {
                    ctx.request_animation_frame();
                }
                if focus_changed {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
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
        let display_style = if input_text.is_empty() {
            self.theme.placeholder_text_style()
        } else {
            text_style.clone()
        };

        let display_layout = ctx
            .layout()
            .shape_text_persistent(
                self.display_layout.as_ref().map(|layout| layout.handle()),
                display_text,
                Size::new(content_width.max(1.0), f32::INFINITY),
                display_style,
            )
            .ok();
        let input_layout = ctx
            .layout()
            .shape_text_persistent(
                self.input_layout.as_ref().map(|layout| layout.handle()),
                input_text,
                Size::new(content_width.max(1.0), f32::INFINITY),
                text_style.clone(),
            )
            .ok();

        let measured_height = display_layout
            .as_ref()
            .map(|layout| layout.measurement().height.max(text_style.line_height))
            .unwrap_or(text_style.line_height);
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
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let padding = self.resolved_padding();
        let content = inset_rect(ctx.bounds(), padding);
        let focus_progress = self.focus_animation.value;

        draw_control_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius,
            metrics,
            mix_color(
                if self.hovered {
                    palette.surface_hover
                } else {
                    palette.surface
                },
                palette.surface_focus,
                focus_progress,
            ),
            mix_color(
                if self.hovered {
                    palette.border_hover
                } else {
                    palette.border
                },
                palette.border_focus,
                focus_progress,
            ),
            (focus_progress > 0.0).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * focus_progress),
            ),
        );

        if let Some(layout) = &self.display_layout {
            ctx.push_clip_rect(content);
            ctx.draw_persistent_text_layout(content.origin, layout);
            ctx.pop_clip();
        }

        if self.focused {
            let caret = self
                .input_layout
                .as_ref()
                .map(|layout| {
                    layout
                        .caret_rect(self.input_text().len())
                        .translate(content.origin.to_vector())
                })
                .unwrap_or(Rect::new(
                    content.x(),
                    content.y(),
                    metrics.caret_width,
                    content.height(),
                ));
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let caret = Rect::new(caret.x(), caret.y(), caret_width, caret.height().max(1.0));
            ctx.set_ime_composition_rect(caret);
            if self.caret_visible {
                ctx.fill(
                    rounded_rect_path(caret, caret_width * 0.5),
                    palette.accent_text,
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.input_text()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.focused = focused;
        if !focused && !self.composition.is_empty() {
            self.composition.clear();
            ctx.request_measure();
        }
        if focused {
            self.reset_caret_blink(ctx);
        } else {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
        }
        set_animation_target(
            &mut self.focus_animation,
            focused as u8 as f32,
            FOCUS_ANIMATION_SECONDS,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Select {
    theme: Box<DefaultTheme>,
    name: String,
    options: Vec<String>,
    selected: Option<usize>,
    placeholder: String,
    expanded: bool,
    hovered_option: Option<usize>,
    hovered_header: bool,
    pressed_header: bool,
    on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl Select {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            options: Vec::new(),
            selected: None,
            placeholder: String::new(),
            expanded: false,
            hovered_option: None,
            hovered_header: false,
            pressed_header: false,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn current_value(&self) -> Option<&str> {
        self.selected
            .and_then(|index| self.options.get(index).map(String::as_str))
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn header_height(&self) -> f32 {
        self.theme.metrics.min_height
    }

    fn current_label(&self) -> String {
        self.current_value()
            .map(str::to_string)
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), self.header_height())
    }

    fn menu_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y() + self.header_height() + 6.0,
            bounds.width(),
            (self.options.len() as f32 * self.header_height())
                .min(self.theme.metrics.select_menu_max_height),
        )
    }

    fn option_rect(&self, bounds: Rect, index: usize) -> Rect {
        let menu = self.menu_rect(bounds);
        Rect::new(
            menu.x(),
            menu.y() + (index as f32 * self.header_height()),
            menu.width(),
            self.header_height(),
        )
    }

    fn option_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }

        self.options.iter().enumerate().find_map(|(index, _)| {
            self.option_rect(bounds, index)
                .contains(position)
                .then_some(index)
        })
    }

    fn select_index(&mut self, index: usize) {
        let index = index.min(self.options.len().saturating_sub(1));
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, self.options[index].clone());
        }
    }

    fn set_hover_state(
        &mut self,
        hovered_header: bool,
        hovered_option: Option<usize>,
        ctx: &mut EventCtx,
    ) {
        if self.hovered_header != hovered_header || self.hovered_option != hovered_option {
            self.hovered_header = hovered_header;
            self.hovered_option = hovered_option;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for Select {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hover_state(
                    self.header_rect(ctx.bounds()).contains(pointer.position),
                    self.option_at(ctx.bounds(), pointer.position),
                    ctx,
                );
            }
            Event::Pointer(pointer) if matches!(pointer.kind, PointerEventKind::Enter) => {
                self.set_hover_state(
                    self.header_rect(ctx.bounds()).contains(pointer.position),
                    self.option_at(ctx.bounds(), pointer.position),
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
                self.hovered_header = self.header_rect(ctx.bounds()).contains(pointer.position);
                self.hovered_option = self.option_at(ctx.bounds(), pointer.position);
                self.pressed_header = self.hovered_header;
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
                let hovered_option = self.option_at(ctx.bounds(), pointer.position);

                if self.pressed_header && hovered_header {
                    self.expanded = !self.expanded;
                    if self.expanded {
                        self.hovered_option = self.selected.or(Some(0));
                    }
                } else if let Some(index) = hovered_option {
                    self.select_index(index);
                    self.expanded = false;
                } else {
                    self.expanded = false;
                }

                self.pressed_header = false;
                self.hovered_header = hovered_header;
                self.hovered_option = if self.expanded {
                    self.hovered_option
                } else {
                    None
                };
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed_header {
                    self.pressed_header = false;
                    self.hovered_header = false;
                    self.hovered_option = None;
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
                            if let Some(index) = self.hovered_option.or(self.selected) {
                                self.select_index(index);
                            }
                            self.expanded = false;
                        } else {
                            self.expanded = true;
                            self.hovered_option = self.selected.or(Some(0));
                        }
                    }
                    "Escape" => self.expanded = false,
                    "ArrowDown" => {
                        if self.expanded {
                            let next = self
                                .hovered_option
                                .unwrap_or_else(|| self.selected.unwrap_or(0))
                                .saturating_add(1)
                                .min(self.options.len() - 1);
                            self.hovered_option = Some(next);
                        } else {
                            let next = self
                                .selected
                                .unwrap_or(0)
                                .saturating_add(1)
                                .min(self.options.len() - 1);
                            self.select_index(next);
                        }
                    }
                    "ArrowUp" => {
                        if self.expanded {
                            let next = self
                                .hovered_option
                                .unwrap_or_else(|| self.selected.unwrap_or(0))
                                .saturating_sub(1);
                            self.hovered_option = Some(next);
                        } else {
                            let next = self.selected.unwrap_or(0).saturating_sub(1);
                            self.select_index(next);
                        }
                    }
                    "Home" => self.select_index(0),
                    "End" => self.select_index(self.options.len() - 1),
                    _ => {}
                }

                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let padding = self.theme.metrics.text_input_padding;
        let text_style = self.theme.body_text_style();
        let widest = self
            .options
            .iter()
            .chain(std::iter::once(&self.placeholder))
            .map(|label| measure_text(ctx, label, &text_style).width)
            .fold(0.0, f32::max);
        let width = (widest + padding.left + padding.right + 24.0)
            .max(self.theme.metrics.button_min_width + 40.0);
        let height = self.header_height();

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let header = self.header_rect(ctx.bounds());
        let label = self.current_label();
        let placeholder = self.current_value().is_none();
        let text_style = if placeholder {
            self.theme.placeholder_text_style()
        } else {
            self.theme.body_text_style()
        };
        let text_measurement = paint_text_measurement(ctx, &label, &text_style);
        let text_rect = vertically_centered_text_rect(
            ctx,
            horizontal_text_inset_rect(header, metrics.text_input_padding),
            Some(text_measurement),
            text_style.line_height,
        );

        draw_control_frame(
            ctx,
            header,
            metrics.corner_radius,
            metrics,
            if self.hovered_header {
                palette.surface_hover
            } else if ctx.is_focused() {
                palette.surface_focus
            } else {
                palette.surface
            },
            if ctx.is_focused() {
                palette.border_focus
            } else if self.hovered_header {
                palette.border_hover
            } else {
                palette.border
            },
            ctx.is_focused().then_some(palette.focus_ring),
        );
        ctx.draw_text(text_rect, label, text_style);
        draw_icon_glyph(
            ctx,
            if self.expanded {
                IconGlyph::ChevronUp
            } else {
                IconGlyph::ChevronDown
            },
            Rect::new(header.max_x() - 28.0, header.y(), 20.0, header.height()),
            palette.text,
        );

        if self.expanded {
            let menu = self.menu_rect(ctx.bounds());
            draw_control_shape(
                ctx,
                menu,
                metrics.corner_radius,
                physical_pixels(ctx, metrics.border_width),
                palette.surface,
                palette.border,
            );
            for (index, option) in self.options.iter().enumerate() {
                let row = self.option_rect(ctx.bounds(), index);
                let selected = self.selected == Some(index);
                let hovered = self.hovered_option == Some(index);
                let text_style = self.theme.body_text_style();
                let text_measurement = paint_text_measurement(ctx, option, &text_style);
                if hovered || selected {
                    ctx.fill(
                        rounded_rect_path(row.inflate(-4.0, -4.0), metrics.corner_radius - 2.0),
                        if hovered {
                            palette.surface_hover
                        } else {
                            palette.surface_pressed
                        },
                    );
                }
                ctx.draw_text(
                    vertically_centered_text_rect(
                        ctx,
                        horizontal_text_inset_rect(row, metrics.text_input_padding),
                        Some(text_measurement),
                        text_style.line_height,
                    ),
                    option.clone(),
                    text_style,
                );
            }
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.expanded {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.expanded.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ComboBox, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.current_label()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered_header || self.hovered_option.is_some();
        node.state.expanded = Some(self.expanded);
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.expanded {
            self.expanded = false;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub type Divider = Separator;
pub type SpinBox = NumberInput;
pub type MultilineTextInput = TextArea;
pub type ComboBox = Select;

pub struct TextInput {
    theme: Box<DefaultTheme>,
    name: String,
    value: String,
    placeholder: String,
    composition: String,
    text_style: Option<TextStyle>,
    padding: Option<Insets>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    hovered: bool,
    focused: bool,
    focus_animation: AnimatedScalar,
    caret_blink: Blink,
    caret_timer: Option<TimerToken>,
    caret_visible: bool,
    visible_measurement: Option<TextMeasurement>,
    input_measurement: Option<TextMeasurement>,
    on_change: Option<Box<dyn FnMut(String)>>,
}

impl TextInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            value: String::new(),
            placeholder: String::new(),
            composition: String::new(),
            text_style: None,
            padding: None,
            min_width: None,
            min_height: None,
            hovered: false,
            focused: false,
            focus_animation: AnimatedScalar::new(0.0),
            caret_blink: Blink::new(CARET_BLINK_PERIOD_SECONDS),
            caret_timer: None,
            caret_visible: true,
            visible_measurement: None,
            input_measurement: None,
            on_change: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.composition.clear();
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    fn input_text(&self) -> String {
        let mut text = self.value.clone();
        text.push_str(&self.composition);
        text
    }

    fn visible_text(&self) -> String {
        let input = self.input_text();
        if input.is_empty() {
            self.placeholder.clone()
        } else {
            input
        }
    }

    fn commit_text_change(&mut self) {
        if let Some(on_change) = &mut self.on_change {
            on_change(self.value.clone());
        }
    }

    fn insert_text(&mut self, text: &str, ctx: &mut EventCtx) {
        if text.is_empty() {
            return;
        }

        self.value.push_str(text);
        self.composition.clear();
        self.commit_text_change();
        if self.focused {
            self.reset_caret_blink(ctx);
        }
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
        ctx.set_handled();
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
        self.caret_visible = self.focused;
        self.arm_caret_blink(ctx);
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn resolved_text_style(&self) -> TextStyle {
        self.text_style
            .clone()
            .unwrap_or_else(|| self.theme.body_text_style())
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.theme.metrics.text_input_padding)
    }

    fn resolved_min_size(&self) -> Size {
        Size::new(
            self.min_width
                .unwrap_or(self.theme.metrics.text_input_min_width),
            self.min_height.unwrap_or(self.theme.metrics.min_height),
        )
    }
}

impl Widget for TextInput {
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
                self.hovered = true;
                if self.focused {
                    self.reset_caret_blink(ctx);
                }
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                self.composition.clear();
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionUpdate { text }) if ctx.is_focused() => {
                self.composition = text.clone();
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionCommit { text }) if ctx.is_focused() => {
                self.insert_text(text, ctx);
            }
            Event::Ime(ImeEvent::CompositionEnd) if ctx.is_focused() => {
                if !self.composition.is_empty() {
                    self.composition.clear();
                    self.reset_caret_blink(ctx);
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Backspace" =>
            {
                if !self.composition.is_empty() {
                    self.composition.clear();
                } else if self.value.pop().is_some() {
                    self.commit_text_change();
                }
                self.reset_caret_blink(ctx);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && self.composition.is_empty() => {
                if let Some(text) = keyboard_text(key) {
                    self.insert_text(text, ctx);
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
                let previous_focus = self.focus_animation.value;
                let animating = self.focus_animation.advance(*time);
                let focus_changed = (self.focus_animation.value - previous_focus).abs() > 1e-4;
                if animating {
                    ctx.request_animation_frame();
                }
                if focus_changed {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let min_size = self.resolved_min_size();
        let visible_text = self.visible_text();
        let input_text = self.input_text();
        let visible_measurement = measure_text(ctx, &visible_text, &text_style);
        let input_measurement = if input_text.is_empty() {
            TextMeasurement {
                width: 0.0,
                height: visible_measurement.height,
                bounds: Rect::new(0.0, 0.0, 0.0, visible_measurement.height),
                ascent: visible_measurement.ascent,
                descent: visible_measurement.descent,
                cap_height: visible_measurement.cap_height,
            }
        } else {
            measure_text(ctx, &input_text, &text_style)
        };

        self.visible_measurement = Some(visible_measurement);
        self.input_measurement = Some(input_measurement);

        let width = (visible_measurement.width + padding.left + padding.right).max(min_size.width);
        let height =
            (visible_measurement.height.max(text_style.line_height) + padding.top + padding.bottom)
                .max(min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let text_style = self.resolved_text_style();
        let padding = self.resolved_padding();
        let focus_progress = self.focus_animation.value;
        let background = mix_color(
            if self.hovered {
                palette.surface_hover
            } else {
                palette.surface
            },
            palette.surface_focus,
            focus_progress,
        );
        let border = mix_color(
            if self.hovered {
                palette.border_hover
            } else {
                palette.border
            },
            palette.border_focus,
            focus_progress,
        );
        let content_rect = inset_rect(ctx.bounds(), padding);
        let display_text = self.visible_text();
        let placeholder = self.input_text().is_empty();

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
        ctx.draw_text(
            content_rect,
            display_text,
            if placeholder {
                self.theme.placeholder_text_style()
            } else {
                text_style.clone()
            },
        );

        if self.focused {
            let caret_width = physical_pixels(ctx, metrics.caret_width);
            let caret_x = content_rect.x()
                + self
                    .input_measurement
                    .map(|measurement| measurement.width)
                    .unwrap_or(0.0);
            let caret_rect = Rect::new(
                caret_x.min((content_rect.max_x() - caret_width).max(content_rect.x())),
                content_rect.y(),
                caret_width,
                content_rect.height().max(text_style.line_height),
            );
            ctx.set_ime_composition_rect(caret_rect);
            if self.caret_visible {
                ctx.fill(
                    rounded_rect_path(caret_rect, caret_width * 0.5),
                    palette.accent_text,
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.input_text()));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.focused = focused;
        if !focused && !self.composition.is_empty() {
            self.composition.clear();
            ctx.request_measure();
        }
        if focused {
            self.reset_caret_blink(ctx);
        } else {
            if let Some(token) = self.caret_timer.take() {
                ctx.cancel_timer(token);
            }
            self.caret_visible = false;
        }
        set_animation_target(
            &mut self.focus_animation,
            focused as u8 as f32,
            FOCUS_ANIMATION_SECONDS,
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
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

fn paint_text_measurement(ctx: &PaintCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
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

    event
        .text
        .as_deref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
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
        bounds.y() + padding.top,
        (bounds.width() - (x - bounds.x()) - padding.right).max(0.0),
        (bounds.height() - padding.top - padding.bottom).max(0.0),
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

fn number_input_stepper_rect(bounds: Rect, metrics: ControlMetrics) -> Rect {
    Rect::new(
        bounds.max_x() - metrics.number_input_stepper_width,
        bounds.y(),
        metrics.number_input_stepper_width,
        bounds.height(),
    )
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

fn measure_text_width_estimate(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.62
}

fn draw_icon_glyph(ctx: &mut PaintCtx, glyph: IconGlyph, bounds: Rect, color: Color) {
    let stroke = StrokeStyle::new(physical_pixels(ctx, 1.8).max(1.0));
    let inset = bounds.inflate(
        -((bounds.width() * 0.2) + (stroke.width * 0.5)),
        -((bounds.height() * 0.2) + (stroke.width * 0.5)),
    );

    match glyph {
        IconGlyph::Add => {
            ctx.stroke(
                line_path(
                    Point::new(rect_center(inset).x, inset.y()),
                    Point::new(rect_center(inset).x, inset.max_y()),
                ),
                color,
                stroke.clone(),
            );
            ctx.stroke(
                line_path(
                    Point::new(inset.x(), rect_center(inset).y),
                    Point::new(inset.max_x(), rect_center(inset).y),
                ),
                color,
                stroke,
            );
        }
        IconGlyph::Remove => {
            ctx.stroke(
                line_path(
                    Point::new(inset.x(), rect_center(inset).y),
                    Point::new(inset.max_x(), rect_center(inset).y),
                ),
                color,
                stroke,
            );
        }
        IconGlyph::Check => {
            ctx.stroke(checkmark_path(inset), color, stroke);
        }
        IconGlyph::ChevronDown => {
            ctx.stroke(chevron_path(inset, Axis::Vertical, 1.0), color, stroke);
        }
        IconGlyph::ChevronUp => {
            ctx.stroke(chevron_path(inset, Axis::Vertical, -1.0), color, stroke);
        }
        IconGlyph::ChevronLeft => {
            ctx.stroke(chevron_path(inset, Axis::Horizontal, -1.0), color, stroke);
        }
        IconGlyph::ChevronRight => {
            ctx.stroke(chevron_path(inset, Axis::Horizontal, 1.0), color, stroke);
        }
        IconGlyph::Close => {
            ctx.stroke(
                line_path(
                    Point::new(inset.x(), inset.y()),
                    Point::new(inset.max_x(), inset.max_y()),
                ),
                color,
                stroke.clone(),
            );
            ctx.stroke(
                line_path(
                    Point::new(inset.max_x(), inset.y()),
                    Point::new(inset.x(), inset.max_y()),
                ),
                color,
                stroke,
            );
        }
        IconGlyph::MoreHorizontal => {
            for offset in [0.2_f32, 0.5, 0.8] {
                ctx.fill(
                    Path::circle(
                        Point::new(inset.x() + (inset.width() * offset), rect_center(inset).y),
                        inset.height() * 0.1,
                    ),
                    color,
                );
            }
        }
        IconGlyph::MoreVertical => {
            for offset in [0.2_f32, 0.5, 0.8] {
                ctx.fill(
                    Path::circle(
                        Point::new(rect_center(inset).x, inset.y() + (inset.height() * offset)),
                        inset.width() * 0.1,
                    ),
                    color,
                );
            }
        }
        IconGlyph::Search => {
            let lens = Rect::new(
                inset.x(),
                inset.y(),
                inset.width() * 0.62,
                inset.height() * 0.62,
            );
            ctx.stroke(
                Path::circle(rect_center(lens), lens.width() * 0.4),
                color,
                stroke.clone(),
            );
            ctx.stroke(
                line_path(
                    Point::new(
                        lens.max_x() - (lens.width() * 0.05),
                        lens.max_y() - (lens.height() * 0.05),
                    ),
                    Point::new(inset.max_x(), inset.max_y()),
                ),
                color,
                stroke,
            );
        }
    }
}

fn chevron_path(bounds: Rect, axis: Axis, direction: f32) -> Path {
    let mut builder = PathBuilder::new();
    match (axis, direction.is_sign_positive()) {
        (Axis::Vertical, true) => {
            builder
                .move_to(Point::new(bounds.x(), bounds.y() + (bounds.height() * 0.3)))
                .line_to(Point::new(
                    rect_center(bounds).x,
                    bounds.max_y() - (bounds.height() * 0.3),
                ))
                .line_to(Point::new(
                    bounds.max_x(),
                    bounds.y() + (bounds.height() * 0.3),
                ));
        }
        (Axis::Vertical, false) => {
            builder
                .move_to(Point::new(
                    bounds.x(),
                    bounds.max_y() - (bounds.height() * 0.3),
                ))
                .line_to(Point::new(
                    rect_center(bounds).x,
                    bounds.y() + (bounds.height() * 0.3),
                ))
                .line_to(Point::new(
                    bounds.max_x(),
                    bounds.max_y() - (bounds.height() * 0.3),
                ));
        }
        (Axis::Horizontal, true) => {
            builder
                .move_to(Point::new(bounds.x() + (bounds.width() * 0.3), bounds.y()))
                .line_to(Point::new(
                    bounds.max_x() - (bounds.width() * 0.3),
                    rect_center(bounds).y,
                ))
                .line_to(Point::new(
                    bounds.x() + (bounds.width() * 0.3),
                    bounds.max_y(),
                ));
        }
        (Axis::Horizontal, false) => {
            builder
                .move_to(Point::new(
                    bounds.max_x() - (bounds.width() * 0.3),
                    bounds.y(),
                ))
                .line_to(Point::new(
                    bounds.x() + (bounds.width() * 0.3),
                    rect_center(bounds).y,
                ))
                .line_to(Point::new(
                    bounds.max_x() - (bounds.width() * 0.3),
                    bounds.max_y(),
                ));
        }
    }
    builder.build()
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

    draw_control_shape(
        ctx,
        bounds,
        radius,
        physical_pixels(ctx, metrics.border_width),
        background,
        border,
    );
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

fn indicator_rect(bounds: Rect, padding: Insets, indicator_size: f32) -> Rect {
    let x = bounds.x() + padding.left;
    let y = bounds.y() + ((bounds.height() - indicator_size) * 0.5);
    Rect::new(x, y, indicator_size, indicator_size)
}

fn checkbox_label_rect(bounds: Rect, padding: Insets, indicator_size: f32, gap: f32) -> Rect {
    let x = bounds.x() + padding.left + indicator_size + gap;
    let width = (bounds.width() - padding.left - padding.right - indicator_size - gap).max(0.0);
    Rect::new(x, bounds.y(), width, bounds.height())
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    ctx.dpi().physical_pixels_to_logical(value)
}

fn centered_text_rect(
    ctx: &PaintCtx,
    bounds: Rect,
    padding: Insets,
    measurement: Option<TextMeasurement>,
    line_height: f32,
) -> Rect {
    let rect = Rect::new(
        bounds.x() + padding.left,
        bounds.y(),
        (bounds.width() - padding.left - padding.right).max(0.0),
        bounds.height(),
    );
    let Some(measurement) = measurement else {
        return rect;
    };

    let width = measurement.width.min(rect.width());
    let height = line_height.max(measurement.height).min(rect.height());

    Rect::new(
        rect.x() + ((rect.width() - width) * 0.5),
        vertically_centered_text_rect_y(ctx, rect, measurement, height),
        width,
        height,
    )
}

fn vertically_centered_text_rect(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: Option<TextMeasurement>,
    line_height: f32,
) -> Rect {
    let Some(measurement) = measurement else {
        return rect;
    };

    let height = line_height.max(measurement.height).min(rect.height());

    Rect::new(
        rect.x(),
        vertically_centered_text_rect_y(ctx, rect, measurement, height),
        rect.width(),
        height,
    )
}

fn vertically_centered_text_rect_y(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: TextMeasurement,
    height: f32,
) -> f32 {
    let optical_centering = window_render_options(ctx.window_id())
        .map(|options| options.optical_vertical_text_alignment_enabled)
        .unwrap_or(true);
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
    let visual_center = (top + bottom) * 0.5;
    let baseline = rect.y() + (rect.height() * 0.5) - visual_center;
    let leading_above = ((height - (measurement.ascent + measurement.descent)).max(0.0)) * 0.5;
    baseline - measurement.ascent - leading_above
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Button, CARET_BLINK_PERIOD_SECONDS, Checkbox, DefaultTheme, FOCUS_ANIMATION_SECONDS,
        HOVER_ANIMATION_SECONDS, Label, NumberInput, PRESS_ANIMATION_SECONDS, RadioButton,
        RadioGroup, Select, Slider, Switch, TOGGLE_ANIMATION_SECONDS, TextArea, TextInput,
    };
    use crate::containers::SizedBox;
    use crate::{HdrThemeMode, SemanticColorToken, WidgetLuminanceRole, resolve_luminance_role};
    use sui_core::{
        Color, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
        PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect, Result, SemanticsRole,
        SemanticsValue, Size, Vector, WidgetId, WindowEvent,
    };
    use sui_render_wgpu::{RgbaImage, WgpuRenderer};
    use sui_runtime::{
        Application, RenderOutput, Runtime, Widget, WindowBuilder, WindowRenderOptions,
        clear_window_render_options, set_window_render_options,
    };
    use sui_scene::{Brush, LayerCompositionMode, SceneCommand, SceneLayerDescriptor};
    use sui_text::{FontRegistry, TextSystem};

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

    fn render_rgba<W>(root: W, feathering_enabled: bool) -> (RenderOutput, RgbaImage)
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        let output = runtime.render(window_id).unwrap();
        let mut renderer = WgpuRenderer::default().with_feathering_enabled(feathering_enabled);
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
                    .map(|layout| sui_text::TextRun {
                        rect: Rect::new(
                            text.origin.x,
                            text.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ),
                        text: layout.text().to_string(),
                        style: layout.style().clone(),
                    }),
                _ => None,
            })
            .expect("text draw command present")
    }

    fn first_shaped_text<'a>(output: &'a RenderOutput) -> &'a sui_text::ShapedText {
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

    fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
                SceneCommand::DrawShapedText(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .map(|layout| sui_text::TextRun {
                        rect: Rect::new(
                            run.origin.x,
                            run.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ),
                        text: layout.text().to_string(),
                        style: layout.style().clone(),
                    }),
                _ => None,
            })
            .expect("text draw command present")
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
    fn label_measures_wrapped_height_when_width_is_constrained() {
        let output = render(SizedBox::new().width(96.0).with_child(Label::new(
            "This label should wrap onto multiple lines when its layout width is constrained.",
        )));

        assert!(output.frame.viewport.height > DefaultTheme::default().typography.body_line_height);
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
    fn button_hover_animation_advances_over_multiple_frames() -> Result<()> {
        let theme = DefaultTheme::default();
        let (mut runtime, window_id) = build_runtime(Button::new("Hover"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(HOVER_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let mid_background = solid_fill_colors(&mid)[0];
        assert_ne!(mid_background, theme.palette.accent);
        assert_ne!(mid_background, theme.palette.accent_hover);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(HOVER_ANIMATION_SECONDS);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let end_background = solid_fill_colors(&end)[0];
        assert_eq!(end_background, theme.palette.accent_hover);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn switch_thumb_animation_tracks_progress_and_completion() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(Switch::new("Wifi"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(12.0, 12.0), false),
        )?;

        runtime.tick(TOGGLE_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(TOGGLE_ANIMATION_SECONDS);
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
    fn slider_thumb_hover_animation_requests_followup_frames_until_complete() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(Slider::new("Gain"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(32.0, 16.0), false),
        )?;

        runtime.tick(HOVER_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(HOVER_ANIMATION_SECONDS);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
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

        runtime.tick(PRESS_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let mid_background = solid_fill_colors(&mid)[0];
        assert_ne!(mid_background, theme.palette.surface_pressed);
        assert_ne!(mid_background, theme.palette.surface_hover);
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(HOVER_ANIMATION_SECONDS);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let end = runtime.render(window_id)?;
        let end_fills = solid_fill_colors(&end);
        assert_ne!(end_fills, solid_fill_colors(&mid));
        assert!(!end_fills.contains(&theme.palette.surface_pressed));
        assert_eq!(runtime.next_wakeup_time(window_id)?, None);
        Ok(())
    }

    #[test]
    fn checkbox_check_indicator_animation_progresses_deterministically() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(Checkbox::new("Subscribe"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 10.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 10.0), false),
        )?;

        runtime.tick(TOGGLE_ANIMATION_SECONDS * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid = runtime.render(window_id)?;
        let fills = solid_fill_colors(&mid);
        assert!(!fills.is_empty());
        assert!(runtime.next_wakeup_time(window_id)?.is_some());

        runtime.tick(TOGGLE_ANIMATION_SECONDS);
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
        let caret_color = theme.palette.accent_text;
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
    fn text_input_focus_animation_settles_into_blink_timer_without_frame_spin() -> Result<()> {
        let (mut runtime, window_id) =
            build_runtime(TextInput::new("Name").placeholder("Type a name"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
        )?;
        let _ = runtime.render(window_id)?;

        let settled_at = FOCUS_ANIMATION_SECONDS + 0.01;
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
        let caret_color = theme.palette.accent_text;
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
        let caret_color = theme.palette.accent_text;
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

        runtime.tick(FOCUS_ANIMATION_SECONDS * 0.5);
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

        let visuals = Button::new("Go").theme(theme).resolved_visuals(true);
        let fills = solid_fill_colors(&render(Button::new("Go").theme(theme)));

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

        let visuals = Button::new("Go").theme(theme).resolved_visuals(true);
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

        let visuals = Button::new("Go").theme(theme).resolved_visuals(false);

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
        assert_eq!(constrained_visuals.track_color, constrained_track[1]);
        assert_eq!(full_visuals.track_color, full_track[1]);
        assert!((constrained_track[1].red - constrained_peak).abs() < f32::EPSILON);
        assert!((full_track[1].red - full_peak).abs() < f32::EPSILON);
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
        let row_center = DefaultTheme::default().metrics.min_height * 0.5;

        assert!((actual_visual_center - row_center).abs() < 0.75);
    }

    #[test]
    fn controls_default_to_touch_safe_heights() {
        let theme = DefaultTheme::default();
        assert_eq!(
            render(Button::new("Go")).frame.viewport.height >= theme.metrics.min_height,
            true
        );
        assert_eq!(
            render(Checkbox::new("Subscribe")).frame.viewport.height >= theme.metrics.min_height,
            true
        );
        assert_eq!(
            render(TextInput::new("Name")).frame.viewport.height >= theme.metrics.min_height,
            true
        );
    }

    #[test]
    fn button_theme_is_public_and_changes_metrics_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.metrics.button_min_width = 156.0;
        theme.metrics.min_height = 52.0;
        theme.typography.body_font_size = 16.0;
        theme.typography.body_line_height = 24.0;
        theme.palette.accent_text = Color::rgba(0.10, 0.12, 0.15, 1.0);

        let output = render(Button::new("Theme").theme(theme));

        assert_eq!(output.frame.viewport, Size::new(156.0, 52.0));
        let label = first_text_run(&output);
        assert_eq!(label.style.font_size, 16.0);
        assert_eq!(label.style.line_height, 24.0);
        assert_eq!(label.style.color, theme.palette.accent_text);
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
        assert_eq!(input.value, Some(SemanticsValue::Number(6.0)));
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
        assert_eq!(input.value, Some(SemanticsValue::Number(2.0)));
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
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("number input text should shape");
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
    fn expanded_select_uses_direct_overlay_layer_metadata() -> Result<()> {
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
            layer_descriptor_for(&output, select.id).expect("select layer descriptor present");

        assert_eq!(select.state.expanded, Some(true));
        assert_eq!(descriptor.composition_mode, LayerCompositionMode::Overlay);
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
            layer_descriptor_for(&after, select.id).expect("select layer descriptor present");

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
        let option_point = Point::new(
            select.bounds.x() + 20.0,
            select.bounds.max_y() + 6.0 + (select.bounds.height() * 1.5),
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
        let row_center = select.bounds.max_y() + 6.0 + (select.bounds.height() * 0.5);

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
        let select = expanded
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ComboBox)
            .expect("select semantics present after expand");
        let descriptor =
            layer_descriptor_for(&expanded, select.id).expect("select layer descriptor present");

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
