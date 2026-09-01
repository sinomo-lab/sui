use std::{cell::RefCell, rc::Rc};

use super::{
    Button, ButtonAppearance, CARET_BLINK_PERIOD_SECONDS, Checkbox, ChoiceAppearance,
    DateTimeInput, DefaultTheme, FieldAppearance, Icon, IconButton, IconButtonPaint, IconGlyph,
    Label, Link, NumberInput, PasswordInput, RadioButton, RadioGroup, Select, Separator, Slider,
    Switch, TextArea, TextInput, paint_icon_button, rect_is_finite,
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
    Color, CustomEvent, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect, Result, SemanticsAction,
    SemanticsActionRequest, SemanticsRole, SemanticsTextRange, SemanticsValue, Size, Vector,
    WidgetId, WindowEvent,
};
use sui_layout::{Alignment, Constraints, Padding as TestPadding};
use sui_reactive::Signal;
use sui_render_wgpu::{RgbaImage, WgpuRenderer};
use sui_runtime::{
    Application, ArrangeCtx, CommandDelivery, CommandTarget, EventCtx, MeasureCtx, PaintCtx,
    RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowBuilder, WindowRenderOptions, clear_window_render_options,
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
            if alpha > 200 && red >= min_channel && green >= min_channel && blue >= min_channel {
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

const INVALIDATE_EXTERNAL_SLIDER_VALUE_KIND: &str = "invalidate-external-slider-value";

struct ExternalValueInvalidationHost {
    child: SingleChild,
}

impl ExternalValueInvalidationHost {
    fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ExternalValueInvalidationHost {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == INVALIDATE_EXTERNAL_SLIDER_VALUE_KIND
        {
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn slider_thumb_center_x(output: &RenderOutput, expected_color: Color) -> f32 {
    let mut thumb_bounds = Vec::new();
    output.frame.scene.visit_commands(&mut |command| {
        if let SceneCommand::FillPath {
            path,
            brush: Brush::Solid(color),
        } = command
            && *color == expected_color
        {
            let bounds = path.bounds();
            if bounds.width() > 8.0
                && bounds.height() > 8.0
                && (bounds.width() - bounds.height()).abs() < 1.0
            {
                thumb_bounds.push(bounds);
            }
        }
    });
    let thumb = thumb_bounds
        .into_iter()
        .max_by(|left, right| left.width().total_cmp(&right.width()))
        .expect("slider thumb fill should be present");
    thumb.x() + thumb.width() * 0.5
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

fn layer_descriptor_for(output: &RenderOutput, owner: WidgetId) -> Option<SceneLayerDescriptor> {
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
fn label_style_when_reads_current_style_for_layout_and_paint() -> Result<()> {
    let initial_color = Color::rgba(0.2, 0.4, 0.8, 1.0);
    let updated_color = Color::rgba(0.8, 0.3, 0.2, 1.0);
    let style = Rc::new(RefCell::new(TextStyle {
        font_size: 12.0,
        line_height: 17.0,
        color: initial_color,
        ..TextStyle::default()
    }));
    let style_reader = Rc::clone(&style);
    let (mut runtime, window_id) = build_runtime(
        Label::new("Reactive style").style_when(move || style_reader.borrow().clone()),
    );

    let initial = runtime.render(window_id)?;
    let initial_run = first_text_run(&initial);
    assert_eq!(initial_run.style.font_size, 12.0);
    assert_eq!(initial_run.style.line_height, 17.0);
    assert_eq!(initial_run.style.color, initial_color);

    *style.borrow_mut() = TextStyle {
        font_size: 19.0,
        line_height: 27.0,
        color: updated_color,
        ..TextStyle::default()
    };
    runtime.handle_event(
        window_id,
        Event::Window(WindowEvent::Resized(Size::new(320.0, 120.0))),
    )?;
    let updated = runtime.render(window_id)?;
    let updated_run = first_text_run(&updated);
    assert_eq!(updated_run.style.font_size, 19.0);
    assert_eq!(updated_run.style.line_height, 27.0);
    assert_eq!(updated_run.style.color, updated_color);
    assert!(updated_run.rect.height() > initial_run.rect.height());
    Ok(())
}

#[test]
fn label_style_builders_follow_last_whole_style_and_property_precedence() {
    let static_color = Color::rgba(0.2, 0.3, 0.4, 1.0);
    let dynamic_color = Color::rgba(0.5, 0.6, 0.7, 1.0);
    let override_color = Color::rgba(0.8, 0.2, 0.4, 1.0);
    let static_style = TextStyle {
        font_size: 13.0,
        line_height: 18.0,
        color: static_color,
        ..TextStyle::default()
    };
    let dynamic_style = TextStyle {
        font_size: 17.0,
        line_height: 25.0,
        color: dynamic_color,
        ..TextStyle::default()
    };

    let dynamic_wins = Label::new("Dynamic")
        .style(static_style.clone())
        .style_when({
            let dynamic_style = dynamic_style.clone();
            move || dynamic_style.clone()
        })
        .resolved_style();
    assert_eq!(dynamic_wins.font_size, 17.0);
    assert_eq!(dynamic_wins.line_height, 25.0);
    assert_eq!(dynamic_wins.color, dynamic_color);

    let static_wins = Label::new("Static")
        .style_when({
            let dynamic_style = dynamic_style.clone();
            move || dynamic_style.clone()
        })
        .style(static_style.clone())
        .resolved_style();
    assert_eq!(static_wins.font_size, 13.0);
    assert_eq!(static_wins.line_height, 18.0);
    assert_eq!(static_wins.color, static_color);

    let property_overrides = Label::new("Overrides")
        .style_when(move || dynamic_style.clone())
        .font_size(21.0)
        .line_height(29.0)
        .color(override_color)
        .resolved_style();
    assert_eq!(property_overrides.font_size, 21.0);
    assert_eq!(property_overrides.line_height, 29.0);
    assert_eq!(property_overrides.color, override_color);
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
            .any(|node| node.role == SemanticsRole::Image && node.name.as_deref() == Some("Agent"))
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
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Hello SUI"))
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
fn selectable_label_copy_shortcut_is_app_managed_by_default() -> Result<()> {
    let selection = SelectionScope::new();
    let (mut runtime, window_id) =
        build_runtime(Label::new("Hello SUI").selectable(selection.clone()));
    let output = runtime.render(window_id)?;
    let label = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Hello SUI"))
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
    assert!(!label.actions.contains(&SemanticsAction::Copy));

    runtime.handle_event(window_id, command_key("a"))?;
    assert_eq!(
        selection.selected_text().as_deref(),
        Some("Hello SUI"),
        "selectable labels should still publish selection state"
    );

    runtime.clipboard().set_text("app-owned");
    runtime.handle_event(window_id, command_key("c"))?;
    assert_eq!(runtime.clipboard().text().as_deref(), Some("app-owned"));

    runtime.clipboard().set_text("");
    runtime.handle_command(
        CommandTarget::FocusedWidget(window_id),
        CommandDelivery::Directed,
        TEXT_COMMAND,
        TextCommand::Copy,
    );
    assert_eq!(runtime.clipboard().text().as_deref(), Some("Hello SUI"));

    Ok(())
}

#[test]
fn selectable_label_can_opt_into_widget_managed_copy() -> Result<()> {
    let selection = SelectionScope::new();
    let (mut runtime, window_id) = build_runtime(
        Label::new("Hello SUI")
            .selectable(selection)
            .copy_to_clipboard(true),
    );
    let output = runtime.render(window_id)?;
    let label = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some("Hello SUI"))
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
    assert!(label.actions.contains(&SemanticsAction::Copy));

    runtime.handle_event(window_id, command_key("a"))?;
    runtime.handle_event(window_id, command_key("c"))?;
    assert_eq!(runtime.clipboard().text().as_deref(), Some("Hello SUI"));

    runtime.clipboard().set_text("");
    assert!(runtime.handle_semantics_action(window_id, label_id, SemanticsActionRequest::Copy,)?);
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
        Label::dynamic("Zoom --", move || text_reader.borrow().clone()).semantic_name("Zoom level"),
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
fn plain_choice_row_focus_wash_fades_without_black_rgb_flash() {
    let theme = DefaultTheme::default();
    let mid = super::choice_frame_visuals(
        &theme,
        ChoiceAppearance::Plain,
        theme.palette.control,
        theme.palette.border,
        0.0,
        0.0,
        0.5,
    )
    .background;
    let settled = super::choice_frame_visuals(
        &theme,
        ChoiceAppearance::Plain,
        theme.palette.control,
        theme.palette.border,
        0.0,
        0.0,
        1.0,
    )
    .background;

    assert!(mid.alpha > 0.0 && mid.alpha < settled.alpha);
    assert_color_approx_eq(mid.with_alpha(settled.alpha), settled);
    assert!(mid.red > 0.1 && mid.green > 0.1 && mid.blue > 0.1);
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
    let expected = super::mix_color(Color::TRANSPARENT, soft_fill, theme.interaction.hover_blend);
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
    let (mut runtime, window_id) = build_runtime(TextInput::new("Name").placeholder("Type a name"));

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
    let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").placeholder("Write notes"));

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

    let selected_base = theme.palette.selection;
    let selected_hover = super::mix_color(selected_base, theme.palette.control_hover, 0.35);
    assert_color_approx_eq(
        solid_fill_colors(&output)[0],
        super::mix_color(selected_base, selected_hover, theme.interaction.hover_blend),
    );
    assert!(
        solid_stroke_colors(&output).contains(&theme.palette.selection_border),
        "selected icon button should retain the accent selection border"
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
        IconGlyph::MessagesSquare,
        IconGlyph::Cloudy,
        IconGlyph::Folders,
        IconGlyph::Settings,
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
    let (mut runtime, window_id) = build_runtime(TextInput::new("Name").placeholder("Type a name"));

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
fn text_input_browser_back_clears_focus_and_disables_ime() -> Result<()> {
    let (mut runtime, window_id) = build_runtime(TextInput::new("API key").value("secret"));

    let _ = runtime.render(window_id)?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, Point::new(20.0, 16.0), true),
    )?;
    assert!(runtime.render(window_id)?.ime_composition_rect.is_some());

    assert!(runtime.dispatch_event(
        window_id,
        Event::Keyboard(KeyboardEvent::new("BrowserBack", KeyState::Pressed)),
    )?);
    assert_eq!(runtime.focused_widget(window_id)?, None);
    assert!(runtime.render(window_id)?.ime_composition_rect.is_none());
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
fn selectable_text_input_copy_shortcut_is_app_managed_by_default() -> Result<()> {
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
    runtime.clipboard().set_text("app-owned");
    runtime.handle_event(window_id, command_key("c"))?;

    assert_eq!(selection.selected_text().as_deref(), Some("Ada Lovelace"));
    assert_eq!(runtime.clipboard().text().as_deref(), Some("app-owned"));
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
            .selectable(selection.clone())
            .copy_to_clipboard(true),
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
fn text_input_can_opt_out_of_widget_managed_copy_shortcut() -> Result<()> {
    let (mut runtime, window_id) = build_runtime(
        TextInput::new("Name")
            .value("Ada Lovelace")
            .copy_to_clipboard(false),
    );

    let _ = runtime.render(window_id)?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, Point::new(8.0, 8.0), true),
    )?;
    runtime.handle_event(window_id, command_key("a"))?;
    runtime.clipboard().set_text("app-owned");
    runtime.handle_event(window_id, command_key("c"))?;
    let output = runtime.render(window_id)?;
    let input = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::TextInput)
        .expect("text input semantics present");

    assert_eq!(runtime.clipboard().text().as_deref(), Some("app-owned"));
    assert!(!input.actions.contains(&SemanticsAction::Copy));
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
        (text_run_visual_center(&text) - (input.bounds.y() + input.bounds.height() * 0.5)).abs()
            < 0.75
    );
    assert!(!solid_fill_colors(&output).contains(&theme.palette.caret));
    assert!(output.ime_composition_rect.is_none());
    Ok(())
}

#[test]
fn text_input_focus_animation_settles_into_blink_timer_without_frame_spin() -> Result<()> {
    let (mut runtime, window_id) = build_runtime(TextInput::new("Name").placeholder("Type a name"));

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
    let (mut runtime, window_id) = build_runtime(TextInput::new("Name").placeholder("Type a name"));

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
    let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").placeholder("Type notes"));

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
    let (mut runtime, window_id) = build_runtime(TextArea::new("Notes").placeholder("Type notes"));

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
fn button_constrained_label_clips_around_the_control_center() {
    const CONTROL_SIZE: Size = Size::new(160.0, 14.0);
    let theme = DefaultTheme::default();
    let style = TextStyle {
        font_size: 28.0,
        line_height: 34.0,
        ..theme.button_text_style()
    };
    let output = render(
        SizedBox::new()
            .size(CONTROL_SIZE)
            .with_child(Button::new("Centered").theme(theme).text_style(style)),
    );
    let text = first_shaped_text(&output);
    let layout = text
        .resolve(output.frame.text_layout_registry.as_ref())
        .expect("constrained button label layout should resolve");
    let line = layout
        .lines()
        .first()
        .expect("constrained button label should contain one line");
    let clip = draw_clip_rect_for(&output, "Centered");
    let control_center = CONTROL_SIZE.height * 0.5;
    let visual_center = text.origin.y + line.baseline + optical_visual_center(layout.measurement());

    assert!(
        clip.height() < layout.measurement().height,
        "fixture must actually constrain the label: clip={clip:?}, measurement={:?}",
        layout.measurement()
    );
    assert!(
        (clip.y() + (clip.height() * 0.5) - control_center).abs() < 0.01,
        "the visible label slice must remain centered in the control: {clip:?}"
    );
    assert!(
        (visual_center - control_center).abs() < 0.75,
        "the label itself must remain centered before clipping"
    );
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
    constrained.hdr.color_roles.accent = SemanticColorToken::from_sdr(constrained.palette.accent)
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
    assert_tall_body_text_centered(&radio_group, "Alpha", theme, theme.metrics.min_height * 0.5);
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
fn slider_value_when_updates_on_repaint_without_slider_event() -> Result<()> {
    let value = Rc::new(RefCell::new(0.25));
    let value_reader = Rc::clone(&value);
    let theme = DefaultTheme::default();
    let (mut runtime, window_id) = build_runtime(ExternalValueInvalidationHost::new(
        SizedBox::new().width(200.0).height(32.0).with_child(
            Slider::new("Opacity")
                .range(0.0, 1.0)
                .step(0.01)
                .value_when(move || *value_reader.borrow()),
        ),
    ));

    let initial = runtime.render(window_id)?;
    let initial_thumb_x = slider_thumb_center_x(&initial, theme.palette.accent);

    *value.borrow_mut() = 0.75;
    runtime.handle_event(
        window_id,
        Event::Custom(CustomEvent::new(INVALIDATE_EXTERNAL_SLIDER_VALUE_KIND)),
    )?;
    let updated = runtime.render(window_id)?;
    let updated_thumb_x = slider_thumb_center_x(&updated, theme.palette.accent);
    assert!(
        updated_thumb_x > initial_thumb_x + 40.0,
        "expected repaint-only external value change to move slider thumb from {initial_thumb_x} to the right, got {updated_thumb_x}"
    );

    let slider = updated
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Slider)
        .expect("slider semantics present after repaint-only external update");
    assert_eq!(
        slider.value,
        Some(SemanticsValue::Range {
            value: 0.75,
            min: 0.0,
            max: 1.0,
        })
    );
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
fn number_input_applies_typed_semantic_numeric_actions_through_step_and_callbacks() -> Result<()> {
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
        .find(|node| node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Count"))
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
        .find(|node| node.role == SemanticsRole::SpinBox && node.name.as_deref() == Some("Count"))
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
    let descriptor = overlay_layer_descriptor(&output).expect("select menu overlay layer present");

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
    let descriptor = overlay_layer_descriptor(&after).expect("select menu overlay layer present");

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
fn expanded_select_in_modal_dialog_paints_and_hits_above_later_body_content() -> Result<()> {
    let mut menu_theme = DefaultTheme::default();
    let menu_red = Color::srgba(0.92, 0.04, 0.04, 1.0);
    menu_theme.palette.surface_raised = menu_red;
    menu_theme.palette.control_hover = menu_red;
    menu_theme.palette.selection = menu_red;

    let mut button_theme = DefaultTheme::default();
    let button_green = Color::srgba(0.04, 0.92, 0.04, 1.0);
    button_theme.palette.control = button_green;
    button_theme.palette.border = button_green;

    let changes = Rc::new(RefCell::new(Vec::new()));
    let on_change = Rc::clone(&changes);
    let presses = Rc::new(RefCell::new(0usize));
    let on_press = Rc::clone(&presses);
    let body = Stack::vertical()
        .alignment(Alignment::Stretch)
        .with_child(
            Select::new("Mode")
                .theme(menu_theme)
                .options(["Automatic", "Linear", "Gamma"])
                .expanded(true)
                .on_change(move |_, value| on_change.borrow_mut().push(value)),
        )
        .with_child(
            Button::new("Later dialog action")
                .theme(button_theme)
                .appearance(ButtonAppearance::Filled)
                .min_width(320.0)
                .on_press(move || *on_press.borrow_mut() += 1),
        );
    let (mut runtime, window_id) = build_runtime(
        SizedBox::new()
            .size(Size::new(640.0, 420.0))
            .with_child(crate::Dialog::new("Choose mode", body).max_width(420.0)),
    );

    let _ = runtime.render(window_id)?;
    runtime.tick(entrance_duration());
    let _ = handle_ready_events(&mut runtime)?;
    let expanded = runtime.render(window_id)?;
    let select = expanded
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::ComboBox)
        .expect("select semantics present in modal dialog");
    let button = expanded
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some("Later dialog action")
        })
        .expect("later dialog button semantics present");
    let menu = overlay_layer_descriptor(&expanded).expect("select menu overlay layer present");
    let overlap = menu
        .bounds
        .intersection(button.bounds)
        .expect("expanded menu should overlap the later dialog button");
    let probe = Point::new(overlap.max_x() - 12.0, overlap.y() + overlap.height() * 0.5);
    assert!(menu.bounds.contains(probe));
    assert!(button.bounds.contains(probe));

    let mut renderer = WgpuRenderer::default().with_feathering_enabled(false);
    renderer.render(&expanded.frame)?;
    let image = renderer.capture_last_frame_rgba(window_id)?;
    let probe_x = probe.x.floor() as usize;
    let probe_y = probe.y.floor() as usize;
    let pixel_offset = ((probe_y * image.width() as usize) + probe_x) * 4;
    let pixel = &image.pixels()[pixel_offset..pixel_offset + 4];
    assert!(
        pixel[0] > pixel[1],
        "the red select menu must paint above the later green dialog button; probe={probe:?}, rgba={pixel:?}, menu={:?}, button={:?}",
        menu.bounds,
        button.bounds,
    );

    let option_point = Point::new(
        menu.bounds.x() + 20.0,
        menu.bounds.y() + select.bounds.height() * 0.5,
    );
    assert!(button.bounds.contains(option_point));
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Down, option_point, true),
    )?;
    runtime.handle_event(
        window_id,
        primary_pointer(PointerEventKind::Up, option_point, false),
    )?;

    assert_eq!(changes.borrow().as_slice(), &["Automatic".to_string()]);
    assert_eq!(*presses.borrow(), 0);
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
    let output = render(
        SizedBox::new().width(180.0).with_child(
            Select::new("Mode")
                .placeholder(placeholder)
                .options(["Automatic", "Linear", "Gamma"]),
        ),
    );
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
        (text_run_visual_center(&text) - (select.bounds.y() + select.bounds.height() * 0.5)).abs()
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
