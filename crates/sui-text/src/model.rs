use std::{collections::HashMap, ops::Deref, ops::Range, sync::Arc};

use sui_core::{Color, FontHandle, Point, Rect, Size, Vector};

use crate::font::ResolvedTextFace;

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

    pub(crate) fn into_normalized(mut self) -> Self {
        if self.paragraphs.is_empty() {
            self.paragraphs
                .push(TextParagraph::new(String::new(), TextStyle::default()));
        }

        for paragraph in &mut self.paragraphs {
            if paragraph.spans.is_empty() {
                paragraph
                    .spans
                    .push(TextSpan::new(String::new(), TextStyle::default()));
            }
        }

        self
    }

    pub(crate) fn span_style(&self, span_id: TextSpanId) -> &TextStyle {
        &self.paragraphs[span_id.paragraph_index].spans[span_id.span_index].style
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutId(u64);

impl TextLayoutId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutVersion(u64);

impl TextLayoutVersion {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutHandle(u64);

impl TextLayoutHandle {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn from_layout_id(id: TextLayoutId) -> Self {
        Self(id.get())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutMetadata {
    pub id: TextLayoutId,
    pub version: TextLayoutVersion,
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

    pub(crate) fn sorted_range(&self, text_len: usize) -> Range<usize> {
        let start = self.anchor.utf8_offset.min(text_len);
        let end = self.focus.utf8_offset.min(text_len);
        if start <= end { start..end } else { end..start }
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
    pub span_id: TextSpanId,
    pub run_index: usize,
    pub line_index: usize,
    pub face_index: usize,
    pub origin_x: f32,
    pub origin_y: f32,
    pub advance: Vector,
    pub scale: f32,
    pub bounds: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextClusterGeometry {
    pub range: Range<usize>,
    pub x_start: f32,
    pub x_end: f32,
    pub glyph_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextParagraphLayout {
    pub paragraph_index: usize,
    pub byte_range: Range<usize>,
    pub line_range: Range<usize>,
    pub run_range: Range<usize>,
    pub cluster_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub rect: Rect,
    pub style: TextParagraphStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    pub paragraph_index: usize,
    pub line_index: usize,
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    pub run_range: Range<usize>,
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
    pub glyph_range: Range<usize>,
    pub rect: Rect,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
    pub direction: TextFlowDirection,
    pub(crate) clusters: Vec<TextClusterGeometry>,
}

impl TextLine {
    pub(crate) fn x_for_offset(&self, offset: usize) -> f32 {
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

    pub(crate) fn offset_for_x(&self, x: f32) -> usize {
        if self.clusters.is_empty() {
            return self.byte_range.start;
        }

        let first = &self.clusters[0];
        let first_min = first.x_start.min(first.x_end);
        if x <= first_min {
            return if first.x_start <= first.x_end {
                first.range.start
            } else {
                first.range.end
            };
        }

        for cluster in &self.clusters {
            let cluster_min = cluster.x_start.min(cluster.x_end);
            let cluster_max = cluster.x_start.max(cluster.x_end);
            let midpoint = (cluster_min + cluster_max) * 0.5;
            if x <= cluster_max {
                let ascending = cluster.x_start <= cluster.x_end;
                return if x <= midpoint {
                    if ascending {
                        cluster.range.start
                    } else {
                        cluster.range.end
                    }
                } else if ascending {
                    cluster.range.end
                } else {
                    cluster.range.start
                };
            }
        }

        let last = self
            .clusters
            .last()
            .expect("text lines with clusters should have a last cluster");
        if last.x_start <= last.x_end {
            last.range.end
        } else {
            last.range.start
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextLayoutData {
    pub metadata: TextLayoutMetadata,
    pub text: String,
    pub box_size: Size,
    pub faces: Vec<ResolvedTextFace>,
    pub measurement: TextMeasurement,
    pub paragraphs: Vec<TextParagraphLayout>,
    pub lines: Vec<TextLine>,
    pub runs: Vec<TextLayoutRun>,
    pub clusters: Vec<TextCluster>,
    pub glyphs: Vec<ShapedGlyph>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub(crate) data: Arc<TextLayoutData>,
    pub(crate) document: Arc<TextDocument>,
    pub(crate) primary_style: TextStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistentTextLayout {
    handle: TextLayoutHandle,
    layout: TextLayout,
}

impl PersistentTextLayout {
    pub fn new(handle: TextLayoutHandle, layout: TextLayout) -> Self {
        Self { handle, layout }
    }

    pub fn handle(&self) -> TextLayoutHandle {
        self.handle
    }

    pub fn version(&self) -> TextLayoutVersion {
        self.layout.version()
    }

    pub fn metadata(&self) -> TextLayoutMetadata {
        self.layout.metadata()
    }

    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }

    pub fn into_parts(self) -> (TextLayoutHandle, TextLayout) {
        (self.handle, self.layout)
    }
}

impl Deref for PersistentTextLayout {
    type Target = TextLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl AsRef<TextLayout> for PersistentTextLayout {
    fn as_ref(&self) -> &TextLayout {
        &self.layout
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLayoutRegistry {
    layouts: HashMap<TextLayoutHandle, TextLayout>,
}

impl TextLayoutRegistry {
    pub fn get(&self, handle: TextLayoutHandle) -> Option<&TextLayout> {
        self.layouts.get(&handle)
    }

    pub fn contains(&self, handle: TextLayoutHandle) -> bool {
        self.layouts.contains_key(&handle)
    }

    pub fn len(&self) -> usize {
        self.layouts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (TextLayoutHandle, &TextLayout)> {
        self.layouts
            .iter()
            .map(|(handle, layout)| (*handle, layout))
    }

    pub(crate) fn insert(&mut self, handle: TextLayoutHandle, layout: TextLayout) {
        self.layouts.insert(handle, layout);
    }
}

#[derive(Debug, Clone)]
pub struct TextLayoutView<'a> {
    pub metadata: TextLayoutMetadata,
    pub text: &'a str,
    pub document: &'a TextDocument,
    pub box_size: Size,
    pub measurement: TextMeasurement,
    pub faces: &'a [ResolvedTextFace],
    pub paragraphs: &'a [TextParagraphLayout],
    pub lines: &'a [TextLine],
    pub runs: &'a [TextLayoutRun],
    pub clusters: &'a [TextCluster],
    pub glyphs: &'a [ShapedGlyph],
}

#[derive(Debug, Clone)]
pub struct TextLineWindow<'a> {
    layout: &'a TextLayout,
    pub metadata: TextLayoutMetadata,
    pub line_range: Range<usize>,
    pub run_range: Range<usize>,
    pub cluster_range: Range<usize>,
    pub glyph_range: Range<usize>,
}

impl<'a> TextLineWindow<'a> {
    pub fn lines(&self) -> &'a [TextLine] {
        &self.layout.data.lines[self.line_range.clone()]
    }

    pub fn bounds(&self) -> Rect {
        self.lines()
            .iter()
            .map(|line| line.rect)
            .reduce(|bounds, rect| bounds.union(rect))
            .unwrap_or(Rect::ZERO)
    }

    pub fn runs(&self) -> &'a [TextLayoutRun] {
        &self.layout.data.runs[self.run_range.clone()]
    }

    pub fn clusters(&self) -> &'a [TextCluster] {
        &self.layout.data.clusters[self.cluster_range.clone()]
    }

    pub fn glyphs(&self) -> &'a [ShapedGlyph] {
        &self.layout.data.glyphs[self.glyph_range.clone()]
    }

    pub fn glyph_instances(&self) -> impl ExactSizeIterator<Item = TextGlyphInstance<'a>> + 'a {
        (self.glyph_range.start..self.glyph_range.end)
            .map(|index| self.layout.glyph_instance(index))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextRunView<'a> {
    pub index: usize,
    pub run: &'a TextLayoutRun,
    pub style: &'a TextStyle,
    pub face: &'a ResolvedTextFace,
    pub line: &'a TextLine,
    pub clusters: &'a [TextCluster],
    pub glyphs: &'a [ShapedGlyph],
}

#[derive(Debug, Clone, Copy)]
pub struct TextGlyphInstance<'a> {
    pub index: usize,
    pub glyph: &'a ShapedGlyph,
    pub style: &'a TextStyle,
    pub face: &'a ResolvedTextFace,
    pub run: &'a TextLayoutRun,
    pub line: &'a TextLine,
}

impl TextLayout {
    pub fn id(&self) -> TextLayoutId {
        self.data.metadata.id
    }

    pub fn version(&self) -> TextLayoutVersion {
        self.data.metadata.version
    }

    pub fn metadata(&self) -> TextLayoutMetadata {
        self.data.metadata
    }

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

    pub fn view(&self) -> TextLayoutView<'_> {
        TextLayoutView {
            metadata: self.metadata(),
            text: self.text(),
            document: self.document(),
            box_size: self.box_size(),
            measurement: self.measurement(),
            faces: self.faces(),
            paragraphs: self.paragraphs(),
            lines: self.lines(),
            runs: self.runs(),
            clusters: self.clusters(),
            glyphs: self.glyphs(),
        }
    }

    pub fn run_style(&self, run_index: usize) -> &TextStyle {
        self.document
            .span_style(self.data.runs[run_index].span_id.clone())
    }

    pub fn run_face(&self, run_index: usize) -> &ResolvedTextFace {
        &self.data.faces[self.data.runs[run_index].face_index]
    }

    pub fn glyph_style(&self, glyph: &ShapedGlyph) -> &TextStyle {
        self.document.span_style(glyph.span_id.clone())
    }

    pub fn glyph_face(&self, glyph: &ShapedGlyph) -> &ResolvedTextFace {
        &self.data.faces[glyph.face_index]
    }

    pub fn run_view(&self, run_index: usize) -> TextRunView<'_> {
        let run = &self.data.runs[run_index];
        TextRunView {
            index: run_index,
            run,
            style: self.run_style(run_index),
            face: self.run_face(run_index),
            line: &self.data.lines[run.line_index],
            clusters: &self.data.clusters[run.cluster_range.clone()],
            glyphs: &self.data.glyphs[run.glyph_range.clone()],
        }
    }

    pub fn run_views(&self) -> impl ExactSizeIterator<Item = TextRunView<'_>> + '_ {
        (0..self.data.runs.len()).map(|index| self.run_view(index))
    }

    pub fn glyph_instance(&self, glyph_index: usize) -> TextGlyphInstance<'_> {
        let glyph = &self.data.glyphs[glyph_index];
        TextGlyphInstance {
            index: glyph_index,
            glyph,
            style: self.glyph_style(glyph),
            face: self.glyph_face(glyph),
            run: &self.data.runs[glyph.run_index],
            line: &self.data.lines[glyph.line_index],
        }
    }

    pub fn glyph_instances(&self) -> impl ExactSizeIterator<Item = TextGlyphInstance<'_>> + '_ {
        (0..self.data.glyphs.len()).map(|index| self.glyph_instance(index))
    }

    pub fn line_window(&self, line_range: Range<usize>) -> TextLineWindow<'_> {
        let line_start = line_range.start.min(self.data.lines.len());
        let line_end = line_range.end.min(self.data.lines.len()).max(line_start);
        let clamped_line_range = line_start..line_end;
        let (run_range, cluster_range, glyph_range) = if clamped_line_range.is_empty() {
            (0..0, 0..0, 0..0)
        } else {
            collapse_range(
                self.data.lines[clamped_line_range.clone()]
                    .iter()
                    .map(|line| line.run_range.clone()),
            )
            .zip(collapse_range(
                self.data.lines[clamped_line_range.clone()]
                    .iter()
                    .map(|line| line.cluster_range.clone()),
            ))
            .zip(collapse_range(
                self.data.lines[clamped_line_range.clone()]
                    .iter()
                    .map(|line| line.glyph_range.clone()),
            ))
            .map(|((run_range, cluster_range), glyph_range)| {
                (run_range, cluster_range, glyph_range)
            })
            .unwrap_or((0..0, 0..0, 0..0))
        };

        TextLineWindow {
            layout: self,
            metadata: self.metadata(),
            line_range: clamped_line_range,
            run_range,
            cluster_range,
            glyph_range,
        }
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

    pub fn hit_test_point(&self, point: Point) -> TextCursor {
        if self.data.lines.is_empty() {
            return TextCursor::default();
        }

        let line = &self.data.lines[self.line_index_for_y(point.y)];
        TextCursor::new(line.offset_for_x(point.x))
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

        let bounds = rects
            .iter()
            .copied()
            .reduce(|bounds, rect| bounds.union(rect));
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

    pub(crate) fn line_index_for_offset(&self, utf8_offset: usize) -> usize {
        let offset = utf8_offset.min(self.data.text.len());
        self.data
            .lines
            .iter()
            .position(|line| offset <= line.byte_range.end)
            .unwrap_or_else(|| self.data.lines.len().saturating_sub(1))
    }

    fn line_index_for_y(&self, y: f32) -> usize {
        self.data
            .lines
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                let left_center = left.rect.y() + (left.rect.height() * 0.5);
                let right_center = right.rect.y() + (right.rect.height() * 0.5);
                (left_center - y)
                    .abs()
                    .partial_cmp(&(right_center - y).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
            .unwrap_or_else(|| self.data.lines.len().saturating_sub(1))
    }

    pub(crate) fn with_document(mut self, document: TextDocument) -> Self {
        self.primary_style = document.primary_style();
        self.document = Arc::new(document);
        self
    }

    #[cfg(test)]
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

fn collapse_range(ranges: impl Iterator<Item = Range<usize>>) -> Option<Range<usize>> {
    ranges.fold(None, |current: Option<Range<usize>>, range| {
        Some(match current {
            Some(existing) => existing.start.min(range.start)..existing.end.max(range.end),
            None => range,
        })
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    pub origin: Point,
    pub layout_handle: TextLayoutHandle,
    pub layout_version: TextLayoutVersion,
    pub bounds: Rect,
}

impl ShapedText {
    pub fn new(origin: Point, layout: &PersistentTextLayout) -> Self {
        Self {
            origin,
            layout_handle: layout.handle(),
            layout_version: layout.version(),
            bounds: layout.measurement().bounds,
        }
    }

    pub fn from_layout(
        origin: Point,
        layout_handle: TextLayoutHandle,
        layout: &TextLayout,
    ) -> Self {
        Self {
            origin,
            layout_handle,
            layout_version: layout.version(),
            bounds: layout.measurement().bounds,
        }
    }

    pub fn translated_bounds(&self) -> Rect {
        self.bounds.translate(self.origin.to_vector())
    }

    pub fn resolve<'a>(&self, registry: &'a TextLayoutRegistry) -> Option<&'a TextLayout> {
        let layout = registry.get(self.layout_handle)?;
        (layout.version() == self.layout_version).then_some(layout)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedTextWindow {
    pub origin: Point,
    pub layout_handle: TextLayoutHandle,
    pub layout_version: TextLayoutVersion,
    pub line_range: Range<usize>,
    pub bounds: Rect,
}

impl ShapedTextWindow {
    pub fn new(origin: Point, layout: &PersistentTextLayout, line_range: Range<usize>) -> Self {
        Self::from_layout(origin, layout.handle(), layout, line_range)
    }

    pub fn from_layout(
        origin: Point,
        layout_handle: TextLayoutHandle,
        layout: &TextLayout,
        line_range: Range<usize>,
    ) -> Self {
        let window = layout.line_window(line_range);
        Self {
            origin,
            layout_handle,
            layout_version: layout.version(),
            line_range: window.line_range.clone(),
            bounds: window.bounds(),
        }
    }

    pub fn translated_bounds(&self) -> Rect {
        self.bounds.translate(self.origin.to_vector())
    }

    pub fn resolve<'a>(&self, registry: &'a TextLayoutRegistry) -> Option<&'a TextLayout> {
        let layout = registry.get(self.layout_handle)?;
        (layout.version() == self.layout_version).then_some(layout)
    }
}
