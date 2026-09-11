use std::{
    collections::hash_map::{DefaultHasher, RandomState},
    hash::{BuildHasher, Hash, Hasher},
};

use sui_core::{FontHandle, Size};

use crate::{
    font::FaceCacheKey,
    model::{TextDocument, TextLayout, TextLayoutId, TextParagraphStyle, TextStyle},
    style::{FontFeatures, FontStretch, FontStyle},
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
    font_families_hash: u64,
    font_size_bits: u32,
    line_height_bits: u32,
    weight: u16,
    style: FontStyle,
    stretch: FontStretch,
    // Features are hashed into a u64 so the key stays `Copy` and cheap to compare; layouts with
    // different OpenType features must not share a cache entry.
    features_hash: u64,
}

impl TextStyleCacheKey {
    fn new(style: &TextStyle) -> Self {
        Self {
            font_handle: style.font.map(FontHandle::get),
            font_families_hash: hash_value(&style.font_families),
            font_size_bits: style.font_size.to_bits(),
            line_height_bits: style.line_height.to_bits(),
            weight: style.weight.value(),
            style: style.style,
            stretch: style.stretch,
            features_hash: hash_features(&style.features),
        }
    }
}

fn hash_features(features: &FontFeatures) -> u64 {
    hash_value(features)
}

fn hash_value(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
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

// Cache-owned references are bounded independently of layouts pinned by widgets
// or immutable scene frames. Large one-off documents bypass this cache.
const MAX_LAYOUT_ENTRIES: usize = 4_096;
const MAX_LAYOUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct TextLayoutCache {
    entries: lru::LruCache<u64, Vec<TextLayoutCacheEntry>>,
    hash_builder: RandomState,
    entry_count: usize,
    retained_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    hits: usize,
    misses: usize,
}

#[derive(Debug, Clone)]
struct TextLayoutCacheEntry {
    key: TextLayoutCacheKey,
    layout: TextLayout,
    retained_bytes: usize,
}

impl Default for TextLayoutCache {
    fn default() -> Self {
        Self::with_limits(MAX_LAYOUT_ENTRIES, MAX_LAYOUT_BYTES)
    }
}

fn allocation_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(size_of::<T>())
}

fn layout_entry_bytes(key: &TextLayoutCacheKey, layout: &TextLayout) -> usize {
    // Shared font bytes belong to the font registry; count layout-owned arrays
    // and strings, conservatively charging shared layout data to each entry.
    let data = &layout.data;
    size_of::<TextLayoutCacheEntry>()
        + size_of::<crate::model::TextLayoutData>()
        + allocation_bytes(&key.paragraphs)
        + key
            .paragraphs
            .iter()
            .map(|paragraph| {
                allocation_bytes(&paragraph.spans)
                    + paragraph
                        .spans
                        .iter()
                        .map(|span| span.text.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>()
        + data.text.capacity()
        + allocation_bytes(&data.faces)
        + allocation_bytes(&data.paragraphs)
        + allocation_bytes(&data.lines)
        + data
            .lines
            .iter()
            .map(|line| allocation_bytes(&line.clusters))
            .sum::<usize>()
        + allocation_bytes(&data.runs)
        + allocation_bytes(&data.clusters)
        + allocation_bytes(&data.glyphs)
        + allocation_bytes(&layout.document.paragraphs)
        + layout
            .document
            .paragraphs
            .iter()
            .map(|paragraph| {
                allocation_bytes(&paragraph.spans)
                    + paragraph
                        .spans
                        .iter()
                        .map(|span| {
                            span.text.capacity()
                                + span.style.features.len() * size_of::<crate::style::FontFeature>()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
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
    pub(crate) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: lru::LruCache::unbounded(),
            hash_builder: RandomState::new(),
            entry_count: 0,
            retained_bytes: 0,
            max_entries,
            max_bytes,
            hits: 0,
            misses: 0,
        }
    }

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
            &self.hash_builder,
            document,
            span_face_keys,
            box_size,
        );
        let cached = self
            .entries
            .get(&hash)
            .and_then(|bucket| {
                bucket.iter().find(|entry| {
                    entry
                        .key
                        .matches_document(document, span_face_keys, box_size)
                })
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
            &self.hash_builder,
            document,
            span_face_keys,
            box_size,
        );
        let key = TextLayoutCacheKey::new(document, span_face_keys, box_size);
        let retained_bytes = layout_entry_bytes(&key, &layout);
        if retained_bytes > self.max_bytes || self.max_entries == 0 {
            return;
        }
        if let Some(existing) = self
            .entries
            .get_mut(&hash)
            .and_then(|bucket| bucket.iter_mut().find(|entry| entry.key == key))
        {
            self.retained_bytes -= existing.retained_bytes;
            existing.layout = layout;
            existing.retained_bytes = retained_bytes;
        } else {
            let bucket = self.entries.get_or_insert_mut(hash, Vec::new);
            bucket.push(TextLayoutCacheEntry {
                key,
                layout,
                retained_bytes,
            });
            self.entry_count += 1;
        }
        self.retained_bytes += retained_bytes;
        while self.entry_count > self.max_entries || self.retained_bytes > self.max_bytes {
            let Some((_, bucket)) = self.entries.pop_lru() else {
                break;
            };
            self.entry_count -= bucket.len();
            self.retained_bytes -= bucket
                .iter()
                .map(|entry| entry.retained_bytes)
                .sum::<usize>();
        }
    }
}
