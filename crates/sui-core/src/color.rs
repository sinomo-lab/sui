#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    #[default]
    Srgb,
    LinearSrgb,
    DisplayP3,
    LinearDisplayP3,
}

impl ColorSpace {
    pub const fn is_linear(self) -> bool {
        matches!(self, Self::LinearSrgb | Self::LinearDisplayP3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub space: ColorSpace,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::rgba(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Self = Self::rgba(1.0, 1.0, 1.0, 1.0);

    pub const fn new(space: ColorSpace, red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            space,
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::srgba(red, green, blue, alpha)
    }

    pub const fn srgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(ColorSpace::Srgb, red, green, blue, alpha)
    }

    pub const fn linear_rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(ColorSpace::LinearSrgb, red, green, blue, alpha)
    }

    pub const fn display_p3(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(ColorSpace::DisplayP3, red, green, blue, alpha)
    }

    pub const fn linear_display_p3(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(ColorSpace::LinearDisplayP3, red, green, blue, alpha)
    }

    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }

    pub fn clamped(self) -> Self {
        Self {
            red: self.red.clamp(0.0, 1.0),
            green: self.green.clamp(0.0, 1.0),
            blue: self.blue.clamp(0.0, 1.0),
            alpha: self.alpha.clamp(0.0, 1.0),
            ..self
        }
    }

    /// Source-over composite this (possibly translucent) color onto an opaque
    /// `backdrop`, blending in the **encoded (gamma) space** — the same
    /// arithmetic CSS uses for `rgba()` overlays. The renderer blends in
    /// linear space, which reads translucent washes noticeably heavier than
    /// their CSS-authored intent, so design tokens specified as CSS rgba
    /// values should be flattened with this before painting.
    ///
    /// Both colors must be in the same encoded space; the backdrop's alpha is
    /// treated as 1. The result is opaque.
    pub fn over(self, backdrop: Color) -> Self {
        let alpha = self.alpha.clamp(0.0, 1.0);
        let inverse = 1.0 - alpha;
        Self {
            space: backdrop.space,
            red: self.red * alpha + backdrop.red * inverse,
            green: self.green * alpha + backdrop.green * inverse,
            blue: self.blue * alpha + backdrop.blue * inverse,
            alpha: 1.0,
        }
    }

    pub fn to_linear_srgb(self) -> Self {
        let decode = if self.space.is_linear() {
            |channel: f32| channel
        } else {
            srgb_transfer_to_linear
        };
        let linear = [decode(self.red), decode(self.green), decode(self.blue)];
        let [red, green, blue] = match self.space {
            ColorSpace::Srgb | ColorSpace::LinearSrgb => linear,
            ColorSpace::DisplayP3 | ColorSpace::LinearDisplayP3 => {
                multiply_matrix3x3(DISPLAY_P3_TO_LINEAR_SRGB, linear)
            }
        };

        Self::linear_rgba(red, green, blue, self.alpha)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

const DISPLAY_P3_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [1.224_940_2, -0.224_940_18, 0.0],
    [-0.042_056_955, 1.042_057, 0.0],
    [-0.019_637_555, -0.078_636_04, 1.098_273_6],
];

fn srgb_transfer_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn multiply_matrix3x3(matrix: [[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
    [
        (matrix[0][0] * vector[0]) + (matrix[0][1] * vector[1]) + (matrix[0][2] * vector[2]),
        (matrix[1][0] * vector[0]) + (matrix[1][1] * vector[1]) + (matrix[1][2] * vector[2]),
        (matrix[2][0] * vector[0]) + (matrix[2][1] * vector[1]) + (matrix[2][2] * vector[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::{Color, ColorSpace};

    #[test]
    fn linear_display_p3_constructor_uses_linear_display_p3_space() {
        let color = Color::linear_display_p3(0.25, 0.5, 0.75, 1.0);

        assert_eq!(color.space, ColorSpace::LinearDisplayP3);
        assert_eq!(color.to_array(), [0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn display_p3_to_linear_srgb_converts_primaries() {
        let converted = Color::display_p3(1.0, 0.0, 0.0, 1.0).to_linear_srgb();

        assert_eq!(converted.space, ColorSpace::LinearSrgb);
        assert!((converted.red - 1.22494).abs() < 0.0001);
        assert!((converted.green + 0.04205).abs() < 0.0001);
        assert!((converted.blue + 0.01963).abs() < 0.0001);
        assert_eq!(converted.alpha, 1.0);
    }

    #[test]
    fn encoded_and_linear_display_p3_match_after_linearization() {
        let encoded = Color::display_p3(0.5, 0.25, 0.75, 1.0).to_linear_srgb();
        let linear =
            Color::linear_display_p3(0.21404114, 0.05087609, 0.52252156, 1.0).to_linear_srgb();

        assert!((encoded.red - linear.red).abs() < 0.0001);
        assert!((encoded.green - linear.green).abs() < 0.0001);
        assert!((encoded.blue - linear.blue).abs() < 0.0001);
        assert_eq!(linear.alpha, 1.0);
    }
}
