//! Width-independent shaping and intrinsic metrics, owned by one FontContext.
//! Eviction drops only preparatory state; published TextLayouts own their geometry.
use std::{hash::Hash, mem::size_of};

use cosmic_text::{AttrsOwned, FamilyOwned, LayoutLine, Metrics, ShapeLine, fontdb};

use crate::model::{TextAlign, TextDirection, TextWrap, TextWritingMode};

const PARAGRAPH_BYTES: usize = 16 * 1024 * 1024;
const GLYPH_BYTES: usize = 4 * 1024 * 1024;

/// Cache-owned storage only, excluding layouts retained by widgets and font data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreparationCacheSnapshot {
    pub entries: usize,
    /// Conservatively charged payload capacity plus per-entry bookkeeping.
    pub retained_bytes: usize,
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextPreparationCacheSnapshot {
    pub paragraphs: PreparationCacheSnapshot,
    pub glyphs: PreparationCacheSnapshot,
}

#[derive(Debug)]
struct Entry<V> {
    value: V,
    bytes: usize,
}

#[derive(Debug)]
pub(crate) struct ByteCache<K: Hash + Eq, V> {
    entries: lru::LruCache<K, Entry<V>>,
    max_entries: usize,
    max_bytes: usize,
    stats: PreparationCacheSnapshot,
}

impl<K: Hash + Eq, V> ByteCache<K, V> {
    pub(crate) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: lru::LruCache::unbounded(),
            max_entries,
            max_bytes,
            stats: Default::default(),
        }
    }

    pub(crate) fn snapshot(&self) -> PreparationCacheSnapshot {
        PreparationCacheSnapshot {
            entries: self.entries.len(),
            ..self.stats
        }
    }

    pub(crate) fn take(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.pop(key) {
            self.stats.hits += 1;
            self.stats.retained_bytes -= entry.bytes;
            Some(entry.value)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(entry) = self.entries.get(key) {
            self.stats.hits += 1;
            Some(&entry.value)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V, payload_bytes: usize) {
        // Charge LRU links and spare hash-table buckets as well as key/value storage.
        let bytes = payload_bytes
            .saturating_add(size_of::<K>() + size_of::<Entry<V>>() + size_of::<[usize; 8]>());
        if bytes > self.max_bytes || self.max_entries == 0 {
            return;
        }
        if let Some(old) = self.entries.pop(&key) {
            self.stats.retained_bytes -= old.bytes;
        }
        while self.entries.len() >= self.max_entries
            || self.stats.retained_bytes > self.max_bytes - bytes
        {
            let Some((_, old)) = self.entries.pop_lru() else {
                break;
            };
            self.stats.retained_bytes -= old.bytes;
            self.stats.evictions += 1;
        }
        self.entries.put(key, Entry { value, bytes });
        self.stats.retained_bytes += bytes;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PreparedSpanKey {
    pub text: String,
    pub attrs: AttrsOwned,
}

/// Exact equality, including full feature settings, rather than hashed style identity.
/// Font IDs/fallbacks belong to the enclosing FontContext. Span metadata is local
/// to the paragraph, so moving an unchanged paragraph does not invalidate shaping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PreparedParagraphKey {
    pub direction: TextDirection,
    pub writing_mode: TextWritingMode,
    pub defaults: AttrsOwned,
    pub spans: Vec<PreparedSpanKey>,
}

impl PreparedParagraphKey {
    pub(crate) fn allocated_bytes(&self) -> usize {
        attrs_bytes(&self.defaults)
            + allocation_bytes(&self.spans)
            + self
                .spans
                .iter()
                .map(|span| span.text.capacity() + attrs_bytes(&span.attrs))
                .sum::<usize>()
    }
}

fn attrs_bytes(attrs: &AttrsOwned) -> usize {
    // FamilyOwned uses SmolStr: charge its full bytes plus Arc headers even when inline.
    let family = match &attrs.family_owned {
        FamilyOwned::Name(name) => name.len() + 2 * size_of::<usize>(),
        _ => 0,
    };
    family + allocation_bytes(&attrs.font_features.features)
}

/// Alignment and wrapping affect line layout, not glyph shaping. Keep them here
/// so measurement and painting can share a shape without sharing stale lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedLineLayoutKey {
    pub width: Option<u32>,
    pub align: TextAlign,
    pub wrap: TextWrap,
}

#[derive(Debug)]
pub(crate) struct PreparedParagraphState {
    pub shape: ShapeLine,
    pub metrics: Metrics,
    pub lines: Vec<LayoutLine>,
    /// None means not laid out yet; a key with no width is unconstrained.
    pub layout_key: Option<PreparedLineLayoutKey>,
}

impl PreparedParagraphState {
    pub(crate) fn allocated_bytes(&self) -> usize {
        // Shapes are built without ellipsizing or decorations. ShapeLine's private
        // ellipsis_span stays None. Count actual Vec capacities, not text length:
        // a grapheme can produce multiple glyphs, and narrow widths grow line arrays.
        allocation_bytes(&self.shape.spans)
            + self
                .shape
                .spans
                .iter()
                .map(|span| {
                    allocation_bytes(&span.words)
                        + allocation_bytes(&span.decoration_spans)
                        + span
                            .words
                            .iter()
                            .map(|word| allocation_bytes(&word.glyphs))
                            .sum::<usize>()
                })
                .sum::<usize>()
            + allocation_bytes(&self.lines)
            + self
                .lines
                .iter()
                .map(|line| allocation_bytes(&line.glyphs) + allocation_bytes(&line.decorations))
                .sum::<usize>()
    }
}

fn allocation_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(size_of::<T>())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphMetricsKey {
    pub font: fontdb::ID,
    pub glyph: u16,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IntrinsicGlyphMetrics {
    pub units_per_em: f32,
    pub cap_height: Option<f32>,
    pub bounds: Option<ttf_parser::Rect>,
}

#[derive(Debug)]
pub(crate) struct PreparationCaches {
    pub paragraphs: ByteCache<PreparedParagraphKey, PreparedParagraphState>,
    pub glyphs: ByteCache<GlyphMetricsKey, IntrinsicGlyphMetrics>,
}

impl Default for PreparationCaches {
    fn default() -> Self {
        Self {
            paragraphs: ByteCache::new(512, PARAGRAPH_BYTES),
            glyphs: ByteCache::new(32_768, GLYPH_BYTES),
        }
    }
}

impl PreparationCaches {
    pub(crate) fn snapshot(&self) -> TextPreparationCacheSnapshot {
        TextPreparationCacheSnapshot {
            paragraphs: self.paragraphs.snapshot(),
            glyphs: self.glyphs.snapshot(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_budget_evicts_lru_and_rejects_oversized_entries_without_flushing() {
        let mut cache = ByteCache::new(100, 1_024);
        cache.insert(1, "first", 300);
        cache.insert(2, "second", 300);
        assert_eq!(cache.get(&1), Some(&"first"));
        cache.insert(3, "third", 300);
        assert_eq!(cache.get(&2), None);
        cache.insert(4, "oversized", 1_025);
        assert_eq!(cache.get(&1), Some(&"first"));
        assert_eq!(cache.get(&3), Some(&"third"));
        assert!(cache.snapshot().retained_bytes <= 1_024);
        assert_eq!(cache.snapshot().evictions, 1);
        let bytes = cache.snapshot().retained_bytes;
        let taken = cache.take(&3).unwrap();
        assert!(cache.snapshot().retained_bytes < bytes);
        cache.insert(3, taken, 300);
        assert_eq!(cache.snapshot().retained_bytes, bytes);
    }

    #[test]
    fn entry_budget_and_replacement_accounting_remain_bounded() {
        let mut cache = ByteCache::new(2, usize::MAX);
        for key in 0..100 {
            cache.insert(key, (), 0);
        }
        assert_eq!(cache.snapshot().entries, 2);
        assert_eq!(cache.snapshot().evictions, 98);
        let bytes = cache.snapshot().retained_bytes;
        cache.insert(99, (), 0);
        assert_eq!(cache.snapshot().retained_bytes, bytes);
    }
}
