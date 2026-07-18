use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cosmic_text::{Attrs, Family, FeatureTag, FontSystem, Metrics, Stretch, Style, Weight, fontdb};
use sui_core::{Error, FontHandle, Rect, Result};
use ttf_parser::GlyphId;

use crate::model::{FontFamilyStack, TextSpanId, TextStyle};
use crate::style::{FontFeatures, FontStretch, FontStyle, FontWeight};

/// Map sui-text's `FontWeight` to cosmic-text's `Weight` (drives bold-face selection and, for
/// variable fonts, the `wght` shaping instance).
pub(crate) fn to_cosmic_weight(weight: FontWeight) -> Weight {
    Weight(weight.value())
}

pub(crate) fn to_cosmic_style(style: FontStyle) -> Style {
    match style {
        FontStyle::Normal => Style::Normal,
        FontStyle::Italic => Style::Italic,
        FontStyle::Oblique => Style::Oblique,
    }
}

pub(crate) fn to_cosmic_stretch(stretch: FontStretch) -> Stretch {
    match stretch {
        FontStretch::UltraCondensed => Stretch::UltraCondensed,
        FontStretch::ExtraCondensed => Stretch::ExtraCondensed,
        FontStretch::Condensed => Stretch::Condensed,
        FontStretch::SemiCondensed => Stretch::SemiCondensed,
        FontStretch::Normal => Stretch::Normal,
        FontStretch::SemiExpanded => Stretch::SemiExpanded,
        FontStretch::Expanded => Stretch::Expanded,
        FontStretch::ExtraExpanded => Stretch::ExtraExpanded,
        FontStretch::UltraExpanded => Stretch::UltraExpanded,
    }
}

pub(crate) fn to_cosmic_features(features: &FontFeatures) -> cosmic_text::FontFeatures {
    let mut out = cosmic_text::FontFeatures::new();
    for feature in features.iter() {
        out.set(FeatureTag::new(&feature.tag), feature.value);
    }
    out
}

#[cfg(test)]
mod attrs_mapping_tests {
    use super::*;
    use crate::style::FontFeature;

    #[test]
    fn weight_maps_to_cosmic_weight() {
        assert_eq!(to_cosmic_weight(FontWeight::NORMAL), Weight(400));
        assert_eq!(to_cosmic_weight(FontWeight::BOLD), Weight(700));
        assert_eq!(to_cosmic_weight(FontWeight::new(550)), Weight(550));
    }

    #[test]
    fn style_maps_to_cosmic_style() {
        assert_eq!(to_cosmic_style(FontStyle::Normal), Style::Normal);
        assert_eq!(to_cosmic_style(FontStyle::Italic), Style::Italic);
        assert_eq!(to_cosmic_style(FontStyle::Oblique), Style::Oblique);
    }

    #[test]
    fn stretch_maps_to_cosmic_stretch() {
        assert_eq!(to_cosmic_stretch(FontStretch::Normal), Stretch::Normal);
        assert_eq!(
            to_cosmic_stretch(FontStretch::Condensed),
            Stretch::Condensed
        );
        assert_eq!(
            to_cosmic_stretch(FontStretch::UltraExpanded),
            Stretch::UltraExpanded
        );
    }

    #[test]
    fn features_map_to_cosmic_features_preserving_tag_value_and_order() {
        let mut features = FontFeatures::new();
        features
            .disable(FontFeature::STANDARD_LIGATURES)
            .enable(FontFeature::SMALL_CAPS);

        let mut expected = cosmic_text::FontFeatures::new();
        expected.set(FeatureTag::new(b"liga"), 0);
        expected.set(FeatureTag::new(b"smcp"), 1);

        assert_eq!(to_cosmic_features(&features), expected);
        assert_eq!(
            to_cosmic_features(&FontFeatures::new()),
            cosmic_text::FontFeatures::new()
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFont {
    data: Arc<[u8]>,
    face_index: u32,
}

impl RegisteredFont {
    pub fn from_bytes(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: Arc::<[u8]>::from(data.into()),
            face_index: 0,
        }
    }

    pub const fn with_face_index(mut self, face_index: u32) -> Self {
        self.face_index = face_index;
        self
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.data)
    }

    pub fn data_ptr(&self) -> usize {
        self.data.as_ptr() as usize
    }

    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontRegistry {
    pub(crate) fonts: HashMap<FontHandle, RegisteredFont>,
}

impl FontRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: FontHandle, font: RegisteredFont) -> Option<RegisteredFont> {
        self.fonts.insert(handle, font)
    }

    pub fn get(&self, handle: FontHandle) -> Option<&RegisteredFont> {
        self.fonts.get(&handle)
    }

    pub fn contains(&self, handle: FontHandle) -> bool {
        self.fonts.contains_key(&handle)
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    pub(crate) fn cache_key(&self) -> Vec<(FontHandle, FaceCacheKey)> {
        // The context retains every registered font's Arc-backed bytes, so their
        // allocation identities cannot be reused while this key is cached.
        let mut key = self
            .fonts
            .iter()
            .map(|(handle, font)| (*handle, FaceCacheKey::from_registered_font(font)))
            .collect::<Vec<_>>();
        key.sort_unstable_by_key(|(handle, _)| *handle);
        key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTextFace {
    data: Arc<[u8]>,
    face_index: u32,
}

impl ResolvedTextFace {
    pub fn from_bytes(data: Arc<[u8]>, face_index: u32) -> Self {
        Self { data, face_index }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn data_ptr(&self) -> usize {
        self.data.as_ptr() as usize
    }

    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.data)
    }

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }

    pub(crate) fn glyph_bounds(
        &self,
        glyph_id: u16,
        origin_x: f32,
        origin_y: f32,
        scale: f32,
    ) -> Option<Rect> {
        let face = ttf_parser::Face::parse(self.bytes(), self.face_index()).ok()?;
        face.glyph_bounding_box(GlyphId(glyph_id)).map(|bbox| {
            let min_x = origin_x + (f32::from(bbox.x_min) * scale);
            let max_x = origin_x + (f32::from(bbox.x_max) * scale);
            let min_y = origin_y - (f32::from(bbox.y_max) * scale);
            let max_y = origin_y - (f32::from(bbox.y_min) * scale);
            Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FaceCacheKey {
    data_ptr: usize,
    data_len: usize,
    face_index: u32,
}

impl FaceCacheKey {
    pub(crate) fn new(face: &ResolvedTextFace) -> Self {
        Self {
            data_ptr: face.data_ptr(),
            data_len: face.data_len(),
            face_index: face.face_index(),
        }
    }

    pub(crate) fn from_registered_font(font: &RegisteredFont) -> Self {
        Self {
            data_ptr: font.data_ptr(),
            data_len: font.data_len(),
            face_index: font.face_index(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSpanInput {
    pub id: TextSpanId,
    pub text: String,
    pub style: TextStyle,
    pub family_name: Option<String>,
}

#[derive(Debug, Clone)]
struct ExplicitFontSpec {
    family_name: String,
}

#[derive(Debug)]
pub(crate) struct FontContext {
    pub font_system: FontSystem,
    default_face: ResolvedTextFace,
    explicit_fonts: HashMap<FontHandle, ExplicitFontSpec>,
    explicit_faces: HashMap<fontdb::ID, ResolvedTextFace>,
    shared_faces: Arc<Mutex<HashMap<fontdb::ID, ResolvedTextFace>>>,
}

impl FontContext {
    pub(crate) fn resolve_span(
        &self,
        span_id: TextSpanId,
        text: String,
        style: &TextStyle,
    ) -> Result<ResolvedSpanInput> {
        let family_name = if let Some(handle) = style.font {
            let spec = self.explicit_fonts.get(&handle).ok_or_else(|| {
                Error::new(format!("font handle {} is not registered", handle.get()))
            })?;
            Some(spec.family_name.clone())
        } else if let Some(families) = style.font_families {
            resolve_font_family_name(self.font_system.db(), families, style)
        } else {
            None
        };

        Ok(ResolvedSpanInput {
            id: span_id,
            text,
            style: style.clone(),
            family_name,
        })
    }

    pub(crate) fn attrs_for_span<'a>(span: &'a ResolvedSpanInput, metadata: usize) -> Attrs<'a> {
        let text_style = &span.style;
        let mut attrs = Attrs::new()
            .metrics(Metrics::new(text_style.font_size, text_style.line_height))
            .metadata(metadata)
            .weight(to_cosmic_weight(text_style.weight))
            .style(to_cosmic_style(text_style.style))
            .stretch(to_cosmic_stretch(text_style.stretch));
        if !text_style.features.is_empty() {
            attrs = attrs.font_features(to_cosmic_features(&text_style.features));
        }

        match span.family_name.as_deref() {
            Some(name) => attrs.family(Family::Name(name)),
            None => attrs,
        }
    }

    pub(crate) fn resolve_face_index(
        &self,
        face_slots: &mut HashMap<fontdb::ID, usize>,
        faces: &mut Vec<ResolvedTextFace>,
        font_id: fontdb::ID,
    ) -> Result<usize> {
        if let Some(index) = face_slots.get(&font_id) {
            return Ok(*index);
        }

        let face = self.resolve_face(font_id)?;
        let index = faces.len();
        faces.push(face);
        face_slots.insert(font_id, index);
        Ok(index)
    }

    pub(crate) fn resolve_face(&self, font_id: fontdb::ID) -> Result<ResolvedTextFace> {
        if let Some(face) = self.explicit_faces.get(&font_id).cloned() {
            return Ok(face);
        }

        if let Some(face) = self
            .shared_faces
            .lock()
            .map_err(|_| Error::new("shared text face cache lock was poisoned"))?
            .get(&font_id)
            .cloned()
        {
            return Ok(face);
        }

        let face = self
            .font_system
            .db()
            .with_face_data(font_id, |font_data, face_index| {
                ResolvedTextFace::from_bytes(Arc::<[u8]>::from(font_data.to_vec()), face_index)
            })
            .ok_or_else(|| {
                Error::new("failed to access font face data from cosmic-text database")
            })?;

        self.shared_faces
            .lock()
            .map_err(|_| Error::new("shared text face cache lock was poisoned"))?
            .insert(font_id, face.clone());

        Ok(face)
    }

    pub(crate) fn default_face(&self) -> &ResolvedTextFace {
        &self.default_face
    }
}

#[derive(Debug)]
pub(crate) struct TextSystemState {
    locale: String,
    font_db: fontdb::Database,
    default_face: ResolvedTextFace,
    default_face_key: FaceCacheKey,
    shared_faces: Arc<Mutex<HashMap<fontdb::ID, ResolvedTextFace>>>,
}

impl TextSystemState {
    pub(crate) fn new() -> Result<Self> {
        let locale = String::from("en-US");
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        load_bundled_fallback_fonts(&mut font_db);
        configure_platform_generic_families(&mut font_db);

        let families = [fontdb::Family::SansSerif];
        let default_font = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .ok_or_else(|| Error::new("failed to locate a font for text rendering"))?;

        let default_face = font_db
            .with_face_data(default_font, |font_data, face_index| {
                ResolvedTextFace::from_bytes(Arc::<[u8]>::from(font_data.to_vec()), face_index)
            })
            .ok_or_else(|| Error::new("failed to access fallback system font data"))?;
        let mut shared_faces = HashMap::new();
        shared_faces.insert(default_font, default_face.clone());

        Ok(Self {
            locale,
            font_db,
            default_face_key: FaceCacheKey::new(&default_face),
            default_face,
            shared_faces: Arc::new(Mutex::new(shared_faces)),
        })
    }

    pub(crate) fn default_face_key(&self) -> FaceCacheKey {
        self.default_face_key
    }

    pub(crate) fn build_font_context(&self, font_registry: &FontRegistry) -> Result<FontContext> {
        let mut font_db = self.font_db.clone();
        let mut explicit_fonts = HashMap::new();
        let mut explicit_faces = HashMap::new();

        for (handle, font) in &font_registry.fonts {
            let ids =
                font_db.load_font_source(fontdb::Source::Binary(Arc::new(font.bytes().to_vec())));
            let mut family_name = None;
            for id in ids {
                let face_info = font_db.face(id).ok_or_else(|| {
                    Error::new("failed to register custom font face in text database")
                })?;
                explicit_faces.insert(
                    id,
                    ResolvedTextFace::from_bytes(font.shared_bytes(), face_info.index),
                );

                if face_info.index == font.face_index() {
                    family_name = Some(
                        face_info
                            .families
                            .first()
                            .map(|(name, _language)| name.clone())
                            .unwrap_or_else(|| face_info.post_script_name.clone()),
                    );
                }
            }

            let family_name = family_name.ok_or_else(|| {
                Error::new("failed to register custom font face in text database")
            })?;
            explicit_fonts.insert(*handle, ExplicitFontSpec { family_name });
        }

        let font_system = FontSystem::new_with_locale_and_db(self.locale.clone(), font_db);
        Ok(FontContext {
            font_system,
            default_face: self.default_face.clone(),
            explicit_fonts,
            explicit_faces,
            shared_faces: Arc::clone(&self.shared_faces),
        })
    }
}

fn resolve_font_family_name(
    font_db: &fontdb::Database,
    stack: FontFamilyStack,
    style: &TextStyle,
) -> Option<String> {
    let families = stack.iter().map(fontdb_family).collect::<Vec<_>>();
    let font_id = font_db.query(&fontdb::Query {
        families: &families,
        weight: Weight(style.weight.value()),
        stretch: to_cosmic_stretch(style.stretch),
        style: to_cosmic_style(style.style),
    })?;
    let face = font_db.face(font_id)?;
    face.families
        .first()
        .map(|(name, _language)| name.clone())
        .or_else(|| (!face.post_script_name.is_empty()).then(|| face.post_script_name.clone()))
}

fn fontdb_family(name: &str) -> Family<'_> {
    if name.eq_ignore_ascii_case("ui-sans-serif")
        || name.eq_ignore_ascii_case("system-ui")
        || name.eq_ignore_ascii_case("-apple-system")
        || name.eq_ignore_ascii_case("BlinkMacSystemFont")
        || name.eq_ignore_ascii_case("sans-serif")
    {
        Family::SansSerif
    } else if name.eq_ignore_ascii_case("ui-serif") || name.eq_ignore_ascii_case("serif") {
        Family::Serif
    } else if name.eq_ignore_ascii_case("ui-monospace") || name.eq_ignore_ascii_case("monospace") {
        Family::Monospace
    } else if name.eq_ignore_ascii_case("cursive") {
        Family::Cursive
    } else if name.eq_ignore_ascii_case("fantasy") {
        Family::Fantasy
    } else {
        Family::Name(name)
    }
}

#[cfg(target_os = "windows")]
const PLATFORM_SANS_FAMILIES: &[&str] = &["Segoe UI Variable Text", "Segoe UI", "Arial"];
#[cfg(target_os = "windows")]
const PLATFORM_SERIF_FAMILIES: &[&str] = &["Cambria", "Georgia", "Times New Roman"];
#[cfg(target_os = "windows")]
const PLATFORM_MONO_FAMILIES: &[&str] =
    &["Cascadia Mono", "Cascadia Code", "Consolas", "Courier New"];

#[cfg(target_os = "macos")]
const PLATFORM_SANS_FAMILIES: &[&str] =
    &["SF Pro Text", "SF Pro Display", "Helvetica Neue", "Arial"];
#[cfg(target_os = "macos")]
const PLATFORM_SERIF_FAMILIES: &[&str] = &["New York", "Times New Roman", "Georgia"];
#[cfg(target_os = "macos")]
const PLATFORM_MONO_FAMILIES: &[&str] = &["SFMono-Regular", "SF Mono", "Menlo", "Monaco"];

#[cfg(all(
    target_arch = "wasm32",
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_SANS_FAMILIES: &[&str] = &["Noto Sans"];
#[cfg(all(
    target_arch = "wasm32",
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_SERIF_FAMILIES: &[&str] = &["Noto Serif"];
#[cfg(all(
    target_arch = "wasm32",
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_MONO_FAMILIES: &[&str] = &["Noto Sans Mono"];

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_SANS_FAMILIES: &[&str] = &["Noto Sans", "Inter", "DejaVu Sans", "Liberation Sans"];
#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_SERIF_FAMILIES: &[&str] = &["Noto Serif", "DejaVu Serif", "Liberation Serif"];
#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "macos"))
))]
const PLATFORM_MONO_FAMILIES: &[&str] = &["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono"];

fn configure_platform_generic_families(font_db: &mut fontdb::Database) {
    if let Some(family) = first_installed_family(font_db, PLATFORM_SANS_FAMILIES) {
        font_db.set_sans_serif_family(family);
    }
    if let Some(family) = first_installed_family(font_db, PLATFORM_SERIF_FAMILIES) {
        font_db.set_serif_family(family);
    }
    if let Some(family) = first_installed_family(font_db, PLATFORM_MONO_FAMILIES) {
        font_db.set_monospace_family(family);
    }
}

fn first_installed_family(font_db: &fontdb::Database, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        font_db.faces().find_map(|face| {
            face.families.iter().find_map(|(family, _language)| {
                family
                    .eq_ignore_ascii_case(candidate)
                    .then(|| family.clone())
            })
        })
    })
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn load_bundled_fallback_fonts(font_db: &mut fontdb::Database) {
    load_bundled_portable_fallback_fonts(font_db);
}

#[cfg(any(target_arch = "wasm32", target_os = "android", test))]
fn load_bundled_portable_fallback_fonts(font_db: &mut fontdb::Database) {
    let sans_ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(
        include_bytes!("../assets/NotoSans-Regular.ttf").to_vec(),
    )));
    let _ = set_sans_serif_family_from_loaded_faces(font_db, &sans_ids);

    let serif_ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(
        include_bytes!("../assets/NotoSerif-Variable.ttf").to_vec(),
    )));
    let _ = set_serif_family_from_loaded_faces(font_db, &serif_ids);

    let mono_ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(
        include_bytes!("../assets/NotoSansMono-Variable.ttf").to_vec(),
    )));
    let _ = set_monospace_family_from_loaded_faces(font_db, &mono_ids);
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn load_bundled_fallback_fonts(_font_db: &mut fontdb::Database) {}

#[cfg(any(target_arch = "wasm32", target_os = "android", test))]
fn set_sans_serif_family_from_loaded_faces(
    font_db: &mut fontdb::Database,
    ids: &[fontdb::ID],
) -> Option<String> {
    let name = preferred_family_from_loaded_faces(font_db, ids, "Noto Sans")?;
    font_db.set_sans_serif_family(name.clone());
    Some(name)
}

#[cfg(any(target_arch = "wasm32", target_os = "android", test))]
fn set_serif_family_from_loaded_faces(
    font_db: &mut fontdb::Database,
    ids: &[fontdb::ID],
) -> Option<String> {
    let name = preferred_family_from_loaded_faces(font_db, ids, "Noto Serif")?;
    font_db.set_serif_family(name.clone());
    Some(name)
}

#[cfg(any(target_arch = "wasm32", target_os = "android", test))]
fn set_monospace_family_from_loaded_faces(
    font_db: &mut fontdb::Database,
    ids: &[fontdb::ID],
) -> Option<String> {
    let name = preferred_family_from_loaded_faces(font_db, ids, "Noto Sans Mono")?;
    font_db.set_monospace_family(name.clone());
    Some(name)
}

#[cfg(any(target_arch = "wasm32", target_os = "android", test))]
fn preferred_family_from_loaded_faces(
    font_db: &fontdb::Database,
    ids: &[fontdb::ID],
    preferred: &str,
) -> Option<String> {
    let mut fallback = None;
    for id in ids {
        let Some(face_info) = font_db.face(*id) else {
            continue;
        };
        for (name, _language) in &face_info.families {
            if name.eq_ignore_ascii_case(preferred) {
                return Some(name.clone());
            }
            fallback.get_or_insert_with(|| name.clone());
        }
        if fallback.is_none() && !face_info.post_script_name.is_empty() {
            fallback = Some(face_info.post_script_name.clone());
        }
    }

    fallback
}

#[cfg(test)]
mod font_database_tests {
    use super::*;

    #[test]
    fn bundled_web_sans_font_becomes_generic_sans_serif() {
        let mut font_db = fontdb::Database::new();
        let ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(
            include_bytes!("../assets/NotoSans-Regular.ttf").to_vec(),
        )));

        let family_name = set_sans_serif_family_from_loaded_faces(&mut font_db, &ids)
            .expect("bundled sans font should expose a family name");

        assert_eq!(family_name, "Noto Sans");
        assert_eq!(font_db.family_name(&fontdb::Family::SansSerif), "Noto Sans");

        let families = [fontdb::Family::SansSerif];
        let font_id = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .expect("generic sans-serif should resolve to bundled font");
        let face = font_db
            .face(font_id)
            .expect("resolved bundled font face should still be registered");
        assert!(face.families.iter().any(|(name, _)| name == "Noto Sans"));
    }

    #[test]
    fn bundled_portable_fonts_resolve_all_theme_generic_stacks_on_native_tests() {
        let mut font_db = fontdb::Database::new();
        load_bundled_portable_fallback_fonts(&mut font_db);
        configure_platform_generic_families(&mut font_db);

        assert_eq!(font_db.family_name(&fontdb::Family::SansSerif), "Noto Sans");
        assert_eq!(font_db.family_name(&fontdb::Family::Serif), "Noto Serif");
        assert_eq!(
            font_db.family_name(&fontdb::Family::Monospace),
            "Noto Sans Mono"
        );

        let cases = [
            (
                FontFamilyStack::new("ui-sans-serif", &["system-ui", "sans-serif"]),
                "Noto Sans",
            ),
            (
                FontFamilyStack::new("ui-serif", &["Georgia", "serif"]),
                "Noto Serif",
            ),
            (
                FontFamilyStack::new("ui-monospace", &["Consolas", "monospace"]),
                "Noto Sans Mono",
            ),
        ];

        for (stack, expected) in cases {
            let style = TextStyle {
                font_families: Some(stack),
                ..TextStyle::default()
            };
            let resolved = resolve_font_family_name(&font_db, stack, &style);
            assert_eq!(resolved.as_deref(), Some(expected));

            let font_id = font_db
                .query(&fontdb::Query {
                    families: &[fontdb_family(stack.primary)],
                    weight: fontdb::Weight::NORMAL,
                    stretch: fontdb::Stretch::Normal,
                    style: fontdb::Style::Normal,
                })
                .expect("bundled generic family should resolve to an actual face");
            let face = font_db
                .face(font_id)
                .expect("resolved bundled face should remain registered");
            assert!(face.families.iter().any(|(name, _)| name == expected));
            assert!(
                font_db
                    .with_face_data(font_id, |data, _index| !data.is_empty())
                    .unwrap_or(false),
                "resolved {expected} face should expose font bytes"
            );
        }
    }

    #[test]
    fn theme_family_stack_resolves_generic_alias_after_missing_family() {
        let mut font_db = fontdb::Database::new();
        let ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(
            include_bytes!("../assets/NotoSans-Regular.ttf").to_vec(),
        )));
        set_sans_serif_family_from_loaded_faces(&mut font_db, &ids)
            .expect("bundled sans font should expose a family name");
        let style = TextStyle {
            font_families: Some(FontFamilyStack::new(
                "Missing UI Family",
                &["system-ui", "sans-serif"],
            )),
            ..TextStyle::default()
        };

        assert_eq!(
            resolve_font_family_name(
                &font_db,
                style.font_families.expect("test family stack"),
                &style,
            )
            .as_deref(),
            Some("Noto Sans")
        );
    }

    #[test]
    fn platform_family_preference_uses_first_installed_candidate() {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_source(fontdb::Source::Binary(Arc::new(
            include_bytes!("../assets/NotoSans-Regular.ttf").to_vec(),
        )));

        assert_eq!(
            first_installed_family(&font_db, &["Missing UI Family", "Noto Sans"]).as_deref(),
            Some("Noto Sans")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_generic_sans_prefers_the_first_installed_ui_family() {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        let expected = first_installed_family(&font_db, PLATFORM_SANS_FAMILIES)
            .expect("Windows should provide at least one supported UI font");

        configure_platform_generic_families(&mut font_db);

        assert_eq!(font_db.family_name(&fontdb::Family::SansSerif), expected);
    }
}
