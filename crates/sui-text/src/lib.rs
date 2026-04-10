#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};

use sui_core::{Color, Error, FontHandle, Point, Rect, Result, Size, Vector};
use ttf_parser::GlyphId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextAlign {
    Start,
    End,
    Left,
    Right,
    Center,
    Justified,
}

impl Default for TextAlign {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextWrap {
    NoWrap,
    Word,
    Character,
}

impl Default for TextWrap {
    fn default() -> Self {
        Self::Word
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextDirection {
    Auto,
    LeftToRight,
    RightToLeft,
}

impl Default for TextDirection {
    fn default() -> Self {
        Self::Auto
    }
}

impl TextDirection {
    fn to_rustybuzz(self) -> Option<rustybuzz::Direction> {
        match self {
            Self::Auto => None,
            Self::LeftToRight => Some(rustybuzz::Direction::LeftToRight),
            Self::RightToLeft => Some(rustybuzz::Direction::RightToLeft),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextWritingMode {
    Horizontal,
    Vertical,
}

impl Default for TextWritingMode {
    fn default() -> Self {
        Self::Horizontal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextFlowDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

impl TextFlowDirection {
    fn from_rustybuzz(direction: rustybuzz::Direction) -> Self {
        match direction {
            rustybuzz::Direction::RightToLeft => Self::RightToLeft,
            rustybuzz::Direction::TopToBottom => Self::TopToBottom,
            rustybuzz::Direction::BottomToTop => Self::BottomToTop,
            _ => Self::LeftToRight,
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TextParagraphStyle {
    pub align: TextAlign,
    pub wrap: TextWrap,
    pub direction: TextDirection,
    pub writing_mode: TextWritingMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    pub text: String,
    pub style: TextStyle,
}

impl TextSpan {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextSpanId {
    pub paragraph_index: usize,
    pub span_index: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextParagraph {
    pub style: TextParagraphStyle,
    pub spans: Vec<TextSpan>,
}

impl TextParagraph {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            style: TextParagraphStyle::default(),
            spans: vec![TextSpan::new(text, style)],
        }
    }

    pub fn from_spans(spans: Vec<TextSpan>) -> Self {
        Self {
            style: TextParagraphStyle::default(),
            spans,
        }
    }

    pub fn text(&self) -> String {
        let mut text = String::new();
        for span in &self.spans {
            text.push_str(&span.text);
        }
        text
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextDocument {
    pub paragraphs: Vec<TextParagraph>,
}

impl TextDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_plain_text(text: impl Into<String>, style: TextStyle) -> Self {
        let text = text.into();
        let paragraphs = text
            .split('\n')
            .map(|segment| TextParagraph::new(segment, style.clone()))
            .collect();
        Self { paragraphs }
    }

    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for (index, paragraph) in self.paragraphs.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            text.push_str(&paragraph.text());
        }
        text
    }

    pub fn primary_style(&self) -> TextStyle {
        self.paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.spans.iter())
            .map(|span| span.style.clone())
            .next()
            .unwrap_or_default()
    }

    fn normalized(&self) -> Self {
        let mut paragraphs = if self.paragraphs.is_empty() {
            vec![TextParagraph::new(String::new(), TextStyle::default())]
        } else {
            self.paragraphs.clone()
        };

        for paragraph in &mut paragraphs {
            if paragraph.spans.is_empty() {
                paragraph
                    .spans
                    .push(TextSpan::new(String::new(), TextStyle::default()));
            }
        }

        Self { paragraphs }
    }

    fn span_style(&self, span_id: TextSpanId) -> &TextStyle {
        &self.paragraphs[span_id.paragraph_index].spans[span_id.span_index].style
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutRequest {
    pub document: TextDocument,
    pub box_size: Option<Size>,
}

impl TextLayoutRequest {
    pub fn new(document: TextDocument) -> Self {
        Self {
            document,
            box_size: None,
        }
    }

    pub fn with_box_size(mut self, box_size: Size) -> Self {
        self.box_size = Some(box_size);
        self
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

    fn glyph_bounds(&self, glyph_id: u16, origin_x: f32, origin_y: f32, scale: f32) -> Option<Rect> {
        let face = rustybuzz::Face::from_slice(self.bytes(), self.face_index())?;
        face.glyph_bounding_box(GlyphId(glyph_id)).map(|bbox| {
            let min_x = origin_x + (f32::from(bbox.x_min) * scale);
            let max_x = origin_x + (f32::from(bbox.x_max) * scale);
            let min_y = origin_y - (f32::from(bbox.y_max) * scale);
            let max_y = origin_y - (f32::from(bbox.y_min) * scale);
            Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMeasurement {
    pub width: f32,
    pub height: f32,
    pub bounds: Rect,
    pub ascent: f32,
    pub descent: f32,
    pub cap_height: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAffinity {
    Upstream,
    Downstream,
}

impl Default for TextAffinity {
    fn default() -> Self {
        Self::Downstream
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextCursor {
    pub utf8_offset: usize,
    pub affinity: TextAffinity,
}

impl TextCursor {
    pub const fn new(utf8_offset: usize) -> Self {
        Self {
            utf8_offset,
            affinity: TextAffinity::Downstream,
        }
    }

    pub const fn with_affinity(mut self, affinity: TextAffinity) -> Self {
        self.affinity = affinity;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: TextCursor,
    pub focus: TextCursor,
}

impl TextSelection {
    pub const fn new(anchor: TextCursor, focus: TextCursor) -> Self {
        Self { anchor, focus }
    }

    fn sorted_range(&self, text_len: usize) -> Range<usize> {
        let start = self.anchor.utf8_offset.min(text_len);
        let end = self.focus.utf8_offset.min(text_len);
        if start <= end {
            start..end
        } else {
            end..start
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextCaret {
    pub cursor: TextCursor,
    pub line_index: usize,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSelectionGeometry {
    pub rects: Vec<Rect>,
    pub bounds: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub cluster: usize,
    pub run_index: usize,
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
pub struct TextParagraphLayout {
    pub paragraph_index: usize,
    pub byte_range: Range<usize>,
    pub line_range: Range<usize>,
    pub rect: Rect,
    pub style: TextParagraphStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    pub paragraph_index: usize,
    pub line_index: usize,
    pub run_index: usize,
    pub byte_range: Range<usize>,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutRun {
    pub paragraph_index: usize,
    pub line_index: usize,
    pub span_id: TextSpanId,
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub cluster_range: Range<usize>,
    pub rect: Rect,
    pub baseline: f32,
    pub face_index: usize,
    pub direction: TextFlowDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub paragraph_index: usize,
    pub byte_range: Range<usize>,
    pub run_range: Range<usize>,
    pub cluster_range: Range<usize>,
    pub rect: Rect,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
    pub direction: TextFlowDirection,
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
    faces: Vec<ResolvedTextFace>,
    measurement: TextMeasurement,
    paragraphs: Vec<TextParagraphLayout>,
    lines: Vec<TextLine>,
    runs: Vec<TextLayoutRun>,
    clusters: Vec<TextCluster>,
    glyphs: Vec<ShapedGlyph>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    data: Arc<TextLayoutData>,
    document: Arc<TextDocument>,
    primary_style: TextStyle,
}

impl TextLayout {
    pub fn text(&self) -> &str {
        &self.data.text
    }

    pub fn document(&self) -> &TextDocument {
        self.document.as_ref()
    }

    pub fn style(&self) -> &TextStyle {
        &self.primary_style
    }

    pub fn box_size(&self) -> Size {
        self.data.box_size
    }

    pub fn measurement(&self) -> TextMeasurement {
        self.data.measurement
    }

    pub fn paragraphs(&self) -> &[TextParagraphLayout] {
        &self.data.paragraphs
    }

    pub fn lines(&self) -> &[TextLine] {
        &self.data.lines
    }

    pub fn runs(&self) -> &[TextLayoutRun] {
        &self.data.runs
    }

    pub fn clusters(&self) -> &[TextCluster] {
        &self.data.clusters
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.data.glyphs
    }

    pub fn faces(&self) -> &[ResolvedTextFace] {
        &self.data.faces
    }

    pub fn face(&self) -> &ResolvedTextFace {
        &self.data.faces[0]
    }

    pub fn run_style(&self, run_index: usize) -> &TextStyle {
        self.document
            .span_style(self.data.runs[run_index].span_id.clone())
    }

    pub fn run_face(&self, run_index: usize) -> &ResolvedTextFace {
        &self.data.faces[self.data.runs[run_index].face_index]
    }

    pub fn glyph_style(&self, glyph: &ShapedGlyph) -> &TextStyle {
        self.run_style(glyph.run_index)
    }

    pub fn glyph_face(&self, glyph: &ShapedGlyph) -> &ResolvedTextFace {
        self.run_face(glyph.run_index)
    }

    pub fn caret(&self, cursor: TextCursor) -> TextCaret {
        let line_index = self.line_index_for_offset(cursor.utf8_offset);
        let line = &self.data.lines[line_index];
        TextCaret {
            cursor,
            line_index,
            rect: Rect::new(
                line.x_for_offset(cursor.utf8_offset),
                line.rect.y(),
                1.0,
                line.rect.height(),
            ),
        }
    }

    pub fn caret_rect(&self, utf8_offset: usize) -> Rect {
        self.caret(TextCursor::new(utf8_offset)).rect
    }

    pub fn selection_geometry(&self, selection: &TextSelection) -> TextSelectionGeometry {
        let range = selection.sorted_range(self.data.text.len());
        if range.start == range.end {
            return TextSelectionGeometry {
                rects: Vec::new(),
                bounds: None,
            };
        }

        let mut rects = Vec::new();

        for line in &self.data.lines {
            let line_start = range.start.max(line.byte_range.start);
            let line_end = range.end.min(line.byte_range.end);
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

        let bounds = rects.iter().copied().reduce(|bounds, rect| bounds.union(rect));
        TextSelectionGeometry { rects, bounds }
    }

    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        self.selection_geometry(&TextSelection::new(
            TextCursor::new(range.start),
            TextCursor::new(range.end),
        ))
        .rects
    }

    pub fn selection_bounds(&self, range: Range<usize>) -> Option<Rect> {
        self.selection_geometry(&TextSelection::new(
            TextCursor::new(range.start),
            TextCursor::new(range.end),
        ))
        .bounds
    }

    fn line_index_for_offset(&self, utf8_offset: usize) -> usize {
        let offset = utf8_offset.min(self.data.text.len());
        self.data
            .lines
            .iter()
            .position(|line| offset <= line.byte_range.end)
            .unwrap_or_else(|| self.data.lines.len().saturating_sub(1))
    }

    fn with_document(mut self, document: TextDocument) -> Self {
        self.primary_style = document.primary_style();
        self.document = Arc::new(document);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextStyleCacheKey {
    font_handle: Option<u64>,
    font_size_bits: u32,
    line_height_bits: u32,
}

impl TextStyleCacheKey {
    fn new(style: &TextStyle) -> Self {
        Self {
            font_handle: style.font.map(FontHandle::get),
            font_size_bits: style.font_size.to_bits(),
            line_height_bits: style.line_height.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextSpanCacheKey {
    text: String,
    style: TextStyleCacheKey,
    face: FaceCacheKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextParagraphCacheKey {
    style: TextParagraphStyle,
    spans: Vec<TextSpanCacheKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextLayoutCacheKey {
    paragraphs: Vec<TextParagraphCacheKey>,
    box_size: Option<SizeCacheKey>,
}

impl TextLayoutCacheKey {
    fn new(
        flattened: &FlattenedTextDocument,
        resolved_spans: &[ResolvedSpanInput],
        box_size: Option<Size>,
    ) -> Self {
        let paragraphs = flattened
            .paragraphs
            .iter()
            .map(|paragraph| TextParagraphCacheKey {
                style: paragraph.style.clone(),
                spans: paragraph
                    .span_range
                    .clone()
                    .map(|index| {
                        let span = &resolved_spans[index];
                        TextSpanCacheKey {
                            text: span.text.clone(),
                            style: TextStyleCacheKey::new(&span.style),
                            face: span.face_key,
                        }
                    })
                    .collect(),
            })
            .collect();

        Self {
            paragraphs,
            box_size: box_size.map(SizeCacheKey::from),
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
        self.measure_document(TextDocument::from_plain_text(text.into(), style), font_registry)
    }

    pub fn measure_document(
        &self,
        document: TextDocument,
        font_registry: &FontRegistry,
    ) -> Result<TextMeasurement> {
        Ok(self
            .layout_document(TextLayoutRequest::new(document), font_registry)?
            .measurement())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.layout_document(
            TextLayoutRequest::new(TextDocument::from_plain_text(text.into(), style))
                .with_box_size(box_size),
            font_registry,
        )
    }

    pub fn layout_document(
        &self,
        request: TextLayoutRequest,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.shape_text_internal(request, font_registry)
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
        request: TextLayoutRequest,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        let normalized_document = request.document.normalized();
        let flattened = FlattenedTextDocument::new(normalized_document.clone());
        let resolved_spans = self.resolve_span_inputs(&flattened, font_registry)?;
        let cache_key = TextLayoutCacheKey::new(&flattened, &resolved_spans, request.box_size);

        if let Some(cached) = self.cached_layout(&cache_key)? {
            return Ok(cached.with_document(normalized_document));
        }

        let layout = self.shape_text_uncached(flattened, resolved_spans, request.box_size)?;
        self.store_layout(cache_key, layout.clone())?;
        Ok(layout.with_document(normalized_document))
    }

    fn shape_text_uncached(
        &self,
        flattened: FlattenedTextDocument,
        resolved_spans: Vec<ResolvedSpanInput>,
        box_size: Option<Size>,
    ) -> Result<TextLayout> {
        let mut faces = Vec::new();
        let mut face_slots = HashMap::new();
        let mut prepared_lines = Vec::with_capacity(flattened.paragraphs.len());
        let mut measured_width = 0.0_f32;
        let mut block_height = 0.0_f32;
        let mut max_ascent = 0.0_f32;
        let mut max_descent = 0.0_f32;
        let mut max_cap_height: Option<f32> = None;

        for paragraph in &flattened.paragraphs {
            let mut runs = Vec::new();
            let mut line_width = 0.0_f32;
            let mut line_ascent = 0.0_f32;
            let mut line_descent = 0.0_f32;
            let mut line_height = 0.0_f32;
            let mut line_direction = match paragraph.style.direction {
                TextDirection::RightToLeft => TextFlowDirection::RightToLeft,
                _ => TextFlowDirection::LeftToRight,
            };

            for span_index in paragraph.span_range.clone() {
                let prepared = shape_span(&resolved_spans[span_index], paragraph.style.direction)?;
                let face_index = register_face(
                    &mut faces,
                    &mut face_slots,
                    prepared.face_key,
                    prepared.face.clone(),
                );

                if matches!(line_direction, TextFlowDirection::LeftToRight)
                    && matches!(prepared.direction, TextFlowDirection::RightToLeft)
                {
                    line_direction = prepared.direction;
                }

                line_width += prepared.width;
                line_ascent = line_ascent.max(prepared.ascent);
                line_descent = line_descent.max(prepared.descent);
                line_height = line_height.max(prepared.line_height);
                if let Some(cap_height) = prepared.cap_height {
                    max_cap_height = Some(
                        max_cap_height
                            .map_or(cap_height, |current: f32| current.max(cap_height)),
                    );
                }

                runs.push(PreparedRun {
                    paragraph_index: paragraph.index,
                    span_id: prepared.id,
                    byte_range: prepared.byte_range,
                    face_index,
                    face: prepared.face,
                    direction: prepared.direction,
                    width: prepared.width,
                    glyphs: prepared.glyphs,
                });
            }

            measured_width = measured_width.max(line_width);
            block_height += line_height;
            max_ascent = max_ascent.max(line_ascent);
            max_descent = max_descent.max(line_descent);
            prepared_lines.push(PreparedLine {
                paragraph_index: paragraph.index,
                byte_range: paragraph.byte_range.clone(),
                style: paragraph.style.clone(),
                width: line_width,
                ascent: line_ascent,
                descent: line_descent,
                line_height,
                direction: line_direction,
                runs,
            });
        }

        let box_size = box_size.unwrap_or(Size::new(
            measured_width,
            block_height.max(max_ascent + max_descent),
        ));
        let block_top = ((box_size.height - block_height).max(0.0)) * 0.5;

        let mut glyphs = Vec::new();
        let mut lines = Vec::with_capacity(prepared_lines.len());
        let mut runs = Vec::new();
        let mut clusters = Vec::new();
        let mut paragraphs = Vec::with_capacity(prepared_lines.len());
        let mut measured_bounds: Option<(f32, f32, f32, f32)> = None;
        let mut line_top = block_top;

        for prepared_line in prepared_lines {
            let line_origin_x = line_origin_x(
                &prepared_line.style,
                prepared_line.direction,
                box_size.width,
                prepared_line.width,
            );
            let baseline = line_top + prepared_line.ascent;
            let line_index = lines.len();
            let line_run_start = runs.len();
            let line_cluster_start = clusters.len();
            let mut line_clusters = Vec::new();
            let mut run_cursor_x = line_origin_x;

            for prepared_run in prepared_line.runs {
                let run_index = runs.len();
                let glyph_start = glyphs.len();
                let cluster_start = clusters.len();
                let run_pen_start = match prepared_run.direction {
                    TextFlowDirection::RightToLeft => run_cursor_x + prepared_run.width,
                    _ => run_cursor_x,
                };
                let mut pen_x = run_pen_start;
                let mut pen_y = baseline;

                for glyph in &prepared_run.glyphs {
                    let origin_x = pen_x + glyph.x_offset;
                    let origin_y = pen_y - glyph.y_offset;
                    let bounds = prepared_run
                        .face
                        .glyph_bounds(glyph.glyph_id, origin_x, origin_y, glyph.scale);

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
                        run_index,
                        line_index,
                        origin_x,
                        origin_y,
                        advance: Vector::new(glyph.x_advance, -glyph.y_advance),
                        scale: glyph.scale,
                        bounds,
                    });

                    pen_x += glyph.x_advance;
                    pen_y -= glyph.y_advance;
                }

                let run_cluster_geometries = build_cluster_geometries(
                    &prepared_run.byte_range,
                    &prepared_run.glyphs,
                    run_pen_start,
                );
                for geometry in &run_cluster_geometries {
                    line_clusters.push(geometry.clone());
                    clusters.push(TextCluster {
                        paragraph_index: prepared_run.paragraph_index,
                        line_index,
                        run_index,
                        byte_range: geometry.range.clone(),
                        rect: Rect::new(
                            geometry.x_start.min(geometry.x_end),
                            line_top,
                            (geometry.x_end - geometry.x_start).abs(),
                            prepared_line.line_height,
                        ),
                    });
                }

                runs.push(TextLayoutRun {
                    paragraph_index: prepared_run.paragraph_index,
                    line_index,
                    span_id: prepared_run.span_id,
                    byte_range: prepared_run.byte_range.clone(),
                    glyph_range: glyph_start..glyphs.len(),
                    cluster_range: cluster_start..clusters.len(),
                    rect: Rect::new(
                        run_cursor_x,
                        line_top,
                        prepared_run.width.max(0.0),
                        prepared_line.line_height,
                    ),
                    baseline,
                    face_index: prepared_run.face_index,
                    direction: prepared_run.direction,
                });

                run_cursor_x += prepared_run.width;
            }

            let line = TextLine {
                paragraph_index: prepared_line.paragraph_index,
                byte_range: prepared_line.byte_range.clone(),
                run_range: line_run_start..runs.len(),
                cluster_range: line_cluster_start..clusters.len(),
                rect: Rect::new(
                    line_origin_x,
                    line_top,
                    prepared_line.width.max(0.0),
                    prepared_line.line_height,
                ),
                baseline,
                ascent: prepared_line.ascent,
                descent: prepared_line.descent,
                width: prepared_line.width,
                direction: prepared_line.direction,
                clusters: line_clusters,
            };

            paragraphs.push(TextParagraphLayout {
                paragraph_index: prepared_line.paragraph_index,
                byte_range: prepared_line.byte_range,
                line_range: line_index..(line_index + 1),
                rect: line.rect,
                style: prepared_line.style,
            });
            lines.push(line);
            line_top += prepared_line.line_height;
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
                    block_height.max(max_ascent + max_descent),
                )
            });

        Ok(TextLayout {
            primary_style: flattened.document.primary_style(),
            document: Arc::new(flattened.document),
            data: Arc::new(TextLayoutData {
                text: flattened.text,
                box_size,
                faces,
                measurement: TextMeasurement {
                    width: measured_width,
                    height: block_height.max(max_ascent + max_descent),
                    bounds,
                    ascent: max_ascent,
                    descent: max_descent,
                    cap_height: max_cap_height,
                },
                paragraphs,
                lines,
                runs,
                clusters,
                glyphs,
            }),
        })
    }

    fn resolve_span_inputs(
        &self,
        flattened: &FlattenedTextDocument,
        font_registry: &FontRegistry,
    ) -> Result<Vec<ResolvedSpanInput>> {
        let mut resolved = Vec::with_capacity(flattened.spans.len());
        for span in &flattened.spans {
            let face = self.resolve_face(span.style.font, font_registry)?;
            resolved.push(ResolvedSpanInput {
                id: span.id.clone(),
                byte_range: span.byte_range.clone(),
                text: span.text.clone(),
                style: span.style.clone(),
                face_key: FaceCacheKey::new(&face),
                face,
            });
        }
        Ok(resolved)
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
struct FlattenedTextDocument {
    document: TextDocument,
    text: String,
    paragraphs: Vec<FlattenedParagraph>,
    spans: Vec<FlattenedSpan>,
}

impl FlattenedTextDocument {
    fn new(document: TextDocument) -> Self {
        let mut text = String::new();
        let mut paragraphs = Vec::with_capacity(document.paragraphs.len());
        let mut spans = Vec::new();

        for (paragraph_index, paragraph) in document.paragraphs.iter().enumerate() {
            let paragraph_start = text.len();
            let span_start = spans.len();
            for (span_index, span) in paragraph.spans.iter().enumerate() {
                let span_start_offset = text.len();
                text.push_str(&span.text);
                spans.push(FlattenedSpan {
                    id: TextSpanId {
                        paragraph_index,
                        span_index,
                    },
                    text: span.text.clone(),
                    byte_range: span_start_offset..text.len(),
                    style: span.style.clone(),
                });
            }

            paragraphs.push(FlattenedParagraph {
                index: paragraph_index,
                byte_range: paragraph_start..text.len(),
                style: paragraph.style.clone(),
                span_range: span_start..spans.len(),
            });

            if paragraph_index + 1 < document.paragraphs.len() {
                text.push('\n');
            }
        }

        Self {
            document,
            text,
            paragraphs,
            spans,
        }
    }
}

#[derive(Debug, Clone)]
struct FlattenedParagraph {
    index: usize,
    byte_range: Range<usize>,
    style: TextParagraphStyle,
    span_range: Range<usize>,
}

#[derive(Debug, Clone)]
struct FlattenedSpan {
    id: TextSpanId,
    text: String,
    byte_range: Range<usize>,
    style: TextStyle,
}

#[derive(Debug, Clone)]
struct ResolvedSpanInput {
    id: TextSpanId,
    byte_range: Range<usize>,
    text: String,
    style: TextStyle,
    face_key: FaceCacheKey,
    face: ResolvedTextFace,
}

#[derive(Debug, Clone)]
struct LineGlyphInput {
    glyph_id: u16,
    cluster: usize,
    x_offset: f32,
    y_offset: f32,
    x_advance: f32,
    y_advance: f32,
    scale: f32,
}

#[derive(Debug, Clone)]
struct ShapedSpan {
    id: TextSpanId,
    byte_range: Range<usize>,
    face_key: FaceCacheKey,
    face: ResolvedTextFace,
    direction: TextFlowDirection,
    width: f32,
    ascent: f32,
    descent: f32,
    line_height: f32,
    cap_height: Option<f32>,
    glyphs: Vec<LineGlyphInput>,
}

#[derive(Debug, Clone)]
struct PreparedRun {
    paragraph_index: usize,
    span_id: TextSpanId,
    byte_range: Range<usize>,
    face_index: usize,
    face: ResolvedTextFace,
    direction: TextFlowDirection,
    width: f32,
    glyphs: Vec<LineGlyphInput>,
}

#[derive(Debug, Clone)]
struct PreparedLine {
    paragraph_index: usize,
    byte_range: Range<usize>,
    style: TextParagraphStyle,
    width: f32,
    ascent: f32,
    descent: f32,
    line_height: f32,
    direction: TextFlowDirection,
    runs: Vec<PreparedRun>,
}

fn shape_span(span: &ResolvedSpanInput, paragraph_direction: TextDirection) -> Result<ShapedSpan> {
    let rustybuzz_face = rustybuzz::Face::from_slice(span.face.bytes(), span.face.face_index())
        .ok_or_else(|| Error::new("failed to parse text face data"))?;

    let units_per_em = rustybuzz_face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return Err(Error::new(
            "text face reported an invalid units-per-em value",
        ));
    }

    let scale = span.style.font_size / units_per_em;
    let ascent = f32::from(rustybuzz_face.ascender()) * scale;
    let descent = f32::from(rustybuzz_face.descender().abs()) * scale;
    let cap_height = ttf_parser::Face::parse(span.face.bytes(), span.face.face_index())
        .ok()
        .and_then(|face| face.capital_height())
        .map(|height| f32::from(height) * scale);
    let natural_line_height = f32::from(rustybuzz_face.height().abs()) * scale;
    let line_height = span
        .style
        .line_height
        .max(natural_line_height)
        .max(span.style.font_size);

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(&span.text);
    buffer.guess_segment_properties();
    if let Some(direction) = paragraph_direction.to_rustybuzz() {
        buffer.set_direction(direction);
    }

    let direction = TextFlowDirection::from_rustybuzz(buffer.direction());
    let shaped = rustybuzz::shape(&rustybuzz_face, &[], buffer);
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
                cluster: span.byte_range.start + info.cluster as usize,
                x_offset: position.x_offset as f32 * scale,
                y_offset: position.y_offset as f32 * scale,
                x_advance: position.x_advance as f32 * scale,
                y_advance: position.y_advance as f32 * scale,
                scale,
            })
        })
        .collect();

    Ok(ShapedSpan {
        id: span.id.clone(),
        byte_range: span.byte_range.clone(),
        face_key: span.face_key,
        face: span.face.clone(),
        direction,
        width,
        ascent,
        descent,
        line_height,
        cap_height,
        glyphs,
    })
}

fn register_face(
    faces: &mut Vec<ResolvedTextFace>,
    slots: &mut HashMap<FaceCacheKey, usize>,
    key: FaceCacheKey,
    face: ResolvedTextFace,
) -> usize {
    if let Some(index) = slots.get(&key) {
        return *index;
    }

    let index = faces.len();
    faces.push(face);
    slots.insert(key, index);
    index
}

fn line_origin_x(
    style: &TextParagraphStyle,
    direction: TextFlowDirection,
    box_width: f32,
    line_width: f32,
) -> f32 {
    match style.align {
        TextAlign::Center => (box_width - line_width) * 0.5,
        TextAlign::End => match direction {
            TextFlowDirection::RightToLeft => 0.0,
            _ => box_width - line_width,
        },
        TextAlign::Left => 0.0,
        TextAlign::Right => box_width - line_width,
        TextAlign::Start | TextAlign::Justified => match direction {
            TextFlowDirection::RightToLeft => box_width - line_width,
            _ => 0.0,
        },
    }
}

fn build_cluster_geometries(
    byte_range: &Range<usize>,
    glyphs: &[LineGlyphInput],
    pen_start_x: f32,
) -> Vec<TextClusterGeometry> {
    if glyphs.is_empty() {
        if byte_range.is_empty() {
            return Vec::new();
        }
        return vec![TextClusterGeometry {
            range: byte_range.clone(),
            x_start: pen_start_x,
            x_end: pen_start_x,
        }];
    }

    let mut clusters = Vec::new();
    let mut pen_x = pen_start_x;
    let mut current_start = glyphs[0].cluster.clamp(byte_range.start, byte_range.end);
    let mut current_x_start = pen_x;

    if current_start > byte_range.start {
        clusters.push(TextClusterGeometry {
            range: byte_range.start..current_start,
            x_start: pen_start_x,
            x_end: pen_start_x,
        });
    }

    for glyph in glyphs {
        let cluster = glyph.cluster.clamp(byte_range.start, byte_range.end);
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
        range: current_start..byte_range.end,
        x_start: current_x_start,
        x_end: pen_x,
    });
    clusters
}

#[cfg(test)]
mod tests {
    use super::{
        FontRegistry, RegisteredFont, TextDocument, TextLayoutCacheSnapshot, TextLayoutRequest,
        TextParagraph, TextSelection, TextSpan, TextStyle, TextSystem,
    };
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
        assert_eq!(layout.paragraphs().len(), 2);
        assert_eq!(layout.lines().len(), 2);
        assert_eq!(layout.runs().len(), 2);
        assert!(!layout.glyphs().is_empty());
        assert!(layout.measurement().width > 0.0);
        assert!(layout.measurement().height >= layout.style().font_size);
        assert_eq!(layout.caret_rect(3).width(), 1.0);
        assert!(!layout.selection_rects(1..8).is_empty());
        assert!(layout.selection_bounds(1..8).is_some());
        assert!(layout
            .selection_geometry(&TextSelection::new(Default::default(), Default::default()))
            .rects
            .is_empty());
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

        assert_eq!(
            system.layout_cache_snapshot(),
            TextLayoutCacheSnapshot {
                entries: 1,
                hits: 0,
                misses: 1,
            }
        );
        assert_eq!(layout.style().color, Color::WHITE);

        let second = system
            .shape_text(
                "cached",
                Size::new(120.0, 24.0),
                TextStyle::new(Color::rgba(0.2, 0.7, 0.9, 1.0)),
                &FontRegistry::new(),
            )
            .unwrap();

        assert_eq!(
            system.layout_cache_snapshot(),
            TextLayoutCacheSnapshot {
                entries: 1,
                hits: 1,
                misses: 1,
            }
        );
        assert_eq!(second.style().color, Color::rgba(0.2, 0.7, 0.9, 1.0));
        assert!(second.shares_storage_with(&layout));
        assert_eq!(second.glyphs(), layout.glyphs());
    }

    #[test]
    fn layout_document_keeps_paragraph_and_span_structure() {
        let system = TextSystem::new();
        let document = TextDocument {
            paragraphs: vec![
                TextParagraph {
                    style: Default::default(),
                    spans: vec![
                        TextSpan::new("hel", TextStyle::new(Color::WHITE)),
                        TextSpan::new("lo", TextStyle::new(Color::BLACK)),
                    ],
                },
                TextParagraph::new("world", TextStyle::new(Color::WHITE)),
            ],
        };

        let layout = system
            .layout_document(
                TextLayoutRequest::new(document).with_box_size(Size::new(200.0, 64.0)),
                &FontRegistry::new(),
            )
            .unwrap();

        assert_eq!(layout.document().paragraphs.len(), 2);
        assert_eq!(layout.paragraphs().len(), 2);
        assert_eq!(layout.lines().len(), 2);
        assert_eq!(layout.runs().len(), 3);
        assert_eq!(layout.run_style(0).color, Color::WHITE);
        assert_eq!(layout.run_style(1).color, Color::BLACK);
        assert_eq!(layout.text(), "hello\nworld");
        assert_eq!(layout.runs()[0].byte_range, 0..3);
        assert_eq!(layout.runs()[1].byte_range, 3..5);
    }
}
