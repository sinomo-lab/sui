use sui_core::{
    Color, ColorSpace, Event, ImageHandle, KeyState, Path, PathBuilder, Point, PointerButton,
    PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    WakeEvent, WidgetId,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::{ImageSource, StrokeStyle, WidgetShader};
use sui_text::{FontFeature, TextStyle};

use crate::{
    ControlMetrics, DefaultTheme, MotionScalar, SemanticTone, ThemeDensity, ThemeTextToken,
    text_align::paint_aligned_text,
};

const SIGNAL_METER_PATTERN: [f32; 12] = [
    0.32, 0.58, 0.86, 0.44, 0.72, 0.96, 0.52, 0.78, 0.38, 0.64, 0.90, 0.48,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    Fill,
    Contain,
    Cover,
    None,
}

pub struct Image {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    image: ImageHandle,
    label: Option<String>,
    fit: ImageFit,
    width: Option<f32>,
    height: Option<f32>,
    aspect_ratio: Option<f32>,
    source_rect: Option<Rect>,
    tint: Option<Color>,
    background: Option<Color>,
    background_reader: Option<Box<dyn Fn() -> Color>>,
    show_border: bool,
    corner_radius: Option<f32>,
    resolved_source_size: Size,
}

impl Image {
    pub fn new(image: ImageHandle) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            image,
            label: None,
            fit: ImageFit::Contain,
            width: None,
            height: None,
            aspect_ratio: None,
            source_rect: None,
            tint: None,
            background: None,
            background_reader: None,
            show_border: true,
            corner_radius: None,
            resolved_source_size: Size::new(96.0, 96.0),
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

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.width = Some(size.width.max(0.0));
        self.height = Some(size.height.max(0.0));
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = Some(aspect_ratio.max(0.01));
        self
    }

    pub fn source_rect(mut self, source_rect: Rect) -> Self {
        self.source_rect = Some(source_rect);
        self
    }

    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self.background_reader = None;
        self
    }

    pub fn background_when<F>(mut self, background: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.background_reader = Some(Box::new(background));
        self
    }

    pub fn without_border(mut self) -> Self {
        self.show_border = false;
        self
    }

    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = Some(corner_radius.max(0.0));
        self
    }

    fn effective_source_size(&self, ctx: &MeasureCtx) -> Size {
        self.source_rect
            .map(|rect| rect.size)
            .or_else(|| ctx.layout().image_size(self.image))
            .unwrap_or(Size::new(96.0, 96.0))
    }

    fn resolved_size(&self) -> Size {
        let aspect_ratio = self.aspect_ratio.unwrap_or_else(|| {
            (self.resolved_source_size.width / self.resolved_source_size.height.max(1.0)).max(0.01)
        });
        match (self.width, self.height) {
            (Some(width), Some(height)) => Size::new(width, height),
            (Some(width), None) => Size::new(width, width / aspect_ratio),
            (None, Some(height)) => Size::new(height * aspect_ratio, height),
            (None, None) => self.resolved_source_size,
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_background(&self) -> Option<Color> {
        self.background_reader
            .as_ref()
            .map(|background| background())
            .or(self.background)
    }

    fn resolved_corner_radius(&self, theme: &DefaultTheme) -> f32 {
        self.corner_radius
            .unwrap_or(theme.metrics.image_corner_radius)
    }
}

impl Widget for Image {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.resolved_source_size = self.effective_source_size(ctx);
        constraints.clamp(self.resolved_size())
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = self.resolved_theme();
        let corner_radius = self.resolved_corner_radius(&theme);
        let image_rect = fit_rect(bounds, self.resolved_source_size, self.fit);
        if let Some(background) = self.resolved_background() {
            ctx.fill(rounded_rect_path(bounds, corner_radius), background);
        }

        ctx.push_clip_rect(bounds);
        let mut source = ImageSource::new(self.image);
        if let Some(source_rect) = self.source_rect {
            source = source.with_source_rect(source_rect);
        }
        if let Some(tint) = self.tint {
            source = source.with_tint(tint);
        }
        ctx.draw_image_source(image_rect, source);
        ctx.pop_clip();

        if self.show_border {
            ctx.stroke(
                rounded_rect_path(bounds, corner_radius),
                theme.palette.border,
                StrokeStyle::new(theme.metrics.border_width.max(1.0)),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Image, ctx.bounds());
        node.name = self.label.clone();
        ctx.push(node);
    }
}

pub struct SignalMeter {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    description: Option<String>,
    active: bool,
    active_reader: Option<Box<dyn Fn() -> bool>>,
    tone: SemanticTone,
    tone_reader: Option<Box<dyn Fn() -> SemanticTone>>,
    bars: usize,
    size: Option<Size>,
    gap: Option<f32>,
}

impl SignalMeter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            description: None,
            active: false,
            active_reader: None,
            tone: SemanticTone::Accent,
            tone_reader: None,
            bars: 12,
            size: None,
            gap: None,
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

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self.active_reader = None;
        self
    }

    pub fn active_when<F>(mut self, active: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.active_reader = Some(Box::new(active));
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self.tone_reader = None;
        self
    }

    pub fn tone_when<F>(mut self, tone: F) -> Self
    where
        F: Fn() -> SemanticTone + 'static,
    {
        self.tone_reader = Some(Box::new(tone));
        self
    }

    pub fn bars(mut self, bars: usize) -> Self {
        self.bars = bars.clamp(3, 64);
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(Size::new(size.width.max(0.0), size.height.max(0.0)));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn is_active(&self) -> bool {
        self.active_reader
            .as_ref()
            .map(|active| active())
            .unwrap_or(self.active)
    }

    fn resolved_tone(&self) -> SemanticTone {
        self.tone_reader
            .as_ref()
            .map(|tone| tone())
            .unwrap_or(self.tone)
    }

    fn bar_count(&self) -> usize {
        self.bars.clamp(3, 64)
    }

    fn resolved_size(&self) -> Size {
        self.size.unwrap_or(Size::new(76.0, 16.0))
    }
}

impl Widget for SignalMeter {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.resolved_size())
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let active = self.is_active();
        let bars = self.bar_count();
        let gap = self.gap.unwrap_or(3.0).min(bounds.width() / bars as f32);
        let total_gap = gap * bars.saturating_sub(1) as f32;
        let bar_w = ((bounds.width() - total_gap) / bars as f32).max(1.0);
        let tone = theme.semantic_tone_color(self.resolved_tone());
        let fill = if active {
            tone
        } else {
            theme.palette.text_muted.with_alpha(0.48)
        };
        let min_h = (bounds.height() * if active { 0.20 } else { 0.12 }).max(1.0);
        let max_h = bounds.height().max(1.0);
        for index in 0..bars {
            let pattern = SIGNAL_METER_PATTERN[index % SIGNAL_METER_PATTERN.len()];
            let level = if active {
                pattern
            } else {
                (pattern * 0.26).max(0.12)
            };
            let bar_h = (min_h + (max_h - min_h) * level).min(max_h);
            let x = bounds.x() + index as f32 * (bar_w + gap);
            let y = bounds.max_y() - bar_h;
            let rect = Rect::new(x, y, bar_w, bar_h);
            ctx.fill(rounded_rect_path(rect, (bar_w * 0.5).min(3.0)), fill);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.description = self.description.clone();
        node.value = Some(SemanticsValue::Text(if self.is_active() {
            "active".to_string()
        } else {
            "idle".to_string()
        }));
        ctx.push(node);
    }
}

pub struct ColorSwatch {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    color: Color,
    color_reader: Option<Box<dyn Fn() -> Color>>,
    size: Option<Size>,
    hovered: bool,
    pressed: bool,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    read_only: bool,
    on_press: Option<Box<dyn FnMut(Color)>>,
}

impl ColorSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            color,
            color_reader: None,
            size: None,
            hovered: false,
            pressed: false,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            read_only: false,
            on_press: None,
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

    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(Size::new(size.width.max(24.0), size.height.max(24.0)));
        self
    }

    pub fn color_when<F>(mut self, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.color_reader = Some(Box::new(color));
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(Color) + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    fn current_color(&self) -> Color {
        self.color_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.color)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn activate(&mut self) {
        let color = self.current_color();
        if let Some(on_press) = &mut self.on_press {
            on_press(color);
        }
    }

    fn resolved_size(&self, theme: &DefaultTheme) -> Size {
        self.size.unwrap_or(Size::new(
            theme.metrics.color_swatch_width,
            theme.metrics.color_swatch_height,
        ))
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

    fn set_pressed(&mut self, pressed: bool, ctx: &mut EventCtx) {
        if self.pressed != pressed {
            let theme = self.resolved_theme();
            self.pressed = pressed;
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

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.press_animation.advance(time)
            | self.focus_animation.advance(time)
    }
}

impl Widget for ColorSwatch {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.read_only {
            if matches!(
                event,
                Event::Pointer(pointer)
                    if matches!(
                        pointer.kind,
                        PointerEventKind::Move
                            | PointerEventKind::Leave
                            | PointerEventKind::Cancel
                    )
            ) {
                ctx.request_paint();
                ctx.request_semantics();
            }
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.set_pressed(true, ctx);
                self.set_hovered(true, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = ctx.bounds().contains(pointer.position);
                if self.pressed && hovered {
                    self.activate();
                }
                self.set_pressed(false, ctx);
                self.set_hovered(hovered, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.set_pressed(false, ctx);
                    self.set_hovered(false, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                }
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate();
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

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        constraints.clamp(self.resolved_size(&theme))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let palette = theme.palette;
        let pressed_offset = self.press_animation.value * theme.interaction.pressed_offset;
        let body = Rect::new(
            ctx.bounds().x(),
            ctx.bounds().y() + pressed_offset,
            ctx.bounds().width(),
            (ctx.bounds().height() - pressed_offset).max(0.0),
        );
        let outer_radius = metrics.corner_radius.min(body.height() * 0.5);
        let inner_inset = metrics.color_swatch_inner_inset + pressed_offset * 0.5;
        let inner_radius = (outer_radius - inner_inset).max(0.0);
        let color = self.current_color();

        if self.focus_animation.value > 0.0 {
            let focus_outset = metrics.focus_ring_outset;
            ctx.stroke(
                rounded_rect_path(
                    ctx.bounds().inflate(focus_outset, focus_outset),
                    outer_radius + focus_outset,
                ),
                palette.focus_ring.with_alpha(self.focus_animation.value),
                StrokeStyle::new(metrics.focus_ring_width.max(1.0)),
            );
        }

        if self.press_animation.value > 0.0 {
            ctx.fill(
                rounded_rect_path(ctx.bounds(), outer_radius),
                mix_color(
                    palette.control_hover,
                    palette.control_active,
                    theme.interaction.pressed_blend * self.press_animation.value,
                ),
            );
        } else if self.hover_animation.value > 0.0 {
            ctx.fill(
                rounded_rect_path(ctx.bounds(), outer_radius),
                palette
                    .control_hover
                    .with_alpha(self.hover_animation.value * palette.control_hover.alpha),
            );
        }

        draw_checkerboard(ctx, body, metrics.color_swatch_checker_size, &theme);
        ctx.fill(
            rounded_rect_path(inset_rect(body, Insets::all(inner_inset)), inner_radius),
            color,
        );
        ctx.stroke(
            rounded_rect_path(body, outer_radius),
            if ctx.is_focused() {
                palette.border_focus
            } else if self.hovered || self.hover_animation.value > 0.0 {
                palette.border_hover
            } else if self.read_only {
                palette
                    .border
                    .with_alpha(theme.interaction.disabled_opacity)
            } else {
                palette.border
            },
            StrokeStyle::new(metrics.border_width.max(1.0)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorSwatch, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(format_color(self.current_color())));
        if !self.read_only {
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        }
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        !self.read_only
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorPaletteSwatch {
    name: String,
    color: Color,
}

impl ColorPaletteSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn color(&self) -> Color {
        self.color
    }
}

pub struct ColorPalette {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    swatches: Vec<ColorPaletteSwatch>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    hover_visual: Option<usize>,
    pressed: Option<usize>,
    press_visual: Option<usize>,
    hover_animation: AnimatedScalar,
    press_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    columns: usize,
    swatch_size: Option<f32>,
    gap: Option<f32>,
    on_change: Option<Box<dyn FnMut(usize, String, Color)>>,
    on_change_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, String, Color)>>,
}

impl ColorPalette {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            swatches: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            columns: 8,
            swatch_size: None,
            gap: None,
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

    pub fn swatch(mut self, swatch: ColorPaletteSwatch) -> Self {
        self.swatches.push(swatch);
        self
    }

    pub fn swatches<I>(mut self, swatches: I) -> Self
    where
        I: IntoIterator<Item = ColorPaletteSwatch>,
    {
        self.swatches.extend(swatches);
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    pub fn swatch_size(mut self, size: f32) -> Self {
        self.swatch_size = Some(size.max(18.0));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String, Color) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, String, Color) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.swatches.len())
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn grid_columns(&self) -> usize {
        self.columns.max(1).min(self.swatches.len().max(1))
    }

    fn grid_rows(&self) -> usize {
        if self.swatches.is_empty() {
            1
        } else {
            self.swatches.len().div_ceil(self.grid_columns())
        }
    }

    fn resolved_swatch_size(&self, theme: &DefaultTheme) -> f32 {
        self.swatch_size
            .unwrap_or(theme.metrics.color_palette_swatch_size)
    }

    fn resolved_gap(&self, theme: &DefaultTheme) -> f32 {
        self.gap.unwrap_or(theme.metrics.color_palette_gap)
    }

    fn swatch_rect(&self, bounds: Rect, index: usize, theme: &DefaultTheme) -> Option<Rect> {
        if index >= self.swatches.len() {
            return None;
        }

        let swatch_size = self.resolved_swatch_size(theme);
        let gap = self.resolved_gap(theme);
        let columns = self.grid_columns();
        let column = index % columns;
        let row = index / columns;
        let x = bounds.x() + column as f32 * (swatch_size + gap);
        let y = bounds.y() + row as f32 * (swatch_size + gap);
        let available_width = (bounds.max_x() - x).max(0.0);
        let available_height = (bounds.max_y() - y).max(0.0);
        let rect = Rect::new(
            x,
            y,
            swatch_size.min(available_width),
            swatch_size.min(available_height),
        );
        (!rect.is_empty()).then_some(rect)
    }

    fn swatch_at(&self, bounds: Rect, position: Point, theme: &DefaultTheme) -> Option<usize> {
        self.swatches.iter().enumerate().find_map(|(index, _)| {
            self.swatch_rect(bounds, index, theme)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn activate(&mut self, ctx: &mut EventCtx, index: usize) {
        if self.swatches.is_empty() {
            return;
        }

        let index = index.min(self.swatches.len() - 1);
        self.selected = Some(index);
        let swatch = &self.swatches[index];
        let name = swatch.name.clone();
        let color = swatch.color;
        if let Some(on_change) = &mut self.on_change {
            on_change(index, name.clone(), color);
        }
        if let Some(on_change_with_ctx) = &mut self.on_change_with_ctx {
            on_change_with_ctx(ctx, index, name, color);
        }
    }

    fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.swatches.is_empty() {
            return;
        }

        let current = self.current_selected().unwrap_or(0) as isize;
        let last = self.swatches.len() as isize - 1;
        let next = (current + delta).clamp(0, last) as usize;
        self.set_hovered(Some(next), ctx);
        self.activate(ctx, next);
    }

    fn selected_value(&self) -> Option<String> {
        self.current_selected()
            .and_then(|index| self.swatches.get(index))
            .map(|swatch| format!("{} {}", swatch.name, format_color(swatch.color)))
    }

    fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }
        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }
        hover_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for ColorPalette {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let theme = self.resolved_theme();
                self.set_hovered(self.swatch_at(ctx.bounds(), pointer.position, &theme), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                let hovered = self.swatch_at(ctx.bounds(), pointer.position, &theme);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                if self.hovered.is_some() {
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = self.resolved_theme();
                let hovered = self.swatch_at(ctx.bounds(), pointer.position, &theme);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(ctx, index);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let columns = self.grid_columns() as isize;
                match key.key.as_str() {
                    "ArrowLeft" => self.move_selection(-1, ctx),
                    "ArrowRight" => self.move_selection(1, ctx),
                    "ArrowUp" => self.move_selection(-columns, ctx),
                    "ArrowDown" => self.move_selection(columns, ctx),
                    "Home" => self.activate(ctx, 0),
                    "End" if !self.swatches.is_empty() => {
                        self.activate(ctx, self.swatches.len() - 1);
                    }
                    "Enter" | " " => {
                        if let Some(selected) = self.current_selected().or(Some(0)) {
                            self.activate(ctx, selected);
                        }
                    }
                    _ => return,
                }
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

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let swatch_size = self.resolved_swatch_size(&theme);
        let gap = self.resolved_gap(&theme);
        let columns = self.grid_columns() as f32;
        let rows = self.grid_rows() as f32;
        constraints.clamp(Size::new(
            columns * swatch_size + (columns - 1.0).max(0.0) * gap,
            rows * swatch_size + (rows - 1.0).max(0.0) * gap,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let swatch_size = self.resolved_swatch_size(&theme);
        let radius = metrics.corner_radius.min(swatch_size * 0.25);
        let selected = self.current_selected();

        if self.focus_animation.value > 0.0 {
            let focus_outset = metrics.focus_ring_outset;
            ctx.stroke(
                rounded_rect_path(
                    ctx.bounds().inflate(focus_outset, focus_outset),
                    radius + focus_outset,
                ),
                palette.focus_ring.with_alpha(self.focus_animation.value),
                StrokeStyle::new(metrics.focus_ring_width.max(1.0)),
            );
        }

        for (index, swatch) in self.swatches.iter().enumerate() {
            let Some(rect) = self.swatch_rect(ctx.bounds(), index, &theme) else {
                continue;
            };
            let selected = selected == Some(index);
            let hovered = self.hovered == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let pressed_offset = press_amount * interaction.pressed_offset;
            let body = Rect::new(
                rect.x(),
                rect.y() + pressed_offset,
                rect.width(),
                (rect.height() - pressed_offset).max(0.0),
            );
            let ring = if selected {
                palette.accent_border
            } else if hovered || hover_amount > 0.0 || press_amount > 0.0 {
                palette.border_hover
            } else {
                palette.border
            };
            let ring_width = if selected {
                metrics.border_width.max(1.0) + 1.0
            } else {
                metrics.border_width.max(1.0)
            };
            let fill_inset = if selected {
                metrics.color_palette_selected_swatch_inset
            } else {
                metrics.color_palette_swatch_inset
            } + pressed_offset * 0.5;
            let fill_rect = inset_rect(body, Insets::all(fill_inset));

            let base_background = if selected {
                mix_color(palette.control, palette.accent, interaction.selected_blend)
            } else {
                palette.control
            };
            let hover_background = if hover_amount > 0.0 {
                mix_color(
                    base_background,
                    palette.control_hover,
                    interaction.hover_blend * hover_amount,
                )
            } else {
                base_background
            };
            let background = if press_amount > 0.0 {
                mix_color(
                    hover_background,
                    palette.control_active,
                    interaction.pressed_blend * press_amount,
                )
            } else {
                hover_background
            };

            if selected {
                ctx.fill(rounded_rect_path(body, radius), background);
            } else if hover_amount > 0.0 || press_amount > 0.0 {
                ctx.fill(rounded_rect_path(rect, radius), background);
            }
            draw_checkerboard(ctx, fill_rect, metrics.color_palette_checker_size, &theme);
            ctx.fill(
                rounded_rect_path(fill_rect, (radius - fill_inset).max(0.0)),
                swatch.color,
            );
            ctx.stroke(
                rounded_rect_path(body, radius),
                ring,
                StrokeStyle::new(ring_width),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = self.selected_value().map(SemanticsValue::Text);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        let selected = self.current_selected();
        let theme = self.resolved_theme();
        for (index, swatch) in self.swatches.iter().enumerate() {
            let Some(rect) = self.swatch_rect(ctx.bounds(), index, &theme) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                color_palette_swatch_id(ctx.widget_id(), index),
                SemanticsRole::ColorSwatch,
                rect,
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(swatch.name.clone());
            node.value = Some(SemanticsValue::Text(format_color(swatch.color)));
            node.state.hovered = self.hovered == Some(index);
            node.state.selected = selected == Some(index);
            node.actions = vec![SemanticsAction::Activate];
            ctx.push(node);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.swatches.is_empty()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn color_palette_swatch_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 7_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(509)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushPreviewShape {
    Round,
    Square,
}

impl BrushPreviewShape {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Round => "Round",
            Self::Square => "Square",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushPreviewSpec {
    pub color: Color,
    pub size: f32,
    pub opacity: f32,
    pub shape: BrushPreviewShape,
}

impl BrushPreviewSpec {
    pub fn new(color: Color, size: f32, opacity: f32, shape: BrushPreviewShape) -> Self {
        Self {
            color,
            size: size.max(1.0),
            opacity: opacity.clamp(0.0, 1.0),
            shape,
        }
    }
}

pub struct BrushPreview {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    kind: String,
    spec: BrushPreviewSpec,
    spec_reader: Option<Box<dyn Fn() -> BrushPreviewSpec>>,
    size: Option<Size>,
}

impl BrushPreview {
    pub fn new(name: impl Into<String>) -> Self {
        let default_theme = DefaultTheme::default();
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            kind: "brush".to_string(),
            spec: BrushPreviewSpec::new(
                default_theme.palette.accent,
                18.0,
                1.0,
                BrushPreviewShape::Round,
            ),
            spec_reader: None,
            size: None,
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

    pub fn spec(mut self, spec: BrushPreviewSpec) -> Self {
        self.spec = spec;
        self.spec_reader = None;
        self
    }

    pub fn spec_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> BrushPreviewSpec + 'static,
    {
        self.spec_reader = Some(Box::new(reader));
        self
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(Size::new(size.width.max(80.0), size.height.max(44.0)));
        self
    }

    fn current_spec(&self) -> BrushPreviewSpec {
        self.spec_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.spec)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn value_text(kind: &str, spec: BrushPreviewSpec) -> String {
        format!(
            "{} {}, {:.0} px, {:.0}% opacity",
            spec.shape.label(),
            kind,
            spec.size,
            spec.opacity * 100.0
        )
    }

    fn resolved_size(&self, theme: &DefaultTheme) -> Size {
        self.size.unwrap_or(Size::new(
            theme.metrics.brush_preview_min_width,
            theme.metrics.brush_preview_min_height,
        ))
    }
}

impl Widget for BrushPreview {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        constraints.clamp(self.resolved_size(&theme))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let spec = self.current_spec();
        let content = inset_rect(bounds, metrics.brush_preview_padding);
        let swatch_width = metrics.brush_preview_swatch_width.min(content.width());
        let swatch = Rect::new(content.x(), content.y(), swatch_width, content.height());
        let sample = Rect::new(
            swatch.max_x() + metrics.brush_preview_swatch_gap,
            content.y(),
            (content.max_x() - swatch.max_x() - metrics.brush_preview_swatch_gap).max(0.0),
            content.height(),
        );
        let preview_color = spec
            .color
            .with_alpha((spec.color.alpha * spec.opacity).clamp(0.0, 1.0));

        ctx.fill(
            rounded_rect_path(bounds, metrics.corner_radius),
            palette.surface_raised,
        );
        ctx.stroke(
            rounded_rect_path(bounds, metrics.corner_radius),
            palette.border,
            StrokeStyle::new(metrics.border_width.max(1.0)),
        );
        draw_checkerboard(ctx, swatch, metrics.brush_preview_checker_size, &theme);
        ctx.stroke(
            rounded_rect_path(swatch, metrics.indicator_corner_radius),
            palette.border.with_alpha(0.70),
            StrokeStyle::new(metrics.border_width.max(1.0)),
        );
        paint_brush_preview_mark(ctx, swatch, spec, preview_color);

        let track = Rect::new(
            sample.x(),
            sample.y() + sample.height() * 0.44,
            sample.width(),
            sample.height() * 0.24,
        );
        draw_checkerboard(ctx, track, metrics.brush_preview_checker_size, &theme);
        paint_brush_preview_stroke(ctx, track, spec, preview_color);

        let text_slot = Rect::new(
            sample.x(),
            sample.max_y() - metrics.brush_preview_text_height,
            sample.width(),
            metrics.brush_preview_text_height,
        );
        let value_text = Self::value_text(&self.kind, spec);
        let text_style = TextStyle {
            font_size: metrics.brush_preview_text_font_size,
            line_height: metrics.brush_preview_text_line_height,
            color: palette.text.with_alpha(0.72),
            ..theme.body_text_style()
        };
        ctx.push_clip_rect(text_slot);
        paint_aligned_text(
            ctx,
            text_slot,
            &value_text,
            &text_style,
            text_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let spec = self.current_spec();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Image, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(Self::value_text(&self.kind, spec)));
        ctx.push(node);
    }
}

fn paint_brush_preview_mark(ctx: &mut PaintCtx, rect: Rect, spec: BrushPreviewSpec, color: Color) {
    let diameter = spec
        .size
        .min(rect.width().min(rect.height()) - 10.0)
        .max(6.0);
    let center = Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    let mark = Rect::new(
        center.x - diameter * 0.5,
        center.y - diameter * 0.5,
        diameter,
        diameter,
    );
    match spec.shape {
        BrushPreviewShape::Round => ctx.fill(Path::circle(center, diameter * 0.5), color),
        BrushPreviewShape::Square => ctx.fill(rounded_rect_path(mark, 2.0), color),
    }
}

fn paint_brush_preview_stroke(
    ctx: &mut PaintCtx,
    rect: Rect,
    spec: BrushPreviewSpec,
    color: Color,
) {
    let dot_count = 7;
    let diameter = (spec.size * 0.62).clamp(4.0, rect.height().max(4.0));
    for index in 0..dot_count {
        let t = if dot_count <= 1 {
            0.0
        } else {
            index as f32 / (dot_count - 1) as f32
        };
        let center = Point::new(
            rect.x() + diameter * 0.5 + t * (rect.width() - diameter).max(0.0),
            rect.y() + rect.height() * 0.5,
        );
        let alpha = 0.35 + t * 0.65;
        let color = color.with_alpha((color.alpha * alpha).clamp(0.0, 1.0));
        match spec.shape {
            BrushPreviewShape::Round => ctx.fill(Path::circle(center, diameter * 0.5), color),
            BrushPreviewShape::Square => ctx.fill(
                rounded_rect_path(
                    Rect::new(
                        center.x - diameter * 0.5,
                        center.y - diameter * 0.5,
                        diameter,
                        diameter,
                    ),
                    1.5,
                ),
                color,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveChannel {
    ColorWheel,
    SaturationValue,
    Hue,
    Saturation,
    Value,
    Alpha,
    EncodingSelector,
    EncodingOption(usize),
    RgbRed,
    RgbGreen,
    RgbBlue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorPickerSemanticPart {
    CurrentColor,
    PreviousColor,
    ColorRange,
    ColorRangeMenu,
    ColorRangeOption(usize),
    SaturationValue,
    Hue,
    Saturation,
    Value,
    Alpha,
    Red,
    Green,
    Blue,
    Hex,
}

pub struct ColorPicker {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    editing_space: ColorSpace,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: f32,
    previous_color: Color,
    show_alpha: bool,
    compact: bool,
    encoding_dropdown_open: bool,
    active: Option<ActiveChannel>,
    focus_animation: AnimatedScalar,
    color_reader: Option<Box<dyn Fn() -> Color>>,
    on_change: Option<Box<dyn FnMut(Color)>>,
}

#[derive(Debug, Clone, Copy)]
struct ColorPickerResolvedState {
    color: Color,
    editing_space: ColorSpace,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: f32,
    hdr_capable: bool,
    max_channel_value: f32,
}

impl ColorPicker {
    const MAX_HDR_VALUE: f32 = 12.0;
    const ENCODING_OPTIONS: [ColorSpace; 4] = [
        ColorSpace::Srgb,
        ColorSpace::LinearSrgb,
        ColorSpace::DisplayP3,
        ColorSpace::LinearDisplayP3,
    ];

    pub fn new(name: impl Into<String>) -> Self {
        let theme = DefaultTheme::default();
        Self::from_color(name, theme.palette.accent)
    }

    pub fn from_color(name: impl Into<String>, color: Color) -> Self {
        let (hue, saturation, value) = rgb_to_hsv(color);
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            editing_space: color.space,
            hue,
            saturation,
            value,
            alpha: color.alpha,
            previous_color: color,
            show_alpha: true,
            compact: false,
            encoding_dropdown_open: false,
            active: None,
            focus_animation: AnimatedScalar::new(0.0),
            color_reader: None,
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

    pub fn show_alpha(mut self, show_alpha: bool) -> Self {
        self.show_alpha = show_alpha;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(Color) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn color_when<F>(mut self, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.color_reader = Some(Box::new(color));
        self
    }

    pub fn color(&self) -> Color {
        hsv_to_color(
            self.editing_space,
            self.hue,
            self.saturation,
            self.value,
            self.alpha,
        )
    }

    fn hdr_capable(&self) -> bool {
        color_space_hdr_capable(self.editing_space)
    }

    fn max_channel_value(&self) -> f32 {
        if self.hdr_capable() {
            Self::MAX_HDR_VALUE
        } else {
            1.0
        }
    }

    fn external_color(&self) -> Option<Color> {
        if self.active.is_none() && !self.encoding_dropdown_open {
            self.color_reader.as_ref().map(|reader| reader())
        } else {
            None
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn layout_metrics_for(&self, theme: &DefaultTheme) -> ControlMetrics {
        if self.compact {
            ControlMetrics::from_tokens(theme.spacing, theme.radius, ThemeDensity::Compact)
        } else {
            theme.metrics
        }
    }

    fn layout_metrics(&self) -> ControlMetrics {
        let theme = self.resolved_theme();
        self.layout_metrics_for(&theme)
    }

    fn resolved_state(&self) -> ColorPickerResolvedState {
        if let Some(color) = self.external_color() {
            let (hue, saturation, value) = rgb_to_hsv(color);
            let hdr_capable = color_space_hdr_capable(color.space);
            return ColorPickerResolvedState {
                color,
                editing_space: color.space,
                hue,
                saturation,
                value,
                alpha: color.alpha,
                hdr_capable,
                max_channel_value: if hdr_capable {
                    Self::MAX_HDR_VALUE
                } else {
                    1.0
                },
            };
        }

        ColorPickerResolvedState {
            color: self.color(),
            editing_space: self.editing_space,
            hue: self.hue,
            saturation: self.saturation,
            value: self.value,
            alpha: self.alpha,
            hdr_capable: self.hdr_capable(),
            max_channel_value: self.max_channel_value(),
        }
    }

    fn sync_external_color(&mut self) -> bool {
        let Some(color) = self.external_color() else {
            return false;
        };
        if !colors_close(self.color(), color) {
            self.apply_color(color);
            return true;
        }
        false
    }

    fn content_inset(&self) -> f32 {
        self.layout_metrics().color_picker_content_inset
    }

    fn panel_gap(&self) -> f32 {
        self.layout_metrics().color_picker_panel_gap
    }

    fn top_bar_height(&self) -> f32 {
        self.layout_metrics().color_picker_top_bar_height
    }

    fn swatch_width(&self) -> f32 {
        self.layout_metrics().color_picker_swatch_width
    }

    fn swatch_gap(&self) -> f32 {
        self.layout_metrics().color_picker_swatch_gap
    }

    fn section_gap(&self) -> f32 {
        self.layout_metrics().color_picker_section_gap
    }

    fn wheel_size(&self) -> f32 {
        self.layout_metrics().color_picker_wheel_size
    }

    fn map_size(&self) -> f32 {
        self.layout_metrics().color_picker_map_size
    }

    fn right_panel_width(&self) -> f32 {
        self.layout_metrics().color_picker_right_panel_width
    }

    fn row_height(&self) -> f32 {
        self.layout_metrics().color_picker_row_height
    }

    fn row_gap(&self) -> f32 {
        self.layout_metrics().color_picker_row_gap
    }

    fn field_height(&self) -> f32 {
        self.layout_metrics().color_picker_field_height
    }

    fn field_gap(&self) -> f32 {
        self.layout_metrics().color_picker_field_gap
    }

    fn dropdown_gap(&self) -> f32 {
        self.layout_metrics().color_picker_dropdown_gap
    }

    fn encoding_menu_row_height(&self) -> f32 {
        self.layout_metrics().color_picker_encoding_menu_row_height
    }

    fn channel_slider_count(&self) -> usize {
        if self.show_alpha { 4 } else { 3 }
    }

    fn desired_size(&self) -> Size {
        let inset = self.content_inset();
        let left_height = self.wheel_size()
            + self.section_gap()
            + self.channel_slider_count() as f32 * self.row_height()
            + self.channel_slider_count().saturating_sub(1) as f32 * self.row_gap();
        let right_height = self.map_size()
            + self.section_gap()
            + 3.0 * self.row_height()
            + 2.0 * self.row_gap()
            + self.field_gap()
            + self.field_height();
        Size::new(
            inset * 2.0 + self.wheel_size() + self.panel_gap() + self.right_panel_width(),
            inset * 2.0 + self.top_bar_height() + self.panel_gap() + left_height.max(right_height),
        )
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, Insets::all(self.content_inset()))
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        Rect::new(
            content.x(),
            content.y(),
            content.width(),
            self.top_bar_height(),
        )
    }

    fn current_swatch_rect(&self, bounds: Rect) -> Rect {
        let header = self.header_rect(bounds);
        Rect::new(header.x(), header.y(), self.swatch_width(), header.height())
    }

    fn previous_swatch_rect(&self, bounds: Rect) -> Rect {
        let current = self.current_swatch_rect(bounds);
        Rect::new(
            current.max_x() + self.swatch_gap(),
            current.y(),
            self.swatch_width(),
            current.height(),
        )
    }

    fn left_column_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let y = self.header_rect(bounds).max_y() + self.panel_gap();
        let width = self.wheel_size().min(content.width());
        Rect::new(content.x(), y, width, content.max_y() - y)
    }

    fn right_column_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let left = self.left_column_rect(bounds);
        Rect::new(
            left.max_x() + self.panel_gap(),
            left.y(),
            content.max_x() - (left.max_x() + self.panel_gap()),
            content.max_y() - left.y(),
        )
    }

    fn color_wheel_rect(&self, bounds: Rect) -> Rect {
        let left = self.left_column_rect(bounds);
        Rect::new(left.x(), left.y(), self.wheel_size(), self.wheel_size())
    }

    fn saturation_value_rect(&self, bounds: Rect) -> Rect {
        let right = self.right_column_rect(bounds);
        Rect::new(
            right.x(),
            right.y(),
            right.width().min(self.map_size()),
            self.map_size(),
        )
    }

    fn left_slider_rect(&self, bounds: Rect, index: usize) -> Rect {
        let wheel = self.color_wheel_rect(bounds);
        let y = wheel.max_y()
            + self.section_gap()
            + index as f32 * (self.row_height() + self.row_gap());
        Rect::new(wheel.x(), y, wheel.width(), self.row_height())
    }

    fn encoding_rect(&self, bounds: Rect) -> Rect {
        let header = self.header_rect(bounds);
        let selector_x = header.x()
            + self.swatch_width()
            + self.swatch_gap()
            + self.swatch_width()
            + self.panel_gap();
        Rect::new(
            selector_x,
            header.y() + ((header.height() - self.field_height()) * 0.5),
            (header.max_x() - selector_x).max(0.0),
            self.field_height(),
        )
    }

    fn encoding_menu_rect(&self, bounds: Rect) -> Rect {
        let encoding = self.encoding_rect(bounds);
        Rect::new(
            encoding.x(),
            encoding.max_y() + self.dropdown_gap(),
            encoding.width(),
            self.encoding_menu_row_height() * Self::ENCODING_OPTIONS.len() as f32,
        )
    }

    fn encoding_option_rect(&self, bounds: Rect, index: usize) -> Rect {
        let menu = self.encoding_menu_rect(bounds);
        Rect::new(
            menu.x(),
            menu.y() + index as f32 * self.encoding_menu_row_height(),
            menu.width(),
            self.encoding_menu_row_height(),
        )
    }

    fn encoding_option_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        if !self.encoding_dropdown_open || !self.encoding_menu_rect(bounds).contains(position) {
            return None;
        }

        Self::ENCODING_OPTIONS
            .iter()
            .enumerate()
            .find_map(|(index, _)| {
                self.encoding_option_rect(bounds, index)
                    .contains(position)
                    .then_some(index)
            })
    }

    fn rgb_row_rect(&self, bounds: Rect, index: usize) -> Rect {
        let map = self.saturation_value_rect(bounds);
        let y =
            map.max_y() + self.section_gap() + index as f32 * (self.row_height() + self.row_gap());
        Rect::new(map.x(), y, map.width(), self.row_height())
    }

    fn hex_rect(&self, bounds: Rect) -> Rect {
        let last_row = self.rgb_row_rect(bounds, 2);
        Rect::new(
            last_row.x(),
            last_row.max_y() + self.field_gap(),
            last_row.width(),
            self.field_height(),
        )
    }

    fn update_from_position(&mut self, bounds: Rect, channel: ActiveChannel, position: Point) {
        match channel {
            ActiveChannel::ColorWheel => {
                let rect = self.color_wheel_rect(bounds);
                let center = Point::new(
                    rect.x() + rect.width() * 0.5,
                    rect.y() + rect.height() * 0.5,
                );
                let dx = position.x - center.x;
                let dy = position.y - center.y;
                let angle = dy.atan2(dx);
                self.hue = ((angle / std::f32::consts::TAU) + 1.0).rem_euclid(1.0);
                self.emit_change();
            }
            ActiveChannel::SaturationValue => {
                let rect = self.saturation_value_rect(bounds);
                self.saturation = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                let t = (1.0 - ((position.y - rect.y()) / rect.height())).clamp(0.0, 1.0);
                self.value = if self.hdr_capable() {
                    hdr_slider_to_value(t)
                } else {
                    t
                };
                self.emit_change();
            }
            ActiveChannel::Hue => {
                let rect = self.left_slider_rect(bounds, 0);
                self.hue = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                self.emit_change();
            }
            ActiveChannel::Saturation => {
                let rect = self.left_slider_rect(bounds, 1);
                self.saturation = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                self.emit_change();
            }
            ActiveChannel::Value => {
                let rect = self.left_slider_rect(bounds, 2);
                let t = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                self.value = if self.hdr_capable() {
                    hdr_slider_to_value(t)
                } else {
                    t
                };
                self.emit_change();
            }
            ActiveChannel::Alpha => {
                let rect = self.left_slider_rect(bounds, 3);
                self.alpha = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                self.emit_change();
            }
            ActiveChannel::EncodingSelector | ActiveChannel::EncodingOption(_) => {}
            ActiveChannel::RgbRed => self.update_rgb_channel_from_position(bounds, 0, position),
            ActiveChannel::RgbGreen => self.update_rgb_channel_from_position(bounds, 1, position),
            ActiveChannel::RgbBlue => self.update_rgb_channel_from_position(bounds, 2, position),
        }
    }

    fn emit_change(&mut self) {
        let color = self.color();
        if let Some(on_change) = &mut self.on_change {
            on_change(color);
        }
    }

    fn apply_color(&mut self, color: Color) {
        self.editing_space = color.space;
        let (hue, saturation, value) = rgb_to_hsv(color);
        self.hue = hue;
        self.saturation = saturation;
        self.value = value;
        self.alpha = color.alpha;
    }

    fn set_editing_space(&mut self, next_space: ColorSpace) {
        if self.editing_space == next_space {
            return;
        }

        let current = self.color();
        self.apply_color(Color::new(
            next_space,
            current.red,
            current.green,
            current.blue,
            current.alpha,
        ));
        self.emit_change();
    }

    fn update_rgb_channel_from_position(
        &mut self,
        bounds: Rect,
        channel_index: usize,
        position: Point,
    ) {
        let rect = self.rgb_row_rect(bounds, channel_index);
        let t = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
        let mut channels = [self.color().red, self.color().green, self.color().blue];
        channels[channel_index] = self.max_channel_value() * t;
        self.apply_color(Color::new(
            self.editing_space,
            channels[0],
            channels[1],
            channels[2],
            self.alpha,
        ));
        self.emit_change();
    }

    fn hit_channel(&self, bounds: Rect, position: Point) -> Option<ActiveChannel> {
        if point_in_wheel_ring(self.color_wheel_rect(bounds), position) {
            Some(ActiveChannel::ColorWheel)
        } else if self.saturation_value_rect(bounds).contains(position) {
            Some(ActiveChannel::SaturationValue)
        } else if self.left_slider_rect(bounds, 0).contains(position) {
            Some(ActiveChannel::Hue)
        } else if self.left_slider_rect(bounds, 1).contains(position) {
            Some(ActiveChannel::Saturation)
        } else if self.left_slider_rect(bounds, 2).contains(position) {
            Some(ActiveChannel::Value)
        } else if self.show_alpha && self.left_slider_rect(bounds, 3).contains(position) {
            Some(ActiveChannel::Alpha)
        } else if self.encoding_rect(bounds).contains(position) {
            Some(ActiveChannel::EncodingSelector)
        } else if self.rgb_row_rect(bounds, 0).contains(position) {
            Some(ActiveChannel::RgbRed)
        } else if self.rgb_row_rect(bounds, 1).contains(position) {
            Some(ActiveChannel::RgbGreen)
        } else if self.rgb_row_rect(bounds, 2).contains(position) {
            Some(ActiveChannel::RgbBlue)
        } else {
            None
        }
    }

    fn color_semantics_text(&self, color: Color, hdr_capable: bool) -> String {
        if hdr_capable && is_hdr_color(color) {
            format!(
                "R {:.3} G {:.3} B {:.3} A {:.3}",
                color.red, color.green, color.blue, color.alpha
            )
        } else {
            format_color(color)
        }
    }

    fn push_color_swatch_semantics(
        &self,
        ctx: &mut SemanticsCtx,
        part: ColorPickerSemanticPart,
        name: &'static str,
        bounds: Rect,
        color: Color,
        hdr_capable: bool,
    ) {
        let mut node = color_picker_child_semantics_node(
            ctx.widget_id(),
            part,
            SemanticsRole::ColorSwatch,
            bounds,
            name,
        );
        node.value = Some(SemanticsValue::Text(
            self.color_semantics_text(color, hdr_capable),
        ));
        ctx.push(node);
    }

    fn push_slider_semantics(
        &self,
        ctx: &mut SemanticsCtx,
        part: ColorPickerSemanticPart,
        name: &'static str,
        bounds: Rect,
        value: f32,
        min: f32,
        max: f32,
    ) {
        let mut node = color_picker_child_semantics_node(
            ctx.widget_id(),
            part,
            SemanticsRole::Slider,
            bounds,
            name,
        );
        node.value = Some(SemanticsValue::Range {
            value: value as f64,
            min: min as f64,
            max: max as f64,
        });
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
    }

    fn push_component_semantics(&self, ctx: &mut SemanticsCtx, resolved: ColorPickerResolvedState) {
        let bounds = ctx.bounds();
        let current = resolved.color;
        self.push_color_swatch_semantics(
            ctx,
            ColorPickerSemanticPart::CurrentColor,
            "Current color",
            self.current_swatch_rect(bounds),
            current,
            resolved.hdr_capable,
        );
        self.push_color_swatch_semantics(
            ctx,
            ColorPickerSemanticPart::PreviousColor,
            "Previous color",
            self.previous_swatch_rect(bounds),
            self.previous_color,
            color_space_hdr_capable(self.previous_color.space),
        );

        let mut range = color_picker_child_semantics_node(
            ctx.widget_id(),
            ColorPickerSemanticPart::ColorRange,
            SemanticsRole::ComboBox,
            self.encoding_rect(bounds),
            "Color range",
        );
        range.value = Some(SemanticsValue::Text(
            editing_space_label(resolved.editing_space).to_string(),
        ));
        range.state.expanded = Some(self.encoding_dropdown_open);
        range.actions = vec![
            SemanticsAction::Focus,
            if self.encoding_dropdown_open {
                SemanticsAction::Collapse
            } else {
                SemanticsAction::Expand
            },
            SemanticsAction::SetValue,
        ];
        ctx.push(range);

        let mut saturation_value = color_picker_child_semantics_node(
            ctx.widget_id(),
            ColorPickerSemanticPart::SaturationValue,
            SemanticsRole::Slider,
            self.saturation_value_rect(bounds),
            "Saturation and value",
        );
        saturation_value.description = Some(format!(
            "Saturation {:.1}%, value {:.3}",
            resolved.saturation * 100.0,
            resolved.value
        ));
        saturation_value.value = Some(SemanticsValue::Range {
            value: resolved.value as f64,
            min: 0.0,
            max: resolved.max_channel_value as f64,
        });
        saturation_value.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(saturation_value);

        self.push_slider_semantics(
            ctx,
            ColorPickerSemanticPart::Hue,
            "Hue",
            self.left_slider_rect(bounds, 0),
            resolved.hue * 360.0,
            0.0,
            360.0,
        );
        self.push_slider_semantics(
            ctx,
            ColorPickerSemanticPart::Saturation,
            "Saturation",
            self.left_slider_rect(bounds, 1),
            resolved.saturation * 100.0,
            0.0,
            100.0,
        );
        self.push_slider_semantics(
            ctx,
            ColorPickerSemanticPart::Value,
            "Value",
            self.left_slider_rect(bounds, 2),
            resolved.value,
            0.0,
            resolved.max_channel_value,
        );
        if self.show_alpha {
            self.push_slider_semantics(
                ctx,
                ColorPickerSemanticPart::Alpha,
                "Alpha",
                self.left_slider_rect(bounds, 3),
                resolved.alpha * 100.0,
                0.0,
                100.0,
            );
        }

        let rgb = current.to_array();
        for (index, (part, name)) in [
            (ColorPickerSemanticPart::Red, "Red"),
            (ColorPickerSemanticPart::Green, "Green"),
            (ColorPickerSemanticPart::Blue, "Blue"),
        ]
        .into_iter()
        .enumerate()
        {
            self.push_slider_semantics(
                ctx,
                part,
                name,
                self.rgb_row_rect(bounds, index),
                rgb[index],
                0.0,
                resolved.max_channel_value,
            );
        }

        let hex_name = if resolved.hdr_capable && is_hdr_color(current) {
            "HDR hex unavailable".to_string()
        } else {
            format!("Hex color {}", format_color(current))
        };
        let hex = color_picker_child_semantics_node(
            ctx.widget_id(),
            ColorPickerSemanticPart::Hex,
            SemanticsRole::Text,
            self.hex_rect(bounds),
            hex_name,
        );
        ctx.push(hex);

        if self.encoding_dropdown_open {
            let menu_id = color_picker_child_semantics_id(
                ctx.widget_id(),
                ColorPickerSemanticPart::ColorRangeMenu,
            );
            let mut menu = color_picker_child_semantics_node(
                ctx.widget_id(),
                ColorPickerSemanticPart::ColorRangeMenu,
                SemanticsRole::Menu,
                self.encoding_menu_rect(bounds),
                "Color range options",
            );
            menu.state.expanded = Some(true);
            ctx.push(menu);

            for (index, space) in Self::ENCODING_OPTIONS.iter().copied().enumerate() {
                let mut item = color_picker_child_semantics_node_with_parent(
                    ctx.widget_id(),
                    menu_id,
                    ColorPickerSemanticPart::ColorRangeOption(index),
                    SemanticsRole::MenuItem,
                    self.encoding_option_rect(bounds, index),
                    editing_space_label(space),
                );
                item.state.selected = space == resolved.editing_space;
                item.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
                ctx.push(item);
            }
        }
    }
}

fn color_picker_child_semantics_id(parent: WidgetId, part: ColorPickerSemanticPart) -> WidgetId {
    let slot = match part {
        ColorPickerSemanticPart::CurrentColor => 1,
        ColorPickerSemanticPart::PreviousColor => 2,
        ColorPickerSemanticPart::ColorRange => 3,
        ColorPickerSemanticPart::ColorRangeMenu => 4,
        ColorPickerSemanticPart::ColorRangeOption(index) => 32 + index as u64,
        ColorPickerSemanticPart::SaturationValue => 5,
        ColorPickerSemanticPart::Hue => 6,
        ColorPickerSemanticPart::Saturation => 7,
        ColorPickerSemanticPart::Value => 8,
        ColorPickerSemanticPart::Alpha => 9,
        ColorPickerSemanticPart::Red => 10,
        ColorPickerSemanticPart::Green => 11,
        ColorPickerSemanticPart::Blue => 12,
        ColorPickerSemanticPart::Hex => 13,
    };
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(TAG | (parent.get().wrapping_mul(397).wrapping_add(slot) & LOW_MASK))
}

fn color_picker_child_semantics_node(
    parent: WidgetId,
    part: ColorPickerSemanticPart,
    role: SemanticsRole,
    bounds: Rect,
    name: impl Into<String>,
) -> SemanticsNode {
    color_picker_child_semantics_node_with_parent(parent, parent, part, role, bounds, name)
}

fn color_picker_child_semantics_node_with_parent(
    owner: WidgetId,
    parent: WidgetId,
    part: ColorPickerSemanticPart,
    role: SemanticsRole,
    bounds: Rect,
    name: impl Into<String>,
) -> SemanticsNode {
    let mut node = SemanticsNode::new(color_picker_child_semantics_id(owner, part), role, bounds);
    node.parent = Some(parent);
    node.name = Some(name.into());
    node
}

impl Widget for ColorPicker {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.sync_external_color() {
            ctx.request_paint();
            ctx.request_semantics();
        }
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if let Some(active) = self.active {
                    self.update_from_position(ctx.bounds(), active, pointer.position);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(option) = self.encoding_option_at(ctx.bounds(), pointer.position) {
                    self.active = Some(ActiveChannel::EncodingOption(option));
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                    return;
                }

                let active = self.hit_channel(ctx.bounds(), pointer.position);
                if let Some(active) = active {
                    self.active = Some(active);
                    if active == ActiveChannel::EncodingSelector {
                        self.encoding_dropdown_open = !self.encoding_dropdown_open;
                    } else {
                        self.encoding_dropdown_open = false;
                        self.update_from_position(ctx.bounds(), active, pointer.position);
                    }
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if self.encoding_dropdown_open {
                    self.encoding_dropdown_open = false;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(active) = self.active.take() {
                    if let ActiveChannel::EncodingOption(index) = active {
                        if self
                            .encoding_option_rect(ctx.bounds(), index)
                            .contains(pointer.position)
                        {
                            self.set_editing_space(Self::ENCODING_OPTIONS[index]);
                        }
                        self.encoding_dropdown_open = false;
                    }
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.active.take().is_some() {
                    self.encoding_dropdown_open = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let saturation_step = if key.modifiers.shift { 0.1 } else { 0.02 };
                let value_step = if key.modifiers.shift { 0.5 } else { 0.1 };
                match key.key.as_str() {
                    "ArrowLeft" => {
                        self.saturation = (self.saturation - saturation_step).clamp(0.0, 1.0)
                    }
                    "ArrowRight" => {
                        self.saturation = (self.saturation + saturation_step).clamp(0.0, 1.0)
                    }
                    "ArrowUp" => {
                        self.value = (self.value + value_step).clamp(0.0, self.max_channel_value())
                    }
                    "ArrowDown" => {
                        self.value = (self.value - value_step).clamp(0.0, self.max_channel_value())
                    }
                    _ => return,
                }
                self.emit_change();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let (changed, active) = advance_scalar(&mut self.focus_animation, *time);
                if changed {
                    ctx.request_paint();
                }
                if active {
                    ctx.request_animation_frame();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_external_color();
        constraints.clamp(self.desired_size())
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let resolved = self.resolved_state();
        let current = resolved.color;
        let wheel = self.color_wheel_rect(ctx.bounds());
        let map = self.saturation_value_rect(ctx.bounds());
        let encoding = self.encoding_rect(ctx.bounds());

        draw_surface(ctx, ctx.bounds(), &theme, self.focus_animation.value);
        paint_picker_header(
            ctx,
            self.current_swatch_rect(ctx.bounds()),
            self.previous_swatch_rect(ctx.bounds()),
            &theme,
            self.previous_color,
            current,
        );
        paint_dropdown(
            ctx,
            encoding,
            &theme,
            editing_space_label(resolved.editing_space),
        );

        paint_color_wheel(ctx, wheel, &theme);
        paint_wheel_marker(ctx, wheel, resolved.hue, &theme);

        paint_saturation_value_plane(
            ctx,
            map,
            resolved.editing_space,
            resolved.hue,
            resolved.max_channel_value,
            &theme,
        );
        let marker = Point::new(
            map.x() + resolved.saturation * map.width(),
            map.y()
                + (1.0
                    - if resolved.hdr_capable {
                        hdr_value_to_slider(resolved.value)
                    } else {
                        resolved.value.clamp(0.0, 1.0)
                    })
                    * map.height(),
        );
        paint_marker(ctx, marker, contrast_color(current, &theme), &theme);

        let rows = [
            ("H", format!("{:.2}", resolved.hue * 360.0)),
            ("S", format!("{:.2}", resolved.saturation * 100.0)),
            ("V", format!("{:.3}", resolved.value)),
            ("A", format!("{:.1}", resolved.alpha * 100.0)),
        ];
        for (index, (label, value_text)) in rows.into_iter().enumerate() {
            if index == 3 && !self.show_alpha {
                continue;
            }
            let rect = self.left_slider_rect(ctx.bounds(), index);
            match index {
                0 => paint_hue_bar(ctx, rect, &theme),
                1 => paint_saturation_bar(
                    ctx,
                    rect,
                    resolved.editing_space,
                    resolved.hue,
                    resolved.value.max(1.0),
                    &theme,
                ),
                2 => paint_value_bar(
                    ctx,
                    rect,
                    resolved.editing_space,
                    resolved.hue,
                    resolved.saturation,
                    resolved.hdr_capable,
                    &theme,
                ),
                _ => {
                    draw_checkerboard(ctx, rect, theme.metrics.color_swatch_checker_size, &theme);
                    paint_alpha_bar(ctx, rect, current, &theme);
                }
            }
            paint_labeled_row_text(ctx, rect, label, &value_text, &theme, palette.placeholder);
            let marker_x = match index {
                0 => rect.x() + resolved.hue * rect.width(),
                1 => rect.x() + resolved.saturation * rect.width(),
                2 => {
                    rect.x()
                        + if resolved.hdr_capable {
                            hdr_value_to_slider(resolved.value) * rect.width()
                        } else {
                            resolved.value.clamp(0.0, 1.0) * rect.width()
                        }
                }
                _ => rect.x() + resolved.alpha * rect.width(),
            };
            paint_marker(
                ctx,
                Point::new(marker_x, rect.y() + rect.height() * 0.5),
                palette.border_focus,
                &theme,
            );
        }

        let rgb = current.to_array();
        let channel_labels = ["R", "G", "B"];
        for (index, label) in channel_labels.into_iter().enumerate() {
            let rect = self.rgb_row_rect(ctx.bounds(), index);
            paint_rgb_channel_bar(
                ctx,
                rect,
                current,
                index,
                resolved.max_channel_value,
                &theme,
            );
            paint_labeled_row_text(
                ctx,
                rect,
                label,
                &format!("{:.3}", rgb[index]),
                &theme,
                palette.placeholder,
            );
            let marker_x =
                rect.x() + (rgb[index] / resolved.max_channel_value).clamp(0.0, 1.0) * rect.width();
            paint_marker(
                ctx,
                Point::new(marker_x, rect.y() + rect.height() * 0.5),
                palette.border_focus,
                &theme,
            );
        }

        if resolved.hdr_capable && is_hdr_color(current) {
            paint_disabled_field(
                ctx,
                self.hex_rect(ctx.bounds()),
                &theme,
                "HDR hex unavailable",
            );
        } else {
            paint_hex_field(
                ctx,
                self.hex_rect(ctx.bounds()),
                &theme,
                &format_color(current),
            );
        }

        if self.encoding_dropdown_open {
            paint_encoding_menu(
                ctx,
                self.encoding_menu_rect(ctx.bounds()),
                &theme,
                resolved.editing_space,
                self.encoding_menu_row_height(),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let resolved = self.resolved_state();
        let current = resolved.color;
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorPicker, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(format!(
            "{} editing space; {} range available",
            editing_space_label(resolved.editing_space),
            if resolved.hdr_capable { "HDR" } else { "SDR" }
        ));
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(
            self.color_semantics_text(current, resolved.hdr_capable),
        ));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
        self.push_component_semantics(ctx, resolved);
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

fn paint_picker_header(
    ctx: &mut PaintCtx,
    current_rect: Rect,
    previous_rect: Rect,
    theme: &DefaultTheme,
    previous: Color,
    current: Color,
) {
    let palette = theme.palette;
    let metrics = theme.metrics;
    let radius = metrics.indicator_corner_radius;
    draw_checkerboard(ctx, current_rect, metrics.color_swatch_checker_size, theme);
    draw_checkerboard(ctx, previous_rect, metrics.color_swatch_checker_size, theme);
    ctx.fill(rounded_rect_path(current_rect, radius), current);
    ctx.fill(rounded_rect_path(previous_rect, radius), previous);
    ctx.stroke(
        rounded_rect_path(current_rect, radius),
        palette.border_focus,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );
    ctx.stroke(
        rounded_rect_path(previous_rect, radius),
        palette.border,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );
}

fn paint_color_wheel(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme) {
    let center = Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    let outer = rect.width().min(rect.height()) * 0.5;
    let inner = outer * 0.55;
    ctx.draw_shader_rect(rect, WidgetShader::ColorWheel);
    ctx.stroke(
        Path::circle(center, outer - 1.0),
        theme.surfaces.color_picker_chrome_border,
        StrokeStyle::new(1.0),
    );
    ctx.stroke(
        Path::circle(center, inner),
        theme.surfaces.color_picker_chrome_border,
        StrokeStyle::new(1.0),
    );
}

fn paint_wheel_marker(ctx: &mut PaintCtx, rect: Rect, hue: f32, theme: &DefaultTheme) {
    let center = Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    let outer = rect.width().min(rect.height()) * 0.5;
    let inner = outer * 0.55;
    let radius = (outer + inner) * 0.5;
    let angle = hue * std::f32::consts::TAU;
    let point = Point::new(
        center.x + angle.cos() * radius,
        center.y + angle.sin() * radius,
    );
    paint_marker(ctx, point, theme.surfaces.color_picker_marker_dark, theme);
}

fn paint_saturation_value_plane(
    ctx: &mut PaintCtx,
    rect: Rect,
    space: ColorSpace,
    hue: f32,
    max_value: f32,
    theme: &DefaultTheme,
) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerSaturationValuePlane {
            color_space: space,
            hue,
            max_value,
        },
    );
    ctx.stroke_rect(
        rect,
        theme.surfaces.color_picker_plane_border,
        StrokeStyle::new(1.0),
    );
    let sdr_marker = Rect::new(
        rect.x(),
        rect.y()
            + rect.height()
                * (1.0
                    - if max_value <= 1.0001 {
                        1.0
                    } else {
                        hdr_value_to_slider(1.0)
                    }),
        rect.width(),
        1.0,
    );
    ctx.fill_rect(sdr_marker, theme.surfaces.color_picker_sdr_marker);
}

fn paint_hue_bar(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme) {
    ctx.draw_shader_rect(rect, WidgetShader::ColorPickerHueBar);
    paint_bar_border(ctx, rect, theme);
}

fn paint_saturation_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    space: ColorSpace,
    hue: f32,
    value: f32,
    theme: &DefaultTheme,
) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerSaturationBar {
            color_space: space,
            hue,
            value,
        },
    );
    paint_bar_border(ctx, rect, theme);
}

fn paint_bar_border(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme) {
    ctx.stroke_rect(
        rect,
        theme.surfaces.color_picker_bar_border,
        StrokeStyle::new(1.0),
    );
}

fn paint_value_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    space: ColorSpace,
    hue: f32,
    saturation: f32,
    hdr_capable: bool,
    theme: &DefaultTheme,
) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerValueBar {
            color_space: space,
            hue,
            saturation,
            max_value: if hdr_capable {
                ColorPicker::MAX_HDR_VALUE
            } else {
                1.0
            },
        },
    );
    paint_bar_border(ctx, rect, theme);
    if hdr_capable {
        let divider_x = rect.x() + rect.width() * 0.5;
        ctx.fill_rect(
            Rect::new(divider_x, rect.y(), 1.0, rect.height()),
            theme.surfaces.color_picker_hdr_divider,
        );
    }
}

fn paint_alpha_bar(ctx: &mut PaintCtx, rect: Rect, color: Color, theme: &DefaultTheme) {
    ctx.draw_shader_rect(rect, WidgetShader::ColorPickerAlphaBar { color });
    paint_bar_border(ctx, rect, theme);
}

fn paint_rgb_channel_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    current: Color,
    channel_index: usize,
    max_value: f32,
    theme: &DefaultTheme,
) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerRgbChannelBar {
            color: current,
            channel: channel_index as u32,
            max_value,
        },
    );
    paint_bar_border(ctx, rect, theme);
}

fn paint_labeled_row_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    label: &str,
    value_text: &str,
    theme: &DefaultTheme,
    value_color: Color,
) {
    let text = theme.text.xs;
    let paint_line_height = text.line_height.min(rect.height()).max(1.0);
    let label_width = (theme.metrics.icon_size + theme.spacing * 2.0).max(20.0);
    let value_width = (rect.width() * 0.36).clamp(56.0, 96.0);
    let label_style = text_token_style(text, theme.palette.accent_text);
    let value_style = numeric_text_style(text_token_style(text, value_color));
    let label_slot = Rect::new(
        rect.x() + theme.spacing * 1.5,
        rect.y(),
        label_width,
        rect.height(),
    );
    let value_slot = Rect::new(
        rect.max_x() - value_width - theme.spacing,
        rect.y(),
        value_width,
        rect.height(),
    );
    ctx.push_clip_rect(label_slot);
    paint_aligned_text(ctx, label_slot, label, &label_style, paint_line_height, 0.0);
    ctx.pop_clip();
    ctx.push_clip_rect(value_slot);
    paint_aligned_text(
        ctx,
        value_slot,
        value_text,
        &value_style,
        paint_line_height,
        1.0,
    );
    ctx.pop_clip();
}

fn paint_dropdown(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, label: &str) {
    let metrics = theme.metrics;
    let radius = metrics.corner_radius;
    let text = theme.text.xs;
    let paint_line_height = text.line_height.min(rect.height()).max(1.0);
    let padding = metrics.text_input_padding;
    let style = text_token_style(text, theme.palette.text);
    let text_slot = Rect::new(
        rect.x() + padding.left.max(theme.spacing * 2.0),
        rect.y(),
        rect.width() - padding.left.max(theme.spacing * 2.0) - metrics.icon_size - theme.spacing,
        rect.height(),
    );
    ctx.fill(rounded_rect_path(rect, radius), theme.palette.control);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        theme.palette.border_focus,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );
    ctx.push_clip_rect(text_slot);
    paint_aligned_text(ctx, text_slot, label, &style, paint_line_height, 0.0);
    ctx.pop_clip();
    ctx.stroke(
        dropdown_chevron_path(rect),
        theme.palette.placeholder,
        StrokeStyle::new(1.4),
    );
}

fn paint_encoding_menu(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    selected: ColorSpace,
    row_height: f32,
) {
    let metrics = theme.metrics;
    let radius = metrics.corner_radius;
    let text = theme.text.xs;
    let paint_line_height = text.line_height.min(row_height).max(1.0);
    ctx.fill(
        rounded_rect_path(rect, radius),
        theme.palette.surface_raised,
    );
    ctx.stroke(
        rounded_rect_path(rect, radius),
        theme.palette.border_focus,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );

    for (index, space) in ColorPicker::ENCODING_OPTIONS.iter().copied().enumerate() {
        let row = Rect::new(
            rect.x(),
            rect.y() + index as f32 * row_height,
            rect.width(),
            row_height,
        );
        if space == selected {
            let selected_rect = inset_rect(row, metrics.menu_item_padding);
            ctx.fill(
                rounded_rect_path(
                    selected_rect,
                    (radius - metrics.menu_item_padding.top).max(0.0),
                ),
                mix_color(
                    theme.palette.control,
                    theme.palette.accent,
                    theme.interaction.selected_blend,
                ),
            );
            ctx.fill_rect(
                Rect::new(
                    selected_rect.x(),
                    selected_rect.y() + theme.spacing,
                    theme.interaction.active_indicator_thickness,
                    (selected_rect.height() - theme.spacing * 2.0).max(0.0),
                ),
                theme.palette.border_focus,
            );
        }
        let label = editing_space_label(space);
        let style = text_token_style(
            text,
            if space == selected {
                theme.palette.accent_text
            } else {
                theme.palette.text
            },
        );
        let text_slot = Rect::new(
            row.x() + metrics.menu_item_padding.left + theme.spacing * 1.5,
            row.y(),
            row.width()
                - metrics.menu_item_padding.left
                - metrics.menu_item_padding.right
                - theme.spacing * 2.0,
            row.height(),
        );
        ctx.push_clip_rect(text_slot);
        paint_aligned_text(ctx, text_slot, label, &style, paint_line_height, 0.0);
        ctx.pop_clip();
    }
}

fn dropdown_chevron_path(rect: Rect) -> Path {
    let center = Point::new(rect.max_x() - 14.0, rect.y() + rect.height() * 0.5);
    let half_width = 4.0;
    let half_height = 2.5;
    let mut path = PathBuilder::new();
    path.move_to(Point::new(center.x - half_width, center.y - half_height));
    path.line_to(Point::new(center.x, center.y + half_height));
    path.line_to(Point::new(center.x + half_width, center.y - half_height));
    path.build()
}

fn paint_hex_field(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, value: &str) {
    let metrics = theme.metrics;
    let text = theme.text.xs;
    let paint_line_height = text.line_height.min(rect.height()).max(1.0);
    let padding = metrics.text_input_padding;
    let style = text_token_style(text, theme.palette.text);
    let text_slot = Rect::new(
        rect.x() + padding.left.max(theme.spacing * 2.0),
        rect.y(),
        rect.width() - (padding.left + padding.right).max(theme.spacing * 4.0),
        rect.height(),
    );
    ctx.fill(
        rounded_rect_path(rect, metrics.corner_radius),
        theme.palette.control,
    );
    ctx.stroke(
        rounded_rect_path(rect, metrics.corner_radius),
        theme.palette.border,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );
    ctx.push_clip_rect(text_slot);
    paint_aligned_text(ctx, text_slot, value, &style, paint_line_height, 0.0);
    ctx.pop_clip();
}

fn paint_disabled_field(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, value: &str) {
    let metrics = theme.metrics;
    let text = theme.text.xs;
    let paint_line_height = text.line_height.min(rect.height()).max(1.0);
    let padding = metrics.text_input_padding;
    let style = text_token_style(text, theme.palette.placeholder);
    let text_slot = Rect::new(
        rect.x() + padding.left.max(theme.spacing * 2.0),
        rect.y(),
        rect.width() - (padding.left + padding.right).max(theme.spacing * 4.0),
        rect.height(),
    );
    ctx.fill(
        rounded_rect_path(rect, metrics.corner_radius),
        mix_color(theme.palette.control, theme.palette.surface, 0.5)
            .with_alpha(theme.interaction.disabled_opacity),
    );
    ctx.stroke(
        rounded_rect_path(rect, metrics.corner_radius),
        theme.palette.border,
        StrokeStyle::new(metrics.border_width.max(1.0)),
    );
    ctx.push_clip_rect(text_slot);
    paint_aligned_text(ctx, text_slot, value, &style, paint_line_height, 0.0);
    ctx.pop_clip();
}

fn point_in_wheel_ring(rect: Rect, position: Point) -> bool {
    let center = Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    let dx = position.x - center.x;
    let dy = position.y - center.y;
    let distance = (dx * dx + dy * dy).sqrt();
    let outer = rect.width().min(rect.height()) * 0.5;
    let inner = outer * 0.55;
    distance >= inner && distance <= outer
}

fn editing_space_label(space: ColorSpace) -> &'static str {
    match space {
        ColorSpace::Srgb => "sRGB",
        ColorSpace::LinearSrgb => "BT709 Linear",
        ColorSpace::DisplayP3 => "Display P3",
        ColorSpace::LinearDisplayP3 => "Display P3 Linear",
    }
}

fn hdr_value_to_slider(value: f32) -> f32 {
    let value = value.clamp(0.0, ColorPicker::MAX_HDR_VALUE);
    if value <= 1.0 {
        value * 0.5
    } else {
        0.5 + (value.ln() / ColorPicker::MAX_HDR_VALUE.ln()) * 0.5
    }
}

fn hdr_slider_to_value(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.5 {
        t / 0.5
    } else {
        ColorPicker::MAX_HDR_VALUE.powf((t - 0.5) / 0.5)
    }
}

fn is_hdr_color(color: Color) -> bool {
    color.red > 1.0 || color.green > 1.0 || color.blue > 1.0
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    crate::animation::Interpolate::interpolate(from, to, amount)
}

type AnimatedScalar = MotionScalar;

fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) -> bool {
    animation.set_target_event(target, duration, easing, ctx)
}

fn set_hover_animation_target(
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

fn set_press_animation_target(
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

fn set_focus_animation_target(
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

fn advance_scalar(animation: &mut AnimatedScalar, time: f64) -> (bool, bool) {
    let previous = animation.value;
    let active = animation.advance(time);
    (animation.changed_since(previous), active)
}

fn fit_rect(bounds: Rect, source: Size, fit: ImageFit) -> Rect {
    if bounds.is_empty() || source.is_empty() {
        return bounds;
    }

    let scale_x = bounds.width() / source.width.max(1.0);
    let scale_y = bounds.height() / source.height.max(1.0);
    let scale = match fit {
        ImageFit::Fill => None,
        ImageFit::Contain => Some(scale_x.min(scale_y)),
        ImageFit::Cover => Some(scale_x.max(scale_y)),
        ImageFit::None => Some(1.0),
    };

    let size = if let Some(scale) = scale {
        Size::new(source.width * scale, source.height * scale)
    } else {
        bounds.size
    };
    Rect::new(
        bounds.x() + ((bounds.width() - size.width) * 0.5),
        bounds.y() + ((bounds.height() - size.height) * 0.5),
        size.width,
        size.height,
    )
}

fn paint_marker(ctx: &mut PaintCtx, center: Point, color: Color, theme: &DefaultTheme) {
    ctx.stroke(
        Path::circle(center, 6.5),
        theme.surfaces.color_picker_marker_outer,
        StrokeStyle::new(2.0),
    );
    ctx.stroke(Path::circle(center, 5.0), color, StrokeStyle::new(1.5));
}

fn draw_surface(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, focus_progress: f32) {
    let focus_progress = focus_progress.clamp(0.0, 1.0);
    ctx.fill(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        theme.palette.surface,
    );
    ctx.stroke(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        mix_color(
            theme.palette.border,
            theme.palette.border_focus,
            focus_progress,
        ),
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );
    if focus_progress > AnimatedScalar::EPSILON {
        let outset = theme.metrics.focus_ring_outset;
        ctx.stroke(
            rounded_rect_path(
                rect.inflate(outset, outset),
                theme.metrics.corner_radius + outset,
            ),
            theme
                .palette
                .focus_ring
                .with_alpha(theme.palette.focus_ring.alpha * focus_progress),
            StrokeStyle::new(theme.metrics.focus_ring_width.max(1.0)),
        );
    }
}

fn draw_checkerboard(ctx: &mut PaintCtx, rect: Rect, cell_size: f32, theme: &DefaultTheme) {
    let light = theme.surfaces.checkerboard_light;
    let dark = theme.surfaces.checkerboard_dark;
    let cell_size = cell_size.max(2.0);
    let cols = (rect.width() / cell_size).ceil() as usize;
    let rows = (rect.height() / cell_size).ceil() as usize;
    ctx.push_clip_rect(rect);
    for row in 0..rows {
        for col in 0..cols {
            let cell = Rect::new(
                rect.x() + col as f32 * cell_size,
                rect.y() + row as f32 * cell_size,
                cell_size,
                cell_size,
            );
            ctx.fill_rect(cell, if (row + col) % 2 == 0 { light } else { dark });
        }
    }
    ctx.pop_clip();
}

fn format_color(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        (color.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn color_space_hdr_capable(space: ColorSpace) -> bool {
    space.is_linear() || matches!(space, ColorSpace::DisplayP3)
}

fn colors_close(left: Color, right: Color) -> bool {
    left.space == right.space
        && (left.red - right.red).abs() < 0.0001
        && (left.green - right.green).abs() < 0.0001
        && (left.blue - right.blue).abs() < 0.0001
        && (left.alpha - right.alpha).abs() < 0.0001
}

fn contrast_color(color: Color, theme: &DefaultTheme) -> Color {
    if perceived_luminance(color) > 0.55 {
        theme.surfaces.color_picker_marker_dark
    } else {
        theme.surfaces.color_picker_marker_light
    }
}

fn perceived_luminance(color: Color) -> f32 {
    (0.299 * color.red) + (0.587 * color.green) + (0.114 * color.blue)
}

fn hsv_to_color(space: ColorSpace, hue: f32, saturation: f32, value: f32, alpha: f32) -> Color {
    let hue = hue.rem_euclid(1.0) * 6.0;
    let sector = hue.floor();
    let fraction = hue - sector;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);
    let (red, green, blue) = match sector as i32 {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    Color::new(space, red, green, blue, alpha)
}

#[cfg(test)]
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32, alpha: f32) -> Color {
    hsv_to_color(ColorSpace::Srgb, hue, saturation, value, alpha)
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let max = color.red.max(color.green).max(color.blue);
    let min = color.red.min(color.green).min(color.blue);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - color.red).abs() <= f32::EPSILON {
        (((color.green - color.blue) / delta).rem_euclid(6.0)) / 6.0
    } else if (max - color.green).abs() <= f32::EPSILON {
        (((color.blue - color.red) / delta) + 2.0) / 6.0
    } else {
        (((color.red - color.green) / delta) + 4.0) / 6.0
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, saturation, max)
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    let mut builder = PathBuilder::new();
    builder.push_rounded_rect(rect, radius);
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

fn numeric_text_style(mut style: TextStyle) -> TextStyle {
    style.features.enable(FontFeature::TABULAR_FIGURES);
    style
}

fn text_token_style(token: ThemeTextToken, color: Color) -> TextStyle {
    TextStyle {
        font_size: token.size.max(1.0),
        line_height: token.line_height.max(1.0),
        color,
        ..TextStyle::default()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

    use super::{
        ActiveChannel, BrushPreview, BrushPreviewShape, BrushPreviewSpec, ColorPalette,
        ColorPaletteSwatch, ColorPicker, ColorPickerSemanticPart, ColorSwatch, Image, SignalMeter,
        color_picker_child_semantics_id, format_color, hsv_to_rgb, rgb_to_hsv,
    };
    use crate::{DefaultTheme, SemanticTone, ThemeTextToken};
    use sui_core::{
        Color, ColorSpace, Event, ImageHandle, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, Result, SemanticsAction, SemanticsRole, SemanticsValue, Size,
        Vector, WidgetId,
    };
    use sui_runtime::{Application, Runtime, Widget, WindowBuilder};
    use sui_scene::{Brush, RegisteredImage, SceneCommand};
    use sui_text::{FontFeature, FontRegistry, TextSystem};

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Media widgets").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
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
            modifiers: sui_core::Modifiers::NONE,
            pointer_kind: sui_core::PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn handle_ready_events(runtime: &mut Runtime) -> Result<usize> {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (window_id, event) in ready {
            runtime.handle_event(window_id, event)?;
        }
        Ok(count)
    }

    fn solid_fill_colors(output: &sui_runtime::RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                colors.push(*color);
            }
        });
        colors
    }

    fn solid_stroke_colors(output: &sui_runtime::RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::StrokeRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::StrokePath {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                colors.push(*color);
            }
        });
        colors
    }

    fn stroke_path_bounds_with_color_and_width(
        output: &sui_runtime::RenderOutput,
        color: Color,
        width: f32,
    ) -> Vec<Rect> {
        let mut bounds = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::StrokePath {
                path,
                brush: Brush::Solid(stroke_color),
                stroke,
            } = command
            {
                if *stroke_color == color && (stroke.width - width).abs() < 0.01 {
                    bounds.push(path.bounds());
                }
            }
        });
        bounds
    }

    fn text_run_for(output: &sui_runtime::RenderOutput, text: &str) -> sui_text::TextRun {
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

    fn text_visual_center_for(output: &sui_runtime::RenderOutput, text: &str) -> f32 {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::DrawText(run) if run.text == text => {
                    let layout = TextSystem::new()
                        .shape_text_run(run, &FontRegistry::new())
                        .expect("text run should shape");
                    let line = layout
                        .lines()
                        .first()
                        .expect("text run should contain one line");
                    Some(run.rect.y() + line.baseline + optical_visual_center(layout.measurement()))
                }
                SceneCommand::DrawShapedText(run) => {
                    let layout = run.resolve(output.frame.text_layout_registry.as_ref())?;
                    if layout.text() != text {
                        return None;
                    }
                    let line = layout
                        .lines()
                        .first()
                        .expect("shaped text should contain one line");
                    Some(run.origin.y + line.baseline + optical_visual_center(layout.measurement()))
                }
                _ => None,
            })
            .expect("text draw command present")
    }

    fn rect_center(rect: Rect) -> Point {
        Point::new(
            rect.x() + rect.width() * 0.5,
            rect.y() + rect.height() * 0.5,
        )
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
    }

    fn assert_text_run_uses_token(run: &sui_text::TextRun, token: ThemeTextToken) {
        assert_eq!(run.style.font_size, token.size);
        assert_eq!(run.style.line_height, token.line_height);
    }

    #[test]
    fn hsv_round_trip_stays_close() {
        let color = Color::rgba(0.25, 0.65, 0.82, 0.75);
        let (hue, saturation, value) = rgb_to_hsv(color);
        let round_trip = hsv_to_rgb(hue, saturation, value, color.alpha);

        assert!((round_trip.red - color.red).abs() < 0.02);
        assert!((round_trip.green - color.green).abs() < 0.02);
        assert!((round_trip.blue - color.blue).abs() < 0.02);
        assert!((round_trip.alpha - color.alpha).abs() < f32::EPSILON);
    }

    #[test]
    fn image_uses_registered_image_size_and_semantics() -> Result<()> {
        let handle = ImageHandle::new(7);
        let mut application = Application::new();
        application.register_image(
            handle,
            RegisteredImage::from_rgba8(32, 16, vec![255; 32 * 16 * 4])?,
        )?;
        let mut runtime = application
            .window(
                WindowBuilder::new()
                    .title("Image")
                    .root(Image::new(handle).label("Preview")),
            )
            .build()?;
        let window_id = runtime.window_ids()[0];

        let output = runtime.render(window_id)?;
        assert_eq!(output.frame.viewport, Size::new(32.0, 16.0));
        let image = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Image)
            .expect("image semantics present");
        assert_eq!(image.name.as_deref(), Some("Preview"));
        Ok(())
    }

    #[test]
    fn signal_meter_paints_tone_bars_and_semantics() -> Result<()> {
        let theme = DefaultTheme::default();
        let tone = theme.semantic_tone_color(SemanticTone::Success);
        let (mut runtime, window_id) = build_runtime(
            SignalMeter::new("Audio input signal")
                .description("Microphone activity")
                .active(true)
                .tone(SemanticTone::Success)
                .bars(10)
                .size(Size::new(120.0, 18.0))
                .theme(theme),
        );

        let output = runtime.render(window_id)?;
        let fills = solid_fill_colors(&output);
        assert!(
            fills.iter().filter(|color| **color == tone).count() >= 10,
            "active signal meter should paint tone-colored bars"
        );
        let meter = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Audio input signal")
            })
            .expect("signal meter semantics should be present");
        assert_eq!(meter.description.as_deref(), Some("Microphone activity"));
        assert_eq!(
            meter.value.as_ref(),
            Some(&SemanticsValue::Text("active".to_string()))
        );
        Ok(())
    }

    #[test]
    fn image_without_border_omits_frame_stroke() -> Result<()> {
        let handle = ImageHandle::new(8);
        let mut application = Application::new();
        application.register_image(
            handle,
            RegisteredImage::from_rgba8(32, 32, vec![255; 32 * 32 * 4])?,
        )?;
        let mut runtime = application
            .window(
                WindowBuilder::new()
                    .title("Image")
                    .root(Image::new(handle).without_border()),
            )
            .build()?;
        let window_id = runtime.window_ids()[0];

        let output = runtime.render(window_id)?;
        let mut image_draws = 0;
        let mut frame_strokes = 0;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawImage { source, .. } if source.image == handle => {
                    image_draws += 1;
                }
                SceneCommand::StrokeRect { .. } | SceneCommand::StrokePath { .. } => {
                    frame_strokes += 1;
                }
                _ => {}
            });

        assert_eq!(image_draws, 1);
        assert_eq!(frame_strokes, 0);
        Ok(())
    }

    #[test]
    fn color_swatch_click_invokes_callback() -> Result<()> {
        let presses = Rc::new(RefCell::new(Vec::new()));
        let on_press = Rc::clone(&presses);
        let color = Color::rgba(0.2, 0.4, 0.8, 1.0);
        let (mut runtime, window_id) = build_runtime(
            ColorSwatch::new("Accent", color)
                .on_press(move |color| on_press.borrow_mut().push(color)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(16.0, 16.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(16.0, 16.0), false),
        )?;

        assert_eq!(presses.borrow().as_slice(), &[color]);

        let output = runtime.render(window_id)?;
        let swatch = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorSwatch)
            .expect("color swatch semantics present");
        assert_eq!(
            swatch.value,
            Some(SemanticsValue::Text("#3366CCFF".to_string()))
        );
        Ok(())
    }

    #[test]
    fn color_swatch_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = theme.palette.control_hover;
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) =
            build_runtime(ColorSwatch::new("Accent", Color::rgba(0.2, 0.4, 0.8, 1.0)).theme(theme));

        let output = runtime.render(window_id)?;
        let swatch = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorSwatch)
            .expect("color swatch semantics present");
        let position = rect_center(swatch.bounds);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, position, false),
        )?;
        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "swatch hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "swatch press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    #[test]
    fn color_swatch_read_only_color_when_syncs_external_value() -> Result<()> {
        let color = Rc::new(RefCell::new(Color::rgba(0.08, 0.22, 0.78, 1.0)));
        let color_reader = Rc::clone(&color);
        let (mut runtime, window_id) = build_runtime(
            ColorSwatch::new("Current brush color", *color.borrow())
                .color_when(move || *color_reader.borrow())
                .read_only(),
        );

        let output = runtime.render(window_id)?;
        let swatch = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some("Current brush color")
            })
            .expect("current color swatch semantics present");
        assert_eq!(
            swatch.value,
            Some(SemanticsValue::Text("#1438C7FF".to_string()))
        );
        assert!(swatch.actions.is_empty());

        *color.borrow_mut() = Color::rgba(0.90, 0.32, 0.18, 1.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(8.0, 8.0), false),
        )?;
        let output = runtime.render(window_id)?;
        let swatch = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch
                    && node.name.as_deref() == Some("Current brush color")
            })
            .expect("current color swatch semantics still present");
        assert_eq!(
            swatch.value,
            Some(SemanticsValue::Text("#E6522EFF".to_string()))
        );
        Ok(())
    }

    #[test]
    fn color_palette_exposes_selected_swatch_semantics() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            ColorPalette::new("Brush palette")
                .swatches([
                    ColorPaletteSwatch::new("Ink", Color::rgba(0.08, 0.10, 0.15, 1.0)),
                    ColorPaletteSwatch::new("Ocean", Color::rgba(0.08, 0.22, 0.78, 1.0)),
                    ColorPaletteSwatch::new("Mint", Color::rgba(0.28, 0.78, 0.58, 1.0)),
                ])
                .selected(1),
        );
        let output = runtime.render(window_id)?;

        let palette = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Brush palette")
            })
            .expect("palette semantics present");
        assert_eq!(
            palette.value,
            Some(SemanticsValue::Text("Ocean #1438C7FF".to_string()))
        );

        let selected = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("Ocean")
            })
            .expect("selected swatch semantics present");
        assert!(selected.state.selected);
        assert_eq!(
            selected.value,
            Some(SemanticsValue::Text("#1438C7FF".to_string()))
        );
        Ok(())
    }

    #[test]
    fn media_widget_defaults_follow_theme_density() -> Result<()> {
        let (mut compact_runtime, compact_window) = build_runtime(
            ColorSwatch::new("Accent", Color::rgba(0.2, 0.4, 0.8, 1.0))
                .theme(DefaultTheme::compact()),
        );
        let compact = compact_runtime.render(compact_window)?.frame.viewport;

        let (mut touch_runtime, touch_window) = build_runtime(
            ColorSwatch::new("Accent", Color::rgba(0.2, 0.4, 0.8, 1.0))
                .theme(DefaultTheme::touch()),
        );
        let touch = touch_runtime.render(touch_window)?.frame.viewport;

        assert_eq!(
            compact,
            Size::new(
                DefaultTheme::compact().metrics.color_swatch_width,
                DefaultTheme::compact().metrics.color_swatch_height,
            )
        );
        assert_eq!(
            touch,
            Size::new(
                DefaultTheme::touch().metrics.color_swatch_width,
                DefaultTheme::touch().metrics.color_swatch_height,
            )
        );
        assert!(touch.width > compact.width);
        assert!(touch.height > compact.height);
        Ok(())
    }

    #[test]
    fn color_palette_defaults_and_overrides_are_density_aware() -> Result<()> {
        let swatches = [
            ColorPaletteSwatch::new("Ink", Color::rgba(0.08, 0.10, 0.15, 1.0)),
            ColorPaletteSwatch::new("Ocean", Color::rgba(0.08, 0.22, 0.78, 1.0)),
        ];
        let (mut compact_runtime, compact_window) = build_runtime(
            ColorPalette::new("Brush palette")
                .theme(DefaultTheme::compact())
                .swatches(swatches.clone()),
        );
        let compact = compact_runtime.render(compact_window)?.frame.viewport;

        let (mut touch_runtime, touch_window) = build_runtime(
            ColorPalette::new("Brush palette")
                .theme(DefaultTheme::touch())
                .swatches(swatches.clone()),
        );
        let touch = touch_runtime.render(touch_window)?.frame.viewport;

        assert!(touch.width > compact.width);
        assert_eq!(
            compact.height,
            DefaultTheme::compact().metrics.color_palette_swatch_size
        );

        let (mut override_runtime, override_window) = build_runtime(
            ColorPalette::new("Brush palette")
                .theme(DefaultTheme::touch())
                .swatches(swatches)
                .swatch_size(18.0)
                .gap(2.0),
        );
        let overridden = override_runtime.render(override_window)?.frame.viewport;
        assert_eq!(overridden, Size::new(38.0, 18.0));
        Ok(())
    }

    #[test]
    fn color_palette_click_invokes_callback_and_selects_swatch() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_writer = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ColorPalette::new("Brush palette")
                .swatches([
                    ColorPaletteSwatch::new("Ink", Color::rgba(0.08, 0.10, 0.15, 1.0)),
                    ColorPaletteSwatch::new("Ocean", Color::rgba(0.08, 0.22, 0.78, 1.0)),
                    ColorPaletteSwatch::new("Mint", Color::rgba(0.28, 0.78, 0.58, 1.0)),
                ])
                .selected(0)
                .on_change(move |index, name, color| {
                    changes_writer.borrow_mut().push((index, name, color));
                }),
        );
        let output = runtime.render(window_id)?;
        let mint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("Mint")
            })
            .expect("target swatch semantics present");
        let position = Point::new(
            mint.bounds.x() + mint.bounds.width() * 0.5,
            mint.bounds.y() + mint.bounds.height() * 0.5,
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;

        assert_eq!(
            changes.borrow().as_slice(),
            &[(2, "Mint".to_string(), Color::rgba(0.28, 0.78, 0.58, 1.0))]
        );
        let output = runtime.render(window_id)?;
        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::ColorSwatch
                && node.name.as_deref() == Some("Mint")
                && node.state.selected
        }));
        Ok(())
    }

    #[test]
    fn color_palette_hover_and_press_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = super::mix_color(
            theme.palette.control,
            theme.palette.control_hover,
            theme.interaction.hover_blend,
        );
        let expected_press = super::mix_color(
            expected_hover,
            theme.palette.control_active,
            theme.interaction.pressed_blend,
        );
        let (mut runtime, window_id) =
            build_runtime(ColorPalette::new("Brush palette").theme(theme).swatches([
                ColorPaletteSwatch::new("Ink", Color::rgba(0.08, 0.10, 0.15, 1.0)),
                ColorPaletteSwatch::new("Ocean", Color::rgba(0.08, 0.22, 0.78, 1.0)),
                ColorPaletteSwatch::new("Mint", Color::rgba(0.28, 0.78, 0.58, 1.0)),
            ]));
        let output = runtime.render(window_id)?;
        let mint = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("Mint")
            })
            .expect("target swatch semantics present");
        let position = rect_center(mint.bounds);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, position, false),
        )?;
        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "palette swatch hover fill should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_press = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_press).contains(&expected_press),
            "palette swatch press fill should not snap to the settled pressed color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_press = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_press).contains(&expected_press));

        Ok(())
    }

    #[test]
    fn brush_preview_exposes_current_brush_semantics_and_paints_sample() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            BrushPreview::new("Brush preview")
                .spec(BrushPreviewSpec::new(
                    Color::rgba(0.10, 0.30, 0.90, 1.0),
                    22.0,
                    0.75,
                    BrushPreviewShape::Square,
                ))
                .size(Size::new(220.0, 64.0)),
        );

        let output = runtime.render(window_id)?;
        let preview = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Image && node.name.as_deref() == Some("Brush preview")
            })
            .expect("brush preview semantics present");
        assert_eq!(
            preview.value,
            Some(SemanticsValue::Text(
                "Square brush, 22 px, 75% opacity".to_string(),
            ))
        );

        let mut fill_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            if matches!(
                command,
                SceneCommand::FillRect { .. } | SceneCommand::FillPath { .. }
            ) {
                fill_count += 1;
            }
        });
        assert!(fill_count > 3);

        let value_text = "Square brush, 22 px, 75% opacity";
        let text = text_run_for(&output, value_text);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("brush preview value should shape");
        let line = layout
            .lines()
            .first()
            .expect("brush preview value should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let theme = DefaultTheme::default();
        let metrics = theme.metrics;
        let bounds = Rect::new(0.0, 0.0, 220.0, 64.0);
        let content = super::inset_rect(bounds, metrics.brush_preview_padding);
        let swatch_width = metrics.brush_preview_swatch_width.min(content.width());
        let sample = Rect::new(
            content.x() + swatch_width + metrics.brush_preview_swatch_gap,
            content.y(),
            (content.width() - swatch_width - metrics.brush_preview_swatch_gap).max(0.0),
            content.height(),
        );
        let text_slot = Rect::new(
            sample.x(),
            sample.max_y() - metrics.brush_preview_text_height,
            sample.width(),
            metrics.brush_preview_text_height,
        );
        let slot_center = text_slot.y() + (text_slot.height() * 0.5);

        assert!((actual_visual_center - slot_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn brush_preview_value_preserves_tall_measurement_and_slot_centering() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.metrics.brush_preview_text_font_size = 28.0;
        theme.metrics.brush_preview_text_line_height = 10.0;
        theme.metrics.brush_preview_text_height = 44.0;
        theme.metrics.brush_preview_min_width = 420.0;
        theme.metrics.brush_preview_min_height = 96.0;
        let metrics = theme.metrics;
        let value_text = "Round brush, 8 px, 100% opacity";

        let (mut runtime, window_id) = build_runtime(
            BrushPreview::new("Brush preview")
                .theme(theme)
                .spec(BrushPreviewSpec::new(
                    Color::rgba(0.10, 0.30, 0.90, 1.0),
                    8.0,
                    1.0,
                    BrushPreviewShape::Round,
                ))
                .size(Size::new(420.0, 96.0)),
        );
        let output = runtime.render(window_id)?;
        let text = text_run_for(&output, value_text);
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("brush preview value should shape");
        let line = layout
            .lines()
            .first()
            .expect("brush preview value should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let bounds = Rect::new(0.0, 0.0, 420.0, 96.0);
        let content = super::inset_rect(bounds, metrics.brush_preview_padding);
        let swatch_width = metrics.brush_preview_swatch_width.min(content.width());
        let sample = Rect::new(
            content.x() + swatch_width + metrics.brush_preview_swatch_gap,
            content.y(),
            (content.width() - swatch_width - metrics.brush_preview_swatch_gap).max(0.0),
            content.height(),
        );
        let text_slot = Rect::new(
            sample.x(),
            sample.max_y() - metrics.brush_preview_text_height,
            sample.width(),
            metrics.brush_preview_text_height,
        );
        let slot_center = text_slot.y() + (text_slot.height() * 0.5);

        assert_eq!(text.style.font_size, 28.0);
        assert_eq!(text.style.line_height, 10.0);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text.rect.x() - text_slot.x()).abs() < 0.75);
        assert!((actual_visual_center - slot_center).abs() < 0.75);
        Ok(())
    }

    #[test]
    fn color_picker_pointer_drag_updates_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::new("Accent picker")
                .on_change(move |color| on_change.borrow_mut().push(color)),
        );

        let initial = runtime.render(window_id)?;
        let map = initial
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Saturation and value"))
            .expect("saturation/value semantics present")
            .bounds;
        let start = rect_center(map);
        let end = Point::new(map.x() + map.width() * 0.80, map.y() + map.height() * 0.35);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let changed_color = *changes
            .borrow()
            .last()
            .expect("color picker emitted change");
        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present");
        assert_eq!(
            picker.value,
            Some(SemanticsValue::Text(format_color(changed_color)))
        );
        Ok(())
    }

    #[test]
    fn color_picker_focus_surface_uses_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let (mut runtime, window_id) = build_runtime(ColorPicker::new("Accent picker"));
        let output = runtime.render(window_id)?;
        let focus_target = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Saturation and value"))
            .expect("color picker saturation/value semantics present")
            .bounds;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, rect_center(focus_target), true),
        )?;
        let _ = runtime.render(window_id)?;

        runtime.tick(focus_duration * 0.5);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let mid = runtime.render(window_id)?;
        assert!(
            !solid_stroke_colors(&mid).contains(&theme.palette.focus_ring),
            "color picker focus ring should not snap to the settled focus color"
        );

        runtime.tick(focus_duration);
        assert!(handle_ready_events(&mut runtime)? >= 1);
        let settled = runtime.render(window_id)?;
        assert!(
            solid_stroke_colors(&settled).contains(&theme.palette.focus_ring),
            "color picker focus ring should settle to the theme focus color"
        );

        Ok(())
    }

    #[test]
    fn color_picker_color_when_syncs_external_color() -> Result<()> {
        let color = Rc::new(RefCell::new(Color::rgba(0.08, 0.22, 0.78, 1.0)));
        let color_reader = Rc::clone(&color);
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::from_color("Brush color", *color.borrow())
                .color_when(move || *color_reader.borrow()),
        );

        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present");
        assert_eq!(
            picker.value,
            Some(SemanticsValue::Text("#1438C7FF".to_string()))
        );

        *color.borrow_mut() = Color::rgba(0.90, 0.32, 0.18, 1.0);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(8.0, 8.0), false),
        )?;
        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics still present");
        assert_eq!(
            picker.value,
            Some(SemanticsValue::Text("#E6522EFF".to_string()))
        );
        Ok(())
    }

    #[test]
    fn color_picker_from_color_preserves_hdr_linear_color_space() {
        let picker = ColorPicker::from_color(
            "HDR accent",
            Color::new(ColorSpace::LinearSrgb, 2.0, 0.5, 0.25, 1.0),
        );

        assert_eq!(picker.color().space, ColorSpace::LinearSrgb);
        assert!((picker.color().red - 2.0).abs() < f32::EPSILON);
        assert!((picker.color().green - 0.5).abs() < f32::EPSILON);
        assert!((picker.color().blue - 0.25).abs() < f32::EPSILON);
        assert!((picker.color().alpha - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_picker_semantics_describe_hdr_editing_mode() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(ColorPicker::from_color(
            "HDR accent",
            Color::new(ColorSpace::LinearSrgb, 2.0, 0.5, 0.25, 1.0),
        ));

        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present");

        assert_eq!(
            picker.description.as_deref(),
            Some("BT709 Linear editing space; HDR range available")
        );
        Ok(())
    }

    #[test]
    fn color_picker_semantics_expose_accessible_components() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(ColorPicker::from_color(
            "Accent picker",
            Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
        ));

        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present");
        let picker_id = picker.id;

        let child = |role: SemanticsRole, name: &str| {
            output.semantics.iter().find(|node| {
                node.parent == Some(picker_id)
                    && node.role == role
                    && node.name.as_deref() == Some(name)
            })
        };

        assert!(child(SemanticsRole::ColorSwatch, "Current color").is_some());
        assert!(child(SemanticsRole::ColorSwatch, "Previous color").is_some());

        let range = child(SemanticsRole::ComboBox, "Color range")
            .expect("color range selector semantics present");
        assert_eq!(
            range.value,
            Some(SemanticsValue::Text("BT709 Linear".to_string()))
        );
        assert_eq!(range.state.expanded, Some(false));

        for name in [
            "Saturation and value",
            "Hue",
            "Saturation",
            "Value",
            "Alpha",
            "Red",
            "Green",
            "Blue",
        ] {
            let slider = child(SemanticsRole::Slider, name)
                .unwrap_or_else(|| panic!("{name} slider semantics present"));
            assert!(
                matches!(slider.value, Some(SemanticsValue::Range { .. })),
                "{name} slider should expose a range value"
            );
        }

        assert!(
            output.semantics.iter().any(|node| {
                node.parent == Some(picker_id)
                    && node.role == SemanticsRole::Text
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("HDR hex unavailable"))
            }),
            "HDR hex field semantics present"
        );
        Ok(())
    }

    #[test]
    fn color_picker_synthetic_semantics_ids_are_javascript_safe_and_distinct() {
        let parent = WidgetId::new(402);
        let parts = [
            ColorPickerSemanticPart::CurrentColor,
            ColorPickerSemanticPart::PreviousColor,
            ColorPickerSemanticPart::ColorRange,
            ColorPickerSemanticPart::ColorRangeMenu,
            ColorPickerSemanticPart::ColorRangeOption(0),
            ColorPickerSemanticPart::ColorRangeOption(1),
            ColorPickerSemanticPart::ColorRangeOption(2),
            ColorPickerSemanticPart::ColorRangeOption(3),
            ColorPickerSemanticPart::SaturationValue,
            ColorPickerSemanticPart::Hue,
            ColorPickerSemanticPart::Saturation,
            ColorPickerSemanticPart::Value,
            ColorPickerSemanticPart::Alpha,
            ColorPickerSemanticPart::Red,
            ColorPickerSemanticPart::Green,
            ColorPickerSemanticPart::Blue,
            ColorPickerSemanticPart::Hex,
        ];
        let mut ids = BTreeSet::new();
        for part in parts {
            let id = color_picker_child_semantics_id(parent, part).get();
            assert!(id <= (1_u64 << 53) - 1, "{id} should be JS-safe");
            assert!(ids.insert(id), "{id} should be unique");
        }
    }

    #[test]
    fn color_picker_columns_are_compact() {
        let picker = ColorPicker::new("Accent picker");
        let bounds = Rect::new(0.0, 0.0, 434.0, 448.0);
        let wheel = picker.color_wheel_rect(bounds);
        let map = picker.saturation_value_rect(bounds);
        let metrics = DefaultTheme::default().metrics;

        assert_eq!(map.x() - wheel.max_x(), metrics.color_picker_panel_gap);
        assert_eq!(map.width(), metrics.color_picker_map_size);
        assert_eq!(map.y(), wheel.y());
    }

    #[test]
    fn color_picker_compact_mode_uses_smaller_measurement() {
        let regular = ColorPicker::new("Accent picker")
            .theme(DefaultTheme::comfortable())
            .desired_size();
        let compact = ColorPicker::new("Accent picker")
            .compact(true)
            .show_alpha(false)
            .desired_size();
        let metrics = DefaultTheme::compact().metrics;

        assert_eq!(
            compact.width,
            metrics.color_picker_wheel_size
                + metrics.color_picker_panel_gap
                + metrics.color_picker_right_panel_width
                + metrics.color_picker_content_inset * 2.0
        );
        assert!(compact.width < regular.width);
        assert!(compact.height < regular.height);
    }

    #[test]
    fn color_picker_theme_density_changes_default_measurement() {
        let compact = ColorPicker::new("Accent picker")
            .theme(DefaultTheme::compact())
            .desired_size();
        let comfortable = ColorPicker::new("Accent picker")
            .theme(DefaultTheme::comfortable())
            .desired_size();
        let touch = ColorPicker::new("Accent picker")
            .theme(DefaultTheme::touch())
            .desired_size();

        assert!(compact.width < comfortable.width);
        assert!(compact.height < comfortable.height);
        assert!(touch.width > comfortable.width);
        assert!(touch.height > comfortable.height);
    }

    #[test]
    fn color_picker_chrome_uses_theme_surface_tokens() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.surfaces.checkerboard_light = Color::rgba(0.91, 0.86, 0.78, 1.0);
        theme.surfaces.checkerboard_dark = Color::rgba(0.66, 0.58, 0.48, 1.0);
        theme.surfaces.color_picker_chrome_border = Color::rgba(0.20, 0.30, 0.42, 0.61);
        theme.surfaces.color_picker_plane_border = Color::rgba(0.30, 0.20, 0.46, 0.62);
        theme.surfaces.color_picker_bar_border = Color::rgba(0.42, 0.25, 0.18, 0.63);
        theme.surfaces.color_picker_marker_outer = Color::rgba(0.98, 0.96, 0.90, 0.94);
        theme.surfaces.color_picker_marker_dark = Color::rgba(0.05, 0.07, 0.10, 0.88);
        theme.surfaces.color_picker_sdr_marker = Color::rgba(0.95, 0.92, 0.84, 0.38);

        let (mut runtime, window_id) = build_runtime(
            ColorPicker::from_color("Accent picker", Color::rgba(0.84, 0.72, 0.18, 1.0))
                .theme(theme),
        );
        let output = runtime.render(window_id)?;
        let fills = solid_fill_colors(&output);
        let strokes = solid_stroke_colors(&output);

        assert!(fills.contains(&theme.surfaces.checkerboard_light));
        assert!(fills.contains(&theme.surfaces.checkerboard_dark));
        assert!(fills.contains(&theme.surfaces.color_picker_sdr_marker));
        assert!(strokes.contains(&theme.surfaces.color_picker_chrome_border));
        assert!(strokes.contains(&theme.surfaces.color_picker_plane_border));
        assert!(strokes.contains(&theme.surfaces.color_picker_bar_border));
        assert!(strokes.contains(&theme.surfaces.color_picker_marker_outer));
        assert!(strokes.contains(&theme.surfaces.color_picker_marker_dark));
        Ok(())
    }

    #[test]
    fn color_picker_header_contains_encoding_selector() {
        let picker = ColorPicker::new("Accent picker");
        let bounds = Rect::new(0.0, 0.0, 434.0, 448.0);
        let header = picker.header_rect(bounds);
        let encoding = picker.encoding_rect(bounds);
        let previous = picker.previous_swatch_rect(bounds);
        let map = picker.saturation_value_rect(bounds);
        let rgb = picker.rgb_row_rect(bounds, 0);

        assert!(header.contains(encoding.origin));
        assert!(header.contains(Point::new(encoding.max_x(), encoding.max_y())));
        assert!(encoding.x() >= previous.max_x() + picker.panel_gap() - 0.01);
        assert!(encoding.max_x() <= header.max_x() + 0.01);
        assert!(rgb.y() > map.max_y());
    }

    #[test]
    fn color_picker_encoding_chevron_is_centered_in_selector() -> Result<()> {
        let picker = ColorPicker::from_color("Accent picker", Color::rgba(0.25, 0.50, 0.75, 0.80));
        let (mut runtime, window_id) = build_runtime(picker);
        let output = runtime.render(window_id)?;
        let bounds = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            output.frame.viewport.height,
        );
        let layout_picker =
            ColorPicker::from_color("Accent picker", Color::rgba(0.25, 0.50, 0.75, 0.80));
        let encoding = layout_picker.encoding_rect(bounds);
        let theme = DefaultTheme::default();
        let chevron =
            stroke_path_bounds_with_color_and_width(&output, theme.palette.placeholder, 1.4)
                .into_iter()
                .find(|rect| {
                    encoding.contains(Point::new(
                        rect.x() + rect.width() * 0.5,
                        rect.y() + rect.height() * 0.5,
                    )) && (rect.width() - 8.0).abs() < 0.75
                        && (rect.height() - 5.0).abs() < 0.75
                })
                .expect("encoding selector chevron stroke should render");
        let chevron_center_y = chevron.y() + chevron.height() * 0.5;
        let selector_center_y = encoding.y() + encoding.height() * 0.5;

        assert!(
            (chevron_center_y - selector_center_y).abs() < 0.75,
            "encoding chevron center {chevron_center_y} did not match selector center {selector_center_y}; chevron={chevron:?}, selector={encoding:?}"
        );
        Ok(())
    }

    #[test]
    fn color_picker_saturation_value_plane_uses_hdr_slider_curve() {
        let mut picker = ColorPicker::from_color(
            "Accent picker",
            Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
        );
        let bounds = Rect::new(0.0, 0.0, 434.0, 448.0);
        let map = picker.saturation_value_rect(bounds);

        picker.update_from_position(
            bounds,
            ActiveChannel::SaturationValue,
            Point::new(map.x() + map.width() * 0.5, map.y() + map.height() * 0.25),
        );

        assert!(
            picker.value > 3.0 && picker.value < 4.0,
            "75% HDR slider position should map logarithmically above 1.0, got {}",
            picker.value
        );
    }

    #[test]
    fn color_picker_rgb_row_drag_updates_color_channels() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::from_color(
                "Accent picker",
                Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
            )
            .on_change(move |color| on_change.borrow_mut().push(color)),
        );

        let initial = runtime.render(window_id)?;
        let red = initial
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Red"))
            .expect("red channel semantics present")
            .bounds;
        let start = Point::new(red.x() + red.width() * 0.20, red.y() + red.height() * 0.5);
        let end = Point::new(red.x() + red.width() * 0.80, red.y() + red.height() * 0.5);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let changed_color = *changes.borrow().last().expect("rgb row emitted change");
        assert!(
            changed_color.red > 8.0,
            "expected HDR red channel after RGB drag, got {}",
            changed_color.red
        );
        assert_eq!(changed_color.space, ColorSpace::LinearSrgb);
        Ok(())
    }

    #[test]
    fn color_picker_numeric_rows_use_tabular_figures_and_end_alignment() -> Result<()> {
        let color = Color::rgba(0.25, 0.50, 0.75, 0.80);
        let (mut runtime, window_id) =
            build_runtime(ColorPicker::from_color("Accent picker", color));
        let output = runtime.render(window_id)?;
        let run = text_run_for(&output, "0.250");
        let bounds = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            output.frame.viewport.height,
        );
        let picker = ColorPicker::from_color("Accent picker", color);
        let row = picker.rgb_row_rect(bounds, 0);
        let theme = DefaultTheme::default();
        let expected_right = row.max_x() - theme.spacing;

        assert!(
            run.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!((run.rect.max_x() - expected_right).abs() < 1.0);
        assert!(run.rect.height() <= theme.text.xs.line_height + 0.01);
        Ok(())
    }

    #[test]
    fn color_picker_numeric_rows_preserve_tall_measurements_and_row_center() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 28.0,
            line_height: 12.0,
        };
        theme.metrics.color_picker_row_height = 48.0;
        let color = Color::rgba(0.25, 0.50, 0.75, 0.80);
        let (mut runtime, window_id) =
            build_runtime(ColorPicker::from_color("Accent picker", color).theme(theme));
        let output = runtime.render(window_id)?;
        let label = text_run_for(&output, "R");
        let value = text_run_for(&output, "0.250");
        let layout = TextSystem::new()
            .shape_text_run(&value, &FontRegistry::new())
            .expect("color picker numeric value should shape");
        let bounds = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            output.frame.viewport.height,
        );
        let picker = ColorPicker::from_color("Accent picker", color).theme(theme);
        let row = picker.rgb_row_rect(bounds, 0);
        let expected_right = row.max_x() - theme.spacing;
        let row_center = row.y() + row.height() * 0.5;
        let value_center = text_visual_center_for(&output, "0.250");

        assert!((value.rect.max_x() - expected_right).abs() < 1.0);
        assert!(
            (value_center - row_center).abs() < 0.75,
            "color picker numeric value center {value_center} did not match row center {row_center}; value rect {:?}, row {:?}, measurement {:?}",
            value.rect,
            row,
            layout.measurement()
        );
        let label_center = text_visual_center_for(&output, "R");
        let label_layout = TextSystem::new()
            .shape_text_run(&label, &FontRegistry::new())
            .expect("color picker channel label should shape");
        assert!(
            (value_center - label_center).abs() < 0.75,
            "color picker numeric value center {value_center} and channel label center {label_center} should share a visual baseline; label rect {:?}, label measurement {:?}",
            label.rect,
            label_layout.measurement()
        );
        Ok(())
    }

    #[test]
    fn color_picker_text_styles_follow_theme_xs_token() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 13.5,
            line_height: 31.0,
        };
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::from_color("Accent picker", Color::rgba(0.25, 0.50, 0.75, 0.80))
                .theme(theme),
        );
        let output = runtime.render(window_id)?;
        let label = text_run_for(&output, "R");
        let value = text_run_for(&output, "0.250");

        assert_text_run_uses_token(&label, theme.text.xs);
        assert_text_run_uses_token(&value, theme.text.xs);
        assert!(
            value
                .style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        Ok(())
    }

    #[test]
    fn color_picker_dropdown_and_fields_preserve_tall_text_measurements() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 28.0,
            line_height: 10.0,
        };
        theme.metrics.color_picker_top_bar_height = 56.0;
        theme.metrics.color_picker_field_height = 46.0;
        theme.metrics.color_picker_encoding_menu_row_height = 46.0;
        let color = Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0);
        let (mut runtime, window_id) =
            build_runtime(ColorPicker::from_color("Accent picker", color).theme(theme));

        let initial = runtime.render(window_id)?;
        let color_range = initial
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Color range"))
            .expect("color range semantics present")
            .bounds;
        let color_range_center = rect_center(color_range);
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, color_range_center, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, color_range_center, false),
        )?;

        let output = runtime.render(window_id)?;
        let bounds = Rect::new(
            0.0,
            0.0,
            output.frame.viewport.width,
            output.frame.viewport.height,
        );
        let picker = ColorPicker::from_color("Accent picker", color).theme(theme);
        let selector = picker.encoding_rect(bounds);
        let menu_item = picker.encoding_option_rect(bounds, 0);
        let hex = picker.hex_rect(bounds);
        let selector_run = text_run_for(&output, "BT709 Linear");
        let menu_run = text_run_for(&output, "sRGB");
        let hex_run = text_run_for(&output, "HDR hex unavailable");
        assert_text_run_uses_token(&selector_run, theme.text.xs);
        assert_text_run_uses_token(&menu_run, theme.text.xs);
        assert_text_run_uses_token(&hex_run, theme.text.xs);
        assert_eq!(hex_run.style.color, theme.palette.placeholder);
        assert!(
            (text_visual_center_for(&output, "BT709 Linear") - rect_center(selector).y).abs()
                < 0.75
        );
        assert!((text_visual_center_for(&output, "sRGB") - rect_center(menu_item).y).abs() < 0.75);
        assert!(
            (text_visual_center_for(&output, "HDR hex unavailable") - rect_center(hex).y).abs()
                < 0.75
        );
        Ok(())
    }

    #[test]
    fn color_picker_encoding_selector_uses_dropdown_options() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::from_color(
                "Accent picker",
                Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
            )
            .on_change(move |color| on_change.borrow_mut().push(color)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(300.0, 40.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(300.0, 40.0), false),
        )?;
        assert!(
            changes.borrow().is_empty(),
            "opening the encoding dropdown should not change color space"
        );
        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present after opening encoding dropdown");
        let range = output
            .semantics
            .iter()
            .find(|node| {
                node.parent == Some(picker.id)
                    && node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some("Color range")
            })
            .expect("color range selector semantics present after opening dropdown");
        assert_eq!(range.state.expanded, Some(true));
        assert!(
            output.semantics.iter().any(|node| {
                node.role == SemanticsRole::MenuItem
                    && node.name.as_deref() == Some("Display P3")
                    && node.actions.contains(&SemanticsAction::Activate)
            }),
            "open color range dropdown should expose Display P3 option"
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(300.0, 129.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(300.0, 129.0), false),
        )?;

        let changed_color = *changes
            .borrow()
            .last()
            .expect("encoding selector emitted change");
        assert_eq!(changed_color.space, ColorSpace::DisplayP3);

        let output = runtime.render(window_id)?;
        let picker = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::ColorPicker)
            .expect("color picker semantics present after encoding change");
        assert_eq!(
            picker.description.as_deref(),
            Some("Display P3 editing space; HDR range available")
        );
        Ok(())
    }
}
