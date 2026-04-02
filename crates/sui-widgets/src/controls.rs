use sui_core::{
    Color, Event, ImeEvent, KeyState, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsValue, Size, ToggleState,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{EventCtx, LayoutCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::StrokeStyle;
use sui_text::{TextMeasurement, TextStyle};

pub struct Label {
    text: String,
    style: TextStyle,
    measurement: Option<TextMeasurement>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            measurement: None,
        }
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
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let measurement = measure_text(ctx, &self.text, &self.style);
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measurement.width,
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
    label: String,
    text_style: TextStyle,
    padding: Insets,
    min_size: Size,
    hovered: bool,
    pressed: bool,
    label_measurement: Option<TextMeasurement>,
    on_press: Option<Box<dyn FnMut()>>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            text_style: TextStyle {
                color: Color::WHITE,
                ..TextStyle::default()
            },
            padding: Insets {
                left: 14.0,
                top: 10.0,
                right: 14.0,
                bottom: 10.0,
            },
            min_size: Size::new(96.0, 40.0),
            hovered: false,
            pressed: false,
            label_measurement: None,
            on_press: None,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_size.width = width.max(0.0);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_size.height = height.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn on_press<F>(mut self, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_press = Some(Box::new(on_press));
        self
    }

    fn activate(&mut self) {
        if let Some(on_press) = &mut self.on_press {
            on_press();
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

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let measurement = measure_text(ctx, &self.label, &self.text_style);
        self.label_measurement = Some(measurement);

        let width =
            (measurement.width + self.padding.left + self.padding.right).max(self.min_size.width);
        let height = (measurement.height.max(self.text_style.line_height)
            + self.padding.top
            + self.padding.bottom)
            .max(self.min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let background = if self.pressed {
            Color::rgba(0.12, 0.29, 0.50, 1.0)
        } else if ctx.is_focused() {
            Color::rgba(0.25, 0.52, 0.88, 1.0)
        } else if self.hovered {
            Color::rgba(0.20, 0.36, 0.62, 1.0)
        } else {
            Color::rgba(0.17, 0.24, 0.37, 1.0)
        };
        let border = if ctx.is_focused() {
            Color::rgba(0.80, 0.87, 1.0, 1.0)
        } else {
            Color::rgba(0.24, 0.33, 0.48, 1.0)
        };

        ctx.fill_bounds(background);
        ctx.stroke_bounds(border, StrokeStyle::new(1.0));
        ctx.draw_text(
            inset_rect(ctx.bounds(), self.padding),
            self.label.clone(),
            self.text_style.clone(),
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

pub struct Checkbox {
    label: String,
    checked: bool,
    text_style: TextStyle,
    padding: Insets,
    indicator_size: f32,
    gap: f32,
    hovered: bool,
    pressed: bool,
    label_measurement: Option<TextMeasurement>,
    on_toggle: Option<Box<dyn FnMut(bool)>>,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
            text_style: TextStyle {
                color: Color::rgba(0.94, 0.96, 0.99, 1.0),
                ..TextStyle::default()
            },
            padding: Insets {
                left: 8.0,
                top: 6.0,
                right: 8.0,
                bottom: 6.0,
            },
            indicator_size: 18.0,
            gap: 10.0,
            hovered: false,
            pressed: false,
            label_measurement: None,
            on_toggle: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
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
            ctx.request_paint();
            ctx.request_semantics();
        }
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
                ctx.release_pointer_capture(pointer.pointer_id);
                if toggle {
                    self.toggle();
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
                self.toggle();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let measurement = measure_text(ctx, &self.label, &self.text_style);
        self.label_measurement = Some(measurement);

        let width = self.padding.left
            + self.indicator_size
            + self.gap
            + measurement.width
            + self.padding.right;
        let height = (self
            .indicator_size
            .max(measurement.height.max(self.text_style.line_height))
            + self.padding.top
            + self.padding.bottom)
            .max(32.0);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let background = if self.hovered {
            Color::rgba(0.15, 0.18, 0.24, 1.0)
        } else {
            Color::rgba(0.12, 0.14, 0.19, 1.0)
        };
        let border = if ctx.is_focused() {
            Color::rgba(0.66, 0.74, 0.93, 1.0)
        } else {
            Color::rgba(0.34, 0.40, 0.50, 1.0)
        };
        let indicator = indicator_rect(ctx.bounds(), self.padding, self.indicator_size);
        let label_rect =
            checkbox_label_rect(ctx.bounds(), self.padding, self.indicator_size, self.gap);

        ctx.fill_bounds(background);
        ctx.stroke_bounds(border, StrokeStyle::new(1.0));
        ctx.fill_rect(indicator, Color::rgba(0.08, 0.10, 0.14, 1.0));
        ctx.stroke_rect(indicator, border, StrokeStyle::new(1.0));
        if self.checked {
            ctx.fill_rect(
                indicator.inflate(-4.0, -4.0),
                Color::rgba(0.31, 0.67, 0.47, 1.0),
            );
        }
        ctx.draw_text(label_rect, self.label.clone(), self.text_style.clone());
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct TextInput {
    name: String,
    value: String,
    placeholder: String,
    composition: String,
    text_style: TextStyle,
    padding: Insets,
    min_size: Size,
    hovered: bool,
    visible_measurement: Option<TextMeasurement>,
    input_measurement: Option<TextMeasurement>,
    on_change: Option<Box<dyn FnMut(String)>>,
}

impl TextInput {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: String::new(),
            placeholder: String::new(),
            composition: String::new(),
            text_style: TextStyle {
                color: Color::rgba(0.95, 0.96, 0.98, 1.0),
                ..TextStyle::default()
            },
            padding: Insets {
                left: 12.0,
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
            },
            min_size: Size::new(220.0, 44.0),
            hovered: false,
            visible_measurement: None,
            input_measurement: None,
            on_change: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
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
        ctx.request_layout();
        ctx.request_paint();
        ctx.request_semantics();
        ctx.set_handled();
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
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
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionStart) if ctx.is_focused() => {
                self.composition.clear();
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Ime(ImeEvent::CompositionUpdate { text }) if ctx.is_focused() => {
                self.composition = text.clone();
                ctx.request_layout();
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
                    ctx.request_layout();
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
                ctx.request_layout();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && self.composition.is_empty() => {
                if let Some(text) = keyboard_text(key) {
                    self.insert_text(text, ctx);
                }
            }
            _ => {}
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let visible_text = self.visible_text();
        let input_text = self.input_text();
        let visible_measurement = measure_text(ctx, &visible_text, &self.text_style);
        let input_measurement = if input_text.is_empty() {
            TextMeasurement {
                width: 0.0,
                height: visible_measurement.height,
                bounds: Rect::new(0.0, 0.0, 0.0, visible_measurement.height),
            }
        } else {
            measure_text(ctx, &input_text, &self.text_style)
        };

        self.visible_measurement = Some(visible_measurement);
        self.input_measurement = Some(input_measurement);

        let width = (visible_measurement.width + self.padding.left + self.padding.right)
            .max(self.min_size.width);
        let height = (visible_measurement.height.max(self.text_style.line_height)
            + self.padding.top
            + self.padding.bottom)
            .max(self.min_size.height);

        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let background = if ctx.is_focused() {
            Color::rgba(0.16, 0.20, 0.28, 1.0)
        } else if self.hovered {
            Color::rgba(0.14, 0.17, 0.24, 1.0)
        } else {
            Color::rgba(0.12, 0.14, 0.19, 1.0)
        };
        let border = if ctx.is_focused() {
            Color::rgba(0.66, 0.74, 0.93, 1.0)
        } else {
            Color::rgba(0.34, 0.40, 0.50, 1.0)
        };
        let content_rect = inset_rect(ctx.bounds(), self.padding);
        let display_text = self.visible_text();
        let placeholder = self.input_text().is_empty();

        ctx.fill_bounds(background);
        ctx.stroke_bounds(border, StrokeStyle::new(1.0));
        ctx.draw_text(
            content_rect,
            display_text,
            if placeholder {
                TextStyle {
                    color: Color::rgba(0.56, 0.61, 0.69, 1.0),
                    ..self.text_style.clone()
                }
            } else {
                self.text_style.clone()
            },
        );

        if ctx.is_focused() {
            let caret_x = content_rect.x()
                + self
                    .input_measurement
                    .map(|measurement| measurement.width)
                    .unwrap_or(0.0);
            let caret_rect = Rect::new(
                caret_x.min(content_rect.max_x()),
                content_rect.y(),
                1.0,
                content_rect.height().max(self.text_style.line_height),
            );
            ctx.set_ime_composition_rect(caret_rect);
            ctx.fill_rect(caret_rect, Color::rgba(0.84, 0.89, 1.0, 1.0));
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
        if !focused && !self.composition.is_empty() {
            self.composition.clear();
            ctx.request_layout();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn measure_text(ctx: &mut LayoutCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
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
    Rect::new(
        x,
        bounds.y() + padding.top,
        width,
        (bounds.height() - padding.top - padding.bottom).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{Button, Checkbox, Label, TextInput};
    use sui_core::{
        Color, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
        PointerButtons, PointerEvent, PointerEventKind, PointerKind, Result, SemanticsRole, Size,
        Vector,
    };
    use sui_runtime::{Application, RenderOutput, Runtime, Widget, WindowBuilder};
    use sui_scene::SceneCommand;

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

    #[test]
    fn label_paints_text_and_exposes_text_semantics() {
        let output = render(Label::new("Hello SUI").color(Color::rgba(0.8, 0.9, 1.0, 1.0)));

        assert!(output.frame.viewport.height >= 18.0);
        assert!(matches!(
            output.frame.scene.commands()[0],
            SceneCommand::DrawText(_)
        ));
        assert_eq!(output.semantics[0].role, SemanticsRole::Text);
        assert_eq!(output.semantics[0].name.as_deref(), Some("Hello SUI"));
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
        let output = render(Button::new("Go").min_width(140.0).min_height(44.0));
        assert_eq!(output.frame.viewport, Size::new(140.0, 44.0));
    }
}
