use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;

#[cfg(test)]
use crate::app::default_dev_theme_reader;
use crate::app::{
    DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color, request_window_refresh,
};

pub(crate) const PAINT_TAB_LABEL: &str = "Paint";
pub(crate) const PAINT_DOCUMENT_WIDTH: usize = 1920;
pub(crate) const PAINT_DOCUMENT_HEIGHT: usize = 1080;
pub(crate) const PAINT_INITIAL_BRUSH_SIZE: f32 = 18.0;
pub(crate) const PAINT_BRUSH_COLOR_NAME: &str = "Brush color";
pub(crate) const PAINT_BRUSH_PREVIEW_NAME: &str = "Brush preview";
pub(crate) const PAINT_ERASER_PREVIEW_NAME: &str = "Eraser preview";
pub(crate) const PAINT_BRUSH_SIZE_PRESETS_NAME: &str = "Brush size presets";
pub(crate) const PAINT_BRUSH_SIZE_NAME: &str = "Brush size";
pub(crate) const PAINT_BRUSH_OPACITY_NAME: &str = "Brush opacity";
pub(crate) const PAINT_BRUSH_SHAPE_NAME: &str = "Brush shape";
pub(crate) const PAINT_BLEND_MODE_NAME: &str = "Blend mode";
pub(crate) const PAINT_FILL_OPACITY_NAME: &str = "Fill opacity";
pub(crate) const PAINT_FILL_BLEND_MODE_NAME: &str = "Fill blend mode";
pub(crate) const PAINT_FIT_VIEW_NAME: &str = "Fit view";
pub(crate) const PAINT_ACTUAL_SIZE_NAME: &str = "Actual size";
pub(crate) const PAINT_COLOR_PRESETS_NAME: &str = "Color presets";
pub(crate) const PAINT_COLOR_EDITOR_NAME: &str = "Color editor";
pub(crate) const PAINT_LAYERS_NAME: &str = "Layers";
pub(crate) const PAINT_LAYER_OPACITY_NAME: &str = "Layer opacity";
pub(crate) const PAINT_LAYER_BLEND_MODE_NAME: &str = "Layer blend mode";
pub(crate) const PAINT_SELECT_LAYER_ABOVE_NAME: &str = "Select layer above";
pub(crate) const PAINT_SELECT_LAYER_BELOW_NAME: &str = "Select layer below";
pub(crate) const PAINT_SCROLL_NAME: &str = "Paint controls";
pub(crate) const PAINT_PROPERTIES_NAME: &str = "Tool properties";
pub(crate) const PAINT_DOCUMENT_BAR_NAME: &str = "Document bar";
pub(crate) const PAINT_DOCUMENT_VIEW_COMMANDS_NAME: &str = "Document view controls";
pub(crate) const PAINT_ZOOM_OUT_NAME: &str = "Zoom out";
pub(crate) const PAINT_ZOOM_READOUT_NAME: &str = "Zoom level";
pub(crate) const PAINT_ZOOM_IN_NAME: &str = "Zoom in";
pub(crate) const PAINT_HISTORY_COMMANDS_NAME: &str = "History commands";
pub(crate) const PAINT_VIEW_COMMANDS_NAME: &str = "View commands";
pub(crate) const PAINT_DOCUMENT_COMMANDS_NAME: &str = "Document commands";
pub(crate) const PAINT_HORIZONTAL_RULER_NAME: &str = "Horizontal ruler";
pub(crate) const PAINT_VERTICAL_RULER_NAME: &str = "Vertical ruler";
pub(crate) const PAINT_DOCUMENT_NAME: &str = "Untitled.sui";
pub(crate) const PAINT_LAYER_NAMES: [&str; 2] = ["Paint", "Paper"];

const PAINT_RULER_EXTENT: f32 = 30.0;
const PAINT_TOOLS: [PixelCanvasTool; 4] = [
    PixelCanvasTool::Brush,
    PixelCanvasTool::Eraser,
    PixelCanvasTool::Fill,
    PixelCanvasTool::Pan,
];
const PAINT_BRUSH_SIZE_PRESETS: [f32; 4] = [8.0, 18.0, 36.0, 72.0];
const PAINT_COLOR_PRESETS: [(&str, Color); 8] = [
    ("Ink", Color::rgba(0.08, 0.10, 0.15, 1.0)),
    ("Ocean", Color::rgba(0.08, 0.22, 0.78, 1.0)),
    ("Sky", Color::rgba(0.16, 0.58, 0.92, 1.0)),
    ("Mint", Color::rgba(0.28, 0.78, 0.58, 1.0)),
    ("Leaf", Color::rgba(0.38, 0.68, 0.22, 1.0)),
    ("Amber", Color::rgba(0.96, 0.66, 0.18, 1.0)),
    ("Coral", Color::rgba(0.90, 0.32, 0.18, 1.0)),
    ("Violet", Color::rgba(0.54, 0.30, 0.84, 1.0)),
];

#[derive(Clone)]
struct PaintDemoState {
    inner: Rc<RefCell<PaintDemoStateInner>>,
}

struct PaintDemoStateInner {
    selected_layer: usize,
    layer_order: Vec<usize>,
    layer_visible: [bool; PAINT_LAYER_NAMES.len()],
    layer_locked: [bool; PAINT_LAYER_NAMES.len()],
    layer_opacity: [f32; PAINT_LAYER_NAMES.len()],
    layer_blend_mode: [PixelCanvasBlendMode; PAINT_LAYER_NAMES.len()],
}

impl PaintDemoState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(PaintDemoStateInner {
                selected_layer: 0,
                layer_order: (0..PAINT_LAYER_NAMES.len()).collect(),
                layer_visible: [true; PAINT_LAYER_NAMES.len()],
                layer_locked: [false, true],
                layer_opacity: [1.0; PAINT_LAYER_NAMES.len()],
                layer_blend_mode: [PixelCanvasBlendMode::Normal; PAINT_LAYER_NAMES.len()],
            })),
        }
    }

    fn selected_layer(&self) -> usize {
        self.inner.borrow().selected_layer
    }

    fn selected_layer_visual_index(&self) -> usize {
        let inner = self.inner.borrow();
        inner
            .layer_order
            .iter()
            .position(|layer| *layer == inner.selected_layer)
            .unwrap_or(inner.selected_layer)
    }

    fn selected_layer_name(&self) -> &'static str {
        paint_layer_name(self.selected_layer())
    }

    fn set_selected_layer(&self, selected_layer: usize) {
        if selected_layer < PAINT_LAYER_NAMES.len() {
            self.inner.borrow_mut().selected_layer = selected_layer;
        }
    }

    fn layer_at_visual_index(&self, index: usize) -> usize {
        self.inner
            .borrow()
            .layer_order
            .get(index)
            .copied()
            .unwrap_or(index)
    }

    fn set_selected_visual_layer(&self, index: usize) {
        self.set_selected_layer(self.layer_at_visual_index(index));
    }

    fn reorder_layers(&self, from: usize, to: usize) {
        let mut inner = self.inner.borrow_mut();
        if from >= inner.layer_order.len() || to >= inner.layer_order.len() || from == to {
            return;
        }
        let layer = inner.layer_order.remove(from);
        inner.layer_order.insert(to, layer);
    }

    fn paint_layer_is_above_paper(&self) -> bool {
        let inner = self.inner.borrow();
        let paint = inner.layer_order.iter().position(|layer| *layer == 0);
        let paper = inner.layer_order.iter().position(|layer| *layer == 1);
        match (paint, paper) {
            (Some(paint), Some(paper)) => paint < paper,
            _ => true,
        }
    }

    fn layer_visible(&self, index: usize) -> bool {
        self.inner
            .borrow()
            .layer_visible
            .get(index)
            .copied()
            .unwrap_or(true)
    }

    fn set_layer_visible(&self, index: usize, visible: bool) {
        if let Some(layer_visible) = self.inner.borrow_mut().layer_visible.get_mut(index) {
            *layer_visible = visible;
        }
    }

    fn layer_locked(&self, index: usize) -> bool {
        self.inner
            .borrow()
            .layer_locked
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    fn selected_layer_locked(&self) -> bool {
        self.layer_locked(self.selected_layer())
    }

    fn set_layer_locked(&self, index: usize, locked: bool) {
        if let Some(layer_locked) = self.inner.borrow_mut().layer_locked.get_mut(index) {
            *layer_locked = locked;
        }
    }

    fn sync_canvas_editable(&self, paint_state: &PixelCanvasState) {
        paint_state.set_editable(!self.selected_layer_locked());
    }

    fn sync_canvas_layers(&self, paint_state: &PixelCanvasState) {
        paint_state.set_display_visible(self.layer_visible(0));
        paint_state.set_display_opacity(self.layer_opacity(0));
        paint_state.set_display_blend_mode(self.layer_blend_mode(0));
        paint_state.set_display_above_paper(self.paint_layer_is_above_paper());
        paint_state.set_paper_visible(self.layer_visible(1));
        paint_state.set_paper_opacity(self.layer_opacity(1));
    }

    fn layer_opacity(&self, index: usize) -> f32 {
        self.inner
            .borrow()
            .layer_opacity
            .get(index)
            .copied()
            .unwrap_or(1.0)
    }

    fn selected_layer_opacity(&self) -> f32 {
        self.layer_opacity(self.selected_layer())
    }

    fn set_layer_opacity(&self, index: usize, opacity: f32) {
        if let Some(layer_opacity) = self.inner.borrow_mut().layer_opacity.get_mut(index) {
            *layer_opacity = opacity.clamp(0.0, 1.0);
        }
    }

    fn set_selected_layer_opacity(&self, opacity: f32) {
        self.set_layer_opacity(self.selected_layer(), opacity);
    }

    fn layer_blend_mode(&self, index: usize) -> PixelCanvasBlendMode {
        self.inner
            .borrow()
            .layer_blend_mode
            .get(index)
            .copied()
            .unwrap_or_default()
    }

    fn selected_layer_blend_mode(&self) -> PixelCanvasBlendMode {
        self.layer_blend_mode(self.selected_layer())
    }

    fn set_layer_blend_mode(&self, index: usize, mode: PixelCanvasBlendMode) {
        if let Some(layer_blend_mode) = self.inner.borrow_mut().layer_blend_mode.get_mut(index) {
            *layer_blend_mode = mode;
        }
    }

    fn set_selected_layer_blend_mode(&self, mode: PixelCanvasBlendMode) {
        self.set_layer_blend_mode(self.selected_layer(), mode);
    }

    fn can_select_layer_above(&self) -> bool {
        self.selected_layer_visual_index() > 0
    }

    fn can_select_layer_below(&self) -> bool {
        self.selected_layer_visual_index() + 1 < PAINT_LAYER_NAMES.len()
    }

    fn select_layer_above(&self) {
        if self.can_select_layer_above() {
            self.set_selected_visual_layer(self.selected_layer_visual_index() - 1);
        }
    }

    fn select_layer_below(&self) {
        if self.can_select_layer_below() {
            self.set_selected_visual_layer(self.selected_layer_visual_index() + 1);
        }
    }
}

pub(crate) fn build_paint_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let paint_state = PixelCanvasState::new();
    build_paint_demo_with_state_and_theme(paint_state, theme_reader)
}

#[cfg(test)]
pub(crate) fn build_paint_demo_with_state(paint_state: PixelCanvasState) -> impl Widget {
    build_paint_demo_with_state_and_theme(paint_state, default_dev_theme_reader())
}

fn build_paint_demo_with_state_and_theme(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let demo_state = PaintDemoState::new();
    paint_state.set_brush_color(Color::rgba(0.08, 0.22, 0.78, 1.0));
    paint_state.set_brush_size(PAINT_INITIAL_BRUSH_SIZE);
    paint_state.set_brush_shape(PixelCanvasBrushShape::Round);
    demo_state.sync_canvas_editable(&paint_state);
    demo_state.sync_canvas_layers(&paint_state);

    Background::new(
        Color::rgba(0.925, 0.94, 0.96, 1.0),
        StatusBarHost::new(
            Stack::vertical()
                .alignment(Alignment::Stretch)
                .with_child(build_paint_toolbar(
                    paint_state.clone(),
                    Rc::clone(&theme_reader),
                ))
                .with_child(
                    SplitView::horizontal(
                        build_paint_tool_rail(paint_state.clone(), Rc::clone(&theme_reader)),
                        SplitView::horizontal(
                            build_paint_canvas_stage(paint_state.clone(), Rc::clone(&theme_reader)),
                            build_paint_properties_panel(
                                paint_state.clone(),
                                demo_state.clone(),
                                Rc::clone(&theme_reader),
                            ),
                        )
                        .name("Canvas and properties")
                        .ratio(0.79)
                        .min_first(420.0)
                        .min_second(304.0),
                    )
                    .name("Paint workspace")
                    .ratio(0.039)
                    .min_first(54.0)
                    .min_second(640.0),
                ),
            build_paint_status_bar(paint_state, demo_state, Rc::clone(&theme_reader)),
        ),
    )
    .brush_when(move || theme_reader().palette.surface)
}

fn build_paint_toolbar(paint_state: PixelCanvasState, theme_reader: DevThemeReader) -> impl Widget {
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
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .extent(44.0)
        .padding(Insets::all(6.0))
        .spacing(8.0)
        .with_child(
            Label::new("SUI Paint")
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.base,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(
            paint_command_group(PAINT_HISTORY_COMMANDS_NAME, &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::Undo, "Undo")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .enabled_when(move || undo_enabled_state.can_undo())
                        .on_press_with_ctx(move |ctx| {
                            undo_state.request_undo();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Redo, "Redo")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .enabled_when(move || redo_enabled_state.can_redo())
                        .on_press_with_ctx(move |ctx| {
                            redo_state.request_redo();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
        .with_child(
            paint_command_group(PAINT_VIEW_COMMANDS_NAME, &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::FitView, PAINT_FIT_VIEW_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(15.0)
                        .on_press_with_ctx(move |ctx| {
                            fit_state.request_fit_view();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::ActualSize, PAINT_ACTUAL_SIZE_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(15.0)
                        .on_press_with_ctx(move |ctx| {
                            actual_size_state.request_actual_size_view();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
        .with_child(
            paint_command_group(PAINT_DOCUMENT_COMMANDS_NAME, &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::Trash, "Clear")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .enabled_when(move || clear_enabled_state.can_clear())
                        .on_press_with_ctx(move |ctx| {
                            clear_state.request_clear();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Download, "Export")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            export_state.request_export_snapshot();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
}

fn paint_command_group(name: &'static str, theme_reader: &DevThemeReader) -> CommandGroup {
    CommandGroup::horizontal(name)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(2.0))
        .spacing(2.0)
        .corner_radius(6.0)
}

fn build_paint_tool_rail(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let selected_state = paint_state.clone();
    ToolPalette::vertical("Paint tools")
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .extent(52.0)
        .padding(Insets::all(6.0))
        .spacing(6.0)
        .items(paint_tool_palette_items())
        .selected(paint_tool_selected_index(paint_state.tool()))
        .selected_when(move || Some(paint_tool_selected_index(selected_state.tool())))
        .on_change_with_ctx(move |ctx, index, _| {
            if let Some(tool) = paint_tool_at(index) {
                paint_state.set_tool(tool);
                request_window_refresh(ctx, true);
            }
        })
}

fn build_paint_status_bar(
    paint_state: PixelCanvasState,
    demo_state: PaintDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let tool_state = paint_state.clone();
    let zoom_state = paint_state.clone();
    let brush_state = paint_state.clone();
    let blend_state = paint_state.clone();
    let cursor_state = paint_state;
    let layer_state = demo_state;

    StatusBar::new()
        .name("Paint status")
        .theme_when(clone_dev_theme_reader(&theme_reader))
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
                paint_tool_parameter_status_text(&brush_state)
            })
            .min_width(150.0),
        )
        .segment(
            StatusBarSegment::dynamic("Blend Normal", move || {
                format!("Blend {}", blend_state.blend_mode().label())
            })
            .min_width(132.0),
        )
        .segment(StatusBarSegment::dynamic(
            "Layer Paint / Normal / 100% / Unlocked",
            move || paint_layer_status_text(&layer_state),
        ))
        .segment(
            StatusBarSegment::new(format!(
                "Document {} x {} px",
                PAINT_DOCUMENT_WIDTH, PAINT_DOCUMENT_HEIGHT
            ))
            .min_width(180.0),
        )
        .segment(
            StatusBarSegment::dynamic("Cursor --", move || paint_cursor_status_text(&cursor_state))
                .min_width(140.0)
                .expand(true),
        )
}

fn paint_tool_parameter_status_text(state: &PixelCanvasState) -> String {
    match state.tool() {
        PixelCanvasTool::Brush => format!(
            "Brush {:.0} px / {:.0}%",
            state.brush_size(),
            state.brush_opacity() * 100.0
        ),
        PixelCanvasTool::Eraser => format!(
            "Eraser {:.0} px / {:.0}%",
            state.brush_size(),
            state.brush_opacity() * 100.0
        ),
        PixelCanvasTool::Fill => format!("Fill {:.0}%", state.brush_opacity() * 100.0),
        PixelCanvasTool::Pan => "Pan view".to_string(),
    }
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

fn paint_cursor_status_text(state: &PixelCanvasState) -> String {
    match state.cursor_position() {
        Some(point) => format!("Cursor {:.0}, {:.0}", point.x, point.y),
        None => "Cursor --".to_string(),
    }
}

fn paint_layer_status_text(state: &PaintDemoState) -> String {
    format!(
        "Layer {} / {} / {} / {}",
        state.selected_layer_name(),
        state.selected_layer_blend_mode().label(),
        paint_layer_opacity_text(state.selected_layer_opacity()),
        if state.selected_layer_locked() {
            "Locked"
        } else {
            "Unlocked"
        }
    )
}

fn paint_layer_detail(state: &PaintDemoState, index: usize) -> String {
    format!(
        "{} / {}",
        state.layer_blend_mode(index).label(),
        paint_layer_opacity_text(state.layer_opacity(index))
    )
}

fn paint_layer_opacity_text(opacity: f32) -> String {
    format!("{:.0}%", opacity.clamp(0.0, 1.0) * 100.0)
}

fn build_paint_canvas_stage(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let horizontal_ruler_state = paint_state.clone();
    let vertical_ruler_state = paint_state.clone();
    let document_size = paint_document_size();
    Background::new(
        Color::rgba(0.89, 0.905, 0.925, 1.0),
        Stack::vertical()
            .alignment(Alignment::Stretch)
            .with_child(build_paint_document_bar(
                paint_state.clone(),
                Rc::clone(&theme_reader),
            ))
            .with_child(Padding::all(
                12.0,
                Stack::vertical()
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Stack::horizontal()
                            .with_child(paint_ruler_corner(Rc::clone(&theme_reader)))
                            .with_child(
                                CanvasRuler::horizontal(PAINT_HORIZONTAL_RULER_NAME, document_size)
                                    .theme_when(clone_dev_theme_reader(&theme_reader))
                                    .extent(PAINT_RULER_EXTENT)
                                    .viewport_when(move || {
                                        (
                                            horizontal_ruler_state.viewport(),
                                            horizontal_ruler_state.viewport_size(),
                                        )
                                    }),
                            ),
                    )
                    .with_child(
                        Stack::horizontal()
                            .alignment(Alignment::Stretch)
                            .with_child(
                                CanvasRuler::vertical(PAINT_VERTICAL_RULER_NAME, document_size)
                                    .theme_when(clone_dev_theme_reader(&theme_reader))
                                    .extent(PAINT_RULER_EXTENT)
                                    .viewport_when(move || {
                                        (
                                            vertical_ruler_state.viewport(),
                                            vertical_ruler_state.viewport_size(),
                                        )
                                    }),
                            )
                            .with_child(
                                PixelCanvas::new(
                                    PAINT_TAB_LABEL,
                                    PAINT_DOCUMENT_WIDTH,
                                    PAINT_DOCUMENT_HEIGHT,
                                )
                                .theme_when(clone_dev_theme_reader(&theme_reader))
                                .state(paint_state)
                                .desired_size(Size::new(960.0, 620.0))
                                .fit_on_first_layout(),
                            ),
                    ),
            )),
    )
    .brush_when(move || theme_reader().palette.surface_raised)
}

fn paint_document_size() -> Size {
    Size::new(PAINT_DOCUMENT_WIDTH as f32, PAINT_DOCUMENT_HEIGHT as f32)
}

fn paint_ruler_corner(theme_reader: DevThemeReader) -> impl Widget {
    Background::new(
        Color::rgba(0.925, 0.936, 0.950, 1.0),
        SizedBox::new()
            .width(PAINT_RULER_EXTENT)
            .height(PAINT_RULER_EXTENT),
    )
    .brush_when(move || theme_reader().palette.surface_raised)
}

fn build_paint_document_bar(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let zoom_out_state = paint_state.clone();
    let zoom_state = paint_state.clone();
    let zoom_in_state = paint_state;
    Toolbar::horizontal()
        .name(PAINT_DOCUMENT_BAR_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .extent(34.0)
        .padding(Insets::all(6.0))
        .spacing(8.0)
        .with_child(
            Label::new(PAINT_DOCUMENT_NAME)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.sm,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(Separator::vertical().length(18.0))
        .with_child(
            Label::new(format!(
                "{} x {} px",
                PAINT_DOCUMENT_WIDTH, PAINT_DOCUMENT_HEIGHT
            ))
            .style(dev_text_style(
                theme_reader(),
                theme_reader().text.xs,
                theme_reader().palette.text_muted,
            ))
            .color_when(dev_theme_color(&theme_reader, |theme| {
                theme.palette.text_muted
            })),
        )
        .with_child(
            Label::new("RGB / 8-bit")
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.xs,
                    theme_reader().palette.text_muted,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(Separator::vertical().length(18.0))
        .with_child(
            paint_command_group(PAINT_DOCUMENT_VIEW_COMMANDS_NAME, &theme_reader)
                .padding(Insets::all(2.0))
                .spacing(3.0)
                .with_child(
                    IconButton::new(IconGlyph::Remove, PAINT_ZOOM_OUT_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(12.0)
                        .on_press_with_ctx(move |ctx| {
                            zoom_out_state.request_zoom_out();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    SizedBox::new().width(78.0).with_child(
                        Label::dynamic("Zoom --", move || paint_zoom_status_text(&zoom_state))
                            .semantic_name(PAINT_ZOOM_READOUT_NAME)
                            .style(dev_text_style(
                                theme_reader(),
                                theme_reader().text.xs,
                                theme_reader().palette.text,
                            ))
                            .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                    ),
                )
                .with_child(
                    IconButton::new(IconGlyph::Add, PAINT_ZOOM_IN_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(12.0)
                        .on_press_with_ctx(move |ctx| {
                            zoom_in_state.request_zoom_in();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
}

fn build_paint_properties_panel(
    paint_state: PixelCanvasState,
    demo_state: PaintDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let picker_reader_state = paint_state.clone();
    let picker_change_state = paint_state.clone();

    DockPanel::new(
        PAINT_PROPERTIES_NAME,
        ScrollView::vertical(Padding::all(
            8.0,
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    PanelSection::new(
                        "Tool options",
                        build_paint_tool_options_panel(
                            paint_state.clone(),
                            Rc::clone(&theme_reader),
                        ),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    PanelSection::new(
                        PAINT_LAYERS_NAME,
                        build_paint_layers_panel(
                            demo_state.clone(),
                            paint_state.clone(),
                            Rc::clone(&theme_reader),
                        ),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .header_action(build_paint_layer_actions(
                        demo_state,
                        paint_state.clone(),
                        Rc::clone(&theme_reader),
                    )),
                )
                .with_child(
                    PanelSection::new(
                        "Color",
                        build_paint_color_panel(paint_state.clone(), Rc::clone(&theme_reader)),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    PanelSection::new(
                        PAINT_COLOR_EDITOR_NAME,
                        SizedBox::new().width(284.0).height(336.0).with_child(
                            ColorPicker::from_color(
                                PAINT_BRUSH_COLOR_NAME,
                                paint_state.brush_color(),
                            )
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .color_when(move || picker_reader_state.brush_color())
                            .compact(true)
                            .show_alpha(false)
                            .on_change(move |color| {
                                picker_change_state.set_brush_color(color);
                            }),
                        ),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .collapsible(true)
                    .collapsed(),
                ),
        ))
        .name(PAINT_SCROLL_NAME),
    )
    .name(PAINT_PROPERTIES_NAME)
    .theme_when(clone_dev_theme_reader(&theme_reader))
    .padding(Insets::ZERO)
}

fn build_paint_color_panel(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let swatch_reader_state = paint_state.clone();
    let palette_reader_state = paint_state.clone();
    let palette_change_state = paint_state;

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(paint_property_row_with_width(
            &theme_reader,
            PAINT_BRUSH_COLOR_NAME,
            104.0,
            ColorSwatch::new(PAINT_BRUSH_COLOR_NAME, swatch_reader_state.brush_color())
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(Size::new(104.0, 32.0))
                .color_when(move || swatch_reader_state.brush_color())
                .read_only(),
        ))
        .with_child(
            ColorPalette::new(PAINT_COLOR_PRESETS_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .swatches(paint_color_palette_swatches())
                .selected_when(move || {
                    paint_color_preset_selected_index(palette_reader_state.brush_color())
                })
                .columns(8)
                .swatch_size(27.0)
                .on_change_with_ctx(move |ctx, _index, _name, color| {
                    palette_change_state.set_brush_color(color);
                    request_window_refresh(ctx, true);
                }),
        )
}

fn build_paint_tool_options_panel(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let selected_state = paint_state.clone();
    SwitchView::new()
        .selected(paint_tool_selected_index(paint_state.tool()))
        .selected_when(move || paint_tool_selected_index(selected_state.tool()))
        .with_child(build_paint_brush_options(
            paint_state.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(build_paint_eraser_options(
            paint_state.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(build_paint_fill_options(
            paint_state.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(build_paint_pan_options(paint_state, theme_reader))
}

fn build_paint_brush_options(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let preview_state = paint_state.clone();
    let preset_reader_state = paint_state.clone();
    let preset_change_state = paint_state.clone();
    let size_reader_state = paint_state.clone();
    let size_state = paint_state.clone();
    let opacity_reader_state = paint_state.clone();
    let opacity_state = paint_state.clone();
    let shape_state = paint_state.clone();
    let blend_mode_state = paint_state.clone();

    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(
            BrushPreview::new(PAINT_BRUSH_PREVIEW_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(Size::new(268.0, 68.0))
                .spec_when(move || {
                    BrushPreviewSpec::new(
                        preview_state.brush_color(),
                        preview_state.brush_size(),
                        preview_state.brush_opacity(),
                        paint_brush_preview_shape(preview_state.brush_shape()),
                    )
                }),
        )
        .with_child(build_brush_size_presets(
            preset_reader_state,
            preset_change_state,
            Rc::clone(&theme_reader),
        ))
        .with_child(build_brush_size_row(
            size_reader_state,
            size_state,
            Rc::clone(&theme_reader),
        ))
        .with_child(paint_property_row(
            &theme_reader,
            PAINT_BRUSH_OPACITY_NAME,
            Slider::new(PAINT_BRUSH_OPACITY_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 1.0)
                .step(0.01)
                .value(paint_state.brush_opacity() as f64)
                .value_when(move || opacity_reader_state.brush_opacity() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    opacity_state.set_brush_opacity(value as f32);
                    request_window_refresh(ctx, true);
                }),
        ))
        .with_child(build_brush_shape_row(
            paint_state.clone(),
            shape_state,
            Rc::clone(&theme_reader),
        ))
        .with_child(build_blend_mode_row(
            &theme_reader,
            PAINT_BLEND_MODE_NAME,
            paint_state,
            blend_mode_state,
        ))
}

fn build_paint_eraser_options(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let preview_state = paint_state.clone();
    let preset_reader_state = paint_state.clone();
    let preset_change_state = paint_state.clone();
    let size_reader_state = paint_state.clone();
    let size_state = paint_state.clone();
    let opacity_reader_state = paint_state.clone();
    let opacity_state = paint_state.clone();
    let shape_state = paint_state.clone();

    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(
            BrushPreview::new(PAINT_ERASER_PREVIEW_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .kind("eraser")
                .size(Size::new(268.0, 68.0))
                .spec_when(move || {
                    BrushPreviewSpec::new(
                        Color::rgba(0.98, 0.99, 1.0, 1.0),
                        preview_state.brush_size(),
                        preview_state.brush_opacity(),
                        paint_brush_preview_shape(preview_state.brush_shape()),
                    )
                }),
        )
        .with_child(build_brush_size_presets(
            preset_reader_state,
            preset_change_state,
            Rc::clone(&theme_reader),
        ))
        .with_child(build_brush_size_row(
            size_reader_state,
            size_state,
            Rc::clone(&theme_reader),
        ))
        .with_child(paint_property_row(
            &theme_reader,
            PAINT_BRUSH_OPACITY_NAME,
            Slider::new(PAINT_BRUSH_OPACITY_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 1.0)
                .step(0.01)
                .value(paint_state.brush_opacity() as f64)
                .value_when(move || opacity_reader_state.brush_opacity() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    opacity_state.set_brush_opacity(value as f32);
                    request_window_refresh(ctx, true);
                }),
        ))
        .with_child(build_brush_shape_row(
            paint_state,
            shape_state,
            theme_reader,
        ))
}

fn build_paint_fill_options(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let opacity_reader_state = paint_state.clone();
    let opacity_state = paint_state.clone();
    let blend_mode_state = paint_state.clone();

    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(paint_property_row(
            &theme_reader,
            PAINT_FILL_OPACITY_NAME,
            Slider::new(PAINT_FILL_OPACITY_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 1.0)
                .step(0.01)
                .value(paint_state.brush_opacity() as f64)
                .value_when(move || opacity_reader_state.brush_opacity() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    opacity_state.set_brush_opacity(value as f32);
                    request_window_refresh(ctx, true);
                }),
        ))
        .with_child(build_blend_mode_row(
            &theme_reader,
            PAINT_FILL_BLEND_MODE_NAME,
            paint_state,
            blend_mode_state,
        ))
}

fn build_paint_pan_options(
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let fit_state = paint_state.clone();
    let actual_size_state = paint_state;

    Stack::horizontal()
        .spacing(6.0)
        .alignment(Alignment::Start)
        .with_child(
            IconButton::new(IconGlyph::FitView, PAINT_FIT_VIEW_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(28.0)
                .icon_size(15.0)
                .on_press_with_ctx(move |ctx| {
                    fit_state.request_fit_view();
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            IconButton::new(IconGlyph::ActualSize, PAINT_ACTUAL_SIZE_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(28.0)
                .icon_size(15.0)
                .on_press_with_ctx(move |ctx| {
                    actual_size_state.request_actual_size_view();
                    request_window_refresh(ctx, true);
                }),
        )
}

fn build_brush_size_presets(
    reader_state: PixelCanvasState,
    change_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    PresetStrip::new(PAINT_BRUSH_SIZE_PRESETS_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .presets(PAINT_BRUSH_SIZE_PRESETS.map(paint_brush_size_preset_label))
        .selected_when(move || paint_brush_size_preset_selected_index(reader_state.brush_size()))
        .item_width(54.0)
        .on_change(move |index, _| {
            if let Some(size) = PAINT_BRUSH_SIZE_PRESETS.get(index).copied() {
                change_state.set_brush_size(size);
            }
        })
}

fn build_brush_size_row(
    reader_state: PixelCanvasState,
    change_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> PropertyRow {
    paint_property_row_with_width(
        &theme_reader,
        PAINT_BRUSH_SIZE_NAME,
        96.0,
        NumberInput::new(PAINT_BRUSH_SIZE_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .range(1.0, 96.0)
            .step(1.0)
            .precision(0)
            .value(reader_state.brush_size() as f64)
            .value_when(move || reader_state.brush_size() as f64)
            .on_change(move |value| {
                change_state.set_brush_size(value as f32);
            }),
    )
}

fn build_brush_shape_row(
    reader_state: PixelCanvasState,
    change_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> PropertyRow {
    paint_property_row(
        &theme_reader,
        PAINT_BRUSH_SHAPE_NAME,
        Select::new(PAINT_BRUSH_SHAPE_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .options(PixelCanvasBrushShape::ALL.map(PixelCanvasBrushShape::label))
            .selected(paint_brush_shape_selected_index(reader_state.brush_shape()))
            .on_change(move |index, _| {
                if let Some(shape) = PixelCanvasBrushShape::ALL.get(index).copied() {
                    change_state.set_brush_shape(shape);
                }
            }),
    )
}

fn build_blend_mode_row(
    theme_reader: &DevThemeReader,
    label: &'static str,
    reader_state: PixelCanvasState,
    change_state: PixelCanvasState,
) -> PropertyRow {
    let selected_state = reader_state.clone();
    paint_property_row(
        theme_reader,
        label,
        Select::new(label)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(PixelCanvasBlendMode::ALL.map(PixelCanvasBlendMode::label))
            .selected(paint_blend_mode_selected_index(reader_state.blend_mode()))
            .selected_when(move || {
                Some(paint_blend_mode_selected_index(selected_state.blend_mode()))
            })
            .on_change_with_ctx(move |ctx, index, _| {
                if let Some(mode) = PixelCanvasBlendMode::ALL.get(index).copied() {
                    change_state.set_blend_mode(mode);
                    request_window_refresh(ctx, true);
                }
            }),
    )
}

fn paint_property_row<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .inline()
        .label_width(92.0)
}

fn paint_property_row_with_width<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    width: f32,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    paint_property_row(theme_reader, label, control).control_width(width)
}

fn paint_property_row_with_label_width<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    label_width: f32,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .inline()
        .label_width(label_width)
}

fn build_paint_layers_panel(
    state: PaintDemoState,
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let selected_layer = state.selected_layer_visual_index();
    let selected_state = state.clone();
    let selection_state = state.clone();
    let selection_paint_state = paint_state.clone();
    let visibility_change_state = state.clone();
    let visibility_paint_state = paint_state.clone();
    let lock_change_state = state.clone();
    let lock_change_paint_state = paint_state.clone();
    let reorder_change_state = state.clone();
    let reorder_paint_state = paint_state.clone();
    let opacity_paint_state = paint_state.clone();
    let blend_paint_state = paint_state;
    let paint_detail_state = state.clone();
    let paper_detail_state = state.clone();
    let paint_visibility_state = state.clone();
    let paper_visibility_state = state.clone();
    let paint_lock_state = state.clone();
    let paper_lock_state = state.clone();
    let opacity_reader_state = state.clone();
    let opacity_change_state = state.clone();
    let blend_initial = paint_blend_mode_selected_index(state.selected_layer_blend_mode());
    let blend_reader_state = state.clone();
    let blend_change_state = state;

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            SizedBox::new().width(284.0).height(112.0).with_child(
                LayerList::new(PAINT_LAYERS_NAME)
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .layers([
                        LayerListItem::new("Paint")
                            .detail_when(move || paint_layer_detail(&paint_detail_state, 0))
                            .thumbnail(Color::rgba(0.16, 0.31, 0.88, 1.0))
                            .visible_when(move || paint_visibility_state.layer_visible(0))
                            .locked_when(move || paint_lock_state.layer_locked(0)),
                        LayerListItem::new("Paper")
                            .detail_when(move || paint_layer_detail(&paper_detail_state, 1))
                            .thumbnail(Color::rgba(0.89, 0.91, 0.94, 1.0))
                            .visible_when(move || paper_visibility_state.layer_visible(1))
                            .locked_when(move || paper_lock_state.layer_locked(1)),
                    ])
                    .selected(selected_layer)
                    .selected_when(move || Some(selected_state.selected_layer_visual_index()))
                    .row_height(46.0)
                    .on_select_with_ctx(move |ctx, index, _| {
                        selection_state.set_selected_visual_layer(index);
                        selection_state.sync_canvas_editable(&selection_paint_state);
                        request_window_refresh(ctx, true);
                    })
                    .on_visibility_change_with_ctx(move |ctx, index, visible| {
                        let layer = visibility_change_state.layer_at_visual_index(index);
                        visibility_change_state.set_layer_visible(layer, visible);
                        visibility_change_state.sync_canvas_layers(&visibility_paint_state);
                        request_window_refresh(ctx, true);
                    })
                    .on_lock_change_with_ctx(move |ctx, index, locked| {
                        let layer = lock_change_state.layer_at_visual_index(index);
                        lock_change_state.set_layer_locked(layer, locked);
                        if lock_change_state.selected_layer() == layer {
                            lock_change_state.sync_canvas_editable(&lock_change_paint_state);
                        }
                        request_window_refresh(ctx, true);
                    })
                    .on_reorder_with_ctx(move |ctx, change| {
                        reorder_change_state.reorder_layers(change.from, change.to);
                        reorder_change_state.sync_canvas_layers(&reorder_paint_state);
                        request_window_refresh(ctx, true);
                    }),
            ),
        )
        .with_child(paint_property_row(
            &theme_reader,
            PAINT_LAYER_OPACITY_NAME,
            Slider::new(PAINT_LAYER_OPACITY_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 1.0)
                .step(0.01)
                .value(opacity_reader_state.selected_layer_opacity() as f64)
                .value_when(move || opacity_reader_state.selected_layer_opacity() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    opacity_change_state.set_selected_layer_opacity(value as f32);
                    if opacity_change_state.selected_layer() <= 1 {
                        opacity_change_state.sync_canvas_layers(&opacity_paint_state);
                    }
                    request_window_refresh(ctx, true);
                }),
        ))
        .with_child(paint_property_row_with_label_width(
            &theme_reader,
            PAINT_LAYER_BLEND_MODE_NAME,
            116.0,
            Select::new(PAINT_LAYER_BLEND_MODE_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .options(PixelCanvasBlendMode::ALL.map(PixelCanvasBlendMode::label))
                .selected(blend_initial)
                .selected_when(move || {
                    Some(paint_blend_mode_selected_index(
                        blend_reader_state.selected_layer_blend_mode(),
                    ))
                })
                .on_change_with_ctx(move |ctx, index, _| {
                    if let Some(mode) = PixelCanvasBlendMode::ALL.get(index).copied() {
                        blend_change_state.set_selected_layer_blend_mode(mode);
                        if blend_change_state.selected_layer() == 0 {
                            blend_change_state.sync_canvas_layers(&blend_paint_state);
                        }
                        request_window_refresh(ctx, true);
                    }
                }),
        ))
}

fn build_paint_layer_actions(
    state: PaintDemoState,
    paint_state: PixelCanvasState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let above_enabled = state.clone();
    let above_state = state.clone();
    let above_paint_state = paint_state.clone();
    let below_enabled = state.clone();
    let below_state = state;
    let below_paint_state = paint_state;
    Stack::horizontal()
        .spacing(4.0)
        .with_child(
            IconButton::new(IconGlyph::ChevronUp, PAINT_SELECT_LAYER_ABOVE_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(24.0)
                .icon_size(12.0)
                .enabled_when(move || above_enabled.can_select_layer_above())
                .on_press_with_ctx(move |ctx| {
                    above_state.select_layer_above();
                    above_state.sync_canvas_editable(&above_paint_state);
                    request_window_refresh(ctx, true);
                }),
        )
        .with_child(
            IconButton::new(IconGlyph::ChevronDown, PAINT_SELECT_LAYER_BELOW_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(24.0)
                .icon_size(12.0)
                .enabled_when(move || below_enabled.can_select_layer_below())
                .on_press_with_ctx(move |ctx| {
                    below_state.select_layer_below();
                    below_state.sync_canvas_editable(&below_paint_state);
                    request_window_refresh(ctx, true);
                }),
        )
}

fn paint_layer_name(index: usize) -> &'static str {
    PAINT_LAYER_NAMES
        .get(index)
        .copied()
        .unwrap_or(PAINT_LAYER_NAMES[0])
}

fn paint_tool_palette_items() -> [ToolPaletteItem; PAINT_TOOLS.len()] {
    PAINT_TOOLS.map(|tool| {
        let (icon, label) = match tool {
            PixelCanvasTool::Brush => (IconGlyph::Brush, "Brush tool"),
            PixelCanvasTool::Eraser => (IconGlyph::Eraser, "Eraser tool"),
            PixelCanvasTool::Fill => (IconGlyph::PaintBucket, "Fill tool"),
            PixelCanvasTool::Pan => (IconGlyph::Hand, "Pan tool"),
        };
        ToolPaletteItem::new(icon, label)
    })
}

fn paint_tool_at(index: usize) -> Option<PixelCanvasTool> {
    PAINT_TOOLS.get(index).copied()
}

fn paint_tool_selected_index(tool: PixelCanvasTool) -> usize {
    PAINT_TOOLS
        .iter()
        .position(|candidate| *candidate == tool)
        .unwrap_or(0)
}

fn paint_brush_shape_selected_index(shape: PixelCanvasBrushShape) -> usize {
    PixelCanvasBrushShape::ALL
        .iter()
        .position(|candidate| *candidate == shape)
        .unwrap_or(0)
}

fn paint_brush_size_preset_label(size: f32) -> String {
    format!("{} px", size.round() as u32)
}

fn paint_brush_size_preset_selected_index(size: f32) -> Option<usize> {
    PAINT_BRUSH_SIZE_PRESETS
        .iter()
        .position(|preset| (size - *preset).abs() < 0.001)
}

fn paint_color_palette_swatches() -> [ColorPaletteSwatch; 8] {
    PAINT_COLOR_PRESETS.map(|(name, color)| ColorPaletteSwatch::new(name, color))
}

fn paint_color_preset_selected_index(color: Color) -> Option<usize> {
    PAINT_COLOR_PRESETS
        .iter()
        .position(|(_, preset)| paint_colors_close(color, *preset))
}

fn paint_colors_close(left: Color, right: Color) -> bool {
    left.space == right.space
        && (left.red - right.red).abs() < 0.0001
        && (left.green - right.green).abs() < 0.0001
        && (left.blue - right.blue).abs() < 0.0001
        && (left.alpha - right.alpha).abs() < 0.0001
}

fn paint_brush_preview_shape(shape: PixelCanvasBrushShape) -> BrushPreviewShape {
    match shape {
        PixelCanvasBrushShape::Square => BrushPreviewShape::Square,
        PixelCanvasBrushShape::Round => BrushPreviewShape::Round,
    }
}

fn paint_blend_mode_selected_index(mode: PixelCanvasBlendMode) -> usize {
    PixelCanvasBlendMode::ALL
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or(0)
}
