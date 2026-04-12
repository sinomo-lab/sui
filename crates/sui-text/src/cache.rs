use std::{
    collections::HashMap,
    collections::hash_map::DefaultHasher,
    hash::{BuildHasher, Hash, Hasher},
};

use sui_core::{FontHandle, Size};

use crate::{
    font::FaceCacheKey,
    model::{TextDocument, TextLayout, TextLayoutId, TextParagraphStyle, TextStyle},
};

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
pub(crate) struct TextLayoutCacheKey {
    paragraphs: Vec<TextParagraphCacheKey>,
    box_size: Option<SizeCacheKey>,
}

impl TextLayoutCacheKey {
    pub(crate) fn stable_layout_id(
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> TextLayoutId {
        let mut state = DefaultHasher::new();
        Self::hash_document_into(&mut state, document, span_face_keys, box_size);
        TextLayoutId::new(state.finish())
    }

    pub(crate) fn new(
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> Self {
        let mut span_index = 0;
        let paragraphs = document
            .paragraphs
            .iter()
            .map(|paragraph| TextParagraphCacheKey {
                style: paragraph.style.clone(),
                spans: paragraph
                    .spans
                    .iter()
                    .map(|span| {
                        let face = span_face_keys[span_index];
                        span_index += 1;
                        TextSpanCacheKey {
                            text: span.text.clone(),
                            style: TextStyleCacheKey::new(&span.style),
                            face,
                        }
                    })
                    .collect(),
            })
            .collect();
        debug_assert_eq!(span_index, span_face_keys.len());

        Self {
            paragraphs,
            box_size: box_size.map(SizeCacheKey::from),
        }
    }

    fn hash_document<S: BuildHasher>(
        hasher: &S,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> u64 {
        let mut state = hasher.build_hasher();
        Self::hash_document_into(&mut state, document, span_face_keys, box_size);
        state.finish()
    }

    fn hash_document_into<H: Hasher>(
        state: &mut H,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) {
        document.paragraphs.len().hash(state);
        let mut span_index = 0;
        for paragraph in &document.paragraphs {
            paragraph.style.hash(state);
            paragraph.spans.len().hash(state);
            for span in &paragraph.spans {
                span.text.hash(state);
                TextStyleCacheKey::new(&span.style).hash(state);
                span_face_keys[span_index].hash(state);
                span_index += 1;
            }
        }
        debug_assert_eq!(span_index, span_face_keys.len());
        box_size.map(SizeCacheKey::from).hash(state);
    }

    fn matches_document(
        &self,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> bool {
        if self.box_size != box_size.map(SizeCacheKey::from) {
            return false;
        }
        if self.paragraphs.len() != document.paragraphs.len() {
            return false;
        }

        let mut span_index = 0;
        for (cached_paragraph, paragraph) in self.paragraphs.iter().zip(&document.paragraphs) {
            if cached_paragraph.style != paragraph.style {
                return false;
            }
            if cached_paragraph.spans.len() != paragraph.spans.len() {
                return false;
            }

            for (cached_span, span) in cached_paragraph.spans.iter().zip(&paragraph.spans) {
                if cached_span.text != span.text
                    || cached_span.style != TextStyleCacheKey::new(&span.style)
                    || cached_span.face != span_face_keys[span_index]
                {
                    return false;
                }
                span_index += 1;
            }
        }

        span_index == span_face_keys.len()
    }
}

#[derive(Debug, Default)]
pub(crate) struct TextLayoutCache {
    entries: HashMap<u64, Vec<TextLayoutCacheEntry>>,
    entry_count: usize,
    hits: usize,
    misses: usize,
}

#[derive(Debug, Clone)]
struct TextLayoutCacheEntry {
    key: TextLayoutCacheKey,
    layout: TextLayout,
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

impl TextLayoutCache {
    pub(crate) fn snapshot(&self) -> TextLayoutCacheSnapshot {
        TextLayoutCacheSnapshot {
            entries: self.entry_count,
            hits: self.hits,
            misses: self.misses,
        }
    }

    pub(crate) fn get(
        &mut self,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> Option<TextLayout> {
        let hash = TextLayoutCacheKey::hash_document(
            self.entries.hasher(),
            document,
            span_face_keys,
            box_size,
        );
        let cached = self
            .entries
            .get(&hash)
            .and_then(|bucket| {
                bucket
                    .iter()
                    .find(|entry| entry.key.matches_document(document, span_face_keys, box_size))
            })
            .map(|entry| entry.layout.clone());
        if cached.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        cached
    }

    pub(crate) fn insert(
        &mut self,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
        layout: TextLayout,
    ) {
        let hash = TextLayoutCacheKey::hash_document(
            self.entries.hasher(),
            document,
            span_face_keys,
            box_size,
        );
        let key = TextLayoutCacheKey::new(document, span_face_keys, box_size);
        let bucket = self.entries.entry(hash).or_default();
        if let Some(existing) = bucket.iter_mut().find(|entry| entry.key == key) {
            existing.layout = layout;
            return;
        }

        bucket.push(TextLayoutCacheEntry { key, layout });
        self.entry_count += 1;
    }
}