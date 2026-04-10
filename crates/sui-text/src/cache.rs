use std::collections::HashMap;

use sui_core::{FontHandle, Size};

use crate::{
    flatten::FlattenedTextDocument,
    font::FaceCacheKey,
    model::{TextLayout, TextParagraphStyle, TextStyle},
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
    pub(crate) fn new(
        flattened: &FlattenedTextDocument,
        span_face_keys: &[FaceCacheKey],
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
                        let span = &flattened.spans[index];
                        TextSpanCacheKey {
                            text: span.text.clone(),
                            style: TextStyleCacheKey::new(&span.style),
                            face: span_face_keys[index],
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
pub(crate) struct TextLayoutCache {
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

impl TextLayoutCache {
    pub(crate) fn snapshot(&self) -> TextLayoutCacheSnapshot {
        TextLayoutCacheSnapshot {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }

    pub(crate) fn get(&mut self, key: &TextLayoutCacheKey) -> Option<TextLayout> {
        let cached = self.entries.get(key).cloned();
        if cached.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        cached
    }

    pub(crate) fn insert(&mut self, key: TextLayoutCacheKey, layout: TextLayout) {
        self.entries.insert(key, layout);
    }
}