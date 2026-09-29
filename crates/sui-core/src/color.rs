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

    /// Convert this color to `space`, keeping alpha. Channels are not clamped,
    /// so colors outside the target gamut keep their out-of-range values.
    pub fn to_space(self, space: ColorSpace) -> Self {
        if self.space == space {
            return self;
        }
        let linear = self.to_linear_srgb();
        let linear = [linear.red, linear.green, linear.blue];
        let [red, green, blue] = match space {
            ColorSpace::Srgb | ColorSpace::LinearSrgb => linear,
            ColorSpace::DisplayP3 | ColorSpace::LinearDisplayP3 => {
                multiply_matrix3x3(LINEAR_SRGB_TO_DISPLAY_P3, linear)
            }
        };
        let encode = if space.is_linear() {
            |channel: f32| channel
        } else {
            srgb_transfer_from_linear
        };
        Self::new(space, encode(red), encode(green), encode(blue), self.alpha)
    }

    /// Blend toward `to` by `amount` (0 to 1) in premultiplied OKLab, the
    /// perceptual mixing CSS `color-mix()` uses by default.
    ///
    /// Blending the premultiplied values keeps a fade from a transparent
    /// color free of a dark fringe: the color holds steady while only its
    /// opacity changes. The endpoints are returned exactly, and blends between
    /// colors in different spaces come out in `to`'s space.
    pub fn mix_oklab(self, to: Color, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        if amount <= 0.0 {
            return self;
        }
        if amount >= 1.0 {
            return to;
        }

        let oklab = |color: Color| {
            let linear = color.to_linear_srgb();
            linear_srgb_to_oklab([linear.red, linear.green, linear.blue])
        };
        let (from_lab, to_lab) = (oklab(self), oklab(to));
        let from_alpha = self.alpha.clamp(0.0, 1.0);
        let to_alpha = to.alpha.clamp(0.0, 1.0);
        let alpha = from_alpha + (to_alpha - from_alpha) * amount;
        let lab: [f32; 3] = std::array::from_fn(|index| {
            if alpha <= 1.0e-6 {
                // Both ends are (nearly) invisible: premultiplying would
                // divide by zero, and any color reads the same.
                from_lab[index] + (to_lab[index] - from_lab[index]) * amount
            } else {
                let from = from_lab[index] * from_alpha;
                let to = to_lab[index] * to_alpha;
                (from + (to - from) * amount) / alpha
            }
        });
        let [red, green, blue] = oklab_to_linear_srgb(lab);
        Self::linear_rgba(red, green, blue, alpha).to_space(to.space)
    }

    /// Build an encoded sRGB color from OKLCH coordinates, reducing chroma
    /// until the color fits the sRGB gamut. See [`Oklch::to_srgb`].
    pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> Self {
        Oklch::new(lightness, chroma, hue, 1.0).to_srgb()
    }

    /// Convert this color to OKLCH. Channels outside the displayable range
    /// are converted as-is, so wide-gamut and extended-range colors keep
    /// their chroma.
    pub fn to_oklch(self) -> Oklch {
        let linear = self.to_linear_srgb();
        let [l, a, b] = linear_srgb_to_oklab([linear.red, linear.green, linear.blue]);
        let chroma = (a * a + b * b).sqrt();
        let hue = if chroma < 1.0e-6 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        };
        Oklch::new(l, chroma, hue, self.alpha)
    }

    /// WCAG 2 relative luminance of the opaque color, clamped to the SDR range.
    pub fn relative_luminance(self) -> f32 {
        let linear = self.to_linear_srgb();
        let clamp = |channel: f32| channel.clamp(0.0, 1.0);
        0.2126 * clamp(linear.red) + 0.7152 * clamp(linear.green) + 0.0722 * clamp(linear.blue)
    }

    /// WCAG 2 contrast ratio between two opaque colors, from 1.0 to 21.0.
    pub fn contrast_ratio(self, other: Color) -> f32 {
        let first = self.relative_luminance();
        let second = other.relative_luminance();
        let (lighter, darker) = if first >= second {
            (first, second)
        } else {
            (second, first)
        };
        (lighter + 0.05) / (darker + 0.05)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

/// A color in the perceptual OKLCH space: `lightness` from 0 to 1, `chroma`
/// from 0 upward (roughly 0.4 at the edge of wide gamuts), and `hue` in
/// degrees. Equal lightness steps read as equal visual steps, which makes it
/// the natural space for deriving hover, pressed, and contrast variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub lightness: f32,
    pub chroma: f32,
    pub hue: f32,
    pub alpha: f32,
}

impl Oklch {
    pub const fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        Self {
            lightness,
            chroma,
            hue,
            alpha,
        }
    }

    pub const fn with_lightness(self, lightness: f32) -> Self {
        Self { lightness, ..self }
    }

    pub const fn with_chroma(self, chroma: f32) -> Self {
        Self { chroma, ..self }
    }

    /// Encoded sRGB, gamut-mapped by reducing chroma at constant lightness
    /// and hue so the result stays recognizably the same color.
    pub fn to_srgb(self) -> Color {
        let [red, green, blue] = self.gamut_mapped_linear(Gamut::Srgb);
        Color::srgba(
            srgb_transfer_from_linear(red),
            srgb_transfer_from_linear(green),
            srgb_transfer_from_linear(blue),
            self.alpha,
        )
    }

    /// Encoded Display P3, gamut-mapped by reducing chroma.
    pub fn to_display_p3(self) -> Color {
        let [red, green, blue] = self.gamut_mapped_linear(Gamut::DisplayP3);
        Color::display_p3(
            srgb_transfer_from_linear(red),
            srgb_transfer_from_linear(green),
            srgb_transfer_from_linear(blue),
            self.alpha,
        )
    }

    /// Linear Display P3, gamut-mapped by reducing chroma. Useful as the base
    /// for extended-range HDR variants that scale linear light.
    pub fn to_linear_display_p3(self) -> Color {
        let [red, green, blue] = self.gamut_mapped_linear(Gamut::DisplayP3);
        Color::linear_display_p3(red, green, blue, self.alpha)
    }

    /// The most saturated chroma sRGB can display at this lightness and hue.
    pub fn max_srgb_chroma(self) -> f32 {
        self.max_chroma(Gamut::Srgb)
    }

    /// The most saturated chroma Display P3 can display at this lightness and hue.
    pub fn max_display_p3_chroma(self) -> f32 {
        self.max_chroma(Gamut::DisplayP3)
    }

    fn gamut_mapped_linear(self, gamut: Gamut) -> [f32; 3] {
        let lightness = self.lightness.clamp(0.0, 1.0);
        let fitted = Self {
            lightness,
            chroma: self.chroma.max(0.0).min(self.max_chroma(gamut)),
            ..self
        };
        fitted
            .linear_in(gamut)
            .map(|channel| channel.clamp(0.0, 1.0))
    }

    fn max_chroma(self, gamut: Gamut) -> f32 {
        let lightness = self.lightness.clamp(0.0, 1.0);
        let fits = |chroma: f32| {
            Self {
                lightness,
                chroma,
                ..self
            }
            .linear_in(gamut)
            .iter()
            .all(|channel| (-1.0e-5..=1.0 + 1.0e-5).contains(channel))
        };
        let (mut low, mut high) = (0.0_f32, 0.5_f32);
        for _ in 0..24 {
            let middle = (low + high) * 0.5;
            if fits(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    }

    fn linear_in(self, gamut: Gamut) -> [f32; 3] {
        let hue = self.hue.to_radians();
        let linear_srgb = oklab_to_linear_srgb([
            self.lightness,
            self.chroma * hue.cos(),
            self.chroma * hue.sin(),
        ]);
        match gamut {
            Gamut::Srgb => linear_srgb,
            Gamut::DisplayP3 => multiply_matrix3x3(LINEAR_SRGB_TO_DISPLAY_P3, linear_srgb),
        }
    }
}

#[derive(Clone, Copy)]
enum Gamut {
    Srgb,
    DisplayP3,
}

const DISPLAY_P3_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [1.224_940_2, -0.224_940_18, 0.0],
    [-0.042_056_955, 1.042_057, 0.0],
    [-0.019_637_555, -0.078_636_04, 1.098_273_6],
];

const LINEAR_SRGB_TO_DISPLAY_P3: [[f32; 3]; 3] = [
    [0.822_462_1, 0.177_538, 0.0],
    [0.033_194_2, 0.966_805_8, 0.0],
    [0.017_082_7, 0.072_397_4, 0.910_519_9],
];

fn linear_srgb_to_oklab([red, green, blue]: [f32; 3]) -> [f32; 3] {
    let l = (0.412_221_46 * red + 0.536_332_55 * green + 0.051_445_99 * blue).cbrt();
    let m = (0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue).cbrt();
    let s = (0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn oklab_to_linear_srgb([lightness, a, b]: [f32; 3]) -> [f32; 3] {
    let l = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

fn srgb_transfer_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_transfer_from_linear(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
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
    use super::{Color, ColorSpace, Oklch};

    fn assert_close(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn oklch_round_trips_srgb_colors() {
        let azure = Color::rgba(23.0 / 255.0, 98.0 / 255.0, 244.0 / 255.0, 1.0);
        let oklch = azure.to_oklch();

        assert_close(oklch.lightness, 0.549, 0.002);
        assert_close(oklch.chroma, 0.230, 0.002);
        assert_close(oklch.hue, 262.0, 0.5);

        let round_trip = oklch.to_srgb();
        assert_close(round_trip.red, azure.red, 0.001);
        assert_close(round_trip.green, azure.green, 0.001);
        assert_close(round_trip.blue, azure.blue, 0.001);
    }

    #[test]
    fn oklch_maps_out_of_gamut_chroma_into_srgb_and_p3() {
        let cyan = Oklch::new(0.55, 0.3, 230.0, 1.0);
        let max_srgb = cyan.max_srgb_chroma();
        let max_p3 = cyan.max_display_p3_chroma();

        assert!(max_srgb < 0.12, "sRGB cyan at this lightness is narrow");
        assert!(max_p3 > max_srgb, "Display P3 reaches further than sRGB");
        let mapped = cyan.to_srgb();
        for channel in [mapped.red, mapped.green, mapped.blue] {
            assert!((0.0..=1.0).contains(&channel));
        }
        assert_close(mapped.to_oklch().lightness, 0.55, 0.003);
        assert_close(mapped.to_oklch().hue, 230.0, 1.0);
    }

    #[test]
    fn achromatic_colors_have_zero_chroma() {
        let gray = Color::rgba(0.5, 0.5, 0.5, 1.0).to_oklch();

        assert!(gray.chroma < 1.0e-4);
        assert_eq!(gray.hue, 0.0);
    }

    #[test]
    fn contrast_ratio_matches_wcag_reference_values() {
        assert_close(Color::BLACK.contrast_ratio(Color::WHITE), 21.0, 0.01);
        assert_close(Color::WHITE.contrast_ratio(Color::WHITE), 1.0, 0.0001);
        let gray = Color::rgba(115.0 / 255.0, 115.0 / 255.0, 115.0 / 255.0, 1.0);
        assert_close(gray.contrast_ratio(Color::WHITE), 4.74, 0.01);
    }

    #[test]
    fn to_space_round_trips_between_encodings_and_gamuts() {
        let azure = Color::rgba(0.09, 0.38, 0.96, 0.8);
        for space in [
            ColorSpace::LinearSrgb,
            ColorSpace::DisplayP3,
            ColorSpace::LinearDisplayP3,
        ] {
            let converted = azure.to_space(space);
            assert_eq!(converted.space, space);
            assert_eq!(converted.alpha, 0.8);
            let back = converted.to_space(ColorSpace::Srgb);
            assert_close(back.red, azure.red, 1.0e-4);
            assert_close(back.green, azure.green, 1.0e-4);
            assert_close(back.blue, azure.blue, 1.0e-4);
        }
        assert_eq!(azure.to_space(ColorSpace::Srgb), azure);
    }

    #[test]
    fn mix_oklab_returns_exact_endpoints() {
        let from = Color::rgba(0.2, 0.4, 0.6, 1.0);
        let to = Color::display_p3(0.9, 0.3, 0.1, 0.5);

        assert_eq!(from.mix_oklab(to, 0.0), from);
        assert_eq!(from.mix_oklab(to, 1.0), to);
        assert_eq!(from.mix_oklab(to, -1.0), from);
        assert_eq!(from.mix_oklab(to, 2.0), to);
        assert_eq!(from.mix_oklab(to, 0.5).space, ColorSpace::DisplayP3);
    }

    #[test]
    fn mix_oklab_fades_from_transparent_without_darkening() {
        let accent = Color::rgba(0.3, 0.6, 1.0, 1.0);

        let halfway = Color::TRANSPARENT.mix_oklab(accent, 0.5);

        // Straight-alpha blending would pass through (0.15, 0.3, 0.5, 0.5).
        assert_close(halfway.alpha, 0.5, 1.0e-6);
        assert_close(halfway.red, accent.red, 1.0e-3);
        assert_close(halfway.green, accent.green, 1.0e-3);
        assert_close(halfway.blue, accent.blue, 1.0e-3);
    }

    #[test]
    fn mix_oklab_blends_lightness_perceptually() {
        let gray = Color::BLACK.mix_oklab(Color::WHITE, 0.5);

        assert_close(gray.to_oklch().lightness, 0.5, 1.0e-3);
        assert!(gray.red > 0.37 && gray.red < 0.40, "got {}", gray.red);
        assert_close(gray.red, gray.green, 1.0e-4);
        assert_close(gray.green, gray.blue, 1.0e-4);
    }

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
