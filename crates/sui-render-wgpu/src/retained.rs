use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CompositionContainerId {
    Root,
    Layer(SceneLayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RetainedPacketId {
    pub(crate) container: CompositionContainerId,
    pub(crate) segment_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TransformNodeId(u64);

impl TransformNodeId {
    const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClipNodeId(u64);

impl ClipNodeId {
    const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EffectNodeId(u64);

impl EffectNodeId {
    const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetainedLayerRenderMode {
    Direct,
    CachedTiles,
}

const DEFAULT_TILE_SIZE_PX: u32 = 384;
const TILE_CACHE_BUDGET_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_ANALYTIC_PATH_CONTOURS: usize = 32;
pub(crate) const MAX_ANALYTIC_PATH_POINTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TileGrid {
    local_content_bounds: Rect,
    tile_size_logical: f32,
    tile_size_device_px: u32,
    scale_bucket: u32,
}

impl TileGrid {
    fn new(descriptor: &sui_scene::SceneLayerDescriptor, scale_factor: f32) -> Self {
        let scale_factor = scale_factor.max(0.001);
        Self {
            local_content_bounds: rect_to_layer_local(descriptor.content_bounds, descriptor),
            tile_size_logical: DEFAULT_TILE_SIZE_PX as f32 / scale_factor,
            tile_size_device_px: DEFAULT_TILE_SIZE_PX,
            scale_bucket: scale_bucket(scale_factor),
        }
    }

    fn is_empty(self) -> bool {
        self.local_content_bounds.is_empty() || self.tile_size_logical <= 0.0
    }

    fn contains_tile(self, tile_x: i32, tile_y: i32) -> bool {
        self.tile_rect(tile_x, tile_y)
            .intersection(self.local_content_bounds)
            .is_some()
    }

    fn tile_rect(self, tile_x: i32, tile_y: i32) -> Rect {
        Rect::new(
            tile_x as f32 * self.tile_size_logical,
            tile_y as f32 * self.tile_size_logical,
            self.tile_size_logical,
            self.tile_size_logical,
        )
    }

    fn tile_range_for_rect(self, rect: Rect) -> Option<((i32, i32), (i32, i32))> {
        let clipped = rect.intersection(self.local_content_bounds)?;
        let min_x = (clipped.x() / self.tile_size_logical).floor() as i32;
        let min_y = (clipped.y() / self.tile_size_logical).floor() as i32;
        let max_x = ((clipped.max_x() / self.tile_size_logical).ceil() as i32).saturating_sub(1);
        let max_y = ((clipped.max_y() / self.tile_size_logical).ceil() as i32).saturating_sub(1);
        Some(((min_x, min_y), (max_x, max_y)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TileAddress {
    pub(crate) layer: SceneLayerId,
    pub(crate) tile_x: i32,
    pub(crate) tile_y: i32,
    pub(crate) scale_bucket: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TileKey {
    layer: SceneLayerId,
    tile_x: i32,
    tile_y: i32,
    scale_bucket: u32,
    content_version: u64,
}

#[derive(Debug, Clone)]
enum TilePayload {
    DirectPacket(DrawOpArena),
}

#[derive(Debug)]
pub(crate) struct RetainedGpuGeometry {
    pub(crate) scene_range: PreparedVertices,
    pub(crate) scene_capacity: u32,
    pub(crate) clip_range: PreparedVertices,
    pub(crate) clip_capacity: u32,
    pub(crate) text_range: PreparedVertices,
    pub(crate) text_capacity: u32,
    pub(crate) dirty: bool,
}

#[derive(Default)]
pub(crate) struct RetainedTileVertexArena {
    pub(crate) scene_buffer: Option<wgpu::Buffer>,
    pub(crate) clip_buffer: Option<wgpu::Buffer>,
    pub(crate) text_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) scene_capacity_vertices: usize,
    pub(crate) clip_capacity_vertices: usize,
    pub(crate) text_instance_capacity: usize,
    pub(crate) used_scene_vertices: usize,
    pub(crate) used_clip_vertices: usize,
    pub(crate) used_text_instances: usize,
}

#[derive(Debug, Default)]
pub(crate) struct RetainedTileUploadPlan {
    pub(crate) in_place_tiles: Vec<TileAddress>,
    pub(crate) appended_tiles: Vec<TileAddress>,
    pub(crate) appended_scene_vertices: usize,
    pub(crate) appended_clip_vertices: usize,
    pub(crate) appended_text_instances: usize,
}

#[derive(Debug)]
pub(crate) struct TileEntry {
    key: TileKey,
    pub(crate) rect: Rect,
    pub(crate) translation: Vector,
    pub(crate) dirty: bool,
    pub(crate) visible: bool,
    pub(crate) last_used_frame: u64,
    pub(crate) memory_cost: usize,
    payload: TilePayload,
    pub(crate) cached_passes: Vec<CachedPassBatch>,
    pub(crate) gpu_geometry: Option<RetainedGpuGeometry>,
}

impl TileEntry {
    pub(crate) fn draw_ops(&self) -> &DrawOpArena {
        match &self.payload {
            TilePayload::DirectPacket(draw_ops) => draw_ops,
        }
    }

    pub(crate) fn scene_vertices(&self) -> &[Vertex] {
        match &self.payload {
            TilePayload::DirectPacket(draw_ops) => &draw_ops.scene_vertices,
        }
    }

    pub(crate) fn clip_vertices(&self) -> &[Vertex] {
        match &self.payload {
            TilePayload::DirectPacket(draw_ops) => &draw_ops.clip_vertices,
        }
    }

    pub(crate) fn text_instances(&self) -> &[TextAtlasInstance] {
        match &self.payload {
            TilePayload::DirectPacket(draw_ops) => &draw_ops.text_instances,
        }
    }

    pub(crate) fn uploaded_vertex_bytes(&self) -> u64 {
        (self.scene_vertices().len() as u64 + self.clip_vertices().len() as u64) * VERTEX_SIZE
            + self.text_instances().len() as u64 * TEXT_ATLAS_INSTANCE_SIZE
    }
}

impl RetainedTileVertexArena {
    pub(crate) fn has_capacity(
        &self,
        scene_vertices: usize,
        clip_vertices: usize,
        text_instances: usize,
    ) -> bool {
        self.scene_capacity_vertices >= scene_vertices
            && self.clip_capacity_vertices >= clip_vertices
            && self.text_instance_capacity >= text_instances
    }

    pub(crate) fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        scene_vertices: usize,
        clip_vertices: usize,
        text_instances: usize,
    ) {
        if self.has_capacity(scene_vertices, clip_vertices, text_instances) {
            return;
        }

        let scene_capacity =
            grow_retained_tile_vertex_capacity(self.scene_capacity_vertices, scene_vertices);
        let clip_capacity =
            grow_retained_tile_vertex_capacity(self.clip_capacity_vertices, clip_vertices);
        let text_capacity =
            grow_retained_tile_vertex_capacity(self.text_instance_capacity, text_instances);

        self.scene_buffer =
            create_empty_vertex_buffer(device, "SUI retained tile scene arena", scene_capacity);
        self.clip_buffer =
            create_empty_vertex_buffer(device, "SUI retained tile clip arena", clip_capacity);
        self.text_instance_buffer = create_empty_text_instance_buffer(
            device,
            "SUI retained tile text instance arena",
            text_capacity,
        );
        self.scene_capacity_vertices = scene_capacity;
        self.clip_capacity_vertices = clip_capacity;
        self.text_instance_capacity = text_capacity;
    }
}

impl RetainedTileUploadPlan {
    pub(crate) fn needs_rebuild(&self, arena: &RetainedTileVertexArena) -> bool {
        (arena.used_scene_vertices > 0
            || arena.used_clip_vertices > 0
            || arena.used_text_instances > 0)
            && !arena.has_capacity(
                arena.used_scene_vertices + self.appended_scene_vertices,
                arena.used_clip_vertices + self.appended_clip_vertices,
                arena.used_text_instances + self.appended_text_instances,
            )
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RetainedCompositorFrameStats {
    pub(crate) visible_layers: usize,
    pub(crate) visible_tiles: usize,
    pub(crate) reused_tiles: usize,
    pub(crate) regenerated_tiles: usize,
    pub(crate) direct_packets: usize,
    pub(crate) tile_memory_bytes: usize,
    pub(crate) tile_generation_time_ms: f64,
    pub(crate) composition_time_ms: f64,
    pub(crate) scene_traversal_time_ms: f64,
    pub(crate) packet_build_count: usize,
    pub(crate) packet_build_time_ms: f64,
    pub(crate) packet_rebuild_new_count: usize,
    pub(crate) packet_rebuild_coordinate_space_count: usize,
    pub(crate) packet_rebuild_signature_count: usize,
    pub(crate) packet_rebuild_scene_count: usize,
    pub(crate) packet_rebuild_state_count: usize,
    pub(crate) packet_normalize_time_ms: f64,
    pub(crate) packet_signature_time_ms: f64,
    pub(crate) packet_raster_state_init_time_ms: f64,
    pub(crate) packet_scene_build_time_ms: f64,
    pub(crate) packet_command_count: usize,
    pub(crate) packet_text_command_count: usize,
    pub(crate) packet_path_command_count: usize,
    pub(crate) packet_clip_path_command_count: usize,
    pub(crate) packet_image_command_count: usize,
    pub(crate) packet_rect_command_count: usize,
    pub(crate) packet_text_command_time_ms: f64,
    pub(crate) packet_path_command_time_ms: f64,
    pub(crate) packet_clip_path_command_time_ms: f64,
    pub(crate) packet_image_command_time_ms: f64,
    pub(crate) packet_rect_command_time_ms: f64,
    pub(crate) slowest_packet_build: Option<RetainedPacketBuildHotspot>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetainedPacketBuildHotspot {
    pub(crate) container_layer_id: Option<u64>,
    pub(crate) owner_widget_id: Option<u64>,
    pub(crate) segment_index: u32,
    pub(crate) total_time_ms: f64,
    pub(crate) scene_build_time_ms: f64,
    pub(crate) command_count: usize,
    pub(crate) text_command_count: usize,
    pub(crate) path_command_count: usize,
    pub(crate) rect_command_count: usize,
    pub(crate) text_command_time_ms: f64,
    pub(crate) path_command_time_ms: f64,
    pub(crate) rect_command_time_ms: f64,
    pub(crate) text_sample: Option<String>,
}

impl RetainedCompositorFrameStats {
    fn record_packet_rebuild(&mut self, reason: PacketRebuildReason) {
        match reason {
            PacketRebuildReason::NewPacket => self.packet_rebuild_new_count += 1,
            PacketRebuildReason::CoordinateSpace => {
                self.packet_rebuild_coordinate_space_count += 1;
            }
            PacketRebuildReason::Signature => self.packet_rebuild_signature_count += 1,
            PacketRebuildReason::Scene => self.packet_rebuild_scene_count += 1,
            PacketRebuildReason::State => self.packet_rebuild_state_count += 1,
        }
    }

    fn record_packet_build_diagnostics(
        &mut self,
        diagnostics: DirectPacketBuildDiagnostics,
        normalize_time_ms: f64,
        signature_time_ms: f64,
    ) {
        self.packet_normalize_time_ms += normalize_time_ms;
        self.packet_signature_time_ms += signature_time_ms;
        self.packet_raster_state_init_time_ms += diagnostics.raster_state_init_time_ms;
        self.packet_scene_build_time_ms += diagnostics.scene_build_time_ms;
        self.packet_command_count += diagnostics.command_count;
        self.packet_text_command_count += diagnostics.text_command_count;
        self.packet_path_command_count += diagnostics.path_command_count;
        self.packet_clip_path_command_count += diagnostics.clip_path_command_count;
        self.packet_image_command_count += diagnostics.image_command_count;
        self.packet_rect_command_count += diagnostics.rect_command_count;
        self.packet_text_command_time_ms += diagnostics.text_command_time_ms;
        self.packet_path_command_time_ms += diagnostics.path_command_time_ms;
        self.packet_clip_path_command_time_ms += diagnostics.clip_path_command_time_ms;
        self.packet_image_command_time_ms += diagnostics.image_command_time_ms;
        self.packet_rect_command_time_ms += diagnostics.rect_command_time_ms;
    }

    fn consider_packet_build_hotspot(&mut self, hotspot: RetainedPacketBuildHotspot) {
        let should_replace = self
            .slowest_packet_build
            .as_ref()
            .map(|current| hotspot.total_time_ms > current.total_time_ms)
            .unwrap_or(true);
        if should_replace {
            self.slowest_packet_build = Some(hotspot);
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetainedFrameSubmission {
    pub(crate) fragments: Vec<RetainedFrameFragment>,
}

#[derive(Debug)]
pub(crate) enum RetainedFrameFragment {
    Transient(DrawOpArena),
    Tile {
        address: TileAddress,
        clip_rect: Option<Rect>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionItem {
    Packet(RetainedPacketId),
    Layer(SceneLayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionPhase {
    Normal,
    Overlay,
    Effect,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TransformNode {
    id: TransformNodeId,
    parent: Option<TransformNodeId>,
    local: Transform,
    world: Transform,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedClipPrimitive {
    Rect(Rect),
    Path {
        path: ScenePath,
        bounds: Rect,
        signature: u64,
    },
}

#[allow(dead_code)]
impl ResolvedClipPrimitive {
    pub(crate) fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            Self::Path { bounds, .. } => *bounds,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ClipNode {
    id: ClipNodeId,
    parent: Option<ClipNodeId>,
    primitive: Option<ResolvedClipPrimitive>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct EffectNode {
    id: EffectNodeId,
    parent: Option<EffectNodeId>,
    composition_mode: sui_scene::LayerCompositionMode,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedRasterState {
    pub(crate) current_transform: Transform,
    pub(crate) clip_stack: Vec<ResolvedClipPrimitive>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    effect_node: EffectNodeId,
}

impl ResolvedRasterState {
    fn signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hash_transform(&mut hasher, self.current_transform);
        self.transform_node.hash(&mut hasher);
        self.clip_node.hash(&mut hasher);
        self.effect_node.hash(&mut hasher);
        for clip in &self.clip_stack {
            match clip {
                ResolvedClipPrimitive::Rect(rect) => {
                    0u8.hash(&mut hasher);
                    hash_rect(&mut hasher, *rect);
                }
                ResolvedClipPrimitive::Path {
                    bounds, signature, ..
                } => {
                    1u8.hash(&mut hasher);
                    hash_rect(&mut hasher, *bounds);
                    signature.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}

fn clip_stack_signature(clips: &[ResolvedClipPrimitive]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for clip in clips {
        match clip {
            ResolvedClipPrimitive::Rect(rect) => {
                0u8.hash(&mut hasher);
                hash_rect(&mut hasher, *rect);
            }
            ResolvedClipPrimitive::Path {
                bounds, signature, ..
            } => {
                1u8.hash(&mut hasher);
                hash_rect(&mut hasher, *bounds);
                signature.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

fn normalized_clip_stack_signature(
    clips: &[ResolvedClipPrimitive],
    normalization_origin: Vector,
) -> u64 {
    if normalization_origin == Vector::ZERO {
        return clip_stack_signature(clips);
    }

    let delta = Vector::new(-normalization_origin.x, -normalization_origin.y);
    let normalized = clips
        .iter()
        .cloned()
        .map(|clip| translate_resolved_clip_primitive(clip, delta))
        .collect::<Vec<_>>();
    clip_stack_signature(&normalized)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct RetainedDirectPacket {
    pub(crate) id: RetainedPacketId,
    pub(crate) scene: Scene,
    pub(crate) initial_state: ResolvedRasterState,
    pub(crate) signature: u64,
    coordinate_space: PacketCoordinateSpace,
    pub(crate) draw_ops: DrawOpArena,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketCoordinateSpace {
    World,
    LayerLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketRebuildReason {
    NewPacket,
    CoordinateSpace,
    Signature,
    Scene,
    State,
}

#[derive(Debug, Clone, Default)]
struct RetainedRootNode {
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    structure_version: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RetainedLayer {
    pub(crate) descriptor: sui_scene::SceneLayerDescriptor,
    pub(crate) parent: Option<SceneLayerId>,
    pub(crate) children: Vec<SceneLayerId>,
    items: Vec<CompositionItem>,
    pub(crate) packet_ids: Vec<RetainedPacketId>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    pub(crate) clip_signature: u64,
    effect_node: EffectNodeId,
    pub(crate) render_mode: RetainedLayerRenderMode,
    pub(crate) content_version: u64,
    pub(crate) structure_version: u64,
    tile_grid: Option<TileGrid>,
    pub(crate) visible_tiles: Vec<TileAddress>,
}

#[derive(Debug, Clone)]
struct PacketSnapshot {
    id: RetainedPacketId,
    scene: Scene,
    initial_state: ResolvedRasterState,
}

#[derive(Debug, Clone)]
struct LayerSnapshot {
    descriptor: sui_scene::SceneLayerDescriptor,
    parent: Option<SceneLayerId>,
    children: Vec<SceneLayerId>,
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    packets: Vec<PacketSnapshot>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    clip_signature: u64,
    effect_node: EffectNodeId,
}

#[derive(Debug, Clone, Default)]
struct RootSnapshot {
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    packets: Vec<PacketSnapshot>,
}

#[derive(Debug, Clone, Default)]
struct CompositorSnapshot {
    root: RootSnapshot,
    layers: HashMap<SceneLayerId, LayerSnapshot>,
}

#[derive(Debug, Clone)]
struct CompositionTraversalState {
    current_transform: Transform,
    transform_node: TransformNodeId,
    transform_stack: Vec<(Transform, TransformNodeId)>,
    clip_stack: Vec<(ResolvedClipPrimitive, ClipNodeId)>,
    effect_node: EffectNodeId,
}

impl Default for CompositionTraversalState {
    fn default() -> Self {
        Self {
            current_transform: Transform::IDENTITY,
            transform_node: TransformNodeId::ROOT,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
            effect_node: EffectNodeId::ROOT,
        }
    }
}

impl CompositionTraversalState {
    fn resolved_state(&self) -> ResolvedRasterState {
        ResolvedRasterState {
            current_transform: self.current_transform,
            clip_stack: self
                .clip_stack
                .iter()
                .map(|(primitive, _)| primitive.clone())
                .collect(),
            transform_node: self.transform_node,
            clip_node: self
                .clip_stack
                .last()
                .map(|(_, node_id)| *node_id)
                .unwrap_or(ClipNodeId::ROOT),
            effect_node: self.effect_node,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetainedCompositorState {
    root: RetainedRootNode,
    pub(crate) layers: HashMap<SceneLayerId, RetainedLayer>,
    pub(crate) packets: HashMap<RetainedPacketId, RetainedDirectPacket>,
    pub(crate) tiles: HashMap<TileAddress, TileEntry>,
    transforms: HashMap<TransformNodeId, TransformNode>,
    clips: HashMap<ClipNodeId, ClipNode>,
    effects: HashMap<EffectNodeId, EffectNode>,
    pub(crate) next_transform_node: u64,
    pub(crate) next_clip_node: u64,
    pub(crate) next_effect_node: u64,
    pub(crate) viewport: Size,
    pub(crate) feather_width_bits: u32,
    pub(crate) frame_index: u64,
    pub(crate) tile_budget_bytes: usize,
    pub(crate) last_frame_stats: RetainedCompositorFrameStats,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) path_cache: PathMeshCache,
}

impl Default for RetainedCompositorState {
    fn default() -> Self {
        Self {
            root: RetainedRootNode::default(),
            layers: HashMap::new(),
            packets: HashMap::new(),
            tiles: HashMap::new(),
            transforms: HashMap::new(),
            clips: HashMap::new(),
            effects: HashMap::new(),
            next_transform_node: 0,
            next_clip_node: 0,
            next_effect_node: 0,
            viewport: Size::ZERO,
            feather_width_bits: 0,
            frame_index: 0,
            tile_budget_bytes: TILE_CACHE_BUDGET_BYTES,
            last_frame_stats: RetainedCompositorFrameStats::default(),
            diagnostics_enabled: true,
            path_cache: PathMeshCache::default(),
        }
    }
}

impl RetainedCompositorState {
    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
        self.path_cache.set_diagnostics_enabled(enabled);
        if !enabled {
            self.last_frame_stats = RetainedCompositorFrameStats::default();
        }
    }

    #[cfg(test)]
    pub(crate) fn prepare_frame(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<DrawOpArena> {
        let mut frame_stats = self.refresh_frame_state(frame, text_engine, feather_width)?;
        let composition_started = self.diagnostics_enabled.then(|| Instant::now());
        let draw_ops = self.compose_draw_ops(frame.viewport, &mut frame_stats)?;
        self.finish_frame(
            frame.viewport,
            feather_width,
            &mut frame_stats,
            composition_started,
        );
        Ok(draw_ops)
    }

    pub(crate) fn prepare_frame_submission(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<RetainedFrameSubmission> {
        let mut frame_stats = self.refresh_frame_state(frame, text_engine, feather_width)?;
        let composition_started = self.diagnostics_enabled.then(|| Instant::now());
        let submission = self.compose_submission(frame.viewport, &mut frame_stats)?;
        self.finish_frame(
            frame.viewport,
            feather_width,
            &mut frame_stats,
            composition_started,
        );
        Ok(submission)
    }

    fn refresh_frame_state(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<RetainedCompositorFrameStats> {
        let viewport_changed = self.viewport != frame.viewport;
        let feather_changed = self.feather_width_bits != feather_width.to_bits();
        self.frame_index = self.frame_index.wrapping_add(1);
        let mut frame_stats = RetainedCompositorFrameStats::default();
        let scene_traversal_started = self.diagnostics_enabled.then(|| Instant::now());
        let snapshot = self.build_snapshot(&frame.scene)?;
        if let Some(started) = scene_traversal_started {
            frame_stats.scene_traversal_time_ms = started.elapsed().as_secs_f64() * 1000.0;
        }
        let tile_generation_started = self.diagnostics_enabled.then(|| Instant::now());
        self.apply_snapshot(
            frame,
            snapshot,
            text_engine,
            feather_width,
            viewport_changed || feather_changed,
            &mut frame_stats,
        )?;
        if let Some(started) = tile_generation_started {
            frame_stats.tile_generation_time_ms = started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(frame_stats)
    }

    fn finish_frame(
        &mut self,
        viewport: Size,
        feather_width: f32,
        frame_stats: &mut RetainedCompositorFrameStats,
        composition_started: Option<Instant>,
    ) {
        if let Some(started) = composition_started {
            frame_stats.composition_time_ms = started.elapsed().as_secs_f64() * 1000.0;
            frame_stats.tile_memory_bytes = self.total_tile_memory_bytes();
            self.last_frame_stats = frame_stats.clone();
        } else {
            self.last_frame_stats = RetainedCompositorFrameStats::default();
        }
        self.viewport = viewport;
        self.feather_width_bits = feather_width.to_bits();
    }

    fn build_snapshot(&mut self, scene: &Scene) -> Result<CompositorSnapshot> {
        self.reset_property_trees();
        let mut snapshot = CompositorSnapshot::default();
        snapshot.root = self.build_container_snapshot(
            CompositionContainerId::Root,
            scene,
            CompositionTraversalState::default(),
            &mut snapshot,
            None,
        )?;
        Ok(snapshot)
    }

    fn build_container_snapshot(
        &mut self,
        container: CompositionContainerId,
        scene: &Scene,
        mut state: CompositionTraversalState,
        snapshot: &mut CompositorSnapshot,
        parent_layer: Option<SceneLayerId>,
    ) -> Result<RootSnapshot> {
        let mut result = RootSnapshot::default();
        let mut normal_items = Vec::new();
        let mut overlay_items = Vec::new();
        let mut effect_items = Vec::new();
        let mut segment_scene = Scene::new();
        let mut segment_start = None::<ResolvedRasterState>;

        for command in scene.commands() {
            match command {
                SceneCommand::Layer(layer) => {
                    flush_container_segment(
                        &self.effects,
                        container,
                        &mut result,
                        &mut normal_items,
                        &mut overlay_items,
                        &mut effect_items,
                        &mut segment_scene,
                        &mut segment_start,
                    );

                    let mut child_state = state.clone();
                    child_state.effect_node = self.push_effect_node(
                        Some(state.effect_node),
                        layer.descriptor.composition_mode,
                    );
                    if layer.descriptor.composition_mode == sui_scene::LayerCompositionMode::Scroll
                        || layer.descriptor.is_stack_surface
                    {
                        let clip = ResolvedClipPrimitive::Rect(layer.descriptor.bounds);
                        let parent = child_state
                            .clip_stack
                            .last()
                            .map(|(_, node_id)| *node_id)
                            .unwrap_or(ClipNodeId::ROOT);
                        let node_id = self.push_clip_node(Some(parent), clip.clone());
                        child_state.clip_stack.push((clip, node_id));
                    }
                    let phase =
                        composition_phase_for_effect_node(child_state.effect_node, &self.effects);
                    let layer_snapshot =
                        self.build_layer_snapshot(layer, parent_layer, child_state, snapshot)?;
                    push_composition_item(
                        phase,
                        CompositionItem::Layer(layer.layer_id()),
                        &mut normal_items,
                        &mut overlay_items,
                        &mut effect_items,
                    );
                    snapshot.layers.insert(layer.layer_id(), layer_snapshot);
                }
                _ => {
                    if segment_start.is_none() {
                        segment_start = Some(state.resolved_state());
                    }
                    segment_scene.push(command.clone());
                    self.apply_command_to_traversal_state(command, &mut state);
                }
            }
        }

        flush_container_segment(
            &self.effects,
            container,
            &mut result,
            &mut normal_items,
            &mut overlay_items,
            &mut effect_items,
            &mut segment_scene,
            &mut segment_start,
        );
        result.items = normal_items;
        result.items.extend(overlay_items);
        result.items.extend(effect_items);
        Ok(result)
    }

    fn build_layer_snapshot(
        &mut self,
        layer: &SceneLayer,
        parent_layer: Option<SceneLayerId>,
        state: CompositionTraversalState,
        snapshot: &mut CompositorSnapshot,
    ) -> Result<LayerSnapshot> {
        let inherited_state = state.resolved_state();
        let container = self.build_container_snapshot(
            CompositionContainerId::Layer(layer.layer_id()),
            &layer.scene,
            state,
            snapshot,
            Some(layer.layer_id()),
        )?;
        let layer_local_coordinate_space = layer.descriptor.composition_mode
            == sui_scene::LayerCompositionMode::Scroll
            || layer.descriptor.cache_policy == sui_scene::LayerCachePolicy::Direct;
        let clip_signature = if layer_local_coordinate_space {
            normalized_clip_stack_signature(
                &inherited_state.clip_stack,
                layer.descriptor.bounds.origin.to_vector(),
            )
        } else {
            clip_stack_signature(&inherited_state.clip_stack)
        };
        let children = container
            .items
            .iter()
            .filter_map(|item| match item {
                CompositionItem::Layer(layer_id) => Some(*layer_id),
                CompositionItem::Packet(_) => None,
            })
            .collect();

        Ok(LayerSnapshot {
            descriptor: layer.descriptor.clone(),
            parent: parent_layer,
            children,
            items: container.items,
            packet_ids: container.packet_ids,
            packets: container.packets,
            transform_node: inherited_state.transform_node,
            clip_node: inherited_state.clip_node,
            clip_signature,
            effect_node: inherited_state.effect_node,
        })
    }

    fn apply_command_to_traversal_state(
        &mut self,
        command: &SceneCommand,
        state: &mut CompositionTraversalState,
    ) {
        match command {
            SceneCommand::PushTransform { transform } => {
                let parent_world = state.current_transform;
                let parent_node = state.transform_node;
                state.transform_stack.push((parent_world, parent_node));
                let world = parent_world.then(*transform);
                state.current_transform = world;
                state.transform_node =
                    self.push_transform_node(Some(parent_node), *transform, world);
            }
            SceneCommand::PopTransform => {
                let (world, node_id) = state
                    .transform_stack
                    .pop()
                    .unwrap_or((Transform::IDENTITY, TransformNodeId::ROOT));
                state.current_transform = world;
                state.transform_node = node_id;
            }
            SceneCommand::PushClip { rect } => {
                let clip =
                    ResolvedClipPrimitive::Rect(state.current_transform.transform_rect_bbox(*rect));
                let parent = state
                    .clip_stack
                    .last()
                    .map(|(_, node_id)| *node_id)
                    .unwrap_or(ClipNodeId::ROOT);
                let node_id = self.push_clip_node(Some(parent), clip.clone());
                state.clip_stack.push((clip, node_id));
            }
            SceneCommand::PushClipPath { path } => {
                let transformed_path = transform_scene_path(path, state.current_transform);
                let bounds = transformed_path.bounds();
                let clip = ResolvedClipPrimitive::Path {
                    signature: hash_path(&transformed_path, Transform::IDENTITY),
                    path: transformed_path,
                    bounds,
                };
                let parent = state
                    .clip_stack
                    .last()
                    .map(|(_, node_id)| *node_id)
                    .unwrap_or(ClipNodeId::ROOT);
                let node_id = self.push_clip_node(Some(parent), clip.clone());
                state.clip_stack.push((clip, node_id));
            }
            SceneCommand::PopClip => {
                let _ = state.clip_stack.pop();
            }
            SceneCommand::Clear(_)
            | SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawText(_)
            | SceneCommand::DrawShapedText(_)
            | SceneCommand::DrawImage { .. }
            | SceneCommand::Layer(_)
            | SceneCommand::Label { .. } => {}
        }
    }

    fn apply_snapshot(
        &mut self,
        frame: &SceneFrame,
        snapshot: CompositorSnapshot,
        text_engine: &mut TextEngine,
        feather_width: f32,
        global_rebuild: bool,
        frame_stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        let render_modes = snapshot
            .layers
            .iter()
            .map(|(layer_id, layer)| {
                (
                    *layer_id,
                    resolve_layer_render_mode(&layer.descriptor, frame.scale_factor),
                )
            })
            .collect::<HashMap<_, _>>();
        let cached_roots = snapshot
            .layers
            .keys()
            .copied()
            .filter(|layer_id| {
                render_modes.get(layer_id) == Some(&RetainedLayerRenderMode::CachedTiles)
                    && nearest_cached_root(
                        snapshot.layers.get(layer_id).and_then(|layer| layer.parent),
                        &snapshot.layers,
                        &render_modes,
                    )
                    .is_none()
            })
            .collect::<HashSet<_>>();
        let cached_tile_owners = snapshot
            .layers
            .keys()
            .copied()
            .map(|layer_id| {
                (
                    layer_id,
                    owning_cached_root(Some(layer_id), &snapshot.layers, &cached_roots),
                )
            })
            .collect::<HashMap<_, _>>();
        let previous_layers = self.layers.clone();
        let layer_translation_deltas = snapshot
            .layers
            .iter()
            .filter_map(|(layer_id, layer_snapshot)| {
                previous_layers.get(layer_id).and_then(|previous| {
                    descriptor_translation_delta(&previous.descriptor, &layer_snapshot.descriptor)
                        .map(|delta| (*layer_id, delta))
                })
            })
            .collect::<HashMap<_, _>>();
        let mut packet_dirty_layers = HashSet::new();
        let mut tiled_damage = HashMap::<SceneLayerId, Option<Rect>>::new();
        let mut cached_scroll_translations = HashMap::<SceneLayerId, Vector>::new();
        let mut cached_scroll_translation_conflicts = HashSet::<SceneLayerId>::new();
        let current_layers = snapshot.layers.keys().copied().collect::<HashSet<_>>();
        let mut root_dirty = global_rebuild;

        for update in &frame.layer_updates {
            if !current_layers.contains(&update.layer_id) {
                if matches!(
                    update.kind,
                    SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources
                ) {
                    if let Some(cached_root) =
                        fallback_cached_root_for_update(update, &snapshot.layers, &cached_roots)
                    {
                        let damage_rect = update.damage.unwrap_or(update.content_bounds);
                        merge_damage_rect(&mut tiled_damage, cached_root, Some(damage_rect));
                    }
                }
                root_dirty = true;
                continue;
            }

            if let Some(cached_root) = cached_tile_owners.get(&update.layer_id).copied().flatten() {
                let cached_root_is_scroll = render_modes.get(&cached_root)
                    == Some(&RetainedLayerRenderMode::CachedTiles)
                    && snapshot.layers.get(&cached_root).is_some_and(|layer| {
                        layer.descriptor.composition_mode == sui_scene::LayerCompositionMode::Scroll
                    });
                if update.kind == SceneLayerUpdateKind::Transform
                    && cached_root == update.layer_id
                    && render_modes[&cached_root] == RetainedLayerRenderMode::CachedTiles
                {
                    let translation_delta = layer_translation_deltas.get(&cached_root).copied();

                    if let Some(delta) = translation_delta {
                        if !register_cached_scroll_translation(
                            &mut cached_scroll_translations,
                            &mut cached_scroll_translation_conflicts,
                            cached_root,
                            delta,
                        ) {
                            packet_dirty_layers.insert(cached_root);
                            merge_damage_rect(
                                &mut tiled_damage,
                                cached_root,
                                update.damage.or(Some(update.paint_bounds)),
                            );
                        }
                    } else if !cached_root_is_scroll {
                        packet_dirty_layers.insert(cached_root);
                        merge_damage_rect(
                            &mut tiled_damage,
                            cached_root,
                            update.damage.or(Some(update.paint_bounds)),
                        );
                    }

                    continue;
                }

                if matches!(
                    update.kind,
                    SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources
                ) || cached_root != update.layer_id
                {
                    packet_dirty_layers.insert(cached_root);
                    merge_damage_rect(
                        &mut tiled_damage,
                        cached_root,
                        update.damage.or(Some(update.paint_bounds)),
                    );
                }

                continue;
            }

            match update.kind {
                SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources => {
                    packet_dirty_layers.insert(update.layer_id);
                }
                SceneLayerUpdateKind::Ordering
                | SceneLayerUpdateKind::Transform
                | SceneLayerUpdateKind::Clip
                | SceneLayerUpdateKind::Effect
                | SceneLayerUpdateKind::Visibility => {}
            }
        }

        let mut valid_packets = HashSet::new();
        valid_packets.extend(snapshot.root.packet_ids.iter().copied());
        for layer in snapshot.layers.values() {
            valid_packets.extend(layer.packet_ids.iter().copied());
        }

        if self.root.items != snapshot.root.items
            || self.root.packet_ids != snapshot.root.packet_ids
        {
            self.root.structure_version = self.root.structure_version.wrapping_add(1);
        }
        self.root.items = snapshot.root.items.clone();
        self.root.packet_ids = snapshot.root.packet_ids.clone();

        for packet in snapshot.root.packets {
            self.upsert_packet(
                frame,
                packet,
                root_dirty,
                PacketCoordinateSpace::World,
                Vector::ZERO,
                text_engine,
                feather_width,
                frame_stats,
            )?;
        }

        let snapshot_layers = snapshot.layers.clone();
        self.layers
            .retain(|layer_id, _| current_layers.contains(layer_id));

        let mut structure_dirty_layers = HashSet::new();

        for (layer_id, layer_snapshot) in snapshot.layers {
            let translation_delta = cached_scroll_translations
                .get(&layer_id)
                .copied()
                .or_else(|| {
                    previous_layers.get(&layer_id).and_then(|previous| {
                        descriptor_translation_delta(
                            &previous.descriptor,
                            &layer_snapshot.descriptor,
                        )
                    })
                });
            let translated_only = translation_delta.is_some();
            let scroll_translated_cached_layer = cached_scroll_translations.contains_key(&layer_id)
                && render_modes[&layer_id] == RetainedLayerRenderMode::CachedTiles
                && layer_snapshot.descriptor.composition_mode
                    == sui_scene::LayerCompositionMode::Scroll;
            let structure_changed = previous_layers.get(&layer_id).is_none_or(|previous| {
                previous.parent != layer_snapshot.parent
                    || (!scroll_translated_cached_layer
                        && previous.children != layer_snapshot.children)
                    || (!scroll_translated_cached_layer
                        && previous.items != layer_snapshot.items)
                    || (!scroll_translated_cached_layer
                        && previous.packet_ids != layer_snapshot.packet_ids)
                    || previous.transform_node != layer_snapshot.transform_node
                    || (render_modes[&layer_id] != RetainedLayerRenderMode::Direct
                        && previous.clip_signature != layer_snapshot.clip_signature)
                    || previous.effect_node != layer_snapshot.effect_node
                    || (!translated_only && previous.descriptor != layer_snapshot.descriptor)
            });

            let content_changed = packet_dirty_layers.contains(&layer_id)
                || previous_layers.get(&layer_id).is_none_or(|previous| {
                    !translated_only
                        && previous.descriptor.bounds != layer_snapshot.descriptor.bounds
                });

            if let Some(delta) = translation_delta {
                if !global_rebuild
                    && previous_layers.get(&layer_id).is_some_and(|previous| {
                        previous.render_mode == RetainedLayerRenderMode::CachedTiles
                    })
                    && render_modes[&layer_id] == RetainedLayerRenderMode::CachedTiles
                {
                    translate_cached_layer_tiles(&mut self.tiles, layer_id, delta, frame.viewport);
                }
            }

            let previous = previous_layers.get(&layer_id);
            let retained = self
                .layers
                .entry(layer_id)
                .or_insert_with(|| RetainedLayer {
                    descriptor: layer_snapshot.descriptor.clone(),
                    parent: layer_snapshot.parent,
                    children: layer_snapshot.children.clone(),
                    items: layer_snapshot.items.clone(),
                    packet_ids: layer_snapshot.packet_ids.clone(),
                    transform_node: layer_snapshot.transform_node,
                    clip_node: layer_snapshot.clip_node,
                    clip_signature: layer_snapshot.clip_signature,
                    effect_node: layer_snapshot.effect_node,
                    render_mode: render_modes[&layer_id],
                    content_version: 0,
                    structure_version: 0,
                    tile_grid: None,
                    visible_tiles: Vec::new(),
                });

            if structure_changed {
                retained.structure_version = previous
                    .map_or(retained.structure_version + 1, |old| {
                        old.structure_version.wrapping_add(1)
                    });
                structure_dirty_layers.insert(layer_id);
            }
            if content_changed {
                retained.content_version = previous.map_or(retained.content_version + 1, |old| {
                    old.content_version.wrapping_add(1)
                });
            }

            retained.descriptor = layer_snapshot.descriptor.clone();
            retained.parent = layer_snapshot.parent;
            retained.children = layer_snapshot.children.clone();
            retained.items = layer_snapshot.items.clone();
            retained.packet_ids = layer_snapshot.packet_ids.clone();
            retained.transform_node = layer_snapshot.transform_node;
            retained.clip_node = layer_snapshot.clip_node;
            retained.clip_signature = layer_snapshot.clip_signature;
            retained.effect_node = layer_snapshot.effect_node;
            retained.render_mode = render_modes[&layer_id];
            if retained.render_mode != RetainedLayerRenderMode::CachedTiles {
                retained.tile_grid = None;
                retained.visible_tiles.clear();
            }

            let packet_dirty =
                global_rebuild || structure_changed || packet_dirty_layers.contains(&layer_id);
            let coordinate_space = if layer_snapshot.descriptor.composition_mode
                == sui_scene::LayerCompositionMode::Scroll
                || layer_snapshot.descriptor.cache_policy == sui_scene::LayerCachePolicy::Direct
            {
                PacketCoordinateSpace::LayerLocal
            } else {
                PacketCoordinateSpace::World
            };
            let normalization_origin = if coordinate_space == PacketCoordinateSpace::LayerLocal {
                layer_snapshot.descriptor.bounds.origin.to_vector()
            } else {
                Vector::ZERO
            };
            for packet in layer_snapshot.packets {
                self.upsert_packet(
                    frame,
                    packet,
                    packet_dirty,
                    coordinate_space,
                    normalization_origin,
                    text_engine,
                    feather_width,
                    frame_stats,
                )?;
            }
        }

        self.packets
            .retain(|packet_id, _| valid_packets.contains(packet_id));
        self.update_tile_cache(
            frame,
            &snapshot_layers,
            &cached_roots,
            &tiled_damage,
            &structure_dirty_layers,
            global_rebuild,
            text_engine,
            feather_width,
            frame_stats,
        )?;
        Ok(())
    }

    fn upsert_packet(
        &mut self,
        frame: &SceneFrame,
        snapshot: PacketSnapshot,
        _forced_dirty: bool,
        coordinate_space: PacketCoordinateSpace,
        normalization_origin: Vector,
        text_engine: &mut TextEngine,
        feather_width: f32,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        let normalize_started = self.diagnostics_enabled.then(Instant::now);
        let snapshot = normalize_packet_snapshot(snapshot, coordinate_space, normalization_origin);
        let normalize_time_ms = normalize_started
            .map(|started| started.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        let signature_started = self.diagnostics_enabled.then(Instant::now);
        let signature = packet_signature(
            &snapshot.scene,
            &snapshot.initial_state,
            frame.viewport,
            feather_width,
        );
        let signature_time_ms = signature_started
            .map(|started| started.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        let rebuild_reason = match self.packets.get(&snapshot.id) {
            None => Some(PacketRebuildReason::NewPacket),
            Some(packet) if packet.coordinate_space != coordinate_space => {
                Some(PacketRebuildReason::CoordinateSpace)
            }
            Some(packet) if packet.signature != signature => {
                if self.diagnostics_enabled {
                    if packet.scene != snapshot.scene {
                        Some(PacketRebuildReason::Scene)
                    } else if packet.initial_state != snapshot.initial_state {
                        Some(PacketRebuildReason::State)
                    } else {
                        Some(PacketRebuildReason::Signature)
                    }
                } else {
                    Some(PacketRebuildReason::Signature)
                }
            }
            Some(packet) if packet.scene != snapshot.scene => Some(PacketRebuildReason::Scene),
            Some(packet) if packet.initial_state != snapshot.initial_state => {
                Some(PacketRebuildReason::State)
            }
            Some(_) => None,
        };

        if let Some(reason) = rebuild_reason {
            let packet_build_started = self.diagnostics_enabled.then(|| Instant::now());
            let (draw_ops, diagnostics) = build_direct_packet_with_diagnostics(
                frame,
                &snapshot.scene,
                &snapshot.initial_state,
                text_engine,
                &mut self.path_cache,
                feather_width,
            )?;
            stats.record_packet_rebuild(reason);
            stats.record_packet_build_diagnostics(
                diagnostics,
                normalize_time_ms,
                signature_time_ms,
            );
            if let Some(started) = packet_build_started {
                let total_time_ms = started.elapsed().as_secs_f64() * 1000.0;
                stats.packet_build_count += 1;
                stats.packet_build_time_ms += total_time_ms;
                let (container_layer_id, owner_widget_id) = match snapshot.id.container {
                    CompositionContainerId::Root => (None, None),
                    CompositionContainerId::Layer(layer_id) => (
                        Some(layer_id.get()),
                        self.layers
                            .get(&layer_id)
                            .map(|layer| layer.descriptor.owner.get()),
                    ),
                };
                stats.consider_packet_build_hotspot(RetainedPacketBuildHotspot {
                    container_layer_id,
                    owner_widget_id,
                    segment_index: snapshot.id.segment_index,
                    total_time_ms,
                    scene_build_time_ms: diagnostics.scene_build_time_ms,
                    command_count: diagnostics.command_count,
                    text_command_count: diagnostics.text_command_count,
                    path_command_count: diagnostics.path_command_count,
                    rect_command_count: diagnostics.rect_command_count,
                    text_command_time_ms: diagnostics.text_command_time_ms,
                    path_command_time_ms: diagnostics.path_command_time_ms,
                    rect_command_time_ms: diagnostics.rect_command_time_ms,
                    text_sample: packet_text_sample(&snapshot.scene),
                });
            }
            self.packets.insert(
                snapshot.id,
                RetainedDirectPacket {
                    id: snapshot.id,
                    scene: snapshot.scene,
                    initial_state: snapshot.initial_state,
                    signature,
                    coordinate_space,
                    draw_ops,
                },
            );
        }

        Ok(())
    }

    #[cfg(test)]
    fn compose_draw_ops(
        &self,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<DrawOpArena> {
        let mut draw_ops = DrawOpArena::default();
        for phase in [
            CompositionPhase::Normal,
            CompositionPhase::Overlay,
            CompositionPhase::Effect,
        ] {
            self.append_items_for_phase(&self.root.items, phase, &mut draw_ops, viewport, stats)?;
        }
        Ok(draw_ops)
    }

    fn compose_submission(
        &self,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<RetainedFrameSubmission> {
        let mut submission = RetainedFrameSubmission {
            fragments: Vec::new(),
        };
        let mut current = DrawOpArena::default();
        for phase in [
            CompositionPhase::Normal,
            CompositionPhase::Overlay,
            CompositionPhase::Effect,
        ] {
            self.append_items_to_submission_for_phase(
                &self.root.items,
                phase,
                &mut current,
                &mut submission,
                viewport,
                stats,
            )?;
        }
        flush_transient_fragment(&mut submission, &mut current);
        Ok(submission)
    }

    #[cfg(test)]
    fn append_items_for_phase(
        &self,
        items: &[CompositionItem],
        phase: CompositionPhase,
        draw_ops: &mut DrawOpArena,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        for item in items {
            match item {
                CompositionItem::Packet(packet_id) => {
                    if let Some(packet) = self.packets.get(packet_id) {
                        if composition_phase_for_effect_node(
                            packet.initial_state.effect_node,
                            &self.effects,
                        ) != phase
                        {
                            continue;
                        }
                        match packet.coordinate_space {
                            PacketCoordinateSpace::World => {
                                draw_ops.append_fragment(&packet.draw_ops);
                            }
                            PacketCoordinateSpace::LayerLocal => {
                                let (origin, clip_stack) = match packet.id.container {
                                    CompositionContainerId::Root => (Vector::ZERO, Vec::new()),
                                    CompositionContainerId::Layer(layer_id) => self
                                        .layers
                                        .get(&layer_id)
                                        .map(|layer| {
                                            (
                                                layer.descriptor.bounds.origin.to_vector(),
                                                resolved_clip_primitives(
                                                    layer.clip_node,
                                                    &self.clips,
                                                ),
                                            )
                                        })
                                        .unwrap_or((Vector::ZERO, Vec::new())),
                                };
                                draw_ops.append_composed_fragment(
                                    &packet.draw_ops,
                                    origin,
                                    &clip_stack,
                                    viewport,
                                )?;
                            }
                        }
                        stats.direct_packets += 1;
                    }
                }
                CompositionItem::Layer(layer_id) => {
                    if let Some(layer) = self.layers.get(layer_id) {
                        match layer.render_mode {
                            RetainedLayerRenderMode::Direct => {
                                stats.visible_layers += 1;
                                self.append_items_for_phase(
                                    &layer.items,
                                    phase,
                                    draw_ops,
                                    viewport,
                                    stats,
                                )?;
                            }
                            RetainedLayerRenderMode::CachedTiles => {
                                let layer_phase = composition_phase_for_effect_node(
                                    layer.effect_node,
                                    &self.effects,
                                );
                                if layer_phase != phase {
                                    self.append_items_for_phase(
                                        &layer.items,
                                        phase,
                                        draw_ops,
                                        viewport,
                                        stats,
                                    )?;
                                    continue;
                                }
                                if !layer.visible_tiles.is_empty() {
                                    stats.visible_layers += 1;
                                }
                                for tile in &layer.visible_tiles {
                                    if let Some(entry) = self.tiles.get(tile) {
                                        let TilePayload::DirectPacket(fragment) = &entry.payload;
                                        draw_ops.append_fragment(fragment);
                                        stats.visible_tiles += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn append_items_to_submission_for_phase(
        &self,
        items: &[CompositionItem],
        phase: CompositionPhase,
        current: &mut DrawOpArena,
        submission: &mut RetainedFrameSubmission,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        for item in items {
            match item {
                CompositionItem::Packet(packet_id) => {
                    if let Some(packet) = self.packets.get(packet_id) {
                        if composition_phase_for_effect_node(
                            packet.initial_state.effect_node,
                            &self.effects,
                        ) != phase
                        {
                            continue;
                        }
                        match packet.coordinate_space {
                            PacketCoordinateSpace::World => {
                                current.append_fragment(&packet.draw_ops);
                            }
                            PacketCoordinateSpace::LayerLocal => {
                                let (origin, clip_stack) = match packet.id.container {
                                    CompositionContainerId::Root => (Vector::ZERO, Vec::new()),
                                    CompositionContainerId::Layer(layer_id) => self
                                        .layers
                                        .get(&layer_id)
                                        .map(|layer| {
                                            (
                                                layer.descriptor.bounds.origin.to_vector(),
                                                resolved_clip_primitives(
                                                    layer.clip_node,
                                                    &self.clips,
                                                ),
                                            )
                                        })
                                        .unwrap_or((Vector::ZERO, Vec::new())),
                                };
                                current.append_composed_fragment(
                                    &packet.draw_ops,
                                    origin,
                                    &clip_stack,
                                    viewport,
                                )?;
                            }
                        }
                        stats.direct_packets += 1;
                    }
                }
                CompositionItem::Layer(layer_id) => {
                    if let Some(layer) = self.layers.get(layer_id) {
                        match layer.render_mode {
                            RetainedLayerRenderMode::Direct => {
                                stats.visible_layers += 1;
                                self.append_items_to_submission_for_phase(
                                    &layer.items,
                                    phase,
                                    current,
                                    submission,
                                    viewport,
                                    stats,
                                )?;
                            }
                            RetainedLayerRenderMode::CachedTiles => {
                                let layer_phase = composition_phase_for_effect_node(
                                    layer.effect_node,
                                    &self.effects,
                                );
                                if layer_phase != phase {
                                    self.append_items_to_submission_for_phase(
                                        &layer.items,
                                        phase,
                                        current,
                                        submission,
                                        viewport,
                                        stats,
                                    )?;
                                    continue;
                                }
                                if !layer.visible_tiles.is_empty() {
                                    stats.visible_layers += 1;
                                }
                                let clip_rect = resolved_clip_bounds(layer.clip_node, &self.clips);
                                for tile in &layer.visible_tiles {
                                    if self.tiles.contains_key(tile) {
                                        flush_transient_fragment(submission, current);
                                        submission.fragments.push(RetainedFrameFragment::Tile {
                                            address: *tile,
                                            clip_rect,
                                        });
                                        stats.visible_tiles += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn update_tile_cache(
        &mut self,
        frame: &SceneFrame,
        snapshot_layers: &HashMap<SceneLayerId, LayerSnapshot>,
        cached_roots: &HashSet<SceneLayerId>,
        tiled_damage: &HashMap<SceneLayerId, Option<Rect>>,
        structure_dirty_layers: &HashSet<SceneLayerId>,
        global_rebuild: bool,
        text_engine: &mut TextEngine,
        feather_width: f32,
        frame_stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        self.tiles
            .retain(|address, _| cached_roots.contains(&address.layer));

        for layer in self.layers.values_mut() {
            if !cached_roots.contains(&layer.descriptor.id) {
                layer.visible_tiles.clear();
            }
        }

        let cached_root_ids = cached_roots.iter().copied().collect::<Vec<_>>();
        for layer_id in cached_root_ids {
            let Some(layer_snapshot) = snapshot_layers.get(&layer_id) else {
                continue;
            };

            let Some(layer) = self.layers.get(&layer_id) else {
                continue;
            };

            let descriptor = layer.descriptor.clone();
            let content_version = layer.content_version;
            let transform_node = layer.transform_node;
            let clip_node = layer.clip_node;

            let tile_grid = TileGrid::new(&descriptor, frame.scale_factor);
            let grid_changed = layer.tile_grid != Some(tile_grid);

            for (address, entry) in self.tiles.iter_mut() {
                if address.layer == layer_id {
                    entry.visible = false;
                    if address.scale_bucket == tile_grid.scale_bucket
                        && (!tile_grid.contains_tile(address.tile_x, address.tile_y)
                            || grid_changed
                            || structure_dirty_layers.contains(&layer_id)
                            || global_rebuild)
                    {
                        entry.dirty = true;
                    }
                }
            }

            if tile_grid.is_empty() {
                if let Some(layer) = self.layers.get_mut(&layer_id) {
                    layer.tile_grid = Some(tile_grid);
                    layer.visible_tiles.clear();
                }
                continue;
            }

            if let Some(damage) = tiled_damage.get(&layer_id) {
                mark_cached_layer_tiles_dirty(
                    &mut self.tiles,
                    layer_id,
                    tile_grid,
                    damage.map(|rect| rect_to_layer_local(rect, &descriptor)),
                );
            }

            let visible_tiles = visible_tiles_for_layer(
                &descriptor,
                tile_grid,
                transform_node,
                clip_node,
                &self.transforms,
                &self.clips,
                frame.viewport,
            );

            for address in &visible_tiles {
                let entry = self.tiles.remove(address);
                let mut entry = if let Some(mut existing) = entry {
                    existing.visible = true;
                    existing.last_used_frame = self.frame_index;
                    if existing.dirty {
                        frame_stats.regenerated_tiles += 1;
                        build_tile_entry(
                            &descriptor,
                            content_version,
                            tile_grid,
                            *address,
                            frame,
                            layer_snapshot,
                            snapshot_layers,
                            &self.clips,
                            &self.packets,
                            &self.effects,
                            text_engine,
                            &mut self.path_cache,
                            feather_width,
                            self.frame_index,
                        )?
                    } else {
                        frame_stats.reused_tiles += 1;
                        existing
                    }
                } else {
                    frame_stats.regenerated_tiles += 1;
                    build_tile_entry(
                        &descriptor,
                        content_version,
                        tile_grid,
                        *address,
                        frame,
                        layer_snapshot,
                        snapshot_layers,
                        &self.clips,
                        &self.packets,
                        &self.effects,
                        text_engine,
                        &mut self.path_cache,
                        feather_width,
                        self.frame_index,
                    )?
                };
                entry.visible = true;
                entry.last_used_frame = self.frame_index;
                self.tiles.insert(*address, entry);
            }

            if let Some(layer) = self.layers.get_mut(&layer_id) {
                layer.tile_grid = Some(tile_grid);
                layer.visible_tiles = visible_tiles;
            }
        }

        self.evict_tiles(scale_bucket(frame.scale_factor));
        Ok(())
    }

    fn total_tile_memory_bytes(&self) -> usize {
        self.tiles.values().map(|entry| entry.memory_cost).sum()
    }

    fn evict_tiles(&mut self, current_scale_bucket: u32) {
        let mut total_bytes = self.total_tile_memory_bytes();
        if total_bytes <= self.tile_budget_bytes {
            return;
        }

        let mut eviction_candidates = self
            .tiles
            .iter()
            .filter(|(_, entry)| !entry.visible)
            .map(|(address, entry)| {
                (
                    *address,
                    entry.key.scale_bucket != current_scale_bucket,
                    entry.last_used_frame,
                )
            })
            .collect::<Vec<_>>();
        eviction_candidates.sort_by_key(|(_, non_current_scale, last_used_frame)| {
            (*non_current_scale, *last_used_frame)
        });

        while total_bytes > self.tile_budget_bytes {
            let Some((address, _, _)) = eviction_candidates.first().copied() else {
                break;
            };
            eviction_candidates.remove(0);
            if let Some(entry) = self.tiles.remove(&address) {
                total_bytes = total_bytes.saturating_sub(entry.memory_cost);
            }
        }
    }
}

fn flush_transient_fragment(submission: &mut RetainedFrameSubmission, current: &mut DrawOpArena) {
    if current.draw_ops.is_empty() {
        return;
    }

    submission
        .fragments
        .push(RetainedFrameFragment::Transient(std::mem::take(current)));
}

fn scale_bucket(scale_factor: f32) -> u32 {
    (scale_factor.max(0.001) * 100.0).round() as u32
}

fn push_composition_item(
    phase: CompositionPhase,
    item: CompositionItem,
    normal_items: &mut Vec<CompositionItem>,
    overlay_items: &mut Vec<CompositionItem>,
    effect_items: &mut Vec<CompositionItem>,
) {
    match phase {
        CompositionPhase::Normal => normal_items.push(item),
        CompositionPhase::Overlay => overlay_items.push(item),
        CompositionPhase::Effect => effect_items.push(item),
    }
}

fn flush_container_segment(
    effects: &HashMap<EffectNodeId, EffectNode>,
    container: CompositionContainerId,
    result: &mut RootSnapshot,
    normal_items: &mut Vec<CompositionItem>,
    overlay_items: &mut Vec<CompositionItem>,
    effect_items: &mut Vec<CompositionItem>,
    segment_scene: &mut Scene,
    segment_start: &mut Option<ResolvedRasterState>,
) {
    if !scene_has_draw_content(segment_scene) {
        *segment_scene = Scene::new();
        *segment_start = None;
        return;
    }

    let initial_state = segment_start
        .take()
        .expect("segment state available before flush");
    let phase = composition_phase_for_effect_node(initial_state.effect_node, effects);
    let packet_id = RetainedPacketId {
        container,
        segment_index: result.packets.len() as u32,
    };
    push_composition_item(
        phase,
        CompositionItem::Packet(packet_id),
        normal_items,
        overlay_items,
        effect_items,
    );
    result.packet_ids.push(packet_id);
    result.packets.push(PacketSnapshot {
        id: packet_id,
        scene: std::mem::take(segment_scene),
        initial_state,
    });
}

fn composition_phase_for_effect_node(
    mut effect_node: EffectNodeId,
    effects: &HashMap<EffectNodeId, EffectNode>,
) -> CompositionPhase {
    let mut phase = CompositionPhase::Normal;

    while let Some(node) = effects.get(&effect_node) {
        phase = match (phase, node.composition_mode) {
            (CompositionPhase::Effect, _) | (_, sui_scene::LayerCompositionMode::Effect) => {
                CompositionPhase::Effect
            }
            (CompositionPhase::Overlay, _) | (_, sui_scene::LayerCompositionMode::Overlay) => {
                CompositionPhase::Overlay
            }
            _ => phase,
        };

        let Some(parent) = node.parent else {
            break;
        };
        effect_node = parent;
    }

    phase
}

fn resolve_layer_render_mode(
    descriptor: &sui_scene::SceneLayerDescriptor,
    _scale_factor: f32,
) -> RetainedLayerRenderMode {
    match descriptor.cache_policy {
        sui_scene::LayerCachePolicy::Direct => RetainedLayerRenderMode::Direct,
        sui_scene::LayerCachePolicy::Cached => RetainedLayerRenderMode::CachedTiles,
        sui_scene::LayerCachePolicy::Auto => {
            if descriptor.composition_mode == sui_scene::LayerCompositionMode::Scroll {
                RetainedLayerRenderMode::CachedTiles
            } else {
                RetainedLayerRenderMode::Direct
            }
        }
    }
}

fn nearest_cached_root(
    mut current: Option<SceneLayerId>,
    layers: &HashMap<SceneLayerId, LayerSnapshot>,
    render_modes: &HashMap<SceneLayerId, RetainedLayerRenderMode>,
) -> Option<SceneLayerId> {
    while let Some(layer_id) = current {
        if render_modes.get(&layer_id) == Some(&RetainedLayerRenderMode::CachedTiles) {
            return Some(layer_id);
        }
        current = layers.get(&layer_id).and_then(|layer| layer.parent);
    }
    None
}

fn owning_cached_root(
    mut current: Option<SceneLayerId>,
    layers: &HashMap<SceneLayerId, LayerSnapshot>,
    cached_roots: &HashSet<SceneLayerId>,
) -> Option<SceneLayerId> {
    while let Some(layer_id) = current {
        if cached_roots.contains(&layer_id) {
            return Some(layer_id);
        }
        current = layers.get(&layer_id).and_then(|layer| layer.parent);
    }

    None
}

fn fallback_cached_root_for_update(
    update: &sui_scene::SceneLayerUpdate,
    layers: &HashMap<SceneLayerId, LayerSnapshot>,
    cached_roots: &HashSet<SceneLayerId>,
) -> Option<SceneLayerId> {
    let damage = update.damage.unwrap_or(update.content_bounds);

    cached_roots
        .iter()
        .copied()
        .filter_map(|layer_id| {
            let descriptor = &layers.get(&layer_id)?.descriptor;
            let intersects = descriptor.content_bounds.intersection(damage).is_some()
                || descriptor.paint_bounds.intersection(damage).is_some();
            if !intersects {
                return None;
            }

            Some((
                layer_id,
                descriptor.content_bounds.width() * descriptor.content_bounds.height(),
            ))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(layer_id, _)| layer_id)
}

fn rect_to_layer_local(rect: Rect, descriptor: &sui_scene::SceneLayerDescriptor) -> Rect {
    rect.translate(Vector::new(-descriptor.bounds.x(), -descriptor.bounds.y()))
}

fn layer_local_to_scene(rect: Rect, descriptor: &sui_scene::SceneLayerDescriptor) -> Rect {
    rect.translate(descriptor.bounds.origin.to_vector())
}

fn merge_damage_rect(
    damage: &mut HashMap<SceneLayerId, Option<Rect>>,
    layer_id: SceneLayerId,
    next: Option<Rect>,
) {
    match damage.entry(layer_id) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(next);
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => match (*entry.get(), next) {
            (None, _) | (_, None) => {
                entry.insert(None);
            }
            (Some(current), Some(next_rect)) => {
                entry.insert(Some(current.union(next_rect)));
            }
        },
    }
}

fn mark_cached_layer_tiles_dirty(
    tiles: &mut HashMap<TileAddress, TileEntry>,
    layer_id: SceneLayerId,
    tile_grid: TileGrid,
    damage_local: Option<Rect>,
) {
    for (address, entry) in tiles.iter_mut() {
        if address.layer != layer_id || address.scale_bucket != tile_grid.scale_bucket {
            continue;
        }

        let intersects_damage =
            damage_local.is_none_or(|damage| entry.rect.intersection(damage).is_some());
        if intersects_damage {
            entry.dirty = true;
        }
    }
}

fn visible_tiles_for_layer(
    descriptor: &sui_scene::SceneLayerDescriptor,
    tile_grid: TileGrid,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    transforms: &HashMap<TransformNodeId, TransformNode>,
    clips: &HashMap<ClipNodeId, ClipNode>,
    viewport: Size,
) -> Vec<TileAddress> {
    let mut visible_tiles = Vec::new();
    let local_visible = rect_to_layer_local(descriptor.paint_bounds, descriptor);
    let Some(((min_x, min_y), (max_x, max_y))) = tile_grid.tile_range_for_rect(local_visible)
    else {
        return visible_tiles;
    };

    let world_transform = transforms
        .get(&transform_node)
        .map(|node| node.world)
        .unwrap_or(Transform::IDENTITY);
    let world_clip = resolved_clip_bounds(clip_node, clips);
    let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);

    for tile_y in min_y..=max_y {
        for tile_x in min_x..=max_x {
            let tile_local = tile_grid.tile_rect(tile_x, tile_y);
            let tile_scene = layer_local_to_scene(tile_local, descriptor);
            let tile_world = world_transform.transform_rect_bbox(tile_scene);
            if tile_world.intersection(viewport_rect).is_none() {
                continue;
            }
            if world_clip.is_some_and(|clip| tile_world.intersection(clip).is_none()) {
                continue;
            }

            visible_tiles.push(TileAddress {
                layer: descriptor.id,
                tile_x,
                tile_y,
                scale_bucket: tile_grid.scale_bucket,
            });
        }
    }

    visible_tiles
}

fn resolved_clip_bounds(
    mut clip_node: ClipNodeId,
    clips: &HashMap<ClipNodeId, ClipNode>,
) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    while clip_node != ClipNodeId::ROOT {
        let Some(node) = clips.get(&clip_node) else {
            break;
        };
        if let Some(primitive) = &node.primitive {
            bounds = Some(match bounds {
                Some(current) => current
                    .intersection(primitive.bounds())
                    .unwrap_or(Rect::ZERO),
                None => primitive.bounds(),
            });
        }
        clip_node = node.parent.unwrap_or(ClipNodeId::ROOT);
    }
    bounds
}

fn resolved_clip_primitives(
    mut clip_node: ClipNodeId,
    clips: &HashMap<ClipNodeId, ClipNode>,
) -> Vec<ResolvedClipPrimitive> {
    let mut primitives = Vec::new();
    while clip_node != ClipNodeId::ROOT {
        let Some(node) = clips.get(&clip_node) else {
            break;
        };
        if let Some(primitive) = &node.primitive {
            primitives.push(primitive.clone());
        }
        clip_node = node.parent.unwrap_or(ClipNodeId::ROOT);
    }
    primitives.reverse();
    primitives
}

fn build_tile_entry(
    descriptor: &sui_scene::SceneLayerDescriptor,
    content_version: u64,
    tile_grid: TileGrid,
    address: TileAddress,
    frame: &SceneFrame,
    layer_snapshot: &LayerSnapshot,
    snapshot_layers: &HashMap<SceneLayerId, LayerSnapshot>,
    clips: &HashMap<ClipNodeId, ClipNode>,
    packets: &HashMap<RetainedPacketId, RetainedDirectPacket>,
    effects: &HashMap<EffectNodeId, EffectNode>,
    text_engine: &mut TextEngine,
    path_cache: &mut PathMeshCache,
    feather_width: f32,
    frame_index: u64,
) -> Result<TileEntry> {
    let tile_local = tile_grid.tile_rect(address.tile_x, address.tile_y);
    let tile_scene = layer_local_to_scene(tile_local, descriptor);
    let fragment = build_cached_tile_fragment(
        frame,
        tile_scene,
        layer_snapshot,
        snapshot_layers,
        clips,
        packets,
        effects,
        composition_phase_for_effect_node(layer_snapshot.effect_node, effects),
        text_engine,
        path_cache,
        feather_width,
    )?;
    let cached_passes = cache_draw_ops(&fragment);
    let payload = TilePayload::DirectPacket(fragment);
    let memory_cost = match &payload {
        TilePayload::DirectPacket(packet) => packet.byte_size(),
    };

    Ok(TileEntry {
        key: TileKey {
            layer: descriptor.id,
            tile_x: address.tile_x,
            tile_y: address.tile_y,
            scale_bucket: address.scale_bucket,
            content_version,
        },
        rect: tile_local,
        translation: Vector::ZERO,
        dirty: false,
        visible: true,
        last_used_frame: frame_index,
        memory_cost,
        payload,
        cached_passes,
        gpu_geometry: None,
    })
}

fn build_cached_tile_fragment(
    frame: &SceneFrame,
    tile_scene_rect: Rect,
    layer_snapshot: &LayerSnapshot,
    snapshot_layers: &HashMap<SceneLayerId, LayerSnapshot>,
    clips: &HashMap<ClipNodeId, ClipNode>,
    packets: &HashMap<RetainedPacketId, RetainedDirectPacket>,
    effects: &HashMap<EffectNodeId, EffectNode>,
    included_phase: CompositionPhase,
    text_engine: &mut TextEngine,
    path_cache: &mut PathMeshCache,
    feather_width: f32,
) -> Result<DrawOpArena> {
    let mut draw_ops = DrawOpArena::default();
    let tile_clip = ResolvedClipPrimitive::Rect(snap_rect_to_device_pixels(
        tile_scene_rect,
        frame.scale_factor,
    ));
    let layer_clips = resolved_clip_primitives(layer_snapshot.clip_node, clips);
    let mut effective_clips = layer_clips.clone();
    effective_clips.push(tile_clip.clone());
    let tile_only = [tile_clip.clone()];

    for item in &layer_snapshot.items {
        match item {
            CompositionItem::Packet(packet_id) => {
                let packet_snapshot = layer_snapshot
                    .packets
                    .iter()
                    .find(|packet| packet.id == *packet_id)
                    .cloned();
                let Some(packet) = packets.get(packet_id) else {
                    continue;
                };
                if composition_phase_for_effect_node(packet.initial_state.effect_node, effects)
                    != included_phase
                {
                    continue;
                }

                match packet.coordinate_space {
                    PacketCoordinateSpace::World => {
                        if layer_snapshot.descriptor.cache_policy == sui_scene::LayerCachePolicy::Auto
                        {
                            let Some(packet_snapshot) = packet_snapshot else {
                                continue;
                            };
                            let has_clip_or_path = !packet_snapshot.initial_state.clip_stack.is_empty()
                                || scene_contains_clip_commands(&packet_snapshot.scene)
                                || scene_contains_path_commands(&packet_snapshot.scene);
                            if has_clip_or_path
                                && !scene_contains_image_commands(&packet_snapshot.scene)
                            {
                                let additional_clips = packet_additional_clips(
                                    &layer_clips,
                                    packet_snapshot.initial_state.clip_stack.as_slice(),
                                )
                                .to_vec();
                                let normalized_snapshot = normalize_packet_snapshot(
                                    packet_snapshot,
                                    PacketCoordinateSpace::LayerLocal,
                                    layer_snapshot.descriptor.bounds.origin.to_vector(),
                                );
                                let mut external_clips = effective_clips.clone();
                                external_clips.extend_from_slice(&additional_clips);
                                let fragment = build_direct_packet(
                                    frame,
                                    &normalized_snapshot.scene,
                                    &normalized_snapshot.initial_state,
                                    text_engine,
                                    path_cache,
                                    feather_width,
                                )?;
                                draw_ops.append_composed_fragment(
                                    &fragment,
                                    layer_snapshot.descriptor.bounds.origin.to_vector(),
                                    &external_clips,
                                    frame.viewport,
                                )?;
                                continue;
                            }
                        }

                        draw_ops.append_composed_fragment(
                            &packet.draw_ops,
                            Vector::ZERO,
                            &tile_only,
                            frame.viewport,
                        )?;
                    }
                    PacketCoordinateSpace::LayerLocal => {
                        let Some(packet_snapshot) = packet_snapshot else {
                            continue;
                        };
                        let additional_clips = packet_additional_clips(
                            &layer_clips,
                            packet_snapshot.initial_state.clip_stack.as_slice(),
                        );
                        let mut external_clips = effective_clips.clone();
                        external_clips.extend_from_slice(additional_clips);
                        draw_ops.append_composed_fragment(
                            &packet.draw_ops,
                            layer_snapshot.descriptor.bounds.origin.to_vector(),
                            &external_clips,
                            frame.viewport,
                        )?;
                    }
                }
            }
            CompositionItem::Layer(child_id) => {
                let Some(child_snapshot) = snapshot_layers.get(child_id) else {
                    continue;
                };
                if composition_phase_for_effect_node(child_snapshot.effect_node, effects)
                    != included_phase
                {
                    continue;
                }
                if tile_scene_rect
                    .intersection(child_snapshot.descriptor.content_bounds)
                    .is_none()
                    && tile_scene_rect
                        .intersection(child_snapshot.descriptor.paint_bounds)
                        .is_none()
                {
                    continue;
                }
                let child_fragment = build_cached_tile_fragment(
                    frame,
                    tile_scene_rect,
                    child_snapshot,
                    snapshot_layers,
                    clips,
                    packets,
                    effects,
                    included_phase,
                    text_engine,
                    path_cache,
                    feather_width,
                )?;
                draw_ops.append_fragment(&child_fragment);
            }
        }
    }

    Ok(draw_ops)
}

fn snap_rect_to_device_pixels(rect: Rect, scale_factor: f32) -> Rect {
    if rect.is_empty() {
        return rect;
    }

    let scale = scale_factor.max(0.001);
    let min_x = (rect.x() * scale).round() / scale;
    let min_y = (rect.y() * scale).round() / scale;
    let max_x = ((rect.x() + rect.width()) * scale).round() / scale;
    let max_y = ((rect.y() + rect.height()) * scale).round() / scale;

    Rect::from_points(Point::new(min_x, min_y), Point::new(max_x, max_y))
}

fn packet_additional_clips<'a>(
    layer_clips: &[ResolvedClipPrimitive],
    packet_clips: &'a [ResolvedClipPrimitive],
) -> &'a [ResolvedClipPrimitive] {
    if packet_clips.starts_with(layer_clips) {
        &packet_clips[layer_clips.len()..]
    } else {
        packet_clips
    }
}

fn scene_contains_clip_commands(scene: &Scene) -> bool {
    scene.commands().iter().any(|command| {
        matches!(
            command,
            SceneCommand::PushClip { .. } | SceneCommand::PushClipPath { .. }
        )
    })
}

fn scene_contains_path_commands(scene: &Scene) -> bool {
    scene.commands().iter().any(|command| {
        matches!(
            command,
            SceneCommand::FillPath { .. } | SceneCommand::StrokePath { .. }
        )
    })
}

fn scene_contains_image_commands(scene: &Scene) -> bool {
    scene
        .commands()
        .iter()
        .any(|command| matches!(command, SceneCommand::DrawImage { .. }))
}

impl RetainedCompositorState {
    fn reset_property_trees(&mut self) {
        self.transforms.clear();
        self.clips.clear();
        self.effects.clear();
        self.transforms.insert(
            TransformNodeId::ROOT,
            TransformNode {
                id: TransformNodeId::ROOT,
                parent: None,
                local: Transform::IDENTITY,
                world: Transform::IDENTITY,
            },
        );
        self.clips.insert(
            ClipNodeId::ROOT,
            ClipNode {
                id: ClipNodeId::ROOT,
                parent: None,
                primitive: None,
            },
        );
        self.effects.insert(
            EffectNodeId::ROOT,
            EffectNode {
                id: EffectNodeId::ROOT,
                parent: None,
                composition_mode: sui_scene::LayerCompositionMode::Normal,
            },
        );
        self.next_transform_node = 1;
        self.next_clip_node = 1;
        self.next_effect_node = 1;
    }

    fn push_transform_node(
        &mut self,
        parent: Option<TransformNodeId>,
        local: Transform,
        world: Transform,
    ) -> TransformNodeId {
        let id = TransformNodeId(self.next_transform_node);
        self.next_transform_node += 1;
        self.transforms.insert(
            id,
            TransformNode {
                id,
                parent,
                local,
                world,
            },
        );
        id
    }

    fn push_clip_node(
        &mut self,
        parent: Option<ClipNodeId>,
        primitive: ResolvedClipPrimitive,
    ) -> ClipNodeId {
        let id = ClipNodeId(self.next_clip_node);
        self.next_clip_node += 1;
        self.clips.insert(
            id,
            ClipNode {
                id,
                parent,
                primitive: Some(primitive),
            },
        );
        id
    }

    fn push_effect_node(
        &mut self,
        parent: Option<EffectNodeId>,
        composition_mode: sui_scene::LayerCompositionMode,
    ) -> EffectNodeId {
        let id = EffectNodeId(self.next_effect_node);
        self.next_effect_node += 1;
        self.effects.insert(
            id,
            EffectNode {
                id,
                parent,
                composition_mode,
            },
        );
        id
    }
}

fn scene_has_draw_content(scene: &Scene) -> bool {
    scene.commands().iter().any(|command| {
        matches!(
            command,
            SceneCommand::Clear(_)
                | SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillPath { .. }
                | SceneCommand::StrokePath { .. }
                | SceneCommand::DrawText(_)
                | SceneCommand::DrawShapedText(_)
                | SceneCommand::DrawImage { .. }
                | SceneCommand::Label { .. }
        )
    })
}

fn normalize_packet_snapshot(
    mut snapshot: PacketSnapshot,
    coordinate_space: PacketCoordinateSpace,
    normalization_origin: Vector,
) -> PacketSnapshot {
    if coordinate_space == PacketCoordinateSpace::LayerLocal && normalization_origin != Vector::ZERO
    {
        let delta = Vector::new(-normalization_origin.x, -normalization_origin.y);
        snapshot.scene.translate(delta);
        snapshot.initial_state = translate_resolved_raster_state(&snapshot.initial_state, delta);
    }
    if coordinate_space == PacketCoordinateSpace::LayerLocal {
        snapshot.initial_state.clip_stack.clear();
        snapshot.initial_state.clip_node = ClipNodeId::ROOT;
    }
    snapshot
}

fn packet_text_sample(scene: &Scene) -> Option<String> {
    for command in scene.commands() {
        let text = match command {
            SceneCommand::Label { text, .. } => Some(text.as_str()),
            SceneCommand::DrawText(run) => Some(run.text.as_str()),
            _ => None,
        };
        if let Some(text) = text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let mut excerpt = trimmed.chars().take(64).collect::<String>();
                if trimmed.chars().count() > 64 {
                    excerpt.push_str("...");
                }
                return Some(excerpt);
            }
        }
    }
    None
}

fn translate_resolved_raster_state(
    state: &ResolvedRasterState,
    delta: Vector,
) -> ResolvedRasterState {
    let mut translated = state.clone();
    translated.clip_stack = translated
        .clip_stack
        .into_iter()
        .map(|clip| translate_resolved_clip_primitive(clip, delta))
        .collect();
    translated
}

fn translate_resolved_clip_primitive(
    primitive: ResolvedClipPrimitive,
    delta: Vector,
) -> ResolvedClipPrimitive {
    match primitive {
        ResolvedClipPrimitive::Rect(rect) => ResolvedClipPrimitive::Rect(rect.translate(delta)),
        ResolvedClipPrimitive::Path { path, bounds, .. } => {
            let translated_path = transform_scene_path(&path, Transform::translation_vector(delta));
            let translated_bounds = bounds.translate(delta);
            ResolvedClipPrimitive::Path {
                signature: hash_path(&translated_path, Transform::IDENTITY),
                path: translated_path,
                bounds: translated_bounds,
            }
        }
    }
}

fn translate_cached_layer_tiles(
    tiles: &mut HashMap<TileAddress, TileEntry>,
    layer_id: SceneLayerId,
    delta: Vector,
    _viewport: Size,
) {
    if delta == Vector::ZERO {
        return;
    }

    for (address, entry) in tiles.iter_mut() {
        if address.layer != layer_id {
            continue;
        }

        entry.translation += delta;
    }
}

fn register_cached_scroll_translation(
    translations: &mut HashMap<SceneLayerId, Vector>,
    conflicts: &mut HashSet<SceneLayerId>,
    layer_id: SceneLayerId,
    delta: Vector,
) -> bool {
    if conflicts.contains(&layer_id) {
        return false;
    }

    match translations.get(&layer_id).copied() {
        Some(existing) if existing != delta => {
            translations.remove(&layer_id);
            conflicts.insert(layer_id);
            false
        }
        Some(_) => true,
        None => {
            translations.insert(layer_id, delta);
            true
        }
    }
}

fn descriptor_translation_delta(
    previous: &sui_scene::SceneLayerDescriptor,
    current: &sui_scene::SceneLayerDescriptor,
) -> Option<Vector> {
    if previous.id != current.id
        || previous.owner != current.owner
        || previous.cache_policy != current.cache_policy
        || previous.composition_mode != current.composition_mode
        || previous.bounds.size != current.bounds.size
        || previous.content_bounds.size != current.content_bounds.size
        || previous.paint_bounds.size != current.paint_bounds.size
    {
        return None;
    }

    let bounds_delta = current.bounds.origin - previous.bounds.origin;
    let content_delta = current.content_bounds.origin - previous.content_bounds.origin;
    let paint_delta = current.paint_bounds.origin - previous.paint_bounds.origin;
    if bounds_delta == content_delta && bounds_delta == paint_delta {
        Some(bounds_delta)
    } else {
        None
    }
}

fn packet_signature(
    scene: &Scene,
    initial_state: &ResolvedRasterState,
    viewport: Size,
    feather_width: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    scene.commands().len().hash(&mut hasher);
    hash_scene(scene, &mut hasher);
    initial_state.signature().hash(&mut hasher);
    viewport.width.to_bits().hash(&mut hasher);
    viewport.height.to_bits().hash(&mut hasher);
    feather_width.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn hash_scene(scene: &Scene, hasher: &mut DefaultHasher) {
    for command in scene.commands() {
        hash_scene_command(command, hasher);
    }
}

fn hash_scene_command(command: &SceneCommand, hasher: &mut DefaultHasher) {
    match command {
        SceneCommand::Clear(color) => {
            0u8.hash(hasher);
            hash_color(*color, hasher);
        }
        SceneCommand::FillRect { rect, brush } => {
            1u8.hash(hasher);
            hash_rect(hasher, *rect);
            hash_brush(brush, hasher);
        }
        SceneCommand::StrokeRect {
            rect,
            brush,
            stroke,
        } => {
            2u8.hash(hasher);
            hash_rect(hasher, *rect);
            hash_brush(brush, hasher);
            stroke.width.to_bits().hash(hasher);
        }
        SceneCommand::FillPath { path, brush } => {
            3u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
            hash_brush(brush, hasher);
        }
        SceneCommand::StrokePath {
            path,
            brush,
            stroke,
        } => {
            4u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
            hash_brush(brush, hasher);
            stroke.width.to_bits().hash(hasher);
        }
        SceneCommand::DrawText(text) => {
            5u8.hash(hasher);
            hash_rect(hasher, text.rect);
            text.text.hash(hasher);
            hash_text_style(&text.style, hasher);
        }
        SceneCommand::DrawShapedText(text) => {
            6u8.hash(hasher);
            hash_point(hasher, text.origin);
            text.layout_handle.get().hash(hasher);
            text.layout_version.get().hash(hasher);
            hash_rect(hasher, text.bounds);
        }
        SceneCommand::DrawImage { rect, source } => {
            7u8.hash(hasher);
            hash_rect(hasher, *rect);
            source.image.get().hash(hasher);
            source
                .source_rect
                .map(|rect| {
                    1u8.hash(hasher);
                    hash_rect(hasher, rect);
                })
                .unwrap_or_else(|| 0u8.hash(hasher));
            source
                .tint
                .map(|color| {
                    1u8.hash(hasher);
                    hash_color(color, hasher);
                })
                .unwrap_or_else(|| 0u8.hash(hasher));
        }
        SceneCommand::PushClip { rect } => {
            8u8.hash(hasher);
            hash_rect(hasher, *rect);
        }
        SceneCommand::PushClipPath { path } => {
            9u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
        }
        SceneCommand::PopClip => {
            10u8.hash(hasher);
        }
        SceneCommand::PushTransform { transform } => {
            11u8.hash(hasher);
            hash_transform(hasher, *transform);
        }
        SceneCommand::PopTransform => {
            12u8.hash(hasher);
        }
        SceneCommand::Layer(layer) => {
            13u8.hash(hasher);
            layer.layer_id().get().hash(hasher);
        }
        SceneCommand::Label { rect, text, color } => {
            14u8.hash(hasher);
            hash_rect(hasher, *rect);
            text.hash(hasher);
            hash_color(*color, hasher);
        }
    }
}

fn hash_brush(brush: &Brush, hasher: &mut DefaultHasher) {
    match brush {
        Brush::Solid(color) => {
            0u8.hash(hasher);
            hash_color(*color, hasher);
        }
    }
}

fn hash_color(color: Color, hasher: &mut DefaultHasher) {
    color.red.to_bits().hash(hasher);
    color.green.to_bits().hash(hasher);
    color.blue.to_bits().hash(hasher);
    color.alpha.to_bits().hash(hasher);
}

fn hash_text_style(style: &TextStyle, hasher: &mut DefaultHasher) {
    style.font.map(|font| font.get()).hash(hasher);
    style.font_size.to_bits().hash(hasher);
    style.line_height.to_bits().hash(hasher);
    hash_color(style.color, hasher);
}
