use sui_core::{
    Color, ColorSpace, Event, ImageHandle, KeyState, Path, PathBuilder, Point, PointerButton,
    PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::{ImageSource, StrokeStyle, WidgetShader};
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
    ColorWheel,
    SaturationValue,
    Hue,
    Saturation,
    Value,
    Alpha,
    EncodingSelector,
    RgbRed,
    RgbGreen,
    RgbBlue,
}

pub struct ColorPicker {
    theme: Box<DefaultTheme>,
    name: String,
    editing_space: ColorSpace,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: f32,
    previous_color: Color,
    show_alpha: bool,
    active: Option<ActiveChannel>,
    on_change: Option<Box<dyn FnMut(Color)>>,
}

impl ColorPicker {
    const MAX_HDR_VALUE: f32 = 12.0;
    const PANEL_GAP: f32 = 14.0;
    const TOP_BAR_HEIGHT: f32 = 52.0;
    const WHEEL_SIZE: f32 = 166.0;
    const MAP_SIZE: f32 = 166.0;
    const ROW_HEIGHT: f32 = 20.0;
    const ROW_GAP: f32 = 8.0;
    const RIGHT_PANEL_WIDTH: f32 = 188.0;

    pub fn new(name: impl Into<String>) -> Self {
        Self::from_color(name, Color::rgba(0.11, 0.43, 0.92, 1.0))
    }

    pub fn from_color(name: impl Into<String>, color: Color) -> Self {
        let (hue, saturation, value) = rgb_to_hsv(color);
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            editing_space: color.space,
            hue,
            saturation,
            value,
            alpha: color.alpha,
            previous_color: color,
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
        hsv_to_color(
            self.editing_space,
            self.hue,
            self.saturation,
            self.value,
            self.alpha,
        )
    }

    fn hdr_capable(&self) -> bool {
        self.editing_space.is_linear() || matches!(self.editing_space, ColorSpace::DisplayP3)
    }

    fn max_channel_value(&self) -> f32 {
        if self.hdr_capable() {
            Self::MAX_HDR_VALUE
        } else {
            1.0
        }
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, Insets::all(14.0))
    }

    fn header_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        Rect::new(
            content.x(),
            content.y(),
            content.width(),
            Self::TOP_BAR_HEIGHT,
        )
    }

    fn left_column_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let y = self.header_rect(bounds).max_y() + Self::PANEL_GAP;
        let width = (content.width() - Self::PANEL_GAP - Self::RIGHT_PANEL_WIDTH).max(220.0);
        Rect::new(content.x(), y, width, content.max_y() - y)
    }

    fn right_column_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        let left = self.left_column_rect(bounds);
        Rect::new(
            left.max_x() + Self::PANEL_GAP,
            left.y(),
            content.max_x() - (left.max_x() + Self::PANEL_GAP),
            content.max_y() - left.y(),
        )
    }

    fn color_wheel_rect(&self, bounds: Rect) -> Rect {
        let left = self.left_column_rect(bounds);
        Rect::new(left.x(), left.y(), Self::WHEEL_SIZE, Self::WHEEL_SIZE)
    }

    fn saturation_value_rect(&self, bounds: Rect) -> Rect {
        let right = self.right_column_rect(bounds);
        Rect::new(
            right.x(),
            right.y(),
            right.width().min(Self::MAP_SIZE),
            Self::MAP_SIZE,
        )
    }

    fn left_slider_rect(&self, bounds: Rect, index: usize) -> Rect {
        let wheel = self.color_wheel_rect(bounds);
        let y = wheel.max_y() + 14.0 + index as f32 * (Self::ROW_HEIGHT + Self::ROW_GAP);
        Rect::new(wheel.x(), y, wheel.width(), Self::ROW_HEIGHT)
    }

    fn encoding_rect(&self, bounds: Rect) -> Rect {
        let map = self.saturation_value_rect(bounds);
        Rect::new(map.x(), map.max_y() + 12.0, map.width(), 28.0)
    }

    fn rgb_row_rect(&self, bounds: Rect, index: usize) -> Rect {
        let encoding = self.encoding_rect(bounds);
        let y = encoding.max_y() + 10.0 + index as f32 * (Self::ROW_HEIGHT + 6.0);
        Rect::new(encoding.x(), y, encoding.width(), Self::ROW_HEIGHT)
    }

    fn hex_rect(&self, bounds: Rect) -> Rect {
        let last_row = self.rgb_row_rect(bounds, 2);
        Rect::new(
            last_row.x(),
            last_row.max_y() + 10.0,
            last_row.width(),
            28.0,
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
                let value_t = (1.0 - ((position.y - rect.y()) / rect.height())).clamp(0.0, 1.0);
                self.value = self.max_channel_value() * value_t;
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
            ActiveChannel::EncodingSelector => self.cycle_editing_space(),
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

    fn cycle_editing_space(&mut self) {
        let next_space = match self.editing_space {
            ColorSpace::LinearSrgb => ColorSpace::DisplayP3,
            ColorSpace::DisplayP3 => ColorSpace::LinearDisplayP3,
            ColorSpace::LinearDisplayP3 => ColorSpace::Srgb,
            ColorSpace::Srgb => ColorSpace::LinearSrgb,
        };
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
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let desired = Size::new(520.0, if self.show_alpha { 420.0 } else { 392.0 });
        constraints.clamp(desired)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let current = self.color();
        let header = self.header_rect(ctx.bounds());
        let wheel = self.color_wheel_rect(ctx.bounds());
        let map = self.saturation_value_rect(ctx.bounds());
        let encoding = self.encoding_rect(ctx.bounds());

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
        paint_picker_header(
            ctx,
            header,
            self.theme.as_ref(),
            &self.name,
            self.previous_color,
            current,
        );

        paint_color_wheel(ctx, wheel);
        paint_wheel_marker(ctx, wheel, self.hue);

        paint_saturation_value_plane(
            ctx,
            map,
            self.editing_space,
            self.hue,
            self.max_channel_value(),
        );
        let marker = Point::new(
            map.x() + self.saturation * map.width(),
            map.y()
                + (1.0 - (self.value / self.max_channel_value()).clamp(0.0, 1.0)) * map.height(),
        );
        paint_marker(ctx, marker, contrast_color(current));

        let rows = [
            ("H", format!("{:.2}", self.hue * 360.0)),
            ("S", format!("{:.2}", self.saturation * 100.0)),
            ("V", format!("{:.3}", self.value)),
            ("A", format!("{:.1}", self.alpha * 100.0)),
        ];
        for (index, (label, value_text)) in rows.into_iter().enumerate() {
            if index == 3 && !self.show_alpha {
                continue;
            }
            let rect = self.left_slider_rect(ctx.bounds(), index);
            match index {
                0 => paint_hue_bar(ctx, rect),
                1 => paint_saturation_bar(
                    ctx,
                    rect,
                    self.editing_space,
                    self.hue,
                    self.value.max(1.0),
                ),
                2 => paint_value_bar(
                    ctx,
                    rect,
                    self.editing_space,
                    self.hue,
                    self.saturation,
                    self.hdr_capable(),
                ),
                _ => {
                    draw_checkerboard(ctx, rect, 4.0);
                    paint_alpha_bar(ctx, rect, current);
                }
            }
            paint_labeled_row_text(ctx, rect, label, &value_text, palette.placeholder);
            let marker_x = match index {
                0 => rect.x() + self.hue * rect.width(),
                1 => rect.x() + self.saturation * rect.width(),
                2 => {
                    rect.x()
                        + if self.hdr_capable() {
                            hdr_value_to_slider(self.value) * rect.width()
                        } else {
                            self.value.clamp(0.0, 1.0) * rect.width()
                        }
                }
                _ => rect.x() + self.alpha * rect.width(),
            };
            paint_marker(
                ctx,
                Point::new(marker_x, rect.y() + rect.height() * 0.5),
                palette.border_focus,
            );
        }

        paint_dropdown(
            ctx,
            encoding,
            self.theme.as_ref(),
            editing_space_label(self.editing_space),
        );

        let rgb = current.to_array();
        let channel_labels = ["R", "G", "B"];
        for (index, label) in channel_labels.into_iter().enumerate() {
            let rect = self.rgb_row_rect(ctx.bounds(), index);
            paint_rgb_channel_bar(ctx, rect, current, index, self.max_channel_value());
            paint_labeled_row_text(
                ctx,
                rect,
                label,
                &format!("{:.3}", rgb[index]),
                palette.placeholder,
            );
            let marker_x =
                rect.x() + (rgb[index] / self.max_channel_value()).clamp(0.0, 1.0) * rect.width();
            paint_marker(
                ctx,
                Point::new(marker_x, rect.y() + rect.height() * 0.5),
                palette.border_focus,
            );
        }

        if self.hdr_capable() && is_hdr_color(current) {
            paint_disabled_field(
                ctx,
                self.hex_rect(ctx.bounds()),
                self.theme.as_ref(),
                "HDR hex unavailable",
            );
        } else {
            paint_hex_field(
                ctx,
                self.hex_rect(ctx.bounds()),
                self.theme.as_ref(),
                &format_color(current),
            );
        }

        ctx.draw_text(
            Rect::new(encoding.x(), encoding.y() - 20.0, encoding.width(), 16.0),
            if is_hdr_color(current) {
                format!("{} • HDR", self.name)
            } else {
                format!("{} • SDR", self.name)
            },
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: palette.placeholder,
                ..TextStyle::default()
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let current = self.color();
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorPicker, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(format!(
            "{} editing space; {} range available",
            editing_space_label(self.editing_space),
            if self.hdr_capable() { "HDR" } else { "SDR" }
        ));
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Text(
            if self.hdr_capable() && is_hdr_color(current) {
                format!(
                    "R {:.3} G {:.3} B {:.3} A {:.3}",
                    current.red, current.green, current.blue, current.alpha
                )
            } else {
                format_color(current)
            },
        ));
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

fn paint_picker_header(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    name: &str,
    previous: Color,
    current: Color,
) {
    let palette = theme.palette;
    let current_rect = Rect::new(rect.x(), rect.y(), 96.0, rect.height());
    let previous_rect = Rect::new(current_rect.max_x() + 10.0, rect.y(), 96.0, rect.height());
    draw_checkerboard(ctx, current_rect, 6.0);
    draw_checkerboard(ctx, previous_rect, 6.0);
    ctx.fill(rounded_rect_path(current_rect, 8.0), current);
    ctx.fill(rounded_rect_path(previous_rect, 8.0), previous);
    ctx.stroke(
        rounded_rect_path(current_rect, 8.0),
        palette.border_focus,
        StrokeStyle::new(1.0),
    );
    ctx.stroke(
        rounded_rect_path(previous_rect, 8.0),
        palette.border,
        StrokeStyle::new(1.0),
    );
    let text_x = previous_rect.max_x() + 14.0;
    ctx.draw_text(
        Rect::new(text_x, rect.y() + 2.0, rect.max_x() - text_x, 18.0),
        name.to_string(),
        theme.body_text_style(),
    );
    ctx.draw_text(
        Rect::new(text_x, rect.y() + 22.0, rect.max_x() - text_x, 16.0),
        if is_hdr_color(current) {
            "HDR working color".to_string()
        } else {
            "SDR working color".to_string()
        },
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: palette.placeholder,
            ..TextStyle::default()
        },
    );
    ctx.draw_text(
        Rect::new(rect.max_x() - 68.0, rect.y() + 4.0, 64.0, 16.0),
        "⌖  ↺".to_string(),
        TextStyle {
            font_size: 14.0,
            line_height: 16.0,
            color: palette.placeholder,
            ..TextStyle::default()
        },
    );
}

fn paint_color_wheel(ctx: &mut PaintCtx, rect: Rect) {
    let center = Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    );
    let outer = rect.width().min(rect.height()) * 0.5;
    let inner = outer * 0.55;
    ctx.draw_shader_rect(rect, WidgetShader::ColorWheel);
    ctx.stroke(
        Path::circle(center, outer - 1.0),
        Color::rgba(0.0, 0.0, 0.0, 0.18),
        StrokeStyle::new(1.0),
    );
    ctx.stroke(
        Path::circle(center, inner),
        Color::rgba(0.0, 0.0, 0.0, 0.18),
        StrokeStyle::new(1.0),
    );
}

fn paint_wheel_marker(ctx: &mut PaintCtx, rect: Rect, hue: f32) {
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
    paint_marker(ctx, point, Color::BLACK.with_alpha(0.8));
}

fn paint_saturation_value_plane(
    ctx: &mut PaintCtx,
    rect: Rect,
    space: ColorSpace,
    hue: f32,
    max_value: f32,
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
        Color::rgba(0.0, 0.0, 0.0, 0.16),
        StrokeStyle::new(1.0),
    );
    let sdr_marker = Rect::new(
        rect.x(),
        rect.y() + rect.height() * (1.0 - (1.0 / max_value).clamp(0.0, 1.0)),
        rect.width(),
        1.0,
    );
    ctx.fill_rect(sdr_marker, Color::WHITE.with_alpha(0.28));
}

fn paint_hue_bar(ctx: &mut PaintCtx, rect: Rect) {
    ctx.draw_shader_rect(rect, WidgetShader::ColorPickerHueBar);
    paint_bar_border(ctx, rect);
}

fn paint_saturation_bar(ctx: &mut PaintCtx, rect: Rect, space: ColorSpace, hue: f32, value: f32) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerSaturationBar {
            color_space: space,
            hue,
            value,
        },
    );
    paint_bar_border(ctx, rect);
}

fn paint_bar_border(ctx: &mut PaintCtx, rect: Rect) {
    ctx.stroke_rect(
        rect,
        Color::rgba(0.0, 0.0, 0.0, 0.14),
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
    paint_bar_border(ctx, rect);
    if hdr_capable {
        let divider_x = rect.x() + rect.width() * 0.5;
        ctx.fill_rect(
            Rect::new(divider_x, rect.y(), 1.0, rect.height()),
            Color::WHITE.with_alpha(0.26),
        );
    }
}

fn paint_alpha_bar(ctx: &mut PaintCtx, rect: Rect, color: Color) {
    ctx.draw_shader_rect(rect, WidgetShader::ColorPickerAlphaBar { color });
    paint_bar_border(ctx, rect);
}

fn paint_rgb_channel_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    current: Color,
    channel_index: usize,
    max_value: f32,
) {
    ctx.draw_shader_rect(
        rect,
        WidgetShader::ColorPickerRgbChannelBar {
            color: current,
            channel: channel_index as u32,
            max_value,
        },
    );
    paint_bar_border(ctx, rect);
}

fn paint_labeled_row_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    label: &str,
    value_text: &str,
    value_color: Color,
) {
    ctx.draw_text(
        Rect::new(rect.x() + 6.0, rect.y() + 2.0, 22.0, rect.height()),
        label.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: Color::rgba(0.93, 0.95, 0.99, 1.0),
            ..TextStyle::default()
        },
    );
    ctx.draw_text(
        Rect::new(rect.max_x() - 72.0, rect.y() + 2.0, 68.0, rect.height()),
        value_text.to_string(),
        TextStyle {
            font_size: 11.0,
            line_height: 16.0,
            color: value_color,
            ..TextStyle::default()
        },
    );
}

fn paint_dropdown(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, label: &str) {
    ctx.fill(
        rounded_rect_path(rect, 8.0),
        Color::rgba(0.10, 0.13, 0.18, 1.0),
    );
    ctx.stroke(
        rounded_rect_path(rect, 8.0),
        theme.palette.border_focus,
        StrokeStyle::new(1.0),
    );
    ctx.draw_text(
        Rect::new(rect.x() + 10.0, rect.y() + 6.0, rect.width() - 26.0, 16.0),
        label.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: Color::rgba(0.88, 0.93, 0.98, 1.0),
            ..TextStyle::default()
        },
    );
    ctx.draw_text(
        Rect::new(rect.max_x() - 16.0, rect.y() + 6.0, 12.0, 16.0),
        "▾".to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: theme.palette.placeholder,
            ..TextStyle::default()
        },
    );
}

fn paint_hex_field(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, value: &str) {
    ctx.fill(
        rounded_rect_path(rect, 8.0),
        Color::rgba(0.12, 0.15, 0.20, 1.0),
    );
    ctx.stroke(
        rounded_rect_path(rect, 8.0),
        theme.palette.border,
        StrokeStyle::new(1.0),
    );
    ctx.draw_text(
        Rect::new(rect.x() + 10.0, rect.y() + 6.0, rect.width() - 16.0, 16.0),
        value.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: Color::rgba(0.86, 0.92, 0.97, 1.0),
            ..TextStyle::default()
        },
    );
}

fn paint_disabled_field(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, value: &str) {
    ctx.fill(
        rounded_rect_path(rect, 8.0),
        Color::rgba(0.09, 0.11, 0.14, 1.0),
    );
    ctx.stroke(
        rounded_rect_path(rect, 8.0),
        theme.palette.border,
        StrokeStyle::new(1.0),
    );
    ctx.draw_text(
        Rect::new(rect.x() + 10.0, rect.y() + 6.0, rect.width() - 16.0, 16.0),
        value.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: theme.palette.placeholder,
            ..TextStyle::default()
        },
    );
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{ColorPicker, ColorSwatch, Image, format_color, hsv_to_rgb, rgb_to_hsv};
    use sui_core::{
        Color, ColorSpace, Event, ImageHandle, Point, PointerButton, PointerButtons, PointerEvent,
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
            primary_pointer(PointerEventKind::Down, Point::new(360.0, 126.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(476.0, 152.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(476.0, 152.0), false),
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

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(426.0, 306.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(526.0, 306.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(526.0, 306.0), false),
        )?;

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
    fn color_picker_encoding_selector_cycles_editing_space() -> Result<()> {
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
            primary_pointer(PointerEventKind::Down, Point::new(424.0, 272.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(424.0, 272.0), false),
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
