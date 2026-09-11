use crate::paint::{PaintValidationResult, validate_widget_shader};
use sui::Color;
use sui::ColorSpace;
use sui::WidgetShader;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingShader {
    pub(crate) shader: WidgetShader,
}

impl BindingShader {
    pub const fn from_widget_shader(shader: WidgetShader) -> Self {
        Self { shader }
    }

    pub const fn color_wheel() -> Self {
        Self::from_widget_shader(WidgetShader::ColorWheel)
    }

    pub const fn hue_bar() -> Self {
        Self::from_widget_shader(WidgetShader::ColorPickerHueBar)
    }

    pub fn saturation_value_plane(
        color_space: ColorSpace,
        hue: f32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerSaturationValuePlane {
            color_space,
            hue,
            max_value,
        })
    }

    pub fn saturation_bar(
        color_space: ColorSpace,
        hue: f32,
        value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerSaturationBar {
            color_space,
            hue,
            value,
        })
    }

    pub fn value_bar(
        color_space: ColorSpace,
        hue: f32,
        saturation: f32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerValueBar {
            color_space,
            hue,
            saturation,
            max_value,
        })
    }

    pub fn alpha_bar(color: Color) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerAlphaBar { color })
    }

    pub fn rgb_channel_bar(
        color: Color,
        channel: u32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        })
    }

    pub const fn widget_shader(self) -> WidgetShader {
        self.shader
    }

    pub(crate) fn new_validated(shader: WidgetShader) -> PaintValidationResult<Self> {
        validate_widget_shader(shader)?;
        Ok(Self::from_widget_shader(shader))
    }
}

impl From<BindingShader> for WidgetShader {
    fn from(value: BindingShader) -> Self {
        value.widget_shader()
    }
}
