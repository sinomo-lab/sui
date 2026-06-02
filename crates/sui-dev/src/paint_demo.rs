use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;

use crate::app::{labeled_settings_control, request_window_refresh};

pub(crate) const PAINT_TAB_LABEL: &str = "Paint";
pub(crate) const PAINT_DOCUMENT_WIDTH: usize = 1920;
pub(crate) const PAINT_DOCUMENT_HEIGHT: usize = 1080;
pub(crate) const PAINT_INITIAL_BRUSH_SIZE: f32 = 18.0;
pub(crate) const PAINT_BRUSH_COLOR_NAME: &str = "Brush color";
pub(crate) const PAINT_BRUSH_SIZE_NAME: &str = "Brush size";
pub(crate) const PAINT_BRUSH_OPACITY_NAME: &str = "Brush opacity";
pub(crate) const PAINT_BRUSH_SHAPE_NAME: &str = "Brush shape";
pub(crate) const PAINT_BLEND_MODE_NAME: &str = "Blend mode";
pub(crate) const PAINT_LAYERS_NAME: &str = "Layers";
pub(crate) const PAINT_SCROLL_NAME: &str = "Paint controls";
pub(crate) const PAINT_LAYER_NAMES: [&str; 2] = ["Paint", "Paper"];

#[derive(Clone)]
struct PaintDemoState {
    inner: Rc<RefCell<PaintDemoStateInner>>,
}

struct PaintDemoStateInner {
    selected_layer: usize,
}

impl PaintDemoState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(PaintDemoStateInner { selected_layer: 0 })),
        }
    }

    fn selected_layer(&self) -> usize {
        self.inner.borrow().selected_layer
    }

    fn selected_layer_name(&self) -> &'static str {
        paint_layer_name(self.selected_layer())
    }

    fn set_selected_layer(&self, selected_layer: usize) {
        if selected_layer < PAINT_LAYER_NAMES.len() {
            self.inner.borrow_mut().selected_layer = selected_layer;
        }
    }
}

pub(crate) fn build_paint_demo() -> impl Widget {
    let paint_state = PixelCanvasState::new();
    let demo_state = PaintDemoState::new();
    paint_state.set_brush_color(Color::rgba(0.08, 0.22, 0.78, 1.0));
    paint_state.set_brush_size(PAINT_INITIAL_BRUSH_SIZE);

    Background::new(
        Color::rgba(0.925, 0.94, 0.96, 1.0),
        StatusBarHost::new(
            Stack::vertical()
                .alignment(Alignment::Stretch)
                .with_child(build_paint_toolbar(paint_state.clone()))
                .with_child(
                    SplitView::horizontal(
                        build_paint_tool_rail(paint_state.clone()),
                        SplitView::horizontal(
                            build_paint_canvas_stage(paint_state.clone()),
                            build_paint_inspector(paint_state.clone(), demo_state.clone()),
                        )
                        .name("Canvas and inspector")
                        .ratio(0.76)
                        .min_first(420.0)
                        .min_second(336.0)
                        .divider_thickness(4.0),
                    )
                    .name("Paint workspace")
                    .ratio(0.039)
                    .min_first(54.0)
                    .min_second(640.0)
                    .divider_thickness(4.0),
                ),
            build_paint_status_bar(paint_state, demo_state),
        ),
    )
}

fn build_paint_toolbar(paint_state: PixelCanvasState) -> impl Widget {
    let undo_state = paint_state.clone();
    let undo_enabled_state = paint_state.clone();
    let redo_state = paint_state.clone();
    let redo_enabled_state = paint_state.clone();
    let fit_state = paint_state.clone();
    let actual_size_state = paint_state.clone();
    let clear_state = paint_state.clone();
    let clear_enabled_state = paint_state.clone();
    let export_state = paint_state.clone();
    Toolbar::horizontal()
        .name("Paint toolbar")
        .extent(44.0)
        .background(Color::rgba(0.97, 0.978, 0.988, 1.0))
        .padding(Insets::all(6.0))
        .spacing(7.0)
        .with_child(
            Label::new("SUI Paint")
                .font_size(15.0)
                .line_height(20.0)
                .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
        )
        .with_child(Separator::vertical().length(24.0))
        .with_child(
            IconButton::new(IconGlyph::Undo, "Undo")
                .size(30.0)
                .icon_size(14.0)
                .enabled_when(move || undo_enabled_state.can_undo())
                .on_press_with_ctx(move |ctx| {
                    undo_state.request_undo();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            IconButton::new(IconGlyph::Redo, "Redo")
                .size(30.0)
                .icon_size(14.0)
                .enabled_when(move || redo_enabled_state.can_redo())
                .on_press_with_ctx(move |ctx| {
                    redo_state.request_redo();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(Separator::vertical().length(24.0))
        .with_child(
            Button::new("Fit")
                .min_width(48.0)
                .min_height(30.0)
                .on_press_with_ctx(move |ctx| {
                    fit_state.request_fit_view();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            Button::new("100%")
                .min_width(54.0)
                .min_height(30.0)
                .on_press_with_ctx(move |ctx| {
                    actual_size_state.request_actual_size_view();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            Button::new("Clear")
                .icon(IconGlyph::Trash)
                .icon_size(14.0)
                .min_width(62.0)
                .min_height(30.0)
                .enabled_when(move || clear_enabled_state.can_clear())
                .on_press_with_ctx(move |ctx| {
                    clear_state.request_clear();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            Button::new("Export")
                .icon(IconGlyph::Download)
                .icon_size(14.0)
                .min_width(70.0)
                .min_height(30.0)
                .on_press_with_ctx(move |ctx| {
                    export_state.request_export_snapshot();
                    request_window_refresh(ctx, true);
                }),
        )
}

fn build_paint_tool_rail(paint_state: PixelCanvasState) -> impl Widget {
    Toolbar::vertical()
        .name("Paint tools")
        .extent(52.0)
        .background(Color::rgba(0.955, 0.965, 0.978, 1.0))
        .padding(Insets::all(6.0))
        .spacing(6.0)
        .with_child(paint_tool_button(
            paint_state.clone(),
            PixelCanvasTool::Brush,
            IconGlyph::Brush,
            "Brush tool",
        ))
        .with_child(paint_tool_button(
            paint_state.clone(),
            PixelCanvasTool::Eraser,
            IconGlyph::Eraser,
            "Eraser tool",
        ))
        .with_child(paint_tool_button(
            paint_state.clone(),
            PixelCanvasTool::Fill,
            IconGlyph::PaintBucket,
            "Fill tool",
        ))
        .with_child(paint_tool_button(
            paint_state,
            PixelCanvasTool::Pan,
            IconGlyph::Hand,
            "Pan tool",
        ))
}

fn paint_tool_button(
    paint_state: PixelCanvasState,
    tool: PixelCanvasTool,
    icon: IconGlyph,
    label: &'static str,
) -> impl Widget {
    let selected_state = paint_state.clone();
    IconButton::new(icon, label)
        .size(40.0)
        .icon_size(20.0)
        .selected_when(move || selected_state.tool() == tool)
        .on_press_with_ctx(move |ctx| {
            paint_state.set_tool(tool);
            request_window_refresh(ctx, true);
        })
}

fn build_paint_status_bar(
    paint_state: PixelCanvasState,
    demo_state: PaintDemoState,
) -> impl Widget {
    let tool_state = paint_state.clone();
    let zoom_state = paint_state.clone();
    let brush_state = paint_state.clone();
    let blend_state = paint_state.clone();
    let export_state = paint_state;
    let layer_state = demo_state;

    StatusBar::new()
        .name("Paint status")
        .height(28.0)
        .segment(
            StatusBarSegment::dynamic("Tool Brush", move || {
                format!("Tool {}", tool_state.tool().label())
            })
            .min_width(108.0),
        )
        .segment(
            StatusBarSegment::dynamic("Zoom --", move || paint_zoom_status_text(&zoom_state))
                .min_width(92.0),
        )
        .segment(
            StatusBarSegment::dynamic("Brush 18 px", move || {
                format!(
                    "Brush {:.0} px / {:.0}%",
                    brush_state.brush_size(),
                    brush_state.brush_opacity() * 100.0
                )
            })
            .min_width(150.0),
        )
        .segment(
            StatusBarSegment::dynamic("Blend Normal", move || {
                format!("Blend {}", blend_state.blend_mode().label())
            })
            .min_width(132.0),
        )
        .segment(
            StatusBarSegment::dynamic("Layer Paint", move || {
                format!("Layer {}", layer_state.selected_layer_name())
            })
            .min_width(112.0),
        )
        .segment(
            StatusBarSegment::new(format!(
                "Document {} x {} px",
                PAINT_DOCUMENT_WIDTH, PAINT_DOCUMENT_HEIGHT
            ))
            .min_width(180.0),
        )
        .segment(
            StatusBarSegment::dynamic("Ready", move || paint_export_status_text(&export_state))
                .min_width(220.0)
                .expand(true),
        )
}

fn paint_zoom_status_text(state: &PixelCanvasState) -> String {
    let viewport = state.viewport();
    let viewport_size = state.viewport_size();
    if viewport_size.width <= 0.0 || viewport_size.height <= 0.0 {
        "Zoom --".to_string()
    } else {
        format!("Zoom {:.0}%", viewport.zoom * 100.0)
    }
}

fn build_paint_canvas_stage(paint_state: PixelCanvasState) -> impl Widget {
    Background::new(
        Color::rgba(0.875, 0.89, 0.91, 1.0),
        Padding::all(
            10.0,
            Stack::vertical().alignment(Alignment::Stretch).with_child(
                PixelCanvas::from_fn(
                    PAINT_TAB_LABEL,
                    PAINT_DOCUMENT_WIDTH,
                    PAINT_DOCUMENT_HEIGHT,
                    |x, y| {
                        let u = x as f32 / (PAINT_DOCUMENT_WIDTH - 1) as f32;
                        let v = y as f32 / (PAINT_DOCUMENT_HEIGHT - 1) as f32;
                        seeded_paint_color(u, v)
                    },
                )
                .state(paint_state)
                .desired_size(Size::new(960.0, 620.0))
                .fit_on_first_layout(),
            ),
        ),
    )
}

fn build_paint_inspector(paint_state: PixelCanvasState, demo_state: PaintDemoState) -> impl Widget {
    let color_state = paint_state.clone();
    let size_state = paint_state.clone();
    let opacity_state = paint_state.clone();
    let shape_state = paint_state.clone();
    let blend_mode_state = paint_state.clone();

    Background::new(
        Color::rgba(0.965, 0.972, 0.982, 1.0),
        ScrollView::vertical(Padding::all(
            10.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(PanelSection::new(
                    "Brush",
                    Stack::vertical()
                        .spacing(8.0)
                        .alignment(Alignment::Stretch)
                        .with_child(labeled_settings_control(
                            PAINT_BRUSH_SIZE_NAME,
                            96.0,
                            NumberInput::new(PAINT_BRUSH_SIZE_NAME)
                                .range(1.0, 96.0)
                                .step(1.0)
                                .precision(0)
                                .value(paint_state.brush_size() as f64)
                                .on_change(move |value| {
                                    size_state.set_brush_size(value as f32);
                                }),
                        ))
                        .with_child(labeled_settings_control(
                            PAINT_BRUSH_OPACITY_NAME,
                            188.0,
                            Slider::new(PAINT_BRUSH_OPACITY_NAME)
                                .range(0.0, 1.0)
                                .step(0.01)
                                .value(paint_state.brush_opacity() as f64)
                                .on_change(move |value| {
                                    opacity_state.set_brush_opacity(value as f32);
                                }),
                        ))
                        .with_child(labeled_settings_control(
                            PAINT_BRUSH_SHAPE_NAME,
                            188.0,
                            Select::new(PAINT_BRUSH_SHAPE_NAME)
                                .options(
                                    PixelCanvasBrushShape::ALL.map(PixelCanvasBrushShape::label),
                                )
                                .selected(paint_brush_shape_selected_index(
                                    paint_state.brush_shape(),
                                ))
                                .on_change(move |index, _| {
                                    if let Some(shape) =
                                        PixelCanvasBrushShape::ALL.get(index).copied()
                                    {
                                        shape_state.set_brush_shape(shape);
                                    }
                                }),
                        ))
                        .with_child(labeled_settings_control(
                            PAINT_BLEND_MODE_NAME,
                            188.0,
                            Select::new(PAINT_BLEND_MODE_NAME)
                                .options(PixelCanvasBlendMode::ALL.map(PixelCanvasBlendMode::label))
                                .selected(paint_blend_mode_selected_index(paint_state.blend_mode()))
                                .on_change(move |index, _| {
                                    if let Some(mode) =
                                        PixelCanvasBlendMode::ALL.get(index).copied()
                                    {
                                        blend_mode_state.set_blend_mode(mode);
                                    }
                                }),
                        )),
                ))
                .with_child(PanelSection::new(
                    PAINT_LAYERS_NAME,
                    build_paint_layers_panel(demo_state),
                ))
                .with_child(PanelSection::new(
                    "Color",
                    SizedBox::new().width(312.0).height(350.0).with_child(
                        ColorPicker::from_color(PAINT_BRUSH_COLOR_NAME, paint_state.brush_color())
                            .compact(true)
                            .show_alpha(false)
                            .on_change(move |color| {
                                color_state.set_brush_color(color);
                            }),
                    ),
                )),
        ))
        .name(PAINT_SCROLL_NAME),
    )
}

fn build_paint_layers_panel(state: PaintDemoState) -> impl Widget {
    let selected_layer = state.selected_layer();
    let selection_state = state;
    SizedBox::new().width(312.0).height(104.0).with_child(
        ListView::new(PAINT_LAYERS_NAME)
            .items([
                ListItem::new("Paint")
                    .detail("Normal / 100%")
                    .accent(Color::rgba(0.16, 0.31, 0.88, 1.0)),
                ListItem::new("Paper")
                    .detail("Background")
                    .accent(Color::rgba(0.89, 0.91, 0.94, 1.0)),
            ])
            .selected(selected_layer)
            .row_height(30.0)
            .on_change(move |index, _| selection_state.set_selected_layer(index)),
    )
}

fn paint_layer_name(index: usize) -> &'static str {
    PAINT_LAYER_NAMES
        .get(index)
        .copied()
        .unwrap_or(PAINT_LAYER_NAMES[0])
}

fn paint_brush_shape_selected_index(shape: PixelCanvasBrushShape) -> usize {
    PixelCanvasBrushShape::ALL
        .iter()
        .position(|candidate| *candidate == shape)
        .unwrap_or(0)
}

fn paint_blend_mode_selected_index(mode: PixelCanvasBlendMode) -> usize {
    PixelCanvasBlendMode::ALL
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or(0)
}

fn paint_export_status_text(state: &PixelCanvasState) -> String {
    match state.latest_export_snapshot() {
        Some(snapshot) => format!(
            "Exported {} x {} px RGBA8, {}",
            snapshot.width(),
            snapshot.height(),
            format_export_bytes(snapshot.byte_len())
        ),
        None => "Ready".to_string(),
    }
}

fn format_export_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

fn seeded_paint_color(u: f32, v: f32) -> Color {
    let dx = u - 0.5;
    let dy = v - 0.5;
    let vignette = (1.0 - ((dx * dx + dy * dy).sqrt() * 1.45)).clamp(0.0, 1.0);
    let wave = ((u * 18.0).sin() * (v * 11.0).cos() * 0.5) + 0.5;
    Color::rgba(
        0.08 + (0.58 * u) + (0.18 * vignette),
        0.18 + (0.42 * v) + (0.14 * wave),
        0.38 + (0.36 * (1.0 - u)) + (0.20 * vignette),
        1.0,
    )
}
