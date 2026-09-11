use crate::handles::BindingImageHandle;
use crate::shader::BindingShader;
use std::fmt;
use sui::Border;
use sui::Brush;
use sui::Color;
use sui::ImageHandle;
use sui::ImageSource;
use sui::PaintCtx;
use sui::Path;
use sui::Point;
use sui::Rect;
use sui::ShadowParams;
use sui::StrokeStyle;
use sui::TextStyle;
use sui::Transform;
use sui::WidgetShader;

#[derive(Debug, Clone, PartialEq)]
pub enum PaintCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        brush: Brush,
    },
    StrokeRect {
        rect: Rect,
        brush: Brush,
        stroke: StrokeStyle,
    },
    FillPath {
        path: Path,
        brush: Brush,
    },
    StrokePath {
        path: Path,
        brush: Brush,
        stroke: StrokeStyle,
    },
    FillRoundedRect {
        rect: Rect,
        radii: [f32; 4],
        brush: Brush,
        border: Option<Border>,
        shadow: Option<ShadowParams>,
    },
    DrawText {
        rect: Rect,
        text: String,
        style: TextStyle,
    },
    DrawImage {
        rect: Rect,
        source: ImageSource,
    },
    DrawImageQuad {
        points: [Point; 4],
        source: ImageSource,
    },
    DrawShaderRect {
        rect: Rect,
        shader: WidgetShader,
    },
    PushClipRect(Rect),
    PushClipPath(Path),
    PopClip,
    PushTransform(Transform),
    PopTransform,
}

impl PaintCommand {
    pub fn apply(self, ctx: &mut PaintCtx) {
        match self {
            Self::Clear(color) => ctx.clear(color),
            Self::FillRect { rect, brush } => ctx.fill_rect(rect, brush),
            Self::StrokeRect {
                rect,
                brush,
                stroke,
            } => ctx.stroke_rect(rect, brush, stroke),
            Self::FillPath { path, brush } => ctx.fill(path, brush),
            Self::StrokePath {
                path,
                brush,
                stroke,
            } => ctx.stroke(path, brush, stroke),
            Self::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            } => ctx.push(sui::SceneCommand::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            }),
            Self::DrawText { rect, text, style } => ctx.draw_text(rect, text, style),
            Self::DrawImage { rect, source } => ctx.draw_image_source(rect, source),
            Self::DrawImageQuad { points, source } => ctx.draw_image_quad_source(points, source),
            Self::DrawShaderRect { rect, shader } => ctx.draw_shader_rect(rect, shader),
            Self::PushClipRect(rect) => ctx.push_clip_rect(rect),
            Self::PushClipPath(path) => ctx.push_clip(path),
            Self::PopClip => ctx.pop_clip(),
            Self::PushTransform(transform) => ctx.push_transform(transform),
            Self::PopTransform => ctx.pop_transform(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaintCommandBuilder {
    pub(crate) commands: Vec<PaintCommand>,
    pub(crate) stack: PaintStackState,
}

impl PaintCommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: PaintCommand) -> PaintValidationResult<&mut Self> {
        validate_paint_command_with_stack(&command, &mut self.stack)?;
        self.commands.push(command);
        Ok(self)
    }

    pub fn clear(&mut self, color: Color) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::Clear(color))
    }

    pub fn fill_rect(
        &mut self,
        rect: Rect,
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRect {
            rect,
            brush: brush.into(),
        })
    }

    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        brush: impl Into<Brush>,
        stroke: StrokeStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::StrokeRect {
            rect,
            brush: brush.into(),
            stroke,
        })
    }

    pub fn fill_path(
        &mut self,
        path: Path,
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillPath {
            path,
            brush: brush.into(),
        })
    }

    pub fn stroke_path(
        &mut self,
        path: Path,
        brush: impl Into<Brush>,
        stroke: StrokeStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::StrokePath {
            path,
            brush: brush.into(),
            stroke,
        })
    }

    pub fn fill_rrect(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: None,
        })
    }

    pub fn draw_shadow(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        shadow: ShadowParams,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: Brush::Solid(Color::TRANSPARENT),
            border: None,
            shadow: Some(shadow),
        })
    }

    pub fn fill_rrect_with_shadow(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
        shadow: ShadowParams,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: Some(shadow),
        })
    }

    pub fn draw_text(
        &mut self,
        rect: Rect,
        text: impl Into<String>,
        style: TextStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawText {
            rect,
            text: text.into(),
            style,
        })
    }

    pub fn draw_image(
        &mut self,
        rect: Rect,
        image: ImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImage {
            rect,
            source: ImageSource::new(image),
        })
    }

    pub fn draw_binding_image(
        &mut self,
        rect: Rect,
        image: BindingImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_image(rect, image.into_sui())
    }

    pub fn draw_image_source(
        &mut self,
        rect: Rect,
        source: ImageSource,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImage { rect, source })
    }

    pub fn draw_image_quad(
        &mut self,
        points: [Point; 4],
        image: ImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImageQuad {
            points,
            source: ImageSource::new(image),
        })
    }

    pub fn draw_binding_image_quad(
        &mut self,
        points: [Point; 4],
        image: BindingImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_image_quad(points, image.into_sui())
    }

    pub fn draw_shader_rect(
        &mut self,
        rect: Rect,
        shader: WidgetShader,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawShaderRect { rect, shader })
    }

    pub fn draw_binding_shader_rect(
        &mut self,
        rect: Rect,
        shader: BindingShader,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_shader_rect(rect, shader.widget_shader())
    }

    pub fn push_clip_rect(&mut self, rect: Rect) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushClipRect(rect))
    }

    pub fn push_clip_path(&mut self, path: Path) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushClipPath(path))
    }

    pub fn pop_clip(&mut self) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PopClip)
    }

    pub fn push_transform(&mut self, transform: Transform) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushTransform(transform))
    }

    pub fn pop_transform(&mut self) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PopTransform)
    }

    pub fn finish(self) -> PaintValidationResult<Vec<PaintCommand>> {
        self.stack.finish()?;
        Ok(self.commands)
    }
}

pub type PaintValidationResult<T> = std::result::Result<T, PaintValidationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintValidationErrorKind {
    NonFiniteGeometry,
    NegativeSize,
    PathTooComplex,
    InvalidStroke,
    InvalidBrush,
    InvalidShader,
    InvalidImage,
    InvalidTextStyle,
    InvalidStackOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintValidationError {
    pub kind: PaintValidationErrorKind,
    pub message: String,
}

impl PaintValidationError {
    pub(crate) fn new(kind: PaintValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for PaintValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PaintValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PaintStackState {
    pub(crate) clip_depth: usize,
    pub(crate) transform_depth: usize,
}

impl PaintStackState {
    pub(crate) fn finish(self) -> PaintValidationResult<()> {
        if self.clip_depth != 0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::InvalidStackOperation,
                format!(
                    "paint command stream has {} unclosed clip scope(s)",
                    self.clip_depth
                ),
            ));
        }
        if self.transform_depth != 0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::InvalidStackOperation,
                format!(
                    "paint command stream has {} unclosed transform scope(s)",
                    self.transform_depth
                ),
            ));
        }
        Ok(())
    }
}

pub(crate) const MAX_BINDING_PATH_ELEMENTS: usize = 4096;
pub(crate) const MAX_BINDING_GRADIENT_STOPS: usize = 8;

pub(crate) fn validate_paint_command(command: &PaintCommand) -> PaintValidationResult<()> {
    validate_paint_command_with_stack(command, &mut PaintStackState::default())
}

pub(crate) fn validate_paint_command_with_stack(
    command: &PaintCommand,
    stack: &mut PaintStackState,
) -> PaintValidationResult<()> {
    match command {
        PaintCommand::Clear(color) => validate_color(*color),
        PaintCommand::FillRect { rect, brush } => {
            validate_rect(*rect)?;
            validate_brush(brush)
        }
        PaintCommand::StrokeRect {
            rect,
            brush,
            stroke,
        } => {
            validate_rect(*rect)?;
            validate_stroke(*stroke)?;
            validate_brush(brush)
        }
        PaintCommand::FillPath { path, brush } => {
            validate_path(path)?;
            validate_brush(brush)
        }
        PaintCommand::StrokePath {
            path,
            brush,
            stroke,
        } => {
            validate_path(path)?;
            validate_stroke(*stroke)?;
            validate_brush(brush)
        }
        PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush,
            border,
            shadow,
        } => {
            validate_rect(*rect)?;
            validate_radii(*radii)?;
            validate_brush(brush)?;
            if let Some(border) = border {
                validate_border(*border)?;
            }
            if let Some(shadow) = shadow {
                validate_shadow(*shadow)?;
            }
            Ok(())
        }
        PaintCommand::DrawText { rect, style, .. } => {
            validate_rect(*rect)?;
            validate_text_style(style)
        }
        PaintCommand::DrawImage { rect, source } => {
            validate_rect(*rect)?;
            validate_image_source(source)
        }
        PaintCommand::DrawImageQuad { points, source } => {
            for point in points {
                validate_point(*point)?;
            }
            validate_image_source(source)
        }
        PaintCommand::DrawShaderRect { rect, shader } => {
            validate_rect(*rect)?;
            validate_widget_shader(*shader)
        }
        PaintCommand::PushClipRect(rect) => {
            validate_rect(*rect)?;
            stack.clip_depth += 1;
            Ok(())
        }
        PaintCommand::PushClipPath(path) => {
            validate_path(path)?;
            stack.clip_depth += 1;
            Ok(())
        }
        PaintCommand::PopClip => {
            if stack.clip_depth == 0 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidStackOperation,
                    "paint command stream popped a clip without a matching push",
                ));
            }
            stack.clip_depth -= 1;
            Ok(())
        }
        PaintCommand::PushTransform(transform) => {
            validate_transform(*transform)?;
            stack.transform_depth += 1;
            Ok(())
        }
        PaintCommand::PopTransform => {
            if stack.transform_depth == 0 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidStackOperation,
                    "paint command stream popped a transform without a matching push",
                ));
            }
            stack.transform_depth -= 1;
            Ok(())
        }
    }
}

pub(crate) fn validate_point(point: Point) -> PaintValidationResult<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "point contains a non-finite coordinate",
        ));
    }
    Ok(())
}

pub(crate) fn validate_rect(rect: Rect) -> PaintValidationResult<()> {
    validate_point(rect.origin)?;
    if !rect.size.width.is_finite() || !rect.size.height.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "rect contains a non-finite size",
        ));
    }
    if rect.size.width < 0.0 || rect.size.height < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NegativeSize,
            "rect size must be non-negative",
        ));
    }
    Ok(())
}

pub(crate) fn validate_color(color: Color) -> PaintValidationResult<()> {
    if color
        .to_array()
        .into_iter()
        .any(|channel| !channel.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidBrush,
            "color contains a non-finite channel",
        ));
    }
    Ok(())
}

pub(crate) fn validate_image_source(source: &ImageSource) -> PaintValidationResult<()> {
    if source.image.get() == 0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidImage,
            "image handle must be non-zero",
        ));
    }
    if let Some(source_rect) = source.source_rect {
        validate_rect(source_rect)?;
    }
    if let Some(tint) = source.tint {
        validate_color(tint)?;
    }
    Ok(())
}

pub(crate) fn validate_text_style(style: &TextStyle) -> PaintValidationResult<()> {
    validate_color(style.color)?;
    if !style.font_size.is_finite() || style.font_size <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text font size must be finite and positive",
        ));
    }
    if !style.line_height.is_finite() || style.line_height <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text line height must be finite and positive",
        ));
    }
    if let Some(font) = style.font
        && font.get() == 0
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text font handle must be non-zero",
        ));
    }
    Ok(())
}

pub(crate) fn validate_widget_shader(shader: WidgetShader) -> PaintValidationResult<()> {
    match shader {
        WidgetShader::ColorWheel | WidgetShader::ColorPickerHueBar => Ok(()),
        WidgetShader::ColorPickerSaturationValuePlane { hue, max_value, .. } => {
            validate_shader_param("hue", hue)?;
            validate_positive_shader_param("max_value", max_value)
        }
        WidgetShader::ColorPickerSaturationBar { hue, value, .. } => {
            validate_shader_param("hue", hue)?;
            validate_shader_param("value", value)
        }
        WidgetShader::ColorPickerValueBar {
            hue,
            saturation,
            max_value,
            ..
        } => {
            validate_shader_param("hue", hue)?;
            validate_shader_param("saturation", saturation)?;
            validate_positive_shader_param("max_value", max_value)
        }
        WidgetShader::ColorPickerAlphaBar { color } => validate_shader_color(color),
        WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        } => {
            validate_shader_color(color)?;
            if channel > 2 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidShader,
                    "rgb channel shader channel must be 0, 1, or 2",
                ));
            }
            validate_positive_shader_param("max_value", max_value)
        }
    }
}

pub(crate) fn validate_shader_color(color: Color) -> PaintValidationResult<()> {
    if color
        .to_array()
        .into_iter()
        .any(|channel| !channel.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            "shader color contains a non-finite channel",
        ));
    }
    Ok(())
}

pub(crate) fn validate_shader_param(name: &str, value: f32) -> PaintValidationResult<()> {
    if !value.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            format!("shader parameter `{name}` must be finite"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_positive_shader_param(name: &str, value: f32) -> PaintValidationResult<()> {
    validate_shader_param(name, value)?;
    if value <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            format!("shader parameter `{name}` must be positive"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_brush(brush: &Brush) -> PaintValidationResult<()> {
    match brush {
        Brush::Solid(color) => validate_color(*color),
        Brush::LinearGradient { start, end, stops } => {
            validate_point(*start)?;
            validate_point(*end)?;
            if stops.len() > MAX_BINDING_GRADIENT_STOPS {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidBrush,
                    format!(
                        "linear gradient has {} stops but the binding limit is {}",
                        stops.len(),
                        MAX_BINDING_GRADIENT_STOPS
                    ),
                ));
            }
            for stop in stops {
                if !stop.offset.is_finite() {
                    return Err(PaintValidationError::new(
                        PaintValidationErrorKind::InvalidBrush,
                        "linear gradient stop offset must be finite",
                    ));
                }
                validate_color(stop.color)?;
            }
            Ok(())
        }
    }
}

pub(crate) fn validate_path(path: &Path) -> PaintValidationResult<()> {
    if path.elements().len() > MAX_BINDING_PATH_ELEMENTS {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::PathTooComplex,
            format!(
                "path has {} elements but the binding limit is {}",
                path.elements().len(),
                MAX_BINDING_PATH_ELEMENTS
            ),
        ));
    }
    validate_rect(path.bounds())
}

pub(crate) fn validate_stroke(stroke: StrokeStyle) -> PaintValidationResult<()> {
    if !stroke.width.is_finite() || stroke.width < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidStroke,
            "stroke width must be finite and non-negative",
        ));
    }
    Ok(())
}

pub(crate) fn validate_radii(radii: [f32; 4]) -> PaintValidationResult<()> {
    for radius in radii {
        if !radius.is_finite() || radius < 0.0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::NonFiniteGeometry,
                "rounded-rect radii must be finite and non-negative",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_border(border: Border) -> PaintValidationResult<()> {
    if !border.width.is_finite() || border.width < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidStroke,
            "border width must be finite and non-negative",
        ));
    }
    validate_color(border.color)
}

pub(crate) fn validate_shadow(shadow: ShadowParams) -> PaintValidationResult<()> {
    if [shadow.offset_x, shadow.offset_y, shadow.blur, shadow.spread]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "shadow geometry must be finite",
        ));
    }
    validate_color(shadow.color)
}

pub(crate) fn validate_transform(transform: Transform) -> PaintValidationResult<()> {
    if [
        transform.xx,
        transform.yx,
        transform.xy,
        transform.yy,
        transform.dx,
        transform.dy,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "transform contains a non-finite component",
        ));
    }
    Ok(())
}
