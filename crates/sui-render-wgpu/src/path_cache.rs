use crate::feathering;
use crate::geometry::AnalyticPathCpuData;
use crate::geometry::CachedGlyphMesh;
use crate::geometry::build_lyon_path;
use crate::text::AnalyticPathCacheKey;
use crate::text::GlyphCacheSnapshot;
use crate::text::PathCacheKey;
use std::hash::Hash;
use std::sync::Arc;
use sui_core::Path as ScenePath;
use sui_core::Result;
use sui_core::Transform;
use sui_scene::StrokeStyle;

pub(crate) const MAX_PATH_CACHE_ENTRIES: usize = 8_192;
pub(crate) const MAX_PATH_CACHE_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const PATH_CACHE_IDLE_FRAMES: u64 = 120;

#[derive(Debug)]
pub(crate) struct CachedGeometry<V> {
    pub(crate) value: Arc<V>,
    pub(crate) bytes: usize,
    pub(crate) last_used_frame: u64,
}

#[derive(Debug)]
pub(crate) struct GeometryCache<K: Hash + Eq, V> {
    pub(crate) entries: lru::LruCache<K, CachedGeometry<V>>,
    pub(crate) bytes: usize,
    pub(crate) max_entries: usize,
    pub(crate) max_bytes: usize,
    pub(crate) frame: u64,
}

impl<K: Hash + Eq, V> GeometryCache<K, V> {
    pub(crate) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: lru::LruCache::unbounded(),
            bytes: 0,
            max_entries,
            max_bytes,
            frame: 0,
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.entries.get_mut(key).map(|entry| {
            entry.last_used_frame = self.frame;
            Arc::clone(&entry.value)
        })
    }

    pub(crate) fn insert(&mut self, key: K, value: Arc<V>, bytes: usize) {
        // A caller may use oversized geometry without making the cache retain
        // it or evicting the entire reusable working set for a one-off draw.
        if bytes > self.max_bytes || self.max_entries == 0 {
            return;
        }
        if let Some(previous) = self.entries.put(
            key,
            CachedGeometry {
                value,
                bytes,
                last_used_frame: self.frame,
            },
        ) {
            self.bytes -= previous.bytes;
        }
        self.bytes += bytes;
        while self.entries.len() > self.max_entries || self.bytes > self.max_bytes {
            self.evict_oldest();
        }
    }

    pub(crate) fn begin_frame(&mut self, frame: u64) {
        self.frame = frame;
        while self.entries.peek_lru().is_some_and(|(_, entry)| {
            frame.saturating_sub(entry.last_used_frame) > PATH_CACHE_IDLE_FRAMES
        }) {
            self.evict_oldest();
        }
    }

    pub(crate) fn evict_oldest(&mut self) {
        if let Some((_, entry)) = self.entries.pop_lru() {
            self.bytes -= entry.bytes;
        }
    }
}

pub(crate) fn geometry_allocation_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

#[derive(Debug)]
pub(crate) struct PathMeshCache {
    pub(crate) meshes: GeometryCache<PathCacheKey, CachedGlyphMesh>,
    pub(crate) analytic_paths: GeometryCache<AnalyticPathCacheKey, AnalyticPathCpuData>,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) hits: usize,
    pub(crate) misses: usize,
    pub(crate) analytic_hits: usize,
    pub(crate) analytic_misses: usize,
}

impl Default for PathMeshCache {
    fn default() -> Self {
        Self {
            meshes: GeometryCache::new(MAX_PATH_CACHE_ENTRIES, MAX_PATH_CACHE_BYTES),
            analytic_paths: GeometryCache::new(MAX_PATH_CACHE_ENTRIES, MAX_PATH_CACHE_BYTES),
            diagnostics_enabled: true,
            hits: 0,
            misses: 0,
            analytic_hits: 0,
            analytic_misses: 0,
        }
    }
}

impl PathMeshCache {
    pub(crate) fn begin_frame(&mut self, frame: u64) {
        self.meshes.begin_frame(frame);
        self.analytic_paths.begin_frame(frame);
    }

    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
    }

    pub(crate) fn cached_fill_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        feather_width: f32,
    ) -> Result<Arc<CachedGlyphMesh>> {
        let key = PathCacheKey::fill(path, transform, feather_width);
        if let Some(mesh) = self.meshes.get(&key) {
            if self.diagnostics_enabled {
                self.hits += 1;
            }
            return Ok(mesh);
        }
        if self.diagnostics_enabled {
            self.misses += 1;
        }
        let lyon_path = build_lyon_path(path, transform);
        let mesh = Arc::new(feathering::build_local_fill_mesh(
            &lyon_path,
            feather_width,
        )?);
        self.cache_mesh(key, Arc::clone(&mesh));
        Ok(mesh)
    }

    pub(crate) fn cached_analytic_fill(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        feather_width: f32,
        build: impl FnOnce() -> Option<AnalyticPathCpuData>,
    ) -> Option<Arc<AnalyticPathCpuData>> {
        let key = AnalyticPathCacheKey::fill(path, transform, feather_width);
        self.cached_analytic(key, build)
    }

    pub(crate) fn cached_analytic_stroke(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        stroke: StrokeStyle,
        feather_width: f32,
        build: impl FnOnce() -> Option<AnalyticPathCpuData>,
    ) -> Option<Arc<AnalyticPathCpuData>> {
        let key = AnalyticPathCacheKey::stroke(path, transform, stroke, feather_width);
        self.cached_analytic(key, build)
    }

    pub(crate) fn cached_stroke_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        stroke: StrokeStyle,
        feather_width: f32,
    ) -> Result<Arc<CachedGlyphMesh>> {
        let key = PathCacheKey::stroke(path, transform, stroke, feather_width);
        if let Some(mesh) = self.meshes.get(&key) {
            if self.diagnostics_enabled {
                self.hits += 1;
            }
            return Ok(mesh);
        }
        if self.diagnostics_enabled {
            self.misses += 1;
        }
        let lyon_path = build_lyon_path(path, transform);
        let mesh = Arc::new(feathering::build_local_stroke_mesh(
            &lyon_path,
            stroke,
            feather_width,
        )?);
        self.cache_mesh(key, Arc::clone(&mesh));
        Ok(mesh)
    }

    pub(crate) fn cache_mesh(&mut self, key: PathCacheKey, mesh: Arc<CachedGlyphMesh>) {
        let bytes = size_of::<CachedGlyphMesh>()
            + geometry_allocation_bytes(&mesh.vertices)
            + geometry_allocation_bytes(&mesh.indices);
        self.meshes.insert(key, mesh, bytes);
    }

    pub(crate) fn cached_analytic(
        &mut self,
        key: AnalyticPathCacheKey,
        build: impl FnOnce() -> Option<AnalyticPathCpuData>,
    ) -> Option<Arc<AnalyticPathCpuData>> {
        if let Some(data) = self.analytic_paths.get(&key) {
            if self.diagnostics_enabled {
                self.analytic_hits += 1;
            }
            return Some(data);
        }
        let data = Arc::new(build()?);
        if self.diagnostics_enabled {
            self.analytic_misses += 1;
        }
        let bytes = size_of::<AnalyticPathCpuData>()
            + geometry_allocation_bytes(&data.contours)
            + geometry_allocation_bytes(&data.points);
        self.analytic_paths.insert(key, Arc::clone(&data), bytes);
        Some(data)
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (usize, usize, usize) {
        (self.meshes.entries.len(), self.hits, self.misses)
    }

    pub(crate) fn snapshot(&self) -> GlyphCacheSnapshot {
        GlyphCacheSnapshot {
            entries: self.meshes.entries.len() + self.analytic_paths.entries.len(),
            hits: self.hits + self.analytic_hits,
            misses: self.misses + self.analytic_misses,
        }
    }
}

#[cfg(test)]
pub(crate) mod geometry_cache_tests {
    use super::*;

    #[test]
    fn cache_budgets_evict_lru_geometry_and_bypass_oversized_draws() {
        let mut cache = GeometryCache::new(2, 12);
        cache.insert(1, Arc::new(vec![1_u8; 4]), 4);
        cache.insert(2, Arc::new(vec![2_u8; 4]), 4);
        let retained = cache.get(&1).unwrap();
        cache.insert(3, Arc::new(vec![3_u8; 4]), 4);
        assert!(cache.get(&2).is_none());
        assert!(cache.get(&1).is_some());
        cache.insert(4, Arc::new(vec![4_u8; 13]), 13);
        assert!(cache.get(&4).is_none());
        assert_eq!(cache.entries.len(), 2);
        // Fit the entry count but exceed the byte budget.
        cache.insert(5, Arc::new(vec![5_u8; 9]), 9);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.bytes, 9);
        assert_eq!(*retained, vec![1; 4]);
    }

    #[test]
    fn reused_geometry_survives_idle_eviction() {
        let mut cache = GeometryCache::new(2, 16);
        cache.begin_frame(1);
        cache.insert(1, Arc::new(1_u32), 4);
        cache.insert(2, Arc::new(2_u32), 4);
        cache.begin_frame(120);
        cache.get(&1).unwrap();
        cache.begin_frame(122);
        assert!(cache.get(&1).is_some());
        assert!(cache.get(&2).is_none());
        assert_eq!(cache.bytes, 4);
    }
}
