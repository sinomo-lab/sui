use sui_core::{
    Color, ColorSpace, Event, ImageHandle, KeyState, Path, PathBuilder, Point, PointerButton,
    PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    WidgetId,
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
    show_border: bool,
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
            show_border: true,
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

    pub fn without_border(mut self) -> Self {
        self.show_border = false;
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

        if self.show_border {
            ctx.stroke(
                rounded_rect_path(bounds, self.corner_radius),
                self.theme.palette.border,
                StrokeStyle::new(self.theme.metrics.border_width.max(1.0)),
            );
        }
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
    color_reader: Option<Box<dyn Fn() -> Color>>,
    width: f32,
    height: f32,
    hovered: bool,
    pressed: bool,
    read_only: bool,
    on_press: Option<Box<dyn FnMut(Color)>>,
}

impl ColorSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            color,
            color_reader: None,
            width: 56.0,
            height: 32.0,
            hovered: false,
            pressed: false,
            read_only: false,
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

    fn activate(&mut self) {
        let color = self.current_color();
        if let Some(on_press) = &mut self.on_press {
            on_press(color);
        }
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
        let color = self.current_color();
        draw_checkerboard(ctx, ctx.bounds(), 6.0);
        ctx.fill(
            rounded_rect_path(inset_rect(ctx.bounds(), Insets::all(1.0)), inner_radius),
            color,
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
        node.value = Some(SemanticsValue::Text(format_color(self.current_color())));
        if !self.read_only {
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        }
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        !self.read_only
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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

const COLOR_PALETTE_SWATCH_SIZE: f32 = 28.0;
const COLOR_PALETTE_GAP: f32 = 6.0;

pub struct ColorPalette {
    theme: Box<DefaultTheme>,
    name: String,
    swatches: Vec<ColorPaletteSwatch>,
    selected: Option<usize>,
    selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    columns: usize,
    swatch_size: f32,
    gap: f32,
    on_change: Option<Box<dyn FnMut(usize, String, Color)>>,
}

impl ColorPalette {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            swatches: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            pressed: None,
            columns: 8,
            swatch_size: COLOR_PALETTE_SWATCH_SIZE,
            gap: COLOR_PALETTE_GAP,
            on_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self.swatch_size = size.max(18.0);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String, Color) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
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

    fn swatch_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.swatches.len() {
            return None;
        }

        let columns = self.grid_columns();
        let column = index % columns;
        let row = index / columns;
        let x = bounds.x() + column as f32 * (self.swatch_size + self.gap);
        let y = bounds.y() + row as f32 * (self.swatch_size + self.gap);
        let available_width = (bounds.max_x() - x).max(0.0);
        let available_height = (bounds.max_y() - y).max(0.0);
        let rect = Rect::new(
            x,
            y,
            self.swatch_size.min(available_width),
            self.swatch_size.min(available_height),
        );
        (!rect.is_empty()).then_some(rect)
    }

    fn swatch_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.swatches.iter().enumerate().find_map(|(index, _)| {
            self.swatch_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    fn activate(&mut self, index: usize) {
        if self.swatches.is_empty() {
            return;
        }

        let index = index.min(self.swatches.len() - 1);
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            let swatch = &self.swatches[index];
            on_change(index, swatch.name.clone(), swatch.color);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.swatches.is_empty() {
            return;
        }

        let current = self.current_selected().unwrap_or(0) as isize;
        let last = self.swatches.len() as isize - 1;
        let next = (current + delta).clamp(0, last) as usize;
        self.hovered = Some(next);
        self.activate(next);
    }

    fn selected_value(&self) -> Option<String> {
        self.current_selected()
            .and_then(|index| self.swatches.get(index))
            .map(|swatch| format!("{} {}", swatch.name, format_color(swatch.color)))
    }
}

impl Widget for ColorPalette {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.swatch_at(ctx.bounds(), pointer.position);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                if self.hovered.take().is_some() {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.hovered = self.swatch_at(ctx.bounds(), pointer.position);
                self.pressed = self.hovered;
                if self.hovered.is_some() {
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
                let hovered = self.swatch_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                }
                self.hovered = hovered;
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    self.hovered = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let columns = self.grid_columns() as isize;
                match key.key.as_str() {
                    "ArrowLeft" => self.move_selection(-1),
                    "ArrowRight" => self.move_selection(1),
                    "ArrowUp" => self.move_selection(-columns),
                    "ArrowDown" => self.move_selection(columns),
                    "Home" => self.activate(0),
                    "End" if !self.swatches.is_empty() => self.activate(self.swatches.len() - 1),
                    "Enter" | " " => {
                        if let Some(selected) = self.current_selected().or(Some(0)) {
                            self.activate(selected);
                        }
                    }
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let columns = self.grid_columns() as f32;
        let rows = self.grid_rows() as f32;
        constraints.clamp(Size::new(
            columns * self.swatch_size + (columns - 1.0).max(0.0) * self.gap,
            rows * self.swatch_size + (rows - 1.0).max(0.0) * self.gap,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let radius = self
            .theme
            .metrics
            .corner_radius
            .min(self.swatch_size * 0.25);
        let selected = self.current_selected();

        if ctx.is_focused() {
            ctx.stroke(
                rounded_rect_path(ctx.bounds().inflate(2.0, 2.0), radius + 2.0),
                palette.focus_ring,
                StrokeStyle::new(self.theme.metrics.focus_ring_width.max(1.0)),
            );
        }

        for (index, swatch) in self.swatches.iter().enumerate() {
            let Some(rect) = self.swatch_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = selected == Some(index);
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let ring = if selected {
                palette.accent_border
            } else if hovered {
                palette.border_focus
            } else {
                palette.border
            };
            let ring_width = if selected { 2.0 } else { 1.0 };
            let fill_rect = inset_rect(rect, Insets::all(if selected { 3.0 } else { 2.0 }));

            if pressed {
                ctx.fill(rounded_rect_path(rect, radius), palette.surface_pressed);
            }
            draw_checkerboard(ctx, fill_rect, 5.0);
            ctx.fill(
                rounded_rect_path(fill_rect, (radius - 2.0).max(0.0)),
                swatch.color,
            );
            ctx.stroke(
                rounded_rect_path(rect, radius),
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
        for (index, swatch) in self.swatches.iter().enumerate() {
            let Some(rect) = self.swatch_rect(ctx.bounds(), index) else {
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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
    name: String,
    kind: String,
    spec: BrushPreviewSpec,
    spec_reader: Option<Box<dyn Fn() -> BrushPreviewSpec>>,
    width: f32,
    height: f32,
}

impl BrushPreview {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: name.into(),
            kind: "brush".to_string(),
            spec: BrushPreviewSpec::new(
                Color::rgba(0.12, 0.28, 0.88, 1.0),
                18.0,
                1.0,
                BrushPreviewShape::Round,
            ),
            spec_reader: None,
            width: 260.0,
            height: 70.0,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
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
        self.width = size.width.max(80.0);
        self.height = size.height.max(44.0);
        self
    }

    fn current_spec(&self) -> BrushPreviewSpec {
        self.spec_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.spec)
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
}

impl Widget for BrushPreview {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.width, self.height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let palette = self.theme.palette;
        let spec = self.current_spec();
        let content = inset_rect(bounds, Insets::all(8.0));
        let swatch = Rect::new(content.x(), content.y(), 54.0, content.height());
        let sample = Rect::new(
            swatch.max_x() + 10.0,
            content.y(),
            (content.max_x() - swatch.max_x() - 10.0).max(0.0),
            content.height(),
        );
        let preview_color = spec
            .color
            .with_alpha((spec.color.alpha * spec.opacity).clamp(0.0, 1.0));

        ctx.fill(rounded_rect_path(bounds, 6.0), palette.surface);
        ctx.stroke(
            rounded_rect_path(bounds, 6.0),
            palette.border,
            StrokeStyle::new(self.theme.metrics.border_width.max(1.0)),
        );
        draw_checkerboard(ctx, swatch, 8.0);
        ctx.stroke(
            rounded_rect_path(swatch, 4.0),
            palette.border.with_alpha(0.70),
            StrokeStyle::new(1.0),
        );
        paint_brush_preview_mark(ctx, swatch, spec, preview_color);

        let track = Rect::new(
            sample.x(),
            sample.y() + sample.height() * 0.44,
            sample.width(),
            sample.height() * 0.24,
        );
        draw_checkerboard(ctx, track, 6.0);
        paint_brush_preview_stroke(ctx, track, spec, preview_color);

        let text_rect = Rect::new(sample.x(), sample.max_y() - 18.0, sample.width(), 16.0);
        ctx.push_clip_rect(text_rect);
        ctx.draw_text(
            text_rect,
            Self::value_text(&self.kind, spec),
            TextStyle {
                font_size: 11.0,
                line_height: 14.0,
                color: palette.text.with_alpha(0.72),
                ..self.theme.body_text_style()
            },
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
    const PANEL_GAP: f32 = 14.0;
    const COMPACT_PANEL_GAP: f32 = 10.0;
    const TOP_BAR_HEIGHT: f32 = 52.0;
    const COMPACT_TOP_BAR_HEIGHT: f32 = 40.0;
    const WHEEL_SIZE: f32 = 166.0;
    const COMPACT_WHEEL_SIZE: f32 = 128.0;
    const MAP_SIZE: f32 = 210.0;
    const COMPACT_MAP_SIZE: f32 = 132.0;
    const ROW_HEIGHT: f32 = 24.0;
    const ROW_GAP: f32 = 8.0;
    const RIGHT_PANEL_WIDTH: f32 = 226.0;
    const COMPACT_RIGHT_PANEL_WIDTH: f32 = 150.0;
    const ENCODING_MENU_ROW_HEIGHT: f32 = 28.0;
    const ENCODING_OPTIONS: [ColorSpace; 4] = [
        ColorSpace::Srgb,
        ColorSpace::LinearSrgb,
        ColorSpace::DisplayP3,
        ColorSpace::LinearDisplayP3,
    ];

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
            compact: false,
            encoding_dropdown_open: false,
            active: None,
            color_reader: None,
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
        if self.compact { 12.0 } else { 14.0 }
    }

    fn panel_gap(&self) -> f32 {
        if self.compact {
            Self::COMPACT_PANEL_GAP
        } else {
            Self::PANEL_GAP
        }
    }

    fn top_bar_height(&self) -> f32 {
        if self.compact {
            Self::COMPACT_TOP_BAR_HEIGHT
        } else {
            Self::TOP_BAR_HEIGHT
        }
    }

    fn swatch_width(&self) -> f32 {
        if self.compact { 64.0 } else { 96.0 }
    }

    fn swatch_gap(&self) -> f32 {
        if self.compact { 8.0 } else { 10.0 }
    }

    fn wheel_size(&self) -> f32 {
        if self.compact {
            Self::COMPACT_WHEEL_SIZE
        } else {
            Self::WHEEL_SIZE
        }
    }

    fn map_size(&self) -> f32 {
        if self.compact {
            Self::COMPACT_MAP_SIZE
        } else {
            Self::MAP_SIZE
        }
    }

    fn right_panel_width(&self) -> f32 {
        if self.compact {
            Self::COMPACT_RIGHT_PANEL_WIDTH
        } else {
            Self::RIGHT_PANEL_WIDTH
        }
    }

    fn channel_slider_count(&self) -> usize {
        if self.show_alpha { 4 } else { 3 }
    }

    fn desired_size(&self) -> Size {
        let inset = self.content_inset();
        let left_height = self.wheel_size()
            + 14.0
            + self.channel_slider_count() as f32 * Self::ROW_HEIGHT
            + self.channel_slider_count().saturating_sub(1) as f32 * Self::ROW_GAP;
        let right_height =
            self.map_size() + 14.0 + 3.0 * Self::ROW_HEIGHT + 2.0 * Self::ROW_GAP + 12.0 + 30.0;
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
        let y = wheel.max_y() + 14.0 + index as f32 * (Self::ROW_HEIGHT + Self::ROW_GAP);
        Rect::new(wheel.x(), y, wheel.width(), Self::ROW_HEIGHT)
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
            header.y() + ((header.height() - 30.0) * 0.5),
            (header.max_x() - selector_x).max(0.0),
            30.0,
        )
    }

    fn encoding_menu_rect(&self, bounds: Rect) -> Rect {
        let encoding = self.encoding_rect(bounds);
        Rect::new(
            encoding.x(),
            encoding.max_y() + 4.0,
            encoding.width(),
            Self::ENCODING_MENU_ROW_HEIGHT * Self::ENCODING_OPTIONS.len() as f32,
        )
    }

    fn encoding_option_rect(&self, bounds: Rect, index: usize) -> Rect {
        let menu = self.encoding_menu_rect(bounds);
        Rect::new(
            menu.x(),
            menu.y() + index as f32 * Self::ENCODING_MENU_ROW_HEIGHT,
            menu.width(),
            Self::ENCODING_MENU_ROW_HEIGHT,
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
        let y = map.max_y() + 14.0 + index as f32 * (Self::ROW_HEIGHT + Self::ROW_GAP);
        Rect::new(map.x(), y, map.width(), Self::ROW_HEIGHT)
    }

    fn hex_rect(&self, bounds: Rect) -> Rect {
        let last_row = self.rgb_row_rect(bounds, 2);
        Rect::new(
            last_row.x(),
            last_row.max_y() + 12.0,
            last_row.width(),
            30.0,
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
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_external_color();
        constraints.clamp(self.desired_size())
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let palette = self.theme.palette;
        let resolved = self.resolved_state();
        let current = resolved.color;
        let header = self.header_rect(ctx.bounds());
        let wheel = self.color_wheel_rect(ctx.bounds());
        let map = self.saturation_value_rect(ctx.bounds());
        let encoding = self.encoding_rect(ctx.bounds());

        draw_surface(ctx, ctx.bounds(), self.theme.as_ref(), ctx.is_focused());
        paint_picker_header(
            ctx,
            header,
            self.theme.as_ref(),
            self.previous_color,
            current,
        );
        paint_dropdown(
            ctx,
            encoding,
            self.theme.as_ref(),
            editing_space_label(resolved.editing_space),
        );

        paint_color_wheel(ctx, wheel);
        paint_wheel_marker(ctx, wheel, resolved.hue);

        paint_saturation_value_plane(
            ctx,
            map,
            resolved.editing_space,
            resolved.hue,
            resolved.max_channel_value,
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
        paint_marker(ctx, marker, contrast_color(current));

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
                0 => paint_hue_bar(ctx, rect),
                1 => paint_saturation_bar(
                    ctx,
                    rect,
                    resolved.editing_space,
                    resolved.hue,
                    resolved.value.max(1.0),
                ),
                2 => paint_value_bar(
                    ctx,
                    rect,
                    resolved.editing_space,
                    resolved.hue,
                    resolved.saturation,
                    resolved.hdr_capable,
                ),
                _ => {
                    draw_checkerboard(ctx, rect, 4.0);
                    paint_alpha_bar(ctx, rect, current);
                }
            }
            paint_labeled_row_text(ctx, rect, label, &value_text, palette.placeholder);
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
            );
        }

        let rgb = current.to_array();
        let channel_labels = ["R", "G", "B"];
        for (index, label) in channel_labels.into_iter().enumerate() {
            let rect = self.rgb_row_rect(ctx.bounds(), index);
            paint_rgb_channel_bar(ctx, rect, current, index, resolved.max_channel_value);
            paint_labeled_row_text(
                ctx,
                rect,
                label,
                &format!("{:.3}", rgb[index]),
                palette.placeholder,
            );
            let marker_x =
                rect.x() + (rgb[index] / resolved.max_channel_value).clamp(0.0, 1.0) * rect.width();
            paint_marker(
                ctx,
                Point::new(marker_x, rect.y() + rect.height() * 0.5),
                palette.border_focus,
            );
        }

        if resolved.hdr_capable && is_hdr_color(current) {
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

        if self.encoding_dropdown_open {
            paint_encoding_menu(
                ctx,
                self.encoding_menu_rect(ctx.bounds()),
                self.theme.as_ref(),
                resolved.editing_space,
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn paint_picker_header(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
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
        Rect::new(
            rect.x() + 6.0,
            rect.y() + ((rect.height() - 16.0) * 0.5),
            22.0,
            16.0,
        ),
        label.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: Color::rgba(0.93, 0.95, 0.99, 1.0),
            ..TextStyle::default()
        },
    );
    ctx.draw_text(
        Rect::new(
            rect.max_x() - 74.0,
            rect.y() + ((rect.height() - 16.0) * 0.5),
            70.0,
            16.0,
        ),
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
        Rect::new(
            rect.x() + 10.0,
            rect.y() + ((rect.height() - 16.0) * 0.5),
            rect.width() - 32.0,
            16.0,
        ),
        label.to_string(),
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            color: Color::rgba(0.88, 0.93, 0.98, 1.0),
            ..TextStyle::default()
        },
    );
    ctx.stroke(
        dropdown_chevron_path(rect),
        theme.palette.placeholder,
        StrokeStyle::new(1.4),
    );
}

fn paint_encoding_menu(ctx: &mut PaintCtx, rect: Rect, theme: &DefaultTheme, selected: ColorSpace) {
    ctx.fill(
        rounded_rect_path(rect, 8.0),
        Color::rgba(0.08, 0.105, 0.145, 1.0),
    );
    ctx.stroke(
        rounded_rect_path(rect, 8.0),
        theme.palette.border_focus,
        StrokeStyle::new(1.0),
    );

    for (index, space) in ColorPicker::ENCODING_OPTIONS.iter().copied().enumerate() {
        let row = Rect::new(
            rect.x(),
            rect.y() + index as f32 * ColorPicker::ENCODING_MENU_ROW_HEIGHT,
            rect.width(),
            ColorPicker::ENCODING_MENU_ROW_HEIGHT,
        );
        if space == selected {
            ctx.fill(
                rounded_rect_path(
                    inset_rect(
                        row,
                        Insets {
                            left: 4.0,
                            top: 3.0,
                            right: 4.0,
                            bottom: 3.0,
                        },
                    ),
                    6.0,
                ),
                theme.palette.border_focus.with_alpha(0.22),
            );
            ctx.fill_rect(
                Rect::new(row.x() + 6.0, row.y() + 7.0, 3.0, row.height() - 14.0),
                theme.palette.border_focus,
            );
        }
        ctx.draw_text(
            Rect::new(row.x() + 14.0, row.y() + 6.0, row.width() - 22.0, 16.0),
            editing_space_label(space),
            TextStyle {
                font_size: 12.0,
                line_height: 16.0,
                color: if space == selected {
                    Color::rgba(0.93, 0.97, 1.0, 1.0)
                } else {
                    Color::rgba(0.78, 0.84, 0.91, 1.0)
                },
                ..TextStyle::default()
            },
        );
    }
}

fn dropdown_chevron_path(rect: Rect) -> Path {
    let center = Point::new(rect.max_x() - 14.0, rect.y() + rect.height() * 0.5 + 1.0);
    let half_width = 4.0;
    let half_height = 2.5;
    let mut path = PathBuilder::new();
    path.move_to(Point::new(center.x - half_width, center.y - half_height));
    path.line_to(Point::new(center.x, center.y + half_height));
    path.line_to(Point::new(center.x + half_width, center.y - half_height));
    path.build()
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
        Rect::new(
            rect.x() + 10.0,
            rect.y() + ((rect.height() - 16.0) * 0.5),
            rect.width() - 16.0,
            16.0,
        ),
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
        Rect::new(
            rect.x() + 10.0,
            rect.y() + ((rect.height() - 16.0) * 0.5),
            rect.width() - 16.0,
            16.0,
        ),
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
    use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

    use super::{
        ActiveChannel, BrushPreview, BrushPreviewShape, BrushPreviewSpec, ColorPalette,
        ColorPaletteSwatch, ColorPicker, ColorPickerSemanticPart, ColorSwatch, Image,
        color_picker_child_semantics_id, format_color, hsv_to_rgb, rgb_to_hsv,
    };
    use sui_core::{
        Color, ColorSpace, Event, ImageHandle, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, Result, SemanticsAction, SemanticsRole, SemanticsValue, Size,
        Vector, WidgetId,
    };
    use sui_runtime::{Application, Runtime, Widget, WindowBuilder};
    use sui_scene::{RegisteredImage, SceneCommand};

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
            primary_pointer(PointerEventKind::Move, Point::new(390.0, 152.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(390.0, 152.0), false),
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

        assert_eq!(map.x() - wheel.max_x(), ColorPicker::PANEL_GAP);
        assert_eq!(map.width(), ColorPicker::MAP_SIZE);
        assert_eq!(map.y(), wheel.y());
    }

    #[test]
    fn color_picker_compact_mode_uses_smaller_measurement() {
        let regular = ColorPicker::new("Accent picker").desired_size();
        let compact = ColorPicker::new("Accent picker")
            .compact(true)
            .show_alpha(false)
            .desired_size();

        assert_eq!(
            compact.width,
            ColorPicker::COMPACT_WHEEL_SIZE
                + ColorPicker::COMPACT_PANEL_GAP
                + ColorPicker::COMPACT_RIGHT_PANEL_WIDTH
                + 24.0
        );
        assert!(compact.width < regular.width);
        assert!(compact.height < regular.height);
    }

    #[test]
    fn color_picker_header_contains_encoding_selector() {
        let picker = ColorPicker::new("Accent picker");
        let bounds = Rect::new(0.0, 0.0, 434.0, 448.0);
        let header = picker.header_rect(bounds);
        let encoding = picker.encoding_rect(bounds);
        let map = picker.saturation_value_rect(bounds);
        let rgb = picker.rgb_row_rect(bounds, 0);

        assert!(header.contains(encoding.origin));
        assert!(header.contains(Point::new(encoding.max_x(), encoding.max_y())));
        assert!(encoding.x() > bounds.x() + 200.0);
        assert!(rgb.y() > map.max_y());
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

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(250.0, 316.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(380.0, 316.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(380.0, 316.0), false),
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
