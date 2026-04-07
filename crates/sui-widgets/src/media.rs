use sui_core::{
    Color, Event, ImageHandle, KeyState, Path, PathBuilder, Point, PointerButton, PointerEventKind,
    Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::{ImageSource, StrokeStyle};
use sui_text::TextStyle;

use crate::DefaultTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    Fill,
    Contain,
    Cover,
    None,
}

pub struct Image {
    theme: Box<DefaultTheme>,
    image: ImageHandle,
    label: Option<String>,
    fit: ImageFit,
    width: Option<f32>,
    height: Option<f32>,
    aspect_ratio: Option<f32>,
    source_rect: Option<Rect>,
    tint: Option<Color>,
    background: Option<Color>,
    corner_radius: f32,
    resolved_source_size: Size,
}

impl Image {
    pub fn new(image: ImageHandle) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            image,
            label: None,
            fit: ImageFit::Contain,
            width: None,
            height: None,
            aspect_ratio: None,
            source_rect: None,
            tint: None,
            background: None,
            corner_radius: 10.0,
            resolved_source_size: Size::new(96.0, 96.0),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self
    }

    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius.max(0.0);
        self
    }

    fn effective_source_size(&self, ctx: &MeasureCtx) -> Size {
        self.source_rect
            .map(|rect| rect.size)
            .or_else(|| ctx.image_size(self.image))
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
}

impl Widget for Image {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.resolved_source_size = self.effective_source_size(ctx);
        constraints.clamp(self.resolved_size())
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let image_rect = fit_rect(bounds, self.resolved_source_size, self.fit);
        if let Some(background) = self.background {
            ctx.fill(rounded_rect_path(bounds, self.corner_radius), background);
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

        ctx.stroke(
            rounded_rect_path(bounds, self.corner_radius),
            self.theme.palette.border,
            StrokeStyle::new(self.theme.metrics.border_width.max(1.0)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Image, ctx.bounds());
        node.name = self.label.clone();
        ctx.push(node);
    }
}

pub struct ColorSwatch {
    theme: Box<DefaultTheme>,
    name: String,
    color: Color,
    width: f32,
    height: f32,
    hovered: bool,
    pressed: bool,
    on_press: Option<Box<dyn FnMut(Color)>>,
}

impl ColorSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            color,
            width: 56.0,
            height: 32.0,
            hovered: false,
            pressed: false,
            on_press: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.width = size.width.max(24.0);
        self.height = size.height.max(24.0);
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut(Color) + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    fn activate(&mut self) {
        if let Some(on_press) = &mut self.on_press {
            on_press(self.color);
        }
    }
}

impl Widget for ColorSwatch {
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
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.pressed = true;
                self.hovered = true;
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
                let hovered = ctx.bounds().contains(pointer.position);
                if self.pressed && hovered {
                    self.activate();
                }
                self.pressed = false;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.hovered {
                    self.hovered = false;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
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
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.width, self.height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let outer_radius = self.theme.metrics.corner_radius;
        let inner_radius = (outer_radius - 1.0).max(0.0);
        draw_checkerboard(ctx, ctx.bounds(), 6.0);
        ctx.fill(
            rounded_rect_path(inset_rect(ctx.bounds(), Insets::all(1.0)), inner_radius),
            self.color,
        );
        ctx.stroke(
            rounded_rect_path(ctx.bounds(), outer_radius),
            if ctx.is_focused() || self.hovered {
                self.theme.palette.border_focus
            } else {
                self.theme.palette.border
            },
            StrokeStyle::new(self.theme.metrics.border_width.max(1.0)),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorSwatch, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(format_color(self.color)));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveChannel {
    SaturationValue,
    Hue,
    Alpha,
}

pub struct ColorPicker {
    theme: Box<DefaultTheme>,
    name: String,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: f32,
    show_alpha: bool,
    active: Option<ActiveChannel>,
    on_change: Option<Box<dyn FnMut(Color)>>,
}

impl ColorPicker {
    pub fn new(name: impl Into<String>) -> Self {
        Self::from_color(name, Color::rgba(0.11, 0.43, 0.92, 1.0))
    }

    pub fn from_color(name: impl Into<String>, color: Color) -> Self {
        let (hue, saturation, value) = rgb_to_hsv(color);
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            hue,
            saturation,
            value,
            alpha: color.alpha,
            show_alpha: true,
            active: None,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn show_alpha(mut self, show_alpha: bool) -> Self {
        self.show_alpha = show_alpha;
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(Color) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn color(&self) -> Color {
        hsv_to_rgb(self.hue, self.saturation, self.value, self.alpha)
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, Insets::all(12.0))
    }

    fn preview_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        Rect::new(content.x(), content.y(), content.width(), 42.0)
    }

    fn saturation_value_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let y = self.preview_rect(bounds).max_y() + 10.0;
        let height = if self.show_alpha { 132.0 } else { 154.0 };
        Rect::new(content.x(), y, content.width(), height)
    }

    fn hue_rect(&self, bounds: Rect) -> Rect {
        let sv = self.saturation_value_rect(bounds);
        Rect::new(sv.x(), sv.max_y() + 10.0, sv.width(), 14.0)
    }

    fn alpha_rect(&self, bounds: Rect) -> Rect {
        let hue = self.hue_rect(bounds);
        Rect::new(hue.x(), hue.max_y() + 10.0, hue.width(), 14.0)
    }

    fn update_from_position(&mut self, bounds: Rect, channel: ActiveChannel, position: Point) {
        match channel {
            ActiveChannel::SaturationValue => {
                let rect = self.saturation_value_rect(bounds);
                let saturation = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
                let value = (1.0 - ((position.y - rect.y()) / rect.height())).clamp(0.0, 1.0);
                self.saturation = saturation;
                self.value = value;
            }
            ActiveChannel::Hue => {
                let rect = self.hue_rect(bounds);
                self.hue = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
            }
            ActiveChannel::Alpha => {
                let rect = self.alpha_rect(bounds);
                self.alpha = ((position.x - rect.x()) / rect.width()).clamp(0.0, 1.0);
            }
        }

        let color = self.color();
        if let Some(on_change) = &mut self.on_change {
            on_change(color);
        }
    }

    fn hit_channel(&self, bounds: Rect, position: Point) -> Option<ActiveChannel> {
        if self.saturation_value_rect(bounds).contains(position) {
            Some(ActiveChannel::SaturationValue)
        } else if self.hue_rect(bounds).contains(position) {
            Some(ActiveChannel::Hue)
        } else if self.show_alpha && self.alpha_rect(bounds).contains(position) {
            Some(ActiveChannel::Alpha)
        } else {
            None
        }
    }
}

impl Widget for ColorPicker {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
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
                let active = self.hit_channel(ctx.bounds(), pointer.position);
                if let Some(active) = active {
                    self.active = Some(active);
                    self.update_from_position(ctx.bounds(), active, pointer.position);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self.active.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.active.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let step = if key.modifiers.shift { 0.1 } else { 0.02 };
                match key.key.as_str() {
                    "ArrowLeft" => self.saturation = (self.saturation - step).clamp(0.0, 1.0),
                    "ArrowRight" => self.saturation = (self.saturation + step).clamp(0.0, 1.0),
                    "ArrowUp" => self.value = (self.value + step).clamp(0.0, 1.0),
                    "ArrowDown" => self.value = (self.value - step).clamp(0.0, 1.0),
                    _ => return,
                }
                let color = self.color();
                if let Some(on_change) = &mut self.on_change {
                    on_change(color);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let desired = Size::new(260.0, if self.show_alpha { 256.0 } else { 232.0 });
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width.max(180.0)
            } else {
                desired.width
            },
            desired.height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let current = self.color();
        let preview = self.preview_rect(ctx.bounds());
        let sv = self.saturation_value_rect(ctx.bounds());
        let hue = self.hue_rect(ctx.bounds());

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
        draw_checkerboard(
            ctx,
            Rect::new(preview.x(), preview.y(), 42.0, preview.height()),
            6.0,
        );
        ctx.fill(
            rounded_rect_path(
                Rect::new(preview.x(), preview.y(), 42.0, preview.height()),
                8.0,
            ),
            current,
        );
        ctx.stroke(
            rounded_rect_path(
                Rect::new(preview.x(), preview.y(), 42.0, preview.height()),
                8.0,
            ),
            palette.border,
            StrokeStyle::new(1.0),
        );
        ctx.draw_text(
            Rect::new(
                preview.x() + 54.0,
                preview.y() + 4.0,
                preview.width() - 54.0,
                16.0,
            ),
            self.name.clone(),
            self.theme.body_text_style(),
        );
        ctx.draw_text(
            Rect::new(
                preview.x() + 54.0,
                preview.y() + 22.0,
                preview.width() - 54.0,
                16.0,
            ),
            format_color(current),
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: palette.placeholder,
                ..TextStyle::default()
            },
        );

        paint_saturation_value_plane(ctx, sv, self.hue);
        paint_hue_bar(ctx, hue);
        if self.show_alpha {
            let alpha = self.alpha_rect(ctx.bounds());
            draw_checkerboard(ctx, alpha, 4.0);
            paint_alpha_bar(ctx, alpha, current);
            let marker_x = alpha.x() + (self.alpha * alpha.width());
            paint_marker(
                ctx,
                Point::new(marker_x, alpha.y() + alpha.height() * 0.5),
                palette.border_focus,
            );
        }

        let marker = Point::new(
            sv.x() + (self.saturation * sv.width()),
            sv.y() + ((1.0 - self.value) * sv.height()),
        );
        paint_marker(ctx, marker, contrast_color(current));

        let hue_marker = Point::new(
            hue.x() + (self.hue * hue.width()),
            hue.y() + hue.height() * 0.5,
        );
        paint_marker(ctx, hue_marker, palette.border_focus);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorPicker, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(format_color(self.color())));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
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

fn paint_saturation_value_plane(ctx: &mut PaintCtx, rect: Rect, hue: f32) {
    let steps = 12;
    for y in 0..steps {
        for x in 0..steps {
            let saturation = x as f32 / (steps - 1) as f32;
            let value = 1.0 - (y as f32 / (steps - 1) as f32);
            let cell = Rect::new(
                rect.x() + rect.width() * (x as f32 / steps as f32),
                rect.y() + rect.height() * (y as f32 / steps as f32),
                rect.width() / steps as f32,
                rect.height() / steps as f32,
            );
            ctx.fill_rect(cell, hsv_to_rgb(hue, saturation, value, 1.0));
        }
    }
    ctx.stroke_rect(
        rect,
        Color::rgba(0.0, 0.0, 0.0, 0.14),
        StrokeStyle::new(1.0),
    );
}

fn paint_hue_bar(ctx: &mut PaintCtx, rect: Rect) {
    let steps = 24;
    for step in 0..steps {
        let start = step as f32 / steps as f32;
        let cell = Rect::new(
            rect.x() + rect.width() * start,
            rect.y(),
            rect.width() / steps as f32,
            rect.height(),
        );
        ctx.fill_rect(cell, hsv_to_rgb(start, 1.0, 1.0, 1.0));
    }
    ctx.stroke_rect(
        rect,
        Color::rgba(0.0, 0.0, 0.0, 0.14),
        StrokeStyle::new(1.0),
    );
}

fn paint_alpha_bar(ctx: &mut PaintCtx, rect: Rect, color: Color) {
    let steps = 20;
    for step in 0..steps {
        let alpha = step as f32 / (steps - 1) as f32;
        let cell = Rect::new(
            rect.x() + rect.width() * (step as f32 / steps as f32),
            rect.y(),
            rect.width() / steps as f32,
            rect.height(),
        );
        ctx.fill_rect(cell, color.with_alpha(alpha));
    }
    ctx.stroke_rect(
        rect,
        Color::rgba(0.0, 0.0, 0.0, 0.14),
        StrokeStyle::new(1.0),
    );
}

fn paint_marker(ctx: &mut PaintCtx, center: Point, color: Color) {
    ctx.stroke(
        Path::circle(center, 6.5),
        Color::WHITE.with_alpha(0.9),
        StrokeStyle::new(2.0),
    );
    ctx.stroke(Path::circle(center, 5.0), color, StrokeStyle::new(1.5));
}

fn draw_surface(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, focused: bool) {
    ctx.fill(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        theme.palette.surface,
    );
    ctx.stroke(
        rounded_rect_path(rect, theme.metrics.corner_radius),
        if focused {
            theme.palette.border_focus
        } else {
            theme.palette.border
        },
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );
}

fn draw_checkerboard(ctx: &mut PaintCtx, rect: Rect, cell_size: f32) {
    let light = Color::rgba(0.98, 0.98, 0.99, 1.0);
    let dark = Color::rgba(0.90, 0.92, 0.95, 1.0);
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

fn contrast_color(color: Color) -> Color {
    if perceived_luminance(color) > 0.55 {
        Color::BLACK.with_alpha(0.85)
    } else {
        Color::WHITE.with_alpha(0.95)
    }
}

fn perceived_luminance(color: Color) -> f32 {
    (0.299 * color.red) + (0.587 * color.green) + (0.114 * color.blue)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32, alpha: f32) -> Color {
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
    Color::rgba(red, green, blue, alpha)
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{ColorPicker, ColorSwatch, Image, format_color, hsv_to_rgb, rgb_to_hsv};
    use sui_core::{
        Color, Event, ImageHandle, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Result, SemanticsRole, SemanticsValue, Size, Vector,
    };
    use sui_runtime::{Application, Runtime, Widget, WindowBuilder};
    use sui_scene::RegisteredImage;

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
    fn color_picker_pointer_drag_updates_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            ColorPicker::new("Accent picker")
                .on_change(move |color| on_change.borrow_mut().push(color)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(36.0, 86.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(208.0, 102.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(208.0, 102.0), false),
        )?;

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
}
