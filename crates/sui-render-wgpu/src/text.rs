use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphFaceCacheKey {
    pub(crate) data_ptr: usize,
    pub(crate) data_len: usize,
    pub(crate) face_index: u32,
}

impl GlyphFaceCacheKey {
    pub(crate) fn new(face: &ResolvedTextFace) -> Self {
        Self {
            data_ptr: face.data_ptr(),
            data_len: face.data_len(),
            face_index: face.face_index(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GlyphCacheKey {
    pub(crate) face: GlyphFaceCacheKey,
    pub(crate) glyph_id: u16,
    pub(crate) scale_bucket: u32,
    pub(crate) coverage_policy: TextCoverageCacheKey,
}

impl GlyphCacheKey {
    pub(crate) fn new(
        face: GlyphFaceCacheKey,
        glyph_id: u16,
        scale_bucket: u32,
        coverage_policy: TextCoveragePolicy,
    ) -> Self {
        Self {
            face,
            glyph_id,
            scale_bucket,
            coverage_policy: TextCoverageCacheKey::from(coverage_policy),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextCoverageCacheKey {
    Linear,
    Gamma { gamma_bits: u32 },
    TwoCoverageMinusCoverageSq,
}

impl From<TextCoveragePolicy> for TextCoverageCacheKey {
    fn from(value: TextCoveragePolicy) -> Self {
        match value.normalized() {
            TextCoveragePolicy::AutomaticByTextLuminance => Self::Linear,
            TextCoveragePolicy::Linear => Self::Linear,
            TextCoveragePolicy::Gamma(gamma) => Self::Gamma {
                gamma_bits: gamma.to_bits(),
            },
            TextCoveragePolicy::TwoCoverageMinusCoverageSq => Self::TwoCoverageMinusCoverageSq,
        }
    }
}

pub(crate) const GLYPH_SCALE_BUCKETS_PER_UNIT: f32 = 16_384.0;

pub(crate) fn glyph_scale_bucket(scale: f32) -> u32 {
    ((scale.max(f32::EPSILON) * GLYPH_SCALE_BUCKETS_PER_UNIT)
        .round()
        .max(1.0)) as u32
}

pub(crate) fn glyph_scale_from_bucket(bucket: u32) -> f32 {
    (bucket.max(1) as f32) / GLYPH_SCALE_BUCKETS_PER_UNIT
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedGlyphVertex {
    pub(crate) position: Point,
    pub(crate) coverage: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedGlyphAtlas {
    pub(crate) scale: f32,
    pub(crate) offset: Vector,
    pub(crate) size: Size,
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) is_color: bool,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct CachedGlyphMesh {
    pub(crate) vertices: Vec<CachedGlyphVertex>,
    pub(crate) indices: Vec<u32>,
}

impl CachedGlyphMesh {
    pub(crate) fn push_vertex(&mut self, position: Point, coverage: f32) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CachedGlyphVertex { position, coverage });
        index
    }

    pub(crate) fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasRectU {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl AtlasRectU {
    const NOTHING: Self = Self {
        min_x: usize::MAX,
        min_y: usize::MAX,
        max_x: 0,
        max_y: 0,
    };

    const EVERYTHING: Self = Self {
        min_x: 0,
        min_y: 0,
        max_x: usize::MAX,
        max_y: usize::MAX,
    };

    fn include_rect(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x + width);
        self.max_y = self.max_y.max(y + height);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextAtlasPlacement {
    pub(crate) x: usize,
    pub(crate) y: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TextAtlasUpload {
    pub(crate) size: (u32, u32),
    pub(crate) offset: (u32, u32),
    pub(crate) extent: (u32, u32),
    pub(crate) full_texture: bool,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct TextAtlas {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
    dirty: AtlasRectU,
    pub(crate) full_upload: bool,
    pub(crate) cursor: (usize, usize),
    pub(crate) row_height: usize,
}

impl Default for TextAtlas {
    fn default() -> Self {
        Self::new(TEXT_ATLAS_WIDTH, TEXT_ATLAS_HEIGHT)
    }
}

impl TextAtlas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
            dirty: AtlasRectU::EVERYTHING,
            full_upload: true,
            cursor: (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING),
            row_height: 0,
        }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    fn allocate(&mut self, width: usize, height: usize) -> Option<TextAtlasPlacement> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return None;
        }

        if self.cursor.0 + width + TEXT_ATLAS_PADDING > self.width {
            self.cursor.0 = TEXT_ATLAS_PADDING;
            self.cursor.1 += self.row_height + TEXT_ATLAS_PADDING;
            self.row_height = 0;
        }

        if self.cursor.1 + height + TEXT_ATLAS_PADDING > self.height {
            return None;
        }

        let placement = TextAtlasPlacement {
            x: self.cursor.0,
            y: self.cursor.1,
        };
        self.cursor.0 += width + TEXT_ATLAS_PADDING;
        self.row_height = self.row_height.max(height);
        Some(placement)
    }

    fn write_rgba(
        &mut self,
        placement: TextAtlasPlacement,
        width: usize,
        height: usize,
        pixels: &[u8],
    ) {
        for row in 0..height {
            let src_start = row * width * 4;
            let src_end = src_start + (width * 4);
            let dst_start = ((placement.y + row) * self.width + placement.x) * 4;
            let dst_end = dst_start + (width * 4);
            self.pixels[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
        }
        self.dirty
            .include_rect(placement.x, placement.y, width, height);
    }

    pub(crate) fn insert_rgba(
        &mut self,
        width: usize,
        height: usize,
        pixels: &[u8],
    ) -> Option<TextAtlasPlacement> {
        let placement = self.allocate(width, height)?;
        self.write_rgba(placement, width, height, pixels);
        Some(placement)
    }

    pub(crate) fn take_upload(&mut self) -> Option<TextAtlasUpload> {
        let dirty = std::mem::replace(&mut self.dirty, AtlasRectU::NOTHING);
        if dirty == AtlasRectU::NOTHING {
            return None;
        }

        if self.full_upload || dirty == AtlasRectU::EVERYTHING {
            self.full_upload = false;
            return Some(TextAtlasUpload {
                size: self.size(),
                offset: (0, 0),
                extent: self.size(),
                full_texture: true,
                pixels: self.pixels.clone(),
            });
        }

        let width = dirty.max_x - dirty.min_x;
        let height = dirty.max_y - dirty.min_y;
        let mut pixels = vec![0; width * height * 4];
        for row in 0..height {
            let src_start = ((dirty.min_y + row) * self.width + dirty.min_x) * 4;
            let src_end = src_start + (width * 4);
            let dst_start = row * width * 4;
            let dst_end = dst_start + (width * 4);
            pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        Some(TextAtlasUpload {
            size: self.size(),
            offset: (dirty.min_x as u32, dirty.min_y as u32),
            extent: (width as u32, height as u32),
            full_texture: false,
            pixels,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PathCacheKind {
    Fill,
    Stroke { line_width_bits: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathCacheKey {
    signature: u64,
    kind: PathCacheKind,
    feather_width_bits: u32,
}

impl PathCacheKey {
    fn fill(path: &ScenePath, transform: Transform, feather_width: f32) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Fill,
            feather_width_bits: feather_width.to_bits(),
        }
    }

    fn stroke(path: &ScenePath, transform: Transform, line_width: f32, feather_width: f32) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Stroke {
                line_width_bits: line_width.to_bits(),
            },
            feather_width_bits: feather_width.to_bits(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PathMeshCache {
    meshes: HashMap<PathCacheKey, CachedGlyphMesh>,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) hits: usize,
    pub(crate) misses: usize,
}

impl Default for PathMeshCache {
    fn default() -> Self {
        Self {
            meshes: HashMap::new(),
            diagnostics_enabled: true,
            hits: 0,
            misses: 0,
        }
    }
}

impl PathMeshCache {
    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
    }

    pub(crate) fn cached_fill_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        feather_width: f32,
    ) -> Result<&CachedGlyphMesh> {
        let key = PathCacheKey::fill(path, transform, feather_width);
        match self.meshes.entry(key) {
            Entry::Occupied(entry) => {
                if self.diagnostics_enabled {
                    self.hits += 1;
                }
                Ok(entry.into_mut())
            }
            Entry::Vacant(entry) => {
                if self.diagnostics_enabled {
                    self.misses += 1;
                }
                let lyon_path = build_lyon_path(path, transform);
                let mesh = feathering::build_local_fill_mesh(&lyon_path, feather_width)?;
                Ok(entry.insert(mesh))
            }
        }
    }

    pub(crate) fn cached_stroke_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        line_width: f32,
        feather_width: f32,
    ) -> Result<&CachedGlyphMesh> {
        let key = PathCacheKey::stroke(path, transform, line_width, feather_width);
        match self.meshes.entry(key) {
            Entry::Occupied(entry) => {
                if self.diagnostics_enabled {
                    self.hits += 1;
                }
                Ok(entry.into_mut())
            }
            Entry::Vacant(entry) => {
                if self.diagnostics_enabled {
                    self.misses += 1;
                }
                let lyon_path = build_lyon_path(path, transform);
                let mesh =
                    feathering::build_local_stroke_mesh(&lyon_path, line_width, feather_width)?;
                Ok(entry.insert(mesh))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (usize, usize, usize) {
        (self.meshes.len(), self.hits, self.misses)
    }

    pub(crate) fn snapshot(&self) -> GlyphCacheSnapshot {
        GlyphCacheSnapshot {
            entries: self.meshes.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }
}
