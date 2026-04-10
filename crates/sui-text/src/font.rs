use std::{collections::HashMap, sync::Arc};

use cosmic_text::{fontdb, Attrs, Family, FontSystem, Metrics};
use sui_core::{Error, FontHandle, Rect, Result};
use ttf_parser::GlyphId;

use crate::model::{TextSpanId, TextStyle};

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
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSpanInput {
    pub id: TextSpanId,
    pub text: String,
    pub style: TextStyle,
    pub family_name: Option<String>,
    pub cache_face_key: FaceCacheKey,
}

#[derive(Debug, Clone)]
struct ExplicitFontSpec {
    family_name: String,
    cache_face_key: FaceCacheKey,
}

#[derive(Debug)]
pub(crate) struct FontContext {
    pub font_system: FontSystem,
    default_face: ResolvedTextFace,
    default_face_key: FaceCacheKey,
    explicit_fonts: HashMap<FontHandle, ExplicitFontSpec>,
}

impl FontContext {
    pub(crate) fn resolve_span(
        &self,
        span_id: TextSpanId,
        text: String,
        style: &TextStyle,
    ) -> Result<ResolvedSpanInput> {
        let (family_name, cache_face_key) = if let Some(handle) = style.font {
            let spec = self.explicit_fonts.get(&handle).ok_or_else(|| {
                Error::new(format!("font handle {} is not registered", handle.get()))
            })?;
            (Some(spec.family_name.clone()), spec.cache_face_key)
        } else {
            (None, self.default_face_key)
        };

        Ok(ResolvedSpanInput {
            id: span_id,
            text,
            style: style.clone(),
            family_name,
            cache_face_key,
        })
    }

    pub(crate) fn attrs_for_span<'a>(span: &'a ResolvedSpanInput, metadata: usize) -> Attrs<'a> {
        let attrs = Attrs::new()
            .metrics(Metrics::new(span.style.font_size, span.style.line_height))
            .metadata(metadata);

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
        self.font_system
            .db()
            .with_face_data(font_id, |font_data, face_index| {
                ResolvedTextFace::from_bytes(Arc::<[u8]>::from(font_data.to_vec()), face_index)
            })
            .ok_or_else(|| Error::new("failed to access font face data from cosmic-text database"))
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
}

impl TextSystemState {
    pub(crate) fn new() -> Result<Self> {
        let locale = String::from("en-US");
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();

        let families = [fontdb::Family::SansSerif];
        let default_font = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .ok_or_else(|| Error::new("failed to locate a system font for text rendering"))?;

        let default_face = font_db
            .with_face_data(default_font, |font_data, face_index| {
                ResolvedTextFace::from_bytes(Arc::<[u8]>::from(font_data.to_vec()), face_index)
            })
            .ok_or_else(|| Error::new("failed to access fallback system font data"))?;

        Ok(Self {
            locale,
            font_db,
            default_face,
        })
    }

    pub(crate) fn build_font_context(&self, font_registry: &FontRegistry) -> Result<FontContext> {
        let mut font_db = self.font_db.clone();
        let mut explicit_fonts = HashMap::new();

        for (handle, font) in &font_registry.fonts {
            let face = ResolvedTextFace::from_bytes(font.shared_bytes(), font.face_index());
            let ids = font_db.load_font_source(fontdb::Source::Binary(Arc::new(font.bytes().to_vec())));
            let face_info = ids
                .iter()
                .find_map(|id| {
                    font_db
                        .face(*id)
                        .filter(|face_info| face_info.index == font.face_index())
                })
                .ok_or_else(|| Error::new("failed to register custom font face in text database"))?;
            let family_name = face_info
                .families
                .first()
                .map(|(name, _language)| name.clone())
                .unwrap_or_else(|| face_info.post_script_name.clone());
            explicit_fonts.insert(
                *handle,
                ExplicitFontSpec {
                    family_name,
                    cache_face_key: FaceCacheKey::new(&face),
                },
            );
        }

        let font_system = FontSystem::new_with_locale_and_db(self.locale.clone(), font_db);
        Ok(FontContext {
            font_system,
            default_face: self.default_face.clone(),
            default_face_key: FaceCacheKey::new(&self.default_face),
            explicit_fonts,
        })
    }
}