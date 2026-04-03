use crate::Size;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiInfo {
    pub scale_factor: f32,
    pub raw_dpi: Option<f32>,
    pub viewport: Size,
    pub surface_size: Size,
}

impl DpiInfo {
    pub const BASE_DPI: f32 = 96.0;

    pub fn new(
        scale_factor: f32,
        raw_dpi: Option<f32>,
        viewport: Size,
        surface_size: Size,
    ) -> Self {
        Self {
            scale_factor: normalize_scale_factor(scale_factor),
            raw_dpi: raw_dpi.filter(|value| value.is_finite() && *value > 0.0),
            viewport,
            surface_size,
        }
    }

    pub fn effective_dpi(self) -> f32 {
        self.scale_factor * Self::BASE_DPI
    }

    pub fn pixels_per_point(self) -> f32 {
        self.scale_factor
    }

    pub fn physical_pixels_to_logical(self, pixels: f32) -> f32 {
        pixels / self.scale_factor
    }

    pub fn logical_to_physical_pixels(self, logical: f32) -> f32 {
        logical * self.scale_factor
    }

    pub fn hairline_width(self) -> f32 {
        self.physical_pixels_to_logical(1.0)
    }
}

impl Default for DpiInfo {
    fn default() -> Self {
        Self::new(1.0, None, Size::ZERO, Size::ZERO)
    }
}

fn normalize_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}
