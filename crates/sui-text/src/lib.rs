#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};

use sui_core::{Color, Error, FontHandle, Point, Rect, Result, Size, Vector};
use ttf_parser::GlyphId;

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub font: Option<FontHandle>,
    pub font_size: f32,
    pub line_height: f32,
    pub color: Color,
}

impl TextStyle {
    pub fn new(color: Color) -> Self {
        Self {
            color,
            ..Self::default()
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 14.0,
            line_height: 18.0,
            color: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub rect: Rect,
    pub text: String,
    pub style: TextStyle,
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

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontRegistry {
    fonts: HashMap<FontHandle, RegisteredFont>,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMeasurement {
    pub width: f32,
    pub height: f32,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub cluster: usize,
    pub line_index: usize,
    pub origin_x: f32,
    pub origin_y: f32,
    pub advance: Vector,
    pub scale: f32,
    pub bounds: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
struct TextClusterGeometry {
    range: Range<usize>,
    x_start: f32,
    x_end: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub byte_range: Range<usize>,
    pub rect: Rect,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
    clusters: Vec<TextClusterGeometry>,
}

impl TextLine {
    fn x_for_offset(&self, offset: usize) -> f32 {
        if self.clusters.is_empty() {
            return self.rect.x();
        }

        let offset = offset.clamp(self.byte_range.start, self.byte_range.end);

        if offset <= self.byte_range.start {
            return self.clusters[0].x_start;
        }

        for cluster in &self.clusters {
            if offset <= cluster.range.end {
                let span = (cluster.range.end.saturating_sub(cluster.range.start)).max(1) as f32;
                let local = offset.saturating_sub(cluster.range.start) as f32;
                let t = (local / span).clamp(0.0, 1.0);
                return cluster.x_start + ((cluster.x_end - cluster.x_start) * t);
            }
        }

        self.clusters
            .last()
            .map(|cluster| cluster.x_end)
            .unwrap_or(self.rect.max_x())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TextLayoutData {
    text: String,
    box_size: Size,
    face: ResolvedTextFace,
    measurement: TextMeasurement,
    glyphs: Vec<ShapedGlyph>,
    lines: Vec<TextLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    data: Arc<TextLayoutData>,
    style: TextStyle,
}

impl TextLayout {
    pub fn text(&self) -> &str {
        &self.data.text
    }

    pub fn style(&self) -> &TextStyle {
        &self.style
    }

    pub fn box_size(&self) -> Size {
        self.data.box_size
    }

    pub fn measurement(&self) -> TextMeasurement {
        self.data.measurement
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.data.glyphs
    }

    pub fn lines(&self) -> &[TextLine] {
        &self.data.lines
    }

    pub fn face(&self) -> &ResolvedTextFace {
        &self.data.face
    }

    pub fn caret_rect(&self, utf8_offset: usize) -> Rect {
        let line = self.line_for_offset(utf8_offset);
        Rect::new(
            line.x_for_offset(utf8_offset),
            line.rect.y(),
            1.0,
            line.rect.height(),
        )
    }

    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        let start = range.start.min(self.data.text.len());
        let end = range.end.min(self.data.text.len());
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        if start == end {
            return Vec::new();
        }

        let mut rects = Vec::new();

        for line in &self.data.lines {
            let line_start = start.max(line.byte_range.start);
            let line_end = end.min(line.byte_range.end);
            if line_start >= line_end {
                continue;
            }

            let x0 = line.x_for_offset(line_start);
            let x1 = line.x_for_offset(line_end);
            let left = x0.min(x1);
            let right = x0.max(x1);
            rects.push(Rect::new(
                left,
                line.rect.y(),
                (right - left).max(0.0),
                line.rect.height(),
            ));
        }

        rects
    }

    pub fn selection_bounds(&self, range: Range<usize>) -> Option<Rect> {
        let mut rects = self.selection_rects(range).into_iter();
        let first = rects.next()?;
        Some(rects.fold(first, |bounds, rect| bounds.union(rect)))
    }

    fn line_for_offset(&self, utf8_offset: usize) -> &TextLine {
        let offset = utf8_offset.min(self.data.text.len());
        self.data
            .lines
            .iter()
            .find(|line| offset <= line.byte_range.end)
            .or_else(|| self.data.lines.last())
            .expect("text layouts always contain at least one line")
    }

    fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    #[cfg(test)]
    fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    pub origin: Point,
    pub layout: TextLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FaceCacheKey {
    data_ptr: usize,
    data_len: usize,
    face_index: u32,
}

impl FaceCacheKey {
    fn new(face: &ResolvedTextFace) -> Self {
        Self {
            data_ptr: face.data_ptr(),
            data_len: face.data_len(),
            face_index: face.face_index(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SizeCacheKey {
    width_bits: u32,
    height_bits: u32,
}

impl From<Size> for SizeCacheKey {
    fn from(value: Size) -> Self {
        Self {
            width_bits: value.width.to_bits(),
            height_bits: value.height.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextLayoutCacheKey {
    text: String,
    face: FaceCacheKey,
    box_size: Option<SizeCacheKey>,
    font_size_bits: u32,
    line_height_bits: u32,
}

impl TextLayoutCacheKey {
    fn new(text: &str, style: &TextStyle, box_size: Option<Size>, face: &ResolvedTextFace) -> Self {
        Self {
            text: text.to_string(),
            face: FaceCacheKey::new(face),
            box_size: box_size.map(SizeCacheKey::from),
            font_size_bits: style.font_size.to_bits(),
            line_height_bits: style.line_height.to_bits(),
        }
    }
}

#[derive(Debug, Default)]
struct TextLayoutCache {
    entries: HashMap<TextLayoutCacheKey, TextLayout>,
    hits: usize,
    misses: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextLayoutCacheSnapshot {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl TextLayoutCacheSnapshot {
    pub const fn requests(self) -> usize {
        self.hits + self.misses
    }

    pub fn hit_rate(self) -> f64 {
        let requests = self.requests();
        if requests == 0 {
            0.0
        } else {
            self.hits as f64 / requests as f64
        }
    }
}

#[derive(Debug, Default)]
pub struct TextSystem {
    state: OnceLock<std::result::Result<TextSystemState, String>>,
    layout_cache: Mutex<TextLayoutCache>,
}

impl TextSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
        font_registry: &FontRegistry,
    ) -> Result<TextMeasurement> {
        Ok(self
            .shape_text_internal(text.into(), style, None, font_registry)?
            .measurement())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.shape_text_internal(text.into(), style, Some(box_size), font_registry)
    }

    pub fn shape_text_run(
        &self,
        run: &TextRun,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.shape_text(
            run.text.clone(),
            run.rect.size,
            run.style.clone(),
            font_registry,
        )
    }

    pub fn layout_cache_snapshot(&self) -> TextLayoutCacheSnapshot {
        self.layout_cache
            .lock()
            .ok()
            .map(|cache| TextLayoutCacheSnapshot {
                entries: cache.entries.len(),
                hits: cache.hits,
                misses: cache.misses,
            })
            .unwrap_or_default()
    }

    fn shape_text_internal(
        &self,
        text: String,
        style: TextStyle,
        box_size: Option<Size>,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        let face = self.resolve_face(style.font, font_registry)?;
        let cache_key = TextLayoutCacheKey::new(&text, &style, box_size, &face);

        if let Some(cached) = self.cached_layout(&cache_key)? {
            return Ok(cached.with_style(style));
        }

        let layout = self.shape_text_uncached(text, style.clone(), box_size, face)?;
        self.store_layout(cache_key, layout.clone())?;
        Ok(layout.with_style(style))
    }

    fn shape_text_uncached(
        &self,
        text: String,
        style: TextStyle,
        box_size: Option<Size>,
        face: ResolvedTextFace,
    ) -> Result<TextLayout> {
        let rustybuzz_face = rustybuzz::Face::from_slice(face.bytes(), face.face_index())
            .ok_or_else(|| Error::new("failed to parse text face data"))?;

        let units_per_em = rustybuzz_face.units_per_em() as f32;
        if units_per_em <= 0.0 {
            return Err(Error::new(
                "text face reported an invalid units-per-em value",
            ));
        }

        let scale = style.font_size / units_per_em;
        let ascent = f32::from(rustybuzz_face.ascender()) * scale;
        let descent = f32::from(rustybuzz_face.descender().abs()) * scale;
        let natural_line_height = f32::from(rustybuzz_face.height().abs()) * scale;
        let line_height = style
            .line_height
            .max(natural_line_height)
            .max(style.font_size);

        let line_specs = collect_line_specs(&text, &rustybuzz_face, scale);
        let measured_width = line_specs
            .iter()
            .map(|line| line.width)
            .fold(0.0_f32, f32::max);
        let line_count = line_specs.len().max(1);
        let block_height = line_height * line_count as f32;
        let box_size = box_size.unwrap_or(Size::new(
            measured_width,
            block_height.max(ascent + descent),
        ));
        let block_top = ((box_size.height - block_height).max(0.0)) * 0.5;

        let mut glyphs = Vec::new();
        let mut lines = Vec::with_capacity(line_specs.len().max(1));
        let mut measured_bounds: Option<(f32, f32, f32, f32)> = None;

        for (line_index, line) in line_specs.iter().enumerate() {
            let line_origin_x = match line.direction {
                rustybuzz::Direction::RightToLeft => box_size.width - line.width,
                _ => 0.0,
            };
            let baseline = block_top + ascent + (line_index as f32 * line_height);
            let line_top = block_top + (line_index as f32 * line_height);
            let mut pen_x = line_origin_x;
            let mut pen_y = baseline;

            for glyph in &line.glyphs {
                let origin_x = pen_x + glyph.x_offset;
                let origin_y = pen_y - glyph.y_offset;
                let bounds = rustybuzz_face
                    .glyph_bounding_box(GlyphId(glyph.glyph_id))
                    .map(|bbox| {
                        let min_x = origin_x + (f32::from(bbox.x_min) * scale);
                        let max_x = origin_x + (f32::from(bbox.x_max) * scale);
                        let min_y = origin_y - (f32::from(bbox.y_max) * scale);
                        let max_y = origin_y - (f32::from(bbox.y_min) * scale);
                        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
                    });

                if let Some(bounds) = bounds {
                    measured_bounds = Some(match measured_bounds {
                        Some((min_x, min_y, max_x, max_y)) => (
                            min_x.min(bounds.x()),
                            min_y.min(bounds.y()),
                            max_x.max(bounds.max_x()),
                            max_y.max(bounds.max_y()),
                        ),
                        None => (bounds.x(), bounds.y(), bounds.max_x(), bounds.max_y()),
                    });
                }

                glyphs.push(ShapedGlyph {
                    glyph_id: glyph.glyph_id,
                    cluster: glyph.cluster,
                    line_index,
                    origin_x,
                    origin_y,
                    advance: Vector::new(glyph.x_advance, -glyph.y_advance),
                    scale,
                    bounds,
                });

                pen_x += glyph.x_advance;
                pen_y -= glyph.y_advance;
            }

            lines.push(TextLine {
                byte_range: line.byte_range.clone(),
                rect: Rect::new(line_origin_x, line_top, line.width.max(0.0), line_height),
                baseline,
                ascent,
                descent,
                width: line.width,
                clusters: build_cluster_geometries(line, line_origin_x),
            });
        }

        if lines.is_empty() {
            lines.push(TextLine {
                byte_range: 0..0,
                rect: Rect::new(0.0, block_top, 0.0, line_height),
                baseline: block_top + ascent,
                ascent,
                descent,
                width: 0.0,
                clusters: Vec::new(),
            });
        }

        let bounds = measured_bounds
            .map(|(min_x, min_y, max_x, max_y)| {
                Rect::new(
                    min_x,
                    min_y,
                    (max_x - min_x).max(0.0),
                    (max_y - min_y).max(0.0),
                )
            })
            .unwrap_or_else(|| {
                Rect::new(
                    0.0,
                    block_top,
                    measured_width,
                    block_height.max(ascent + descent),
                )
            });

        Ok(TextLayout {
            data: Arc::new(TextLayoutData {
                text,
                box_size,
                face,
                measurement: TextMeasurement {
                    width: measured_width,
                    height: block_height.max(ascent + descent),
                    bounds,
                },
                glyphs,
                lines,
            }),
            style,
        })
    }

    fn cached_layout(&self, key: &TextLayoutCacheKey) -> Result<Option<TextLayout>> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        let cached = cache.entries.get(key).cloned();
        if let Some(layout) = cached {
            cache.hits += 1;
            return Ok(Some(layout));
        }

        cache.misses += 1;
        Ok(None)
    }

    fn store_layout(&self, key: TextLayoutCacheKey, layout: TextLayout) -> Result<()> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        cache.entries.insert(key, layout);
        Ok(())
    }

    fn resolve_face(
        &self,
        handle: Option<FontHandle>,
        font_registry: &FontRegistry,
    ) -> Result<ResolvedTextFace> {
        if let Some(handle) = handle {
            let font = font_registry.get(handle).ok_or_else(|| {
                Error::new(format!("font handle {} is not registered", handle.get()))
            })?;
            return Ok(ResolvedTextFace::from_bytes(
                font.shared_bytes(),
                font.face_index(),
            ));
        }

        let state = self
            .state
            .get_or_init(|| TextSystemState::new().map_err(|error| error.to_string()));
        match state {
            Ok(state) => Ok(state.default_face.clone()),
            Err(message) => Err(Error::new(message.clone())),
        }
    }
}

#[derive(Debug)]
struct TextSystemState {
    default_face: ResolvedTextFace,
}

impl TextSystemState {
    fn new() -> Result<Self> {
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

        Ok(Self { default_face })
    }
}

#[derive(Debug, Clone)]
struct LineGlyphInput {
    glyph_id: u16,
    cluster: usize,
    x_offset: f32,
    y_offset: f32,
    x_advance: f32,
    y_advance: f32,
}

#[derive(Debug, Clone)]
struct LineSpec {
    byte_range: Range<usize>,
    direction: rustybuzz::Direction,
    width: f32,
    glyphs: Vec<LineGlyphInput>,
}

fn collect_line_specs(text: &str, face: &rustybuzz::Face<'_>, scale: f32) -> Vec<LineSpec> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;

    for segment in text.split('\n') {
        let line_end = line_start + segment.len();
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(segment);
        buffer.guess_segment_properties();
        let direction = buffer.direction();
        let shaped = rustybuzz::shape(face, &[], buffer);
        let glyph_infos = shaped.glyph_infos();
        let glyph_positions = shaped.glyph_positions();
        let width = glyph_positions
            .iter()
            .map(|position| position.x_advance as f32 * scale)
            .sum::<f32>()
            .abs();

        let glyphs = glyph_infos
            .iter()
            .zip(glyph_positions.iter())
            .filter_map(|(info, position)| {
                let glyph_id = u16::try_from(info.glyph_id).ok()?;
                Some(LineGlyphInput {
                    glyph_id,
                    cluster: line_start + info.cluster as usize,
                    x_offset: position.x_offset as f32 * scale,
                    y_offset: position.y_offset as f32 * scale,
                    x_advance: position.x_advance as f32 * scale,
                    y_advance: position.y_advance as f32 * scale,
                })
            })
            .collect();

        lines.push(LineSpec {
            byte_range: line_start..line_end,
            direction,
            width,
            glyphs,
        });

        line_start = line_end.saturating_add(1);
    }

    if lines.is_empty() {
        lines.push(LineSpec {
            byte_range: 0..0,
            direction: rustybuzz::Direction::LeftToRight,
            width: 0.0,
            glyphs: Vec::new(),
        });
    }

    lines
}

fn build_cluster_geometries(line: &LineSpec, line_origin_x: f32) -> Vec<TextClusterGeometry> {
    if line.glyphs.is_empty() {
        return Vec::new();
    }

    let mut clusters = Vec::new();
    let mut pen_x = line_origin_x;
    let mut current_start = line.glyphs[0]
        .cluster
        .clamp(line.byte_range.start, line.byte_range.end);
    let mut current_x_start = pen_x;

    if current_start > line.byte_range.start {
        clusters.push(TextClusterGeometry {
            range: line.byte_range.start..current_start,
            x_start: line_origin_x,
            x_end: line_origin_x,
        });
    }

    for glyph in &line.glyphs {
        let cluster = glyph
            .cluster
            .clamp(line.byte_range.start, line.byte_range.end);
        if cluster != current_start {
            clusters.push(TextClusterGeometry {
                range: current_start..cluster.max(current_start),
                x_start: current_x_start,
                x_end: pen_x,
            });
            current_start = cluster;
            current_x_start = pen_x;
        }
        pen_x += glyph.x_advance;
    }

    clusters.push(TextClusterGeometry {
        range: current_start..line.byte_range.end,
        x_start: current_x_start,
        x_end: pen_x,
    });
    clusters
}

#[cfg(test)]
mod tests {
    use super::{FontRegistry, RegisteredFont, TextLayoutCacheSnapshot, TextStyle, TextSystem};
    use sui_core::{Color, FontHandle, Size};

    fn load_test_font() -> RegisteredFont {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        let families = [fontdb::Family::SansSerif];
        let font_id = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .expect("system font available for text tests");

        font_db
            .with_face_data(font_id, |font_data, face_index| {
                RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
            })
            .expect("font data should be readable from system font database")
    }

    #[test]
    fn text_system_shapes_text_and_reports_geometry() {
        let system = TextSystem::new();
        let layout = system
            .shape_text(
                "hello\nworld",
                Size::new(120.0, 48.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();

        assert_eq!(layout.box_size(), Size::new(120.0, 48.0));
        assert_eq!(layout.lines().len(), 2);
        assert!(!layout.glyphs().is_empty());
        assert!(layout.measurement().width > 0.0);
        assert!(layout.measurement().height >= layout.style().font_size);
        assert_eq!(layout.caret_rect(3).width(), 1.0);
        assert!(!layout.selection_rects(1..8).is_empty());
        assert!(layout.selection_bounds(1..8).is_some());
    }

    #[test]
    fn text_system_uses_registered_font_handles() {
        let system = TextSystem::new();
        let handle = FontHandle::new(19);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let layout = system
            .shape_text(
                "registered",
                Size::new(160.0, 28.0),
                TextStyle {
                    font: Some(handle),
                    ..TextStyle::new(Color::WHITE)
                },
                &fonts,
            )
            .unwrap();

        assert_eq!(
            layout.face().face_index(),
            fonts.get(handle).unwrap().face_index()
        );
        assert_eq!(
            layout.face().shared_bytes(),
            fonts.get(handle).unwrap().shared_bytes()
        );
    }

    #[test]
    fn text_system_reuses_cached_layouts_across_color_changes() {
        let system = TextSystem::new();
        let layout = system
            .shape_text(
                "cached",
                Size::new(120.0, 24.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();

        assert_eq!(system.layout_cache_snapshot(), TextLayoutCacheSnapshot {
            entries: 1,
            hits: 0,
            misses: 1,
        });
        assert_eq!(layout.style().color, Color::WHITE);

        let second = system
            .shape_text(
                "cached",
                Size::new(120.0, 24.0),
                TextStyle::new(Color::rgba(0.2, 0.7, 0.9, 1.0)),
                &FontRegistry::new(),
            )
            .unwrap();

        assert_eq!(system.layout_cache_snapshot(), TextLayoutCacheSnapshot {
            entries: 1,
            hits: 1,
            misses: 1,
        });
        assert_eq!(second.style().color, Color::rgba(0.2, 0.7, 0.9, 1.0));
        assert!(second.shares_storage_with(&layout));
        assert_eq!(second.glyphs(), layout.glyphs());
    }
}
