//! Font styling primitives: weight, slant, width, and OpenType feature settings.
//!
//! These are sui-text-native types. They map to the shaping backend (cosmic-text) only at the
//! `Attrs` boundary (see `font.rs`), so callers never depend on cosmic-text directly.

/// OpenType weight axis value. Conventional named weights span 100..=900; for variable fonts any
/// value in the font's `wght` range is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMIBOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Font slant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// Font width (the standard nine CSS `font-stretch` keywords / OS/2 width classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

/// A single OpenType feature setting: a 4-byte tag plus a value (`0` disables, `1` enables, and
/// larger values select an alternate/parameter for features that take one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: u32,
}

impl FontFeature {
    pub const KERNING: [u8; 4] = *b"kern";
    pub const STANDARD_LIGATURES: [u8; 4] = *b"liga";
    pub const CONTEXTUAL_LIGATURES: [u8; 4] = *b"clig";
    pub const DISCRETIONARY_LIGATURES: [u8; 4] = *b"dlig";
    pub const SMALL_CAPS: [u8; 4] = *b"smcp";
    pub const CAPITALS_TO_SMALL_CAPS: [u8; 4] = *b"c2sc";
    pub const TABULAR_FIGURES: [u8; 4] = *b"tnum";
    pub const OLDSTYLE_FIGURES: [u8; 4] = *b"onum";
    pub const FRACTIONS: [u8; 4] = *b"frac";
    pub const SLASHED_ZERO: [u8; 4] = *b"zero";

    /// Stylistic set `n` (1..=20), e.g. `stylistic_set(1)` -> `ss01`.
    pub const fn stylistic_set(n: u8) -> [u8; 4] {
        let tens = b'0' + (n / 10);
        let ones = b'0' + (n % 10);
        [b's', b's', tens, ones]
    }

    pub const fn on(tag: [u8; 4]) -> Self {
        Self { tag, value: 1 }
    }

    pub const fn off(tag: [u8; 4]) -> Self {
        Self { tag, value: 0 }
    }

    pub const fn set(tag: [u8; 4], value: u32) -> Self {
        Self { tag, value }
    }
}

/// An ordered collection of OpenType feature settings applied to a text run. Later settings for
/// the same tag take precedence in the shaper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FontFeatures {
    features: Vec<FontFeature>,
}

impl FontFeatures {
    pub const fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }

    /// Enable a feature (`value = 1`).
    pub fn enable(&mut self, tag: [u8; 4]) -> &mut Self {
        self.features.push(FontFeature::on(tag));
        self
    }

    /// Disable a feature (`value = 0`).
    pub fn disable(&mut self, tag: [u8; 4]) -> &mut Self {
        self.features.push(FontFeature::off(tag));
        self
    }

    /// Set a feature to an explicit value.
    pub fn set(&mut self, tag: [u8; 4], value: u32) -> &mut Self {
        self.features.push(FontFeature::set(tag, value));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FontFeature> {
        self.features.iter()
    }
}

impl FromIterator<FontFeature> for FontFeatures {
    fn from_iter<I: IntoIterator<Item = FontFeature>>(iter: I) -> Self {
        Self {
            features: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_named_values_and_default() {
        assert_eq!(FontWeight::NORMAL.value(), 400);
        assert_eq!(FontWeight::BOLD.value(), 700);
        assert_eq!(FontWeight::THIN.value(), 100);
        assert_eq!(FontWeight::BLACK.value(), 900);
        assert_eq!(FontWeight::new(550).value(), 550);
        assert_eq!(FontWeight::default(), FontWeight::NORMAL);
    }

    #[test]
    fn style_and_stretch_defaults_are_normal() {
        assert_eq!(FontStyle::default(), FontStyle::Normal);
        assert_eq!(FontStretch::default(), FontStretch::Normal);
    }

    #[test]
    fn feature_tag_constants_and_constructors() {
        assert_eq!(FontFeature::STANDARD_LIGATURES, *b"liga");
        assert_eq!(FontFeature::SMALL_CAPS, *b"smcp");
        assert_eq!(FontFeature::stylistic_set(1), *b"ss01");
        assert_eq!(FontFeature::stylistic_set(12), *b"ss12");
        assert_eq!(FontFeature::on(*b"liga"), FontFeature { tag: *b"liga", value: 1 });
        assert_eq!(FontFeature::off(*b"liga"), FontFeature { tag: *b"liga", value: 0 });
        assert_eq!(FontFeature::set(*b"aalt", 3).value, 3);
    }

    #[test]
    fn feature_collection_records_settings_in_order() {
        let mut features = FontFeatures::new();
        assert!(features.is_empty());
        features
            .disable(FontFeature::STANDARD_LIGATURES)
            .enable(FontFeature::SMALL_CAPS)
            .set(FontFeature::TABULAR_FIGURES, 1);

        let collected: Vec<_> = features.iter().copied().collect();
        assert_eq!(
            collected,
            vec![
                FontFeature::off(*b"liga"),
                FontFeature::on(*b"smcp"),
                FontFeature::set(*b"tnum", 1),
            ]
        );
        assert_eq!(features.len(), 3);
        assert!(!features.is_empty());
    }

    #[test]
    fn features_equality_is_order_sensitive() {
        let a: FontFeatures = [FontFeature::on(*b"liga"), FontFeature::on(*b"smcp")]
            .into_iter()
            .collect();
        let b: FontFeatures = [FontFeature::on(*b"smcp"), FontFeature::on(*b"liga")]
            .into_iter()
            .collect();
        assert_ne!(a, b);

        let c: FontFeatures = [FontFeature::on(*b"liga"), FontFeature::on(*b"smcp")]
            .into_iter()
            .collect();
        assert_eq!(a, c);
    }
}
