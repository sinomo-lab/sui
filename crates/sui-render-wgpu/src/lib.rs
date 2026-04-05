#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::Instant,
};

use bytemuck::{Pod, Zeroable};
use lyon_path::{
    Path as LyonPath, PathEvent, builder::PathBuilder as LyonPathBuilder, iterator::PathIterator,
    math::point,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use sui_core::{
    Color, ColorSpace, Error, ImageHandle, Path as ScenePath, PathElement, Point, Rect, Result,
    Size, Transform, Vector, WindowId,
};
use sui_scene::{
    Brush, RegisteredImage, RegisteredImageFormat, Scene, SceneCommand, SceneFrame, SceneLayer,
    SceneLayerId, SceneLayerUpdateKind, StrokeStyle,
};
use sui_text::{
    FontRegistry, ResolvedTextFace, ShapedGlyph as SceneShapedGlyph, ShapedText, TextLayout,
    TextLayoutCacheSnapshot, TextRun, TextStyle, TextSystem,
};
use ttf_parser::GlyphId;
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCapabilities {
    pub supports_color_management: bool,
    pub supports_offscreen_surfaces: bool,
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self {
            supports_color_management: true,
            supports_offscreen_surfaces: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererInterop {
    pub raw_wgpu_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphCacheSnapshot {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl GlyphCacheSnapshot {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererTextCacheSnapshot {
    pub layout: TextLayoutCacheSnapshot,
    pub glyph: GlyphCacheSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererFrameStats {
    pub pass_count: usize,
    pub draw_count: usize,
    pub uploaded_vertex_bytes: u64,
    pub visible_layer_count: usize,
    pub visible_tile_count: usize,
    pub reused_tile_count: usize,
    pub regenerated_tile_count: usize,
    pub direct_packet_count: usize,
    pub tile_memory_bytes: u64,
    pub tile_generation_time_us: u64,
    pub composition_time_us: u64,
}

impl RendererFrameStats {
    fn from_prepared_frame(prepared: &PreparedFrameBatches) -> Self {
        Self {
            pass_count: prepared.passes.len().max(1),
            draw_count: prepared
                .passes
                .iter()
                .map(|pass| pass.clip_paths.len() + pass.draws.len())
                .sum(),
            uploaded_vertex_bytes: (prepared.scene_vertices.len() as u64
                + prepared.clip_vertices.len() as u64)
                * VERTEX_SIZE,
            visible_layer_count: 0,
            visible_tile_count: 0,
            reused_tile_count: 0,
            regenerated_tile_count: 0,
            direct_packet_count: 0,
            tile_memory_bytes: 0,
            tile_generation_time_us: 0,
            composition_time_us: 0,
        }
    }

    fn with_compositor_stats(mut self, stats: RetainedCompositorFrameStats) -> Self {
        self.visible_layer_count = stats.visible_layers;
        self.visible_tile_count = stats.visible_tiles;
        self.reused_tile_count = stats.reused_tiles;
        self.regenerated_tile_count = stats.regenerated_tiles;
        self.direct_packet_count = stats.direct_packets;
        self.tile_memory_bytes = stats.tile_memory_bytes as u64;
        self.tile_generation_time_us = (stats.tile_generation_time_ms * 1000.0).round() as u64;
        self.composition_time_us = (stats.composition_time_ms * 1000.0).round() as u64;
        self
    }
}

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    feather_width: f32,
    frames_rendered: usize,
    capabilities: RendererCapabilities,
    last_frames: HashMap<WindowId, SceneFrame>,
    last_frame_stats: HashMap<WindowId, RendererFrameStats>,
    shared: Option<SharedRenderer>,
    text_engine: Option<TextEngine>,
    image_cache: HashMap<ImageHandle, CachedImageTexture>,
    compositors: HashMap<WindowId, RetainedCompositorState>,
    surfaces: HashMap<WindowId, SurfaceState>,
    offscreen_targets: HashMap<WindowId, OffscreenTarget>,
    frame_resources: FrameResources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CompositionContainerId {
    Root,
    Layer(SceneLayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RetainedPacketId {
    container: CompositionContainerId,
    segment_index: u32,
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
enum RetainedLayerRenderMode {
    Direct,
    CachedTiles,
}

const DEFAULT_TILE_SIZE_PX: u32 = 256;
const TILE_CACHE_BUDGET_BYTES: usize = 32 * 1024 * 1024;

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
struct TileAddress {
    layer: SceneLayerId,
    tile_x: i32,
    tile_y: i32,
    scale_bucket: u32,
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

#[derive(Debug, Clone)]
struct TileEntry {
    key: TileKey,
    rect: Rect,
    dirty: bool,
    visible: bool,
    layer_local: bool,
    last_used_frame: u64,
    memory_cost: usize,
    payload: TilePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct RetainedCompositorFrameStats {
    visible_layers: usize,
    visible_tiles: usize,
    reused_tiles: usize,
    regenerated_tiles: usize,
    direct_packets: usize,
    tile_memory_bytes: usize,
    tile_generation_time_ms: f64,
    composition_time_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionItem {
    Packet(RetainedPacketId),
    Layer(SceneLayerId),
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
enum ResolvedClipPrimitive {
    Rect(Rect),
    Path {
        path: ScenePath,
        bounds: Rect,
        signature: u64,
    },
}

#[allow(dead_code)]
impl ResolvedClipPrimitive {
    fn bounds(&self) -> Rect {
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
struct ResolvedRasterState {
    current_transform: Transform,
    clip_stack: Vec<ResolvedClipPrimitive>,
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
                    bounds,
                    signature,
                    ..
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct RetainedDirectPacket {
    id: RetainedPacketId,
    scene: Scene,
    initial_state: ResolvedRasterState,
    signature: u64,
    coordinate_space: PacketCoordinateSpace,
    draw_ops: DrawOpArena,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketCoordinateSpace {
    World,
    LayerLocal,
}

#[derive(Debug, Clone, Default)]
struct RetainedRootNode {
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    structure_version: u64,
}

#[derive(Debug, Clone)]
struct RetainedLayer {
    descriptor: sui_scene::SceneLayerDescriptor,
    parent: Option<SceneLayerId>,
    children: Vec<SceneLayerId>,
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    effect_node: EffectNodeId,
    render_mode: RetainedLayerRenderMode,
    content_version: u64,
    structure_version: u64,
    tile_grid: Option<TileGrid>,
    visible_tiles: Vec<TileAddress>,
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
struct RetainedCompositorState {
    root: RetainedRootNode,
    layers: HashMap<SceneLayerId, RetainedLayer>,
    packets: HashMap<RetainedPacketId, RetainedDirectPacket>,
    tiles: HashMap<TileAddress, TileEntry>,
    transforms: HashMap<TransformNodeId, TransformNode>,
    clips: HashMap<ClipNodeId, ClipNode>,
    effects: HashMap<EffectNodeId, EffectNode>,
    next_transform_node: u64,
    next_clip_node: u64,
    next_effect_node: u64,
    viewport: Size,
    feather_width_bits: u32,
    frame_index: u64,
    tile_budget_bytes: usize,
    last_frame_stats: RetainedCompositorFrameStats,
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
        }
    }
}

impl RetainedCompositorState {
    fn prepare_frame(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<DrawOpArena> {
        let viewport_changed = self.viewport != frame.viewport;
        let feather_changed = self.feather_width_bits != feather_width.to_bits();
        self.frame_index = self.frame_index.wrapping_add(1);
        let snapshot = self.build_snapshot(&frame.scene)?;
        let mut frame_stats = RetainedCompositorFrameStats::default();
        let tile_generation_started = Instant::now();
        self.apply_snapshot(
            frame,
            snapshot,
            text_engine,
            feather_width,
            viewport_changed || feather_changed,
            &mut frame_stats,
        )?;
        frame_stats.tile_generation_time_ms = tile_generation_started.elapsed().as_secs_f64() * 1000.0;
        let composition_started = Instant::now();
        let draw_ops = self.compose_draw_ops(&mut frame_stats);
        frame_stats.composition_time_ms = composition_started.elapsed().as_secs_f64() * 1000.0;
        frame_stats.tile_memory_bytes = self.total_tile_memory_bytes();
        self.last_frame_stats = frame_stats;
        self.viewport = frame.viewport;
        self.feather_width_bits = feather_width.to_bits();
        Ok(draw_ops)
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
        let mut segment_scene = Scene::new();
        let mut segment_start = None::<ResolvedRasterState>;

        let flush_segment = |result: &mut RootSnapshot,
                             segment_scene: &mut Scene,
                             segment_start: &mut Option<ResolvedRasterState>| {
            if !scene_has_draw_content(segment_scene) {
                *segment_scene = Scene::new();
                *segment_start = None;
                return;
            }

            let packet_id = RetainedPacketId {
                container,
                segment_index: result.packets.len() as u32,
            };
            result.items.push(CompositionItem::Packet(packet_id));
            result.packet_ids.push(packet_id);
            result.packets.push(PacketSnapshot {
                id: packet_id,
                scene: std::mem::take(segment_scene),
                initial_state: segment_start.take().expect("segment state available before flush"),
            });
        };

        for command in scene.commands() {
            match command {
                SceneCommand::Layer(layer) => {
                    flush_segment(&mut result, &mut segment_scene, &mut segment_start);

                    let mut child_state = state.clone();
                    child_state.effect_node = self.push_effect_node(
                        Some(state.effect_node),
                        layer.descriptor.composition_mode,
                    );
                    let layer_snapshot = self.build_layer_snapshot(
                        layer,
                        parent_layer,
                        child_state,
                        snapshot,
                    )?;
                    result.items.push(CompositionItem::Layer(layer.layer_id()));
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

        flush_segment(&mut result, &mut segment_scene, &mut segment_start);
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
                state.transform_node = self.push_transform_node(Some(parent_node), *transform, world);
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
                let clip = ResolvedClipPrimitive::Rect(
                    state.current_transform.transform_rect_bbox(*rect),
                );
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
            .map(|(layer_id, layer)| (*layer_id, resolve_layer_render_mode(&layer.descriptor, frame.scale_factor)))
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
        let cached_coverage = snapshot
            .layers
            .keys()
            .copied()
            .map(|layer_id| {
                (
                    layer_id,
                    nearest_cached_root(Some(layer_id), &snapshot.layers, &render_modes),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut packet_dirty_layers = HashSet::new();
        let mut tiled_damage = HashMap::<SceneLayerId, Option<Rect>>::new();
        let current_layers = snapshot.layers.keys().copied().collect::<HashSet<_>>();
        let mut root_dirty = global_rebuild;

        for update in &frame.layer_updates {
            if !current_layers.contains(&update.layer_id) {
                root_dirty = true;
                continue;
            }

            if let Some(cached_root) = cached_coverage.get(&update.layer_id).copied().flatten() {
                if matches!(
                    update.kind,
                    SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources
                ) {
                    merge_damage_rect(
                        &mut tiled_damage,
                        cached_root,
                        update.damage.or(Some(update.content_bounds)),
                    );
                }
                if cached_root == update.layer_id {
                    continue;
                }
            }

            match update.kind {
                SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources => {
                    packet_dirty_layers.insert(update.layer_id);
                }
                SceneLayerUpdateKind::Transform
                | SceneLayerUpdateKind::Clip
                | SceneLayerUpdateKind::Effect
                | SceneLayerUpdateKind::Visibility => {}
            }
        }

        let mut valid_packets = HashSet::new();
        valid_packets.extend(snapshot.root.packet_ids.iter().copied());
        for (layer_id, layer) in &snapshot.layers {
            if cached_coverage.get(layer_id).copied().flatten().is_none() {
                valid_packets.extend(layer.packet_ids.iter().copied());
            }
        }

        if self.root.items != snapshot.root.items || self.root.packet_ids != snapshot.root.packet_ids {
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
            )?;
        }

        let snapshot_layers = snapshot.layers.clone();
        let previous_layers = self.layers.clone();
        self.layers.retain(|layer_id, _| current_layers.contains(layer_id));

        let mut structure_dirty_layers = HashSet::new();

        for (layer_id, layer_snapshot) in snapshot.layers {
            let translated_only = previous_layers.get(&layer_id).is_some_and(|previous| {
                descriptor_translation_delta(&previous.descriptor, &layer_snapshot.descriptor).is_some()
            });
            let structure_changed = previous_layers.get(&layer_id).is_none_or(|previous| {
                previous.parent != layer_snapshot.parent
                    || previous.children != layer_snapshot.children
                    || previous.items != layer_snapshot.items
                    || previous.packet_ids != layer_snapshot.packet_ids
                    || previous.transform_node != layer_snapshot.transform_node
                    || previous.clip_node != layer_snapshot.clip_node
                    || previous.effect_node != layer_snapshot.effect_node
                    || (!translated_only && previous.descriptor != layer_snapshot.descriptor)
            });

            let content_changed = packet_dirty_layers.contains(&layer_id)
                || previous_layers.get(&layer_id).is_none_or(|previous| {
                    !translated_only && previous.descriptor.bounds != layer_snapshot.descriptor.bounds
                });

            let previous = previous_layers.get(&layer_id);
            let retained = self.layers.entry(layer_id).or_insert_with(|| RetainedLayer {
                descriptor: layer_snapshot.descriptor.clone(),
                parent: layer_snapshot.parent,
                children: layer_snapshot.children.clone(),
                items: layer_snapshot.items.clone(),
                packet_ids: layer_snapshot.packet_ids.clone(),
                transform_node: layer_snapshot.transform_node,
                clip_node: layer_snapshot.clip_node,
                effect_node: layer_snapshot.effect_node,
                render_mode: render_modes[&layer_id],
                content_version: 0,
                structure_version: 0,
                tile_grid: None,
                visible_tiles: Vec::new(),
            });

            if structure_changed {
                retained.structure_version = previous
                    .map_or(retained.structure_version + 1, |old| old.structure_version.wrapping_add(1));
                structure_dirty_layers.insert(layer_id);
            }
            if content_changed {
                retained.content_version = previous
                    .map_or(retained.content_version + 1, |old| old.content_version.wrapping_add(1));
            }

            retained.descriptor = layer_snapshot.descriptor.clone();
            retained.parent = layer_snapshot.parent;
            retained.children = layer_snapshot.children.clone();
            retained.items = layer_snapshot.items.clone();
            retained.packet_ids = layer_snapshot.packet_ids.clone();
            retained.transform_node = layer_snapshot.transform_node;
            retained.clip_node = layer_snapshot.clip_node;
            retained.effect_node = layer_snapshot.effect_node;
            retained.render_mode = render_modes[&layer_id];
            if retained.render_mode != RetainedLayerRenderMode::CachedTiles {
                retained.tile_grid = None;
                retained.visible_tiles.clear();
            }

            if cached_coverage.get(&layer_id).copied().flatten().is_none() {
                let packet_dirty = global_rebuild || structure_changed || packet_dirty_layers.contains(&layer_id);
                let coordinate_space = if render_modes[&layer_id] == RetainedLayerRenderMode::Direct {
                    PacketCoordinateSpace::LayerLocal
                } else {
                    PacketCoordinateSpace::World
                };
                let normalization_origin = layer_snapshot.descriptor.bounds.origin.to_vector();
                for packet in layer_snapshot.packets {
                    self.upsert_packet(
                        frame,
                        packet,
                        packet_dirty,
                        coordinate_space,
                        normalization_origin,
                        text_engine,
                        feather_width,
                    )?;
                }
            }
        }

        self.packets.retain(|packet_id, _| valid_packets.contains(packet_id));
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
        forced_dirty: bool,
        coordinate_space: PacketCoordinateSpace,
        normalization_origin: Vector,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<()> {
        let snapshot = normalize_packet_snapshot(snapshot, coordinate_space, normalization_origin);
        let signature = packet_signature(&snapshot.scene, &snapshot.initial_state, frame.viewport, feather_width);
        let should_rebuild = forced_dirty
            || self
                .packets
                .get(&snapshot.id)
                .is_none_or(|packet| {
                    packet.coordinate_space != coordinate_space
                        || packet.id != snapshot.id
                        ||
                    packet.signature != signature
                        || packet.scene != snapshot.scene
                        || packet.initial_state != snapshot.initial_state
                });

        if should_rebuild {
            let draw_ops = build_direct_packet(
                frame,
                &snapshot.scene,
                &snapshot.initial_state,
                text_engine,
                feather_width,
            )?;
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

    fn compose_draw_ops(&self, stats: &mut RetainedCompositorFrameStats) -> DrawOpArena {
        let mut draw_ops = DrawOpArena::default();
        self.append_items(&self.root.items, &mut draw_ops, stats);
        draw_ops
    }

    fn append_items(
        &self,
        items: &[CompositionItem],
        draw_ops: &mut DrawOpArena,
        stats: &mut RetainedCompositorFrameStats,
    ) {
        for item in items {
            match item {
                CompositionItem::Packet(packet_id) => {
                    if let Some(packet) = self.packets.get(packet_id) {
                        match packet.coordinate_space {
                            PacketCoordinateSpace::World => {
                                draw_ops.append_fragment(&packet.draw_ops);
                            }
                            PacketCoordinateSpace::LayerLocal => {
                                let origin = match packet.id.container {
                                    CompositionContainerId::Root => Vector::ZERO,
                                    CompositionContainerId::Layer(layer_id) => self
                                        .layers
                                        .get(&layer_id)
                                        .map(|layer| layer.descriptor.bounds.origin.to_vector())
                                        .unwrap_or(Vector::ZERO),
                                };
                                draw_ops.append_transformed_fragment(
                                    &packet.draw_ops,
                                    Transform::translation_vector(origin),
                                );
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
                                self.append_items(&layer.items, draw_ops, stats);
                            }
                            RetainedLayerRenderMode::CachedTiles => {
                                if !layer.visible_tiles.is_empty() {
                                    stats.visible_layers += 1;
                                }
                                for tile in &layer.visible_tiles {
                                    if let Some(entry) = self.tiles.get(tile) {
                                        let TilePayload::DirectPacket(fragment) = &entry.payload;
                                        if entry.layer_local {
                                            draw_ops.append_transformed_fragment(
                                                fragment,
                                                Transform::translation(
                                                    layer.descriptor.bounds.x(),
                                                    layer.descriptor.bounds.y(),
                                                ),
                                            );
                                        } else {
                                            draw_ops.append_fragment(fragment);
                                        }
                                        stats.visible_tiles += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
        self.tiles.retain(|address, _| cached_roots.contains(&address.layer));

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
            let layer_local_tiles = cached_layer_translation_fast_path(
                transform_node,
                clip_node,
                layer.effect_node,
            );

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
                            layer_local_tiles,
                            frame,
                            layer_snapshot,
                            snapshot_layers,
                            text_engine,
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
                        layer_local_tiles,
                        frame,
                        layer_snapshot,
                        snapshot_layers,
                        text_engine,
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
            .map(|(address, entry)| (*address, entry.key.scale_bucket != current_scale_bucket, entry.last_used_frame))
            .collect::<Vec<_>>();
        eviction_candidates.sort_by_key(|(_, non_current_scale, last_used_frame)| (*non_current_scale, *last_used_frame));

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


fn scale_bucket(scale_factor: f32) -> u32 {
    (scale_factor.max(0.001) * 100.0).round() as u32
}

fn resolve_layer_render_mode(
    descriptor: &sui_scene::SceneLayerDescriptor,
    scale_factor: f32,
) -> RetainedLayerRenderMode {
    match descriptor.cache_policy {
        sui_scene::LayerCachePolicy::Direct => RetainedLayerRenderMode::Direct,
        sui_scene::LayerCachePolicy::Cached => RetainedLayerRenderMode::CachedTiles,
        sui_scene::LayerCachePolicy::Auto => {
            let max_dimension_px = descriptor
                .content_bounds
                .width()
                .max(descriptor.content_bounds.height())
                * scale_factor.max(1.0);
            if descriptor.composition_mode == sui_scene::LayerCompositionMode::Scroll
                || max_dimension_px > DEFAULT_TILE_SIZE_PX as f32
            {
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

        let intersects_damage = damage_local.is_none_or(|damage| entry.rect.intersection(damage).is_some());
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
    let Some(((min_x, min_y), (max_x, max_y))) = tile_grid.tile_range_for_rect(local_visible) else {
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
                Some(current) => current.intersection(primitive.bounds()).unwrap_or(Rect::ZERO),
                None => primitive.bounds(),
            });
        }
        clip_node = node.parent.unwrap_or(ClipNodeId::ROOT);
    }
    bounds
}

fn build_tile_entry(
    descriptor: &sui_scene::SceneLayerDescriptor,
    content_version: u64,
    tile_grid: TileGrid,
    address: TileAddress,
    layer_local: bool,
    frame: &SceneFrame,
    layer_snapshot: &LayerSnapshot,
    snapshot_layers: &HashMap<SceneLayerId, LayerSnapshot>,
    text_engine: &mut TextEngine,
    feather_width: f32,
    frame_index: u64,
) -> Result<TileEntry> {
    let tile_local = tile_grid.tile_rect(address.tile_x, address.tile_y);
    let tile_scene = layer_local_to_scene(tile_local, descriptor);
    let mut fragment = build_cached_tile_fragment(
        frame,
        tile_scene,
        layer_snapshot,
        snapshot_layers,
        text_engine,
        feather_width,
    )?;
    if layer_local {
        fragment.transform_in_place(Transform::translation(
            -descriptor.bounds.x(),
            -descriptor.bounds.y(),
        ));
    }
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
        dirty: false,
        visible: true,
        layer_local,
        last_used_frame: frame_index,
        memory_cost,
        payload,
    })
}

fn build_cached_tile_fragment(
    frame: &SceneFrame,
    tile_scene_rect: Rect,
    layer_snapshot: &LayerSnapshot,
    snapshot_layers: &HashMap<SceneLayerId, LayerSnapshot>,
    text_engine: &mut TextEngine,
    feather_width: f32,
) -> Result<DrawOpArena> {
    let mut draw_ops = DrawOpArena::default();

    for item in &layer_snapshot.items {
        match item {
            CompositionItem::Packet(packet_id) => {
                let Some(packet_snapshot) = layer_snapshot.packets.iter().find(|packet| packet.id == *packet_id) else {
                    continue;
                };
                let mut tile_state = packet_snapshot.initial_state.clone();
                tile_state
                    .clip_stack
                    .push(ResolvedClipPrimitive::Rect(tile_scene_rect));
                let fragment = build_direct_packet(
                    frame,
                    &packet_snapshot.scene,
                    &tile_state,
                    text_engine,
                    feather_width,
                )?;
                draw_ops.append_fragment(&fragment);
            }
            CompositionItem::Layer(child_id) => {
                let Some(child_snapshot) = snapshot_layers.get(child_id) else {
                    continue;
                };
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
                    text_engine,
                    feather_width,
                )?;
                draw_ops.append_fragment(&child_fragment);
            }
        }
    }

    Ok(draw_ops)
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
    if coordinate_space == PacketCoordinateSpace::LayerLocal && normalization_origin != Vector::ZERO {
        let delta = Vector::new(-normalization_origin.x, -normalization_origin.y);
        snapshot.scene.translate(delta);
        snapshot.initial_state = translate_resolved_raster_state(&snapshot.initial_state, delta);
    }
    snapshot
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
        ResolvedClipPrimitive::Path {
            path,
            bounds,
            ..
        } => {
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

fn cached_layer_translation_fast_path(
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    effect_node: EffectNodeId,
) -> bool {
    transform_node == TransformNodeId::ROOT
        && clip_node == ClipNodeId::ROOT
        && effect_node == EffectNodeId::ROOT
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
        SceneCommand::StrokeRect { rect, brush, stroke } => {
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
        SceneCommand::StrokePath { path, brush, stroke } => {
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
            text.layout.text().hash(hasher);
            hash_text_style(text.layout.style(), hasher);
            text.layout.box_size().width.to_bits().hash(hasher);
            text.layout.box_size().height.to_bits().hash(hasher);
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

#[derive(Default)]
struct FrameResources {
    scene_vertices: Option<DynamicVertexBuffer>,
    clip_vertices: Option<DynamicVertexBuffer>,
    stencil: Option<StencilTarget>,
}

struct DynamicVertexBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
}

struct StencilTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "RGBA image pixel buffer length {} does not match {}x{} image size",
                pixels.len(),
                width,
                height
            )));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const DEFAULT_FEATHER_WIDTH: f32 = 1.0;
const AA_FLATTEN_TOLERANCE: f32 = 0.1;

impl WgpuRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.set_feather_width(feather_width);
        self
    }

    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    pub fn set_feather_width(&mut self, feather_width: f32) {
        self.feather_width = feather_width.max(0.0);
    }

    pub fn register_window(&mut self, window_id: WindowId, window: Arc<Window>) -> Result<()> {
        let physical_size = window.inner_size();
        let size = normalize_surface_size(physical_size.width, physical_size.height);
        let state = self.create_surface_state(window, size)?;

        self.surfaces.insert(window_id, state);
        self.offscreen_targets.remove(&window_id);
        Ok(())
    }

    pub fn remove_window(&mut self, window_id: WindowId) {
        self.surfaces.remove(&window_id);
        self.offscreen_targets.remove(&window_id);
        self.last_frames.remove(&window_id);
        self.last_frame_stats.remove(&window_id);
        self.compositors.remove(&window_id);
    }

    pub fn render(&mut self, frame: &SceneFrame) -> Result<()> {
        let viewport = normalize_framebuffer_size(frame.surface_size);
        let mut frame_stats = RendererFrameStats::default();

        if let Some(size) = viewport {
            if self.surfaces.contains_key(&frame.window_id) {
                frame_stats = self.render_surface(frame, size)?;
            } else {
                frame_stats = self.render_offscreen(frame, size)?;
            }
        }

        self.frames_rendered += 1;
        self.last_frames.insert(frame.window_id, frame.clone());
        self.last_frame_stats.insert(frame.window_id, frame_stats);
        Ok(())
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        self.capabilities
    }

    pub fn frames_rendered(&self) -> usize {
        self.frames_rendered
    }

    pub fn last_frame(&self, window_id: WindowId) -> Option<&SceneFrame> {
        self.last_frames.get(&window_id)
    }

    pub fn last_frame_stats(&self, window_id: WindowId) -> Option<RendererFrameStats> {
        self.last_frame_stats.get(&window_id).copied()
    }

    pub fn text_cache_snapshot(&self) -> RendererTextCacheSnapshot {
        self.text_engine
            .as_ref()
            .map(TextEngine::cache_snapshot)
            .unwrap_or_default()
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen render target available for screenshot capture",
                window_id.get()
            ))
        })?;

        let bytes_per_row = target.size.0 * 4;
        let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = padded_bytes_per_row as u64 * target.size.1 as u64;
        let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI screenshot readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI screenshot readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(target.size.1),
                },
            },
            wgpu::Extent3d {
                width: target.size.0,
                height: target.size.1,
                depth_or_array_layers: 1,
            },
        );
        shared.queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        shared
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| {
                Error::new(format!(
                    "failed to poll device for screenshot capture: {error}"
                ))
            })?;
        receiver
            .recv()
            .map_err(|error| {
                Error::new(format!(
                    "failed to receive screenshot readback completion: {error}"
                ))
            })?
            .map_err(|error| {
                Error::new(format!("failed to map screenshot readback buffer: {error}"))
            })?;

        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((target.size.0 * target.size.1 * 4) as usize);
        for row in 0..target.size.1 as usize {
            let start = row * padded_bytes_per_row as usize;
            let row_slice = &mapped[start..start + bytes_per_row as usize];
            for chunk in row_slice.chunks_exact(4) {
                pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            }
        }
        drop(mapped);
        buffer.unmap();

        RgbaImage::new(target.size.0, target.size.1, pixels)
    }

    fn ensure_shared(&mut self, compatible_surface: Option<&wgpu::Surface<'_>>) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
            }))
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SUI renderer device"),
            ..Default::default()
        }))
        .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI image bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        self.shared = Some(SharedRenderer {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
            image_bind_group_layout,
            image_sampler,
        });

        Ok(())
    }

    fn render_surface(&mut self, frame: &SceneFrame, size: (u32, u32)) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;
        self.configure_surface_if_needed(frame.window_id, size)?;

        let (frame_texture, suboptimal) = loop {
            let result = {
                let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                    Error::new(format!(
                        "missing surface for window {}",
                        frame.window_id.get()
                    ))
                })?;
                surface.surface.get_current_texture()
            };

            match result {
                wgpu::CurrentSurfaceTexture::Success(texture) => break (texture, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => break (texture, true),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.configure_surface(frame.window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    self.recreate_surface(frame.window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(RendererFrameStats::default());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(Error::new(
                        "wgpu surface acquisition triggered a validation error",
                    ));
                }
            }
        };

        let view = frame_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let format = {
            let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                Error::new(format!(
                    "missing surface for window {}",
                    frame.window_id.get()
                ))
            })?;
            surface.config.format
        };

        let frame_stats = self.encode_scene(frame, format, &view)?;
        frame_texture.present();

        if suboptimal {
            self.configure_surface(frame.window_id, size)?;
        }

        Ok(frame_stats)
    }

    fn render_offscreen(&mut self, frame: &SceneFrame, size: (u32, u32)) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let recreate = self
            .offscreen_targets
            .get(&frame.window_id)
            .is_none_or(|target| target.size != size || target.format != format);

        if recreate {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SUI offscreen frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            self.offscreen_targets.insert(
                frame.window_id,
                OffscreenTarget {
                    texture,
                    format,
                    size,
                },
            );
        }

        let target = self
            .offscreen_targets
            .get(&frame.window_id)
            .ok_or_else(|| {
                Error::new(format!(
                    "missing offscreen target for window {}",
                    frame.window_id.get()
                ))
            })?;
        let view = target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.encode_scene(frame, target.format, &view)
    }

    fn configure_surface_if_needed(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        if surface.config.width == size.0 && surface.config.height == size.1 {
            return Ok(());
        }

        self.configure_surface(window_id, size)
    }

    fn configure_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        let config = configure_surface(&surface.surface, &shared.adapter, &shared.device, size)?;
        surface.config = config;
        Ok(())
    }

    fn recreate_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let window = self
            .surfaces
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?
            .window
            .clone();
        let state = self.create_surface_state(window, size)?;
        self.surfaces.insert(window_id, state);
        Ok(())
    }

    fn encode_scene(
        &mut self,
        frame: &SceneFrame,
        target_format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
    ) -> Result<RendererFrameStats> {
        let feather_width = self.feather_width;
        let (draw_ops, compositor_stats) = {
            if self.text_engine.is_none() {
                self.text_engine = Some(TextEngine::new()?);
            }
            let text_engine = self
                .text_engine
                .as_mut()
                .expect("text engine initialized before draw-op construction");
            let compositor = self.compositors.entry(frame.window_id).or_default();
            let draw_ops = compositor.prepare_frame(frame, text_engine, feather_width)?;
            (draw_ops, compositor.last_frame_stats)
        };
        let framebuffer_size = normalize_framebuffer_size(frame.surface_size).unwrap_or((1, 1));
        let prepared = prepare_frame_batches(draw_ops, frame.viewport, framebuffer_size);
        let frame_stats = RendererFrameStats::from_prepared_frame(&prepared)
            .with_compositor_stats(compositor_stats);

        let mut image_bind_groups = HashMap::new();
        for pass in &prepared.passes {
            for draw in &pass.draws {
                let DrawOpKind::Image { handle } = draw.kind else {
                    continue;
                };
                if image_bind_groups.contains_key(&handle) {
                    continue;
                }

                let image = frame.image_registry.get(handle).ok_or_else(|| {
                    Error::new(format!("image handle {} is not registered", handle.get()))
                })?;
                image_bind_groups.insert(handle, self.ensure_image_bind_group(handle, image)?);
            }
        }

        {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            self.frame_resources
                .ensure_scene_buffer(&shared.device, prepared.scene_vertices.len() as u64 * VERTEX_SIZE);
            if let Some(buffer) = self.frame_resources.scene_vertices.as_ref() {
                if !prepared.scene_vertices.is_empty() {
                    shared.queue.write_buffer(
                        &buffer.buffer,
                        0,
                        bytemuck::cast_slice(&prepared.scene_vertices),
                    );
                }
            }

            self.frame_resources
                .ensure_clip_buffer(&shared.device, prepared.clip_vertices.len() as u64 * VERTEX_SIZE);
            if let Some(buffer) = self.frame_resources.clip_vertices.as_ref() {
                if !prepared.clip_vertices.is_empty() {
                    shared.queue.write_buffer(
                        &buffer.buffer,
                        0,
                        bytemuck::cast_slice(&prepared.clip_vertices),
                    );
                }
            }

            if prepared.passes.iter().any(|pass| !pass.clip_paths.is_empty()) {
                self.frame_resources
                    .ensure_stencil(&shared.device, framebuffer_size);
            }
        }

        let mut encoder = {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            shared
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SUI scene encoder"),
                })
        };
        if prepared.passes.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI scene clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
        } else {
            let shared = self
                .shared
                .as_mut()
                .expect("renderer shared state initialized");
            let scene_buffer = self
                .frame_resources
                .scene_vertices
                .as_ref()
                .expect("scene buffer available when rendering batched passes");
            let clip_buffer = self.frame_resources.clip_vertices.as_ref();
            let stencil_view = self.frame_resources.stencil.as_ref().map(|target| {
                let _ = &target.texture;
                &target.view
            });

            for (index, pass) in prepared.passes.iter().enumerate() {
                let load_op = if index == 0 {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    })
                } else {
                    wgpu::LoadOp::Load
                };
                let depth_stencil_attachment = if pass.clip_paths.is_empty() {
                    None
                } else {
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: stencil_view
                            .expect("stencil view available for path-clipped pass"),
                        depth_ops: None,
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Store,
                        }),
                    })
                };
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SUI scene batch pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });

                if !pass.clip_paths.is_empty() {
                    let clip_pipeline = shared.clip_pipeline(target_format);
                    render_pass.set_pipeline(clip_pipeline);
                    let clip_buffer = clip_buffer
                        .as_ref()
                        .expect("clip buffer available for path-clipped pass");
                    render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1);
                    for (clip_index, clip_path) in pass.clip_paths.iter().enumerate() {
                        render_pass.set_stencil_reference(clip_index as u32);
                        render_pass.set_vertex_buffer(
                            0,
                            vertex_buffer_slice(&clip_buffer.buffer, clip_path.vertices),
                        );
                        render_pass.draw(0..clip_path.vertices.len, 0..1);
                    }
                }

                let mut current_kind = None;
                for draw in &pass.draws {
                    match draw.clip_rect {
                        Some(scissor) => render_pass.set_scissor_rect(
                            scissor.x,
                            scissor.y,
                            scissor.width,
                            scissor.height,
                        ),
                        None => {
                            render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1)
                        }
                    }

                    if current_kind != Some(draw.kind) {
                        let pipeline = match (draw.kind, pass.clip_paths.is_empty()) {
                            (DrawOpKind::Solid, true) => shared.pipeline(target_format),
                            (DrawOpKind::Solid, false) => shared.clipped_pipeline(target_format),
                            (DrawOpKind::Image { .. }, true) => shared.image_pipeline(target_format),
                            (DrawOpKind::Image { .. }, false) => {
                                shared.clipped_image_pipeline(target_format)
                            }
                        };
                        render_pass.set_pipeline(pipeline);
                        current_kind = Some(draw.kind);
                    }

                    if !pass.clip_paths.is_empty() {
                        render_pass.set_stencil_reference(pass.clip_paths.len() as u32);
                    }

                    match draw.kind {
                        DrawOpKind::Solid => {}
                        DrawOpKind::Image { handle } => {
                            let bind_group = image_bind_groups
                                .get(&handle)
                                .expect("image bind group prepared before batched render pass");
                            render_pass.set_bind_group(0, bind_group, &[]);
                        }
                    }

                    render_pass.set_vertex_buffer(
                        0,
                        vertex_buffer_slice(&scene_buffer.buffer, draw.vertices),
                    );
                    render_pass.draw(0..draw.vertices.len, 0..1);
                }
            }
        }

        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .queue
            .submit([encoder.finish()]);
        Ok(frame_stats)
    }

    fn ensure_image_bind_group(
        &mut self,
        handle: ImageHandle,
        image: &RegisteredImage,
    ) -> Result<wgpu::BindGroup> {
        if let Some(cached) = self.image_cache.get(&handle) {
            return Ok(cached.bind_group.clone());
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let texture_format = match image.format() {
            RegisteredImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI image texture"),
            size: wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        shared.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width() * 4),
                rows_per_image: Some(image.height()),
            },
            wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI image bind group"),
            layout: &shared.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shared.image_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        self.image_cache.insert(
            handle,
            CachedImageTexture {
                _texture: texture,
                _view: view,
                bind_group: bind_group.clone(),
            },
        );

        Ok(bind_group)
    }

    fn create_surface_state(
        &mut self,
        window: Arc<Window>,
        size: (u32, u32),
    ) -> Result<SurfaceState> {
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| Error::new(format!("failed to create wgpu surface: {error}")))?;
        self.ensure_shared(Some(&surface))?;

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let config = configure_surface(&surface, &shared.adapter, &shared.device, size)?;

        Ok(SurfaceState {
            window,
            surface,
            config,
        })
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self {
            instance: wgpu::Instance::default(),
            feather_width: DEFAULT_FEATHER_WIDTH,
            frames_rendered: 0,
            capabilities: RendererCapabilities::default(),
            last_frames: HashMap::new(),
            last_frame_stats: HashMap::new(),
            shared: None,
            text_engine: None,
            image_cache: HashMap::new(),
            compositors: HashMap::new(),
            surfaces: HashMap::new(),
            offscreen_targets: HashMap::new(),
            frame_resources: FrameResources::default(),
        }
    }
}

impl fmt::Debug for WgpuRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuRenderer")
            .field("feather_width", &self.feather_width)
            .field("frames_rendered", &self.frames_rendered)
            .field("capabilities", &self.capabilities)
            .field("last_frame_count", &self.last_frames.len())
            .field("last_frame_stats_count", &self.last_frame_stats.len())
            .field("has_device", &self.shared.is_some())
            .field("surface_count", &self.surfaces.len())
            .finish()
    }
}

impl FrameResources {
    fn ensure_scene_buffer(&mut self, device: &wgpu::Device, size: u64) {
        Self::ensure_dynamic_buffer(&mut self.scene_vertices, device, size, "SUI scene vertices");
    }

    fn ensure_clip_buffer(&mut self, device: &wgpu::Device, size: u64) {
        Self::ensure_dynamic_buffer(&mut self.clip_vertices, device, size, "SUI clip vertices");
    }

    fn ensure_dynamic_buffer(
        slot: &mut Option<DynamicVertexBuffer>,
        device: &wgpu::Device,
        required_size: u64,
        label: &str,
    ) {
        if required_size == 0 {
            return;
        }

        let needs_recreate = slot
            .as_ref()
            .is_none_or(|buffer| buffer.capacity < required_size);
        if !needs_recreate {
            return;
        }

        let capacity = next_dynamic_buffer_capacity(required_size);
        *slot = Some(DynamicVertexBuffer {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        });
    }

    fn ensure_stencil(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let needs_recreate = self.stencil.as_ref().is_none_or(|target| target.size != size);
        if !needs_recreate {
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI scene stencil"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil = Some(StencilTarget {
            texture,
            view,
            size,
        });
    }
}

fn next_dynamic_buffer_capacity(required_size: u64) -> u64 {
    required_size.max(4096).next_power_of_two()
}

struct SharedRenderer {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: HashMap<(wgpu::TextureFormat, PipelineKind), wgpu::RenderPipeline>,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
}

impl SharedRenderer {
    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Solid)
    }

    fn clipped_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Clipped)
    }

    fn clip_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::ClipMask)
    }

    fn image_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Textured)
    }

    fn clipped_image_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::TexturedClipped)
    }

    fn pipeline_for(
        &mut self,
        format: wgpu::TextureFormat,
        kind: PipelineKind,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.entry((format, kind)).or_insert_with(|| {
            let shader_label = match kind {
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => {
                    "SUI solid scene shader"
                }
                PipelineKind::Textured | PipelineKind::TexturedClipped => {
                    "SUI textured scene shader"
                }
            };
            let shader_source = match kind {
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => {
                    SHADER_SOURCE
                }
                PipelineKind::Textured | PipelineKind::TexturedClipped => TEXTURED_SHADER_SOURCE,
            };
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(shader_label),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });

            let depth_stencil = match kind {
                PipelineKind::Solid | PipelineKind::Textured => None,
                PipelineKind::Clipped | PipelineKind::TexturedClipped => {
                    Some(wgpu::DepthStencilState {
                        format: STENCIL_FORMAT,
                        depth_write_enabled: Some(false),
                        depth_compare: Some(wgpu::CompareFunction::Always),
                        stencil: wgpu::StencilState {
                            front: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::Equal,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Keep,
                            },
                            back: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::Equal,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Keep,
                            },
                            read_mask: u32::MAX,
                            write_mask: 0,
                        },
                        bias: wgpu::DepthBiasState::default(),
                    })
                }
                PipelineKind::ClipMask => Some(wgpu::DepthStencilState {
                    format: STENCIL_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Equal,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::IncrementClamp,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Equal,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::IncrementClamp,
                        },
                        read_mask: u32::MAX,
                        write_mask: u32::MAX,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
            };
            let fragment_targets = [Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            let layout = match kind {
                PipelineKind::Textured | PipelineKind::TexturedClipped => Some(
                    self.device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("SUI textured scene pipeline layout"),
                            bind_group_layouts: &[Some(&self.image_bind_group_layout)],
                            immediate_size: 0,
                        }),
                ),
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => None,
            };

            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(match kind {
                        PipelineKind::Solid => "SUI solid scene pipeline",
                        PipelineKind::Clipped => "SUI clipped scene pipeline",
                        PipelineKind::Textured => "SUI textured scene pipeline",
                        PipelineKind::TexturedClipped => "SUI clipped textured scene pipeline",
                        PipelineKind::ClipMask => "SUI clip mask pipeline",
                    }),
                    layout: layout.as_ref(),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Vertex::layout()],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: match kind {
                        PipelineKind::ClipMask => None,
                        PipelineKind::Solid
                        | PipelineKind::Clipped
                        | PipelineKind::Textured
                        | PipelineKind::TexturedClipped => Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs_main"),
                            targets: &fragment_targets,
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                        }),
                    },
                    multiview_mask: None,
                    cache: None,
                })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PipelineKind {
    Solid,
    Clipped,
    Textured,
    TexturedClipped,
    ClipMask,
}

struct CachedImageTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

struct SurfaceState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct OffscreenTarget {
    texture: wgpu::Texture,
    format: wgpu::TextureFormat,
    size: (u32, u32),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
    tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2];

    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[derive(Clone, Copy)]
struct TessellatedPoint;

impl FillVertexConstructor<[f32; 2]> for TessellatedPoint {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

impl StrokeVertexConstructor<[f32; 2]> for TessellatedPoint {
    fn new_vertex(&mut self, vertex: StrokeVertex<'_, '_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

#[derive(Debug, Clone, Copy)]
struct MeshVertex {
    position: Point,
    color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphFaceCacheKey {
    data_ptr: usize,
    data_len: usize,
    face_index: u32,
}

impl GlyphFaceCacheKey {
    fn new(face: &ResolvedTextFace) -> Self {
        let data = face.shared_bytes();
        Self {
            data_ptr: data.as_ptr() as usize,
            data_len: data.len(),
            face_index: face.face_index(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    face: GlyphFaceCacheKey,
    glyph_id: u16,
    feather_width_bits: u32,
}

impl GlyphCacheKey {
    fn new(face: GlyphFaceCacheKey, glyph_id: u16, feather_width: f32) -> Self {
        Self {
            face,
            glyph_id,
            feather_width_bits: feather_width.to_bits(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CachedGlyphVertex {
    position: Point,
    coverage: f32,
}

#[derive(Debug, Default, Clone)]
struct CachedGlyphMesh {
    vertices: Vec<CachedGlyphVertex>,
    indices: Vec<u32>,
}

impl CachedGlyphMesh {
    fn push_vertex(&mut self, position: Point, coverage: f32) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CachedGlyphVertex { position, coverage });
        index
    }

    fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Default, Clone)]
struct SceneMesh {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl SceneMesh {
    fn colored_vertex(&mut self, position: Point, color: Color) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(MeshVertex { position, color });
        index
    }

    fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Clone)]
struct FlattenedContour {
    points: Vec<Point>,
    closed: bool,
}

#[derive(Debug, Clone, Copy)]
struct AaPathPoint {
    position: Point,
    normal: Vector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeatheredPathType {
    Open,
    Closed,
}

#[cfg(test)]
fn build_vertices(frame: &SceneFrame, text_engine: &mut TextEngine) -> Result<Vec<Vertex>> {
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)?;
    let mut vertices = Vec::new();
    for op in &draw_ops.draw_ops {
        vertices.extend_from_slice(draw_ops.scene_vertices(op.vertices));
    }
    Ok(vertices)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawOpKind {
    Solid,
    Image { handle: ImageHandle },
}

#[derive(Debug, Clone)]
struct DrawOp {
    kind: DrawOpKind,
    vertices: PreparedVertices,
    clip_rect: Option<Rect>,
    clip_state_index: usize,
}

#[derive(Debug, Default, Clone)]
struct DrawOpArena {
    scene_vertices: Vec<Vertex>,
    clip_vertices: Vec<Vertex>,
    clip_states: Vec<ClipState>,
    draw_ops: Vec<DrawOp>,
}

#[derive(Debug, Clone)]
struct ClipState {
    clip_paths: Vec<PreparedVertices>,
}

#[derive(Debug, Clone)]
struct PreparedFrameBatches {
    scene_vertices: Vec<Vertex>,
    clip_vertices: Vec<Vertex>,
    passes: Vec<PreparedPassBatch>,
}

#[derive(Debug, Clone)]
struct PreparedPassBatch {
    clip_state_index: usize,
    clip_paths: Vec<PreparedClipPath>,
    draws: Vec<PreparedDrawBatch>,
}

#[derive(Debug, Clone, Copy)]
struct PreparedClipPath {
    vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy)]
struct PreparedDrawBatch {
    kind: DrawOpKind,
    clip_rect: Option<ScissorRect>,
    vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedVertices {
    start: u32,
    len: u32,
}

impl PreparedVertices {
    fn offset(self, delta: u32) -> Self {
        Self {
            start: self.start + delta,
            len: self.len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn prepare_frame_batches(
    draw_ops: DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> PreparedFrameBatches {
    let passes = batch_draw_ops(&draw_ops, viewport, framebuffer_size);
    PreparedFrameBatches {
        scene_vertices: draw_ops.scene_vertices,
        clip_vertices: draw_ops.clip_vertices,
        passes,
    }
}

fn batch_draw_ops(
    draw_ops: &DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Vec<PreparedPassBatch> {
    let mut passes = Vec::new();

    for op in &draw_ops.draw_ops {
        let share_pass = passes
            .last()
            .is_some_and(|pass: &PreparedPassBatch| pass.clip_state_index == op.clip_state_index);
        if !share_pass {
            let clip_state = &draw_ops.clip_states[op.clip_state_index];
            passes.push(PreparedPassBatch {
                clip_state_index: op.clip_state_index,
                clip_paths: clip_state
                    .clip_paths
                    .iter()
                    .copied()
                    .map(|vertices| PreparedClipPath { vertices })
                    .collect(),
                draws: Vec::new(),
            });
        }

        let pass = passes
            .last_mut()
            .expect("prepared pass created before draw insertion");
        let clip_rect = op
            .clip_rect
            .and_then(|rect| rect_to_scissor(rect, viewport, framebuffer_size));
        if let Some(previous) = pass.draws.last_mut() {
            let previous_end = previous.vertices.start + previous.vertices.len;
            if previous.kind == op.kind
                && previous.clip_rect == clip_rect
                && previous_end == op.vertices.start
            {
                previous.vertices.len += op.vertices.len;
                continue;
            }
        }

        pass.draws.push(PreparedDrawBatch {
            kind: op.kind,
            clip_rect,
            vertices: op.vertices,
        });
    }

    passes
}

fn build_direct_packet(
    frame: &SceneFrame,
    scene: &Scene,
    initial_state: &ResolvedRasterState,
    text_engine: &mut TextEngine,
    feather_width: f32,
) -> Result<DrawOpArena> {
    let mut draw_ops = DrawOpArena::default();
    let mut state = SceneRasterState::from_resolved(initial_state, &mut draw_ops, frame.viewport)?;
    let mut builder = SceneDrawOpBuilder {
        frame,
        text_engine,
        feather_width,
        scratch_vertices: Vec::new(),
        clip_scratch_vertices: Vec::new(),
    };
    builder.build_scene(scene, &mut draw_ops, &mut state)?;
    Ok(draw_ops)
}

struct SceneDrawOpBuilder<'a> {
    frame: &'a SceneFrame,
    text_engine: &'a mut TextEngine,
    feather_width: f32,
    scratch_vertices: Vec<Vertex>,
    clip_scratch_vertices: Vec<Vertex>,
}

impl SceneDrawOpBuilder<'_> {
    fn build_scene(
        &mut self,
        scene: &Scene,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
    ) -> Result<()> {
        for command in scene.commands() {
            self.build_command(command, draw_ops, state)?;
        }

        Ok(())
    }

    fn build_command(
        &mut self,
        command: &SceneCommand,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
    ) -> Result<()> {
        let viewport = self.frame.viewport;

        match command {
            SceneCommand::Clear(color) => {
                self.scratch_vertices.clear();
                append_rect(
                    &mut self.scratch_vertices,
                    Rect::new(0.0, 0.0, viewport.width, viewport.height),
                    *color,
                    viewport,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::FillRect { rect, brush } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_painted_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *color,
                    viewport,
                    self.feather_width,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_stroke_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *color,
                    *stroke,
                    viewport,
                    self.feather_width,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::FillPath { path, brush } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_painted_path(
                    &mut self.scratch_vertices,
                    state,
                    path,
                    *color,
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::StrokePath {
                path,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_stroked_path(
                    &mut self.scratch_vertices,
                    state,
                    path,
                    *color,
                    *stroke,
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::DrawText(text) => {
                self.scratch_vertices.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_vertices,
                    state,
                    text,
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::DrawShapedText(text) => {
                self.scratch_vertices.clear();
                self.text_engine.append_shaped_text(
                    &mut self.scratch_vertices,
                    state,
                    text,
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
            SceneCommand::DrawImage { rect, source } => {
                self.scratch_vertices.clear();
                let image = self.frame.image_registry.get(source.image).ok_or_else(|| {
                    Error::new(format!(
                        "image handle {} is not registered",
                        source.image.get()
                    ))
                })?;
                append_image(&mut self.scratch_vertices, state, *rect, source, image, viewport);
                push_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                    },
                    &self.scratch_vertices,
                    state,
                );
            }
            SceneCommand::PushClip { rect } => {
                state.push_clip(*rect);
            }
            SceneCommand::PushClipPath { path } => {
                state.push_clip_path(path, viewport, draw_ops, &mut self.clip_scratch_vertices)?;
            }
            SceneCommand::PopClip => {
                state.pop_clip(draw_ops);
            }
            SceneCommand::PushTransform { transform } => {
                state.push_transform(*transform);
            }
            SceneCommand::PopTransform => {
                state.pop_transform();
            }
            SceneCommand::Layer(layer) => {
                return Err(Error::new(format!(
                    "retained direct packet compiler encountered nested layer {}",
                    layer.layer_id().get()
                )));
            }
            SceneCommand::Label { rect, text, color } => {
                self.scratch_vertices.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_vertices,
                    state,
                    &TextRun {
                        rect: *rect,
                        text: text.clone(),
                        style: TextStyle::new(*color),
                    },
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SceneRasterState {
    current_transform: Transform,
    transform_stack: Vec<Transform>,
    clip_stack: Vec<ClipPrimitive>,
    path_clip_state_id: u64,
    active_path_clips: Vec<PreparedVertices>,
    clip_state_index: usize,
}

impl SceneRasterState {
    fn new(draw_ops: &mut DrawOpArena) -> Self {
        let clip_state_index = draw_ops.push_clip_state(&[]);
        Self {
            current_transform: Transform::IDENTITY,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
            path_clip_state_id: 0,
            active_path_clips: Vec::new(),
            clip_state_index,
        }
    }

    fn from_resolved(
        resolved: &ResolvedRasterState,
        draw_ops: &mut DrawOpArena,
        viewport: Size,
    ) -> Result<Self> {
        let mut state = Self::new(draw_ops);
        state.current_transform = resolved.current_transform;
        state.transform_stack.clear();
        state.path_clip_state_id = 0;
        state.active_path_clips.clear();
        state.clip_stack.clear();

        for clip in &resolved.clip_stack {
            match clip {
                ResolvedClipPrimitive::Rect(rect) => {
                    state.clip_stack.push(ClipPrimitive::Rect(*rect));
                }
                ResolvedClipPrimitive::Path {
                    path,
                    bounds,
                    ..
                } => {
                    let mut scratch = Vec::new();
                    if !path.is_empty() && !viewport.is_empty() {
                        let lyon_path = build_lyon_path(path, Transform::IDENTITY);
                        append_tessellated_filled_lyon_path_vertices(
                            &mut scratch,
                            &lyon_path,
                            viewport,
                        )?;
                    }
                    let vertices = draw_ops.push_clip_vertices(&scratch);
                    state.active_path_clips.push(vertices);
                    state
                        .clip_stack
                        .push(ClipPrimitive::Path { bounds: *bounds });
                }
            }
        }

        state.clip_state_index = draw_ops.push_clip_state(&state.active_path_clips);
        Ok(state)
    }
}

#[derive(Debug, Clone)]
enum ClipPrimitive {
    Rect(Rect),
    Path { bounds: Rect },
}

impl ClipPrimitive {
    fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            Self::Path { bounds, .. } => *bounds,
        }
    }
}

impl SceneRasterState {
    fn push_clip(&mut self, rect: Rect) {
        let transformed = self.current_transform.transform_rect_bbox(rect);
        self.clip_stack.push(ClipPrimitive::Rect(transformed));
    }

    fn push_clip_path(
        &mut self,
        path: &ScenePath,
        viewport: Size,
        draw_ops: &mut DrawOpArena,
        scratch_vertices: &mut Vec<Vertex>,
    ) -> Result<()> {
        let bounds = self.current_transform.transform_rect_bbox(path.bounds());
        scratch_vertices.clear();
        if !path.is_empty() && !viewport.is_empty() {
            let lyon_path = build_lyon_path(path, self.current_transform);
            append_tessellated_filled_lyon_path_vertices(scratch_vertices, &lyon_path, viewport)?;
        }
        let vertices = draw_ops.push_clip_vertices(scratch_vertices);
        self.clip_stack.push(ClipPrimitive::Path { bounds });
        self.active_path_clips.push(vertices);
        self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
        self.clip_state_index = draw_ops.push_clip_state(&self.active_path_clips);
        Ok(())
    }

    fn pop_clip(&mut self, draw_ops: &mut DrawOpArena) {
        if matches!(self.clip_stack.pop(), Some(ClipPrimitive::Path { .. })) {
            let _ = self.active_path_clips.pop();
            self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
            self.clip_state_index = draw_ops.push_clip_state(&self.active_path_clips);
        }
    }

    fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(self.current_transform);
        self.current_transform = self.current_transform.then(transform);
    }

    fn pop_transform(&mut self) {
        self.current_transform = self.transform_stack.pop().unwrap_or(Transform::IDENTITY);
    }

    fn current_clip_bounds(&self) -> Option<Rect> {
        let mut clips = self.clip_stack.iter().map(ClipPrimitive::bounds);
        let first = clips.next()?;
        Some(clips.fold(first, |current, clip| {
            current.intersection(clip).unwrap_or(Rect::ZERO)
        }))
    }

    fn visible_rect(&self, rect: Rect) -> Option<Rect> {
        let transformed = self.current_transform.transform_rect_bbox(rect);

        match self.current_clip_bounds() {
            Some(clip) => transformed.intersection(clip),
            None => Some(transformed),
        }
    }
}

fn hash_transform(hasher: &mut DefaultHasher, transform: Transform) {
    transform.xx.to_bits().hash(hasher);
    transform.yx.to_bits().hash(hasher);
    transform.xy.to_bits().hash(hasher);
    transform.yy.to_bits().hash(hasher);
    transform.dx.to_bits().hash(hasher);
    transform.dy.to_bits().hash(hasher);
}

fn transform_scene_path(path: &ScenePath, transform: Transform) -> ScenePath {
    let mut builder = ScenePath::builder();
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                builder.move_to(transform.transform_point(*point));
            }
            PathElement::LineTo(point) => {
                builder.line_to(transform.transform_point(*point));
            }
            PathElement::QuadTo { ctrl, to } => {
                builder.quad_to(transform.transform_point(*ctrl), transform.transform_point(*to));
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                builder.cubic_to(
                    transform.transform_point(*ctrl1),
                    transform.transform_point(*ctrl2),
                    transform.transform_point(*to),
                );
            }
            PathElement::Close => {
                builder.close();
            }
        }
    }
    builder.build()
}

fn hash_rect(hasher: &mut DefaultHasher, rect: Rect) {
    rect.origin.x.to_bits().hash(hasher);
    rect.origin.y.to_bits().hash(hasher);
    rect.size.width.to_bits().hash(hasher);
    rect.size.height.to_bits().hash(hasher);
}

fn hash_point(hasher: &mut DefaultHasher, point: Point) {
    point.x.to_bits().hash(hasher);
    point.y.to_bits().hash(hasher);
}

fn hash_path(path: &ScenePath, transform: Transform) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_transform(&mut hasher, transform);
    hash_rect(&mut hasher, path.bounds());
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                0u8.hash(&mut hasher);
                hash_point(&mut hasher, *point);
            }
            PathElement::LineTo(point) => {
                1u8.hash(&mut hasher);
                hash_point(&mut hasher, *point);
            }
            PathElement::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_point(&mut hasher, *ctrl);
                hash_point(&mut hasher, *to);
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_point(&mut hasher, *ctrl1);
                hash_point(&mut hasher, *ctrl2);
                hash_point(&mut hasher, *to);
            }
            PathElement::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

#[derive(Debug, Default)]
struct TextEngine {
    system: TextSystem,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyphMesh>,
    glyph_cache_hits: usize,
    glyph_cache_misses: usize,
}

impl TextEngine {
    fn new() -> Result<Self> {
        Ok(Self::default())
    }

    fn append_text_run(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        text: &TextRun,
        font_registry: &FontRegistry,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if text.rect.is_empty() || text.text.is_empty() || viewport.is_empty() {
            return Ok(());
        }

        let layout = self.shape_text_run(text, font_registry)?;
        self.append_text_layout(
            vertices,
            state,
            Point::new(text.rect.x(), text.rect.y()),
            &layout,
            viewport,
            feather_width,
        )
    }

    fn append_shaped_text(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        text: &ShapedText,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        self.append_text_layout(
            vertices,
            state,
            text.origin,
            &text.layout,
            viewport,
            feather_width,
        )
    }

    fn append_text_layout(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if layout.measurement().width <= 0.0 || layout.measurement().height <= 0.0 {
            return Ok(());
        }

        let translated_bounds = layout.measurement().bounds.translate(origin.to_vector());
        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        let face = rustybuzz::Face::from_slice(layout.face().bytes(), layout.face().face_index())
            .ok_or_else(|| Error::new("failed to parse shaped text face data"))?;
        let layout_rect = Rect::from_origin_size(origin, layout.box_size());
        let face_key = GlyphFaceCacheKey::new(layout.face());

        for glyph in layout.glyphs() {
            if let Some(bounds) = glyph
                .bounds
                .map(|bounds| bounds.translate(origin.to_vector()))
            {
                if bounds.intersection(layout_rect).is_none() {
                    continue;
                }

                if let Some(clip) = state.current_clip_bounds() {
                    let transformed = state.current_transform.transform_rect_bbox(bounds);
                    if transformed.intersection(clip).is_none() {
                        continue;
                    }
                }
            }

            let mut translated_glyph = glyph.clone();
            translated_glyph.origin_x += origin.x;
            translated_glyph.origin_y += origin.y;
            if let Some(bounds) = translated_glyph.bounds {
                translated_glyph.bounds = Some(bounds.translate(origin.to_vector()));
            }

            if let Some(mesh) = self.cached_glyph_mesh(face_key, &face, glyph.glyph_id, feather_width)? {
                append_cached_glyph_mesh(
                    vertices,
                    mesh,
                    &translated_glyph,
                    layout.style().color,
                    state.current_transform,
                    viewport,
                );
            }
        }

        Ok(())
    }

    fn shape_text_run(&self, text: &TextRun, font_registry: &FontRegistry) -> Result<TextLayout> {
        self.system.shape_text_run(text, font_registry)
    }

    fn cached_glyph_mesh(
        &mut self,
        face_key: GlyphFaceCacheKey,
        face: &rustybuzz::Face<'_>,
        glyph_id: u16,
        feather_width: f32,
    ) -> Result<Option<&CachedGlyphMesh>> {
        let key = GlyphCacheKey::new(face_key, glyph_id, feather_width);
        if self.glyph_cache.contains_key(&key) {
            self.glyph_cache_hits += 1;
            return Ok(self.glyph_cache.get(&key));
        }

        self.glyph_cache_misses += 1;
        let Some(mesh) = build_cached_glyph_mesh(face, glyph_id, feather_width)? else {
            return Ok(None);
        };

        self.glyph_cache.insert(key.clone(), mesh);
        Ok(self.glyph_cache.get(&key))
    }

    #[cfg(test)]
    fn glyph_cache_stats(&self) -> (usize, usize, usize) {
        (
            self.glyph_cache.len(),
            self.glyph_cache_hits,
            self.glyph_cache_misses,
        )
    }

    fn cache_snapshot(&self) -> RendererTextCacheSnapshot {
        RendererTextCacheSnapshot {
            layout: self.system.layout_cache_snapshot(),
            glyph: GlyphCacheSnapshot {
                entries: self.glyph_cache.len(),
                hits: self.glyph_cache_hits,
                misses: self.glyph_cache_misses,
            },
        }
    }
}

fn build_cached_glyph_mesh(
    face: &rustybuzz::Face<'_>,
    glyph_id: u16,
    feather_width: f32,
) -> Result<Option<CachedGlyphMesh>> {
    let mut path_builder = LyonPath::builder();
    {
        let mut outline = CachedGlyphOutlineBuilder {
            builder: &mut path_builder,
            contour_open: false,
        };
        if face.outline_glyph(GlyphId(glyph_id), &mut outline).is_none() {
            return Ok(None);
        }
        outline.finish();
    }

    let path = path_builder.build();
    Ok(Some(build_local_glyph_mesh(&path, feather_width)?))
}

fn build_local_glyph_mesh(path: &LyonPath, feather_width: f32) -> Result<CachedGlyphMesh> {
    let mut mesh = CachedGlyphMesh::default();
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(path, &FillOptions::default(), &mut builder)
        .map_err(|error| Error::new(format!("failed to tessellate filled path: {error}")))?;

    for position in &buffers.vertices {
        mesh.push_vertex(Point::new(position[0], position[1]), 1.0);
    }
    mesh.indices.extend(buffers.indices.iter().copied());

    if feather_width > 0.0 {
        let contours = flatten_path_contours(path);
        for contour in &contours {
            if !contour.closed || contour.points.len() < 3 {
                continue;
            }

            let mut aa_points = build_closed_aa_points(&contour.points);
            if !normals_point_to_transparent_side(contour, &contours, feather_width) {
                for point in &mut aa_points {
                    point.normal = negate_vector(point.normal);
                }
            }

            append_local_fill_fringe_for_contour(&mut mesh, &aa_points, feather_width);
        }
    }

    Ok(mesh)
}

fn append_local_fill_fringe_for_contour(
    mesh: &mut CachedGlyphMesh,
    contour: &[AaPathPoint],
    feather_width: f32,
) {
    if contour.len() < 3 || feather_width <= 0.0 {
        return;
    }

    let base_index = mesh.vertices.len() as u32;
    let mut previous_inner = 0;
    let mut previous_outer = 0;

    for (index, point) in contour.iter().enumerate() {
        let delta = scale_vector(point.normal, 0.5 * feather_width);
        let inner = mesh.push_vertex(offset_point(point.position, negate_vector(delta)), 1.0);
        let outer = mesh.push_vertex(offset_point(point.position, delta), 0.0);

        if index > 0 {
            mesh.add_triangle(inner, previous_inner, previous_outer);
            mesh.add_triangle(previous_outer, outer, inner);
        }

        previous_inner = inner;
        previous_outer = outer;
    }

    let first_inner = base_index;
    let first_outer = base_index + 1;
    mesh.add_triangle(first_inner, previous_inner, previous_outer);
    mesh.add_triangle(previous_outer, first_outer, first_inner);
}

fn append_cached_glyph_mesh(
    vertices: &mut Vec<Vertex>,
    mesh: &CachedGlyphMesh,
    glyph: &SceneShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
) {
    let color = color.clamped();
    let rgba = shader_color(color);
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let positioned = Point::new(
            glyph.origin_x + (vertex.position.x * glyph.scale),
            glyph.origin_y + (vertex.position.y * glyph.scale),
        );
        let transformed = transform.transform_point(positioned);
        let ndc = to_ndc(transformed.x, transformed.y, viewport);
        vertices.push(Vertex {
            position: ndc,
            color: [rgba[0], rgba[1], rgba[2], color.alpha * vertex.coverage],
            tex_coords: [0.0, 0.0],
        });
    }
}

fn append_painted_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    if state.visible_rect(path.bounds()).is_none() {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    append_filled_aa_lyon_path(vertices, &lyon_path, color, viewport, feather_width)
}

fn append_stroked_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    let line_width = stroke.width.max(1.0);
    if state
        .visible_rect(path.bounds().inflate(
            (line_width + feather_width) * 0.5,
            (line_width + feather_width) * 0.5,
        ))
        .is_none()
    {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    append_feathered_stroke(
        vertices,
        &lyon_path,
        color,
        line_width,
        viewport,
        feather_width,
    );
    Ok(())
}

fn append_filled_aa_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    tessellate_filled_lyon_path(vertices, path, color, viewport)?;
    append_fill_fringe(vertices, path, color, viewport, feather_width);
    Ok(())
}

fn tessellate_filled_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
) -> Result<()> {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(path, &FillOptions::default(), &mut builder)
        .map_err(|error| Error::new(format!("failed to tessellate filled path: {error}")))?;

    append_indexed_triangles(vertices, &buffers, color, viewport);
    Ok(())
}

fn append_tessellated_filled_lyon_path_vertices(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    viewport: Size,
) -> Result<()> {
    tessellate_filled_lyon_path(vertices, path, Color::rgba(0.0, 0.0, 0.0, 0.0), viewport)
}

fn build_lyon_path(path: &ScenePath, transform: Transform) -> LyonPath {
    let mut builder = LyonPath::builder();
    let mut contour_open = false;

    for element in path.elements() {
        match element {
            PathElement::MoveTo(point_value) => {
                if contour_open {
                    LyonPathBuilder::end(&mut builder, false);
                }
                LyonPathBuilder::begin(
                    &mut builder,
                    transform_path_point(*point_value, transform),
                    &[],
                );
                contour_open = true;
            }
            PathElement::LineTo(point_value) => {
                if contour_open {
                    LyonPathBuilder::line_to(
                        &mut builder,
                        transform_path_point(*point_value, transform),
                        &[],
                    );
                }
            }
            PathElement::QuadTo { ctrl, to } => {
                if contour_open {
                    LyonPathBuilder::quadratic_bezier_to(
                        &mut builder,
                        transform_path_point(*ctrl, transform),
                        transform_path_point(*to, transform),
                        &[],
                    );
                }
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                if contour_open {
                    LyonPathBuilder::cubic_bezier_to(
                        &mut builder,
                        transform_path_point(*ctrl1, transform),
                        transform_path_point(*ctrl2, transform),
                        transform_path_point(*to, transform),
                        &[],
                    );
                }
            }
            PathElement::Close => {
                if contour_open {
                    LyonPathBuilder::end(&mut builder, true);
                    contour_open = false;
                }
            }
        }
    }

    if contour_open {
        LyonPathBuilder::end(&mut builder, false);
    }

    builder.build()
}

fn transform_path_point(point_value: Point, transform: Transform) -> lyon_path::math::Point {
    let scene = transform.transform_point(point_value);
    point(scene.x, scene.y)
}

fn append_indexed_triangles(
    vertices: &mut Vec<Vertex>,
    buffers: &VertexBuffers<[f32; 2], u32>,
    color: Color,
    viewport: Size,
) {
    if viewport.is_empty() {
        return;
    }

    let rgba = shader_color(color);
    for index in &buffers.indices {
        let position = buffers.vertices[*index as usize];
        let ndc = to_ndc(position[0], position[1], viewport);
        vertices.push(Vertex {
            position: [ndc[0], ndc[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        });
    }
}

struct CachedGlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    builder: &'a mut B,
    contour_open: bool,
}

impl<'a, B> CachedGlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    fn point(&self, x: f32, y: f32) -> lyon_path::math::Point {
        point(x, -y)
    }

    fn finish(&mut self) {
        if self.contour_open {
            LyonPathBuilder::end(self.builder, true);
            self.contour_open = false;
        }
    }
}

impl<B> ttf_parser::OutlineBuilder for CachedGlyphOutlineBuilder<'_, B>
where
    B: LyonPathBuilder,
{
    fn move_to(&mut self, x: f32, y: f32) {
        if self.contour_open {
            LyonPathBuilder::end(self.builder, true);
        }
        LyonPathBuilder::begin(self.builder, self.point(x, y), &[]);
        self.contour_open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        LyonPathBuilder::line_to(self.builder, self.point(x, y), &[]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        LyonPathBuilder::quadratic_bezier_to(
            self.builder,
            self.point(x1, y1),
            self.point(x, y),
            &[],
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        LyonPathBuilder::cubic_bezier_to(
            self.builder,
            self.point(x1, y1),
            self.point(x2, y2),
            self.point(x, y),
            &[],
        );
    }

    fn close(&mut self) {
        self.finish();
    }
}

fn append_image(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    source: &sui_scene::ImageSource,
    image: &RegisteredImage,
    viewport: Size,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let transformed = state.current_transform.transform_rect_bbox(rect);
    let Some(visible) = (match state.current_clip_bounds() {
        Some(clip) => transformed.intersection(clip),
        None => Some(transformed),
    }) else {
        return;
    };

    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let image_width = image.width() as f32;
    let image_height = image.height() as f32;
    let source_rect = source
        .source_rect
        .unwrap_or(Rect::new(0.0, 0.0, image_width, image_height));
    let source_min_x = source_rect.x().clamp(0.0, image_width);
    let source_min_y = source_rect.y().clamp(0.0, image_height);
    let source_max_x = source_rect.max_x().clamp(source_min_x, image_width);
    let source_max_y = source_rect.max_y().clamp(source_min_y, image_height);
    if source_max_x <= source_min_x || source_max_y <= source_min_y {
        return;
    }

    let u0 = source_min_x / image_width;
    let v0 = source_min_y / image_height;
    let u1 = source_max_x / image_width;
    let v1 = source_max_y / image_height;

    let left = ((visible.x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let right = ((visible.max_x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let top = ((visible.y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);
    let bottom = ((visible.max_y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);

    let uv_left = u0 + ((u1 - u0) * left);
    let uv_right = u0 + ((u1 - u0) * right);
    let uv_top = v0 + ((v1 - v0) * top);
    let uv_bottom = v0 + ((v1 - v0) * bottom);
    let min = to_ndc(visible.x(), visible.y(), viewport);
    let max = to_ndc(visible.max_x(), visible.max_y(), viewport);
    let tint = source.tint.unwrap_or(Color::WHITE).clamped().to_array();

    vertices.extend_from_slice(&[
        Vertex {
            position: [min[0], min[1]],
            color: tint,
            tex_coords: [uv_left, uv_top],
        },
        Vertex {
            position: [max[0], min[1]],
            color: tint,
            tex_coords: [uv_right, uv_top],
        },
        Vertex {
            position: [min[0], max[1]],
            color: tint,
            tex_coords: [uv_left, uv_bottom],
        },
        Vertex {
            position: [min[0], max[1]],
            color: tint,
            tex_coords: [uv_left, uv_bottom],
        },
        Vertex {
            position: [max[0], min[1]],
            color: tint,
            tex_coords: [uv_right, uv_top],
        },
        Vertex {
            position: [max[0], max[1]],
            color: tint,
            tex_coords: [uv_right, uv_bottom],
        },
    ]);
}

fn append_stroke_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
    feather_width: f32,
) {
    if rect.is_empty() {
        return;
    }

    let thickness = stroke
        .width
        .max(1.0)
        .min((rect.width() * 0.5).max(1.0))
        .min((rect.height() * 0.5).max(1.0));

    let top = Rect::new(rect.x(), rect.y(), rect.width(), thickness);
    let bottom = Rect::new(rect.x(), rect.max_y() - thickness, rect.width(), thickness);
    let left = Rect::new(
        rect.x(),
        rect.y() + thickness,
        thickness,
        (rect.height() - (thickness * 2.0)).max(0.0),
    );
    let right = Rect::new(
        rect.max_x() - thickness,
        rect.y() + thickness,
        thickness,
        (rect.height() - (thickness * 2.0)).max(0.0),
    );

    append_painted_rect(vertices, state, top, color, viewport, feather_width);
    append_painted_rect(vertices, state, bottom, color, viewport, feather_width);
    append_painted_rect(vertices, state, left, color, viewport, feather_width);
    append_painted_rect(vertices, state, right, color, viewport, feather_width);
}

fn append_painted_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if let Some(visible) = state.visible_rect(rect) {
        append_feathered_rect(vertices, visible, color, viewport, feather_width);
    }
}

fn append_feathered_rect(
    vertices: &mut Vec<Vertex>,
    rect: Rect,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    append_rect(vertices, rect, color, viewport);

    if feather_width <= 0.0 {
        return;
    }

    let points = [
        Point::new(rect.x(), rect.y()),
        Point::new(rect.max_x(), rect.y()),
        Point::new(rect.max_x(), rect.max_y()),
        Point::new(rect.x(), rect.max_y()),
    ];
    let aa_points = build_closed_aa_points(&points);
    append_fill_fringe_for_contour(vertices, &aa_points, color, viewport, feather_width);
}

fn append_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, viewport: Size) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let min = to_ndc(rect.x(), rect.y(), viewport);
    let max = to_ndc(rect.max_x(), rect.max_y(), viewport);
    let rgba = shader_color(color);

    vertices.extend_from_slice(&[
        Vertex {
            position: [min[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
    ]);
}

fn push_draw_op(
    draw_ops: &mut DrawOpArena,
    kind: DrawOpKind,
    vertices: &[Vertex],
    state: &SceneRasterState,
) {
    if vertices.is_empty() {
        return;
    }

    let vertex_span = draw_ops.push_scene_vertices(vertices);
    draw_ops.draw_ops.push(DrawOp {
        kind,
        vertices: vertex_span,
        clip_rect: state.current_clip_bounds(),
        clip_state_index: state.clip_state_index,
    });
}

impl DrawOpArena {
    fn transform_in_place(&mut self, transform: Transform) {
        if transform.is_identity() {
            return;
        }

        for vertex in &mut self.scene_vertices {
            let point = transform.transform_point(Point::new(vertex.position[0], vertex.position[1]));
            vertex.position = [point.x, point.y];
        }
        for vertex in &mut self.clip_vertices {
            let point = transform.transform_point(Point::new(vertex.position[0], vertex.position[1]));
            vertex.position = [point.x, point.y];
        }
        for draw_op in &mut self.draw_ops {
            draw_op.clip_rect = draw_op
                .clip_rect
                .map(|rect| transform.transform_rect_bbox(rect));
        }
    }

    fn append_transformed_fragment(&mut self, fragment: &DrawOpArena, transform: Transform) {
        if transform.is_identity() {
            self.append_fragment(fragment);
            return;
        }

        let mut transformed = fragment.clone();
        transformed.transform_in_place(transform);
        self.append_fragment(&transformed);
    }

    fn append_fragment(&mut self, fragment: &DrawOpArena) {
        let scene_delta = self.scene_vertices.len() as u32;
        let clip_delta = self.clip_vertices.len() as u32;
        let clip_state_delta = self.clip_states.len();

        self.scene_vertices.extend_from_slice(&fragment.scene_vertices);
        self.clip_vertices.extend_from_slice(&fragment.clip_vertices);
        self.clip_states.extend(fragment.clip_states.iter().map(|clip_state| ClipState {
            clip_paths: clip_state
                .clip_paths
                .iter()
                .copied()
                .map(|vertices| vertices.offset(clip_delta))
                .collect(),
        }));
        self.draw_ops.extend(fragment.draw_ops.iter().cloned().map(|mut draw_op| {
            draw_op.vertices = draw_op.vertices.offset(scene_delta);
            draw_op.clip_state_index += clip_state_delta;
            draw_op
        }));
    }

    fn byte_size(&self) -> usize {
        self.scene_vertices.len() * std::mem::size_of::<Vertex>()
            + self.clip_vertices.len() * std::mem::size_of::<Vertex>()
            + self.clip_states.iter().map(|clip| clip.clip_paths.len() * std::mem::size_of::<PreparedVertices>()).sum::<usize>()
            + self.draw_ops.len() * std::mem::size_of::<DrawOp>()
    }

    fn push_scene_vertices(&mut self, vertices: &[Vertex]) -> PreparedVertices {
        let start = self.scene_vertices.len() as u32;
        self.scene_vertices.extend_from_slice(vertices);
        PreparedVertices {
            start,
            len: vertices.len() as u32,
        }
    }

    fn push_clip_vertices(&mut self, vertices: &[Vertex]) -> PreparedVertices {
        let start = self.clip_vertices.len() as u32;
        self.clip_vertices.extend_from_slice(vertices);
        PreparedVertices {
            start,
            len: vertices.len() as u32,
        }
    }

    fn push_clip_state(&mut self, clip_paths: &[PreparedVertices]) -> usize {
        self.clip_states.push(ClipState {
            clip_paths: clip_paths.to_vec(),
        });
        self.clip_states.len() - 1
    }

    #[cfg(test)]
    fn scene_vertices(&self, span: PreparedVertices) -> &[Vertex] {
        &self.scene_vertices[span.start as usize..(span.start + span.len) as usize]
    }
}

const VERTEX_SIZE: u64 = std::mem::size_of::<Vertex>() as u64;

fn vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * VERTEX_SIZE;
    let end = start + vertices.len as u64 * VERTEX_SIZE;
    buffer.slice(start..end)
}

fn rect_to_scissor(
    rect: Rect,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Option<ScissorRect> {
    if rect.is_empty() || viewport.is_empty() {
        return None;
    }

    let framebuffer_width = framebuffer_size.0.max(1);
    let framebuffer_height = framebuffer_size.1.max(1);
    let scale_x = framebuffer_width as f32 / viewport.width.max(1.0);
    let scale_y = framebuffer_height as f32 / viewport.height.max(1.0);

    let min_x = (rect.x().max(0.0) * scale_x)
        .floor()
        .clamp(0.0, framebuffer_width as f32) as u32;
    let min_y = (rect.y().max(0.0) * scale_y)
        .floor()
        .clamp(0.0, framebuffer_height as f32) as u32;
    let max_x = ((rect.x() + rect.width()).min(viewport.width) * scale_x)
        .ceil()
        .clamp(0.0, framebuffer_width as f32) as u32;
    let max_y = ((rect.y() + rect.height()).min(viewport.height) * scale_y)
        .ceil()
        .clamp(0.0, framebuffer_height as f32) as u32;

    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let scissor = ScissorRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    if scissor.x == 0
        && scissor.y == 0
        && scissor.width == framebuffer_width
        && scissor.height == framebuffer_height
    {
        None
    } else {
        Some(scissor)
    }
}

fn to_ndc(x: f32, y: f32, viewport: Size) -> [f32; 2] {
    [
        ((x / viewport.width) * 2.0) - 1.0,
        1.0 - ((y / viewport.height) * 2.0),
    ]
}

fn normalize_framebuffer_size(size: Size) -> Option<(u32, u32)> {
    if size.is_empty() {
        None
    } else {
        Some(normalize_surface_size(
            size.width.round() as u32,
            size.height.round() as u32,
        ))
    }
}

fn normalize_surface_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

fn append_fill_fringe(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        return;
    }

    let contours = flatten_path_contours(path);

    for contour in &contours {
        if !contour.closed || contour.points.len() < 3 {
            continue;
        }

        let mut aa_points = build_closed_aa_points(&contour.points);
        if !normals_point_to_transparent_side(contour, &contours, feather_width) {
            for point in &mut aa_points {
                point.normal = negate_vector(point.normal);
            }
        }

        append_fill_fringe_for_contour(vertices, &aa_points, color, viewport, feather_width);
    }
}

fn append_fill_fringe_for_contour(
    vertices: &mut Vec<Vertex>,
    contour: &[AaPathPoint],
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if contour.len() < 3 || viewport.is_empty() || feather_width <= 0.0 {
        return;
    }

    let mut mesh = SceneMesh::default();
    let transparent = Color::TRANSPARENT;
    let mut previous_inner = 0;
    let mut previous_outer = 0;

    for (index, point) in contour.iter().enumerate() {
        let delta = scale_vector(point.normal, 0.5 * feather_width);
        let inner = mesh.colored_vertex(offset_point(point.position, negate_vector(delta)), color);
        let outer = mesh.colored_vertex(offset_point(point.position, delta), transparent);

        if index > 0 {
            mesh.add_triangle(inner, previous_inner, previous_outer);
            mesh.add_triangle(previous_outer, outer, inner);
        }

        previous_inner = inner;
        previous_outer = outer;
    }

    let first_inner = 0;
    let first_outer = 1;
    mesh.add_triangle(first_inner, previous_inner, previous_outer);
    mesh.add_triangle(previous_outer, first_outer, first_inner);

    append_scene_mesh(vertices, &mesh, viewport);
}

fn append_feathered_stroke(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    line_width: f32,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        append_hard_stroked_lyon_path(vertices, path, color, line_width, viewport);
        return;
    }

    let contours = flatten_path_contours(path);

    for contour in contours {
        let path_type = if contour.closed {
            FeatheredPathType::Closed
        } else {
            FeatheredPathType::Open
        };

        let aa_points = if contour.closed {
            build_closed_aa_points(&contour.points)
        } else {
            build_open_aa_points(&contour.points)
        };

        append_stroke_contour(
            vertices,
            &aa_points,
            path_type,
            color,
            line_width,
            viewport,
            feather_width,
        );
    }
}

fn append_hard_stroked_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    line_width: f32,
    viewport: Size,
) {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = StrokeTessellator::new();
    if tessellator
        .tessellate_path(
            path,
            &StrokeOptions::default().with_line_width(line_width),
            &mut builder,
        )
        .is_ok()
    {
        append_indexed_triangles(vertices, &buffers, color, viewport);
    }
}

fn append_stroke_contour(
    vertices: &mut Vec<Vertex>,
    path: &[AaPathPoint],
    path_type: FeatheredPathType,
    color: Color,
    line_width: f32,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        return;
    }

    let n = path.len() as u32;
    if n < 2 || viewport.is_empty() || line_width <= 0.0 {
        return;
    }

    let transparent = Color::TRANSPARENT;
    let mut mesh = SceneMesh::default();

    let thin_line = line_width <= 0.9 * feather_width;
    if thin_line {
        let opacity = (line_width / feather_width).clamp(0.0, 1.0);
        let mid_color = multiply_color_alpha(color, opacity);
        let mut previous_base = 0;

        for (index, point) in path.iter().enumerate() {
            let outer = mesh.colored_vertex(
                offset_point(point.position, scale_vector(point.normal, feather_width)),
                transparent,
            );
            let middle = mesh.colored_vertex(point.position, mid_color);
            let inner = mesh.colored_vertex(
                offset_point(point.position, scale_vector(point.normal, -feather_width)),
                transparent,
            );

            if path_type == FeatheredPathType::Closed || index > 0 {
                mesh.add_triangle(previous_base + 0, previous_base + 1, outer);
                mesh.add_triangle(previous_base + 1, outer, middle);
                mesh.add_triangle(previous_base + 1, previous_base + 2, middle);
                mesh.add_triangle(previous_base + 2, middle, inner);
            }

            previous_base = outer;
        }

        if path_type == FeatheredPathType::Closed {
            mesh.add_triangle(previous_base + 0, previous_base + 1, 0);
            mesh.add_triangle(previous_base + 1, 0, 1);
            mesh.add_triangle(previous_base + 1, previous_base + 2, 1);
            mesh.add_triangle(previous_base + 2, 1, 2);
        }
    } else {
        let inner_radius = 0.5 * (line_width - feather_width);
        let outer_radius = 0.5 * (line_width + feather_width);

        match path_type {
            FeatheredPathType::Closed => {
                let mut previous_base = 0;

                for (index, point) in path.iter().enumerate() {
                    let outer_pos = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, outer_radius)),
                        transparent,
                    );
                    let inner_pos = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, inner_radius)),
                        color,
                    );
                    let inner_neg = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                        color,
                    );
                    let outer_neg = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                        transparent,
                    );

                    if index > 0 {
                        mesh.add_triangle(previous_base + 0, previous_base + 1, outer_pos);
                        mesh.add_triangle(previous_base + 1, outer_pos, inner_pos);
                        mesh.add_triangle(previous_base + 1, previous_base + 2, inner_pos);
                        mesh.add_triangle(previous_base + 2, inner_pos, inner_neg);
                        mesh.add_triangle(previous_base + 2, previous_base + 3, inner_neg);
                        mesh.add_triangle(previous_base + 3, inner_neg, outer_neg);
                    }

                    previous_base = outer_pos;
                }

                mesh.add_triangle(previous_base + 0, previous_base + 1, 0);
                mesh.add_triangle(previous_base + 1, 0, 1);
                mesh.add_triangle(previous_base + 1, previous_base + 2, 1);
                mesh.add_triangle(previous_base + 2, 1, 2);
                mesh.add_triangle(previous_base + 2, previous_base + 3, 2);
                mesh.add_triangle(previous_base + 3, 2, 3);
            }
            FeatheredPathType::Open => {
                let first = path[0];
                let first_extrude = scale_vector(vector_rot90(first.normal), feather_width);
                let first_base = mesh.colored_vertex(
                    offset_point(
                        offset_point(first.position, scale_vector(first.normal, outer_radius)),
                        first_extrude,
                    ),
                    transparent,
                );
                mesh.colored_vertex(
                    offset_point(first.position, scale_vector(first.normal, inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(first.position, scale_vector(first.normal, -inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(
                        offset_point(first.position, scale_vector(first.normal, -outer_radius)),
                        first_extrude,
                    ),
                    transparent,
                );
                mesh.add_triangle(first_base + 0, first_base + 1, first_base + 2);
                mesh.add_triangle(first_base + 0, first_base + 2, first_base + 3);

                let mut previous_base = first_base;
                for point in path.iter().skip(1).take(path.len().saturating_sub(2)) {
                    let base = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, outer_radius)),
                        transparent,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, inner_radius)),
                        color,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                        color,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                        transparent,
                    );

                    mesh.add_triangle(previous_base + 0, previous_base + 1, base + 0);
                    mesh.add_triangle(previous_base + 1, base + 0, base + 1);
                    mesh.add_triangle(previous_base + 1, previous_base + 2, base + 1);
                    mesh.add_triangle(previous_base + 2, base + 1, base + 2);
                    mesh.add_triangle(previous_base + 2, previous_base + 3, base + 2);
                    mesh.add_triangle(previous_base + 3, base + 2, base + 3);

                    previous_base = base;
                }

                let last = path[path.len() - 1];
                let last_extrude = scale_vector(vector_rot90(last.normal), -feather_width);
                let last_base = mesh.colored_vertex(
                    offset_point(
                        offset_point(last.position, scale_vector(last.normal, outer_radius)),
                        last_extrude,
                    ),
                    transparent,
                );
                mesh.colored_vertex(
                    offset_point(last.position, scale_vector(last.normal, inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(last.position, scale_vector(last.normal, -inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(
                        offset_point(last.position, scale_vector(last.normal, -outer_radius)),
                        last_extrude,
                    ),
                    transparent,
                );

                mesh.add_triangle(previous_base + 0, previous_base + 1, last_base + 0);
                mesh.add_triangle(previous_base + 1, last_base + 0, last_base + 1);
                mesh.add_triangle(previous_base + 1, previous_base + 2, last_base + 1);
                mesh.add_triangle(previous_base + 2, last_base + 1, last_base + 2);
                mesh.add_triangle(previous_base + 2, previous_base + 3, last_base + 2);
                mesh.add_triangle(previous_base + 3, last_base + 2, last_base + 3);
                mesh.add_triangle(last_base + 0, last_base + 1, last_base + 2);
                mesh.add_triangle(last_base + 0, last_base + 2, last_base + 3);
            }
        }
    }

    append_scene_mesh(vertices, &mesh, viewport);
}

fn append_scene_mesh(vertices: &mut Vec<Vertex>, mesh: &SceneMesh, viewport: Size) {
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let ndc = to_ndc(vertex.position.x, vertex.position.y, viewport);
        vertices.push(Vertex {
            position: ndc,
            color: shader_color(vertex.color),
            tex_coords: [0.0, 0.0],
        });
    }
}

fn shader_color(color: Color) -> [f32; 4] {
    let color = color.clamped();
    let to_linear = match color.space {
        ColorSpace::LinearSrgb => |channel: f32| channel,
        ColorSpace::Srgb | ColorSpace::DisplayP3 => srgb_transfer_to_linear,
    };

    [
        to_linear(color.red),
        to_linear(color.green),
        to_linear(color.blue),
        color.alpha,
    ]
}

fn srgb_transfer_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn flatten_path_contours(path: &LyonPath) -> Vec<FlattenedContour> {
    let mut contours = Vec::new();
    let mut current = Vec::new();

    for event in path.iter().flattened(AA_FLATTEN_TOLERANCE) {
        match event {
            PathEvent::Begin { at } => {
                current.clear();
                current.push(Point::new(at.x, at.y));
            }
            PathEvent::Line { to, .. } => {
                let point = Point::new(to.x, to.y);
                if current
                    .last()
                    .is_none_or(|last| !points_nearly_equal(*last, point))
                {
                    current.push(point);
                }
            }
            PathEvent::End { close, .. } => {
                if close
                    && current.len() > 1
                    && points_nearly_equal(current[0], *current.last().unwrap_or(&current[0]))
                {
                    current.pop();
                }

                if current.len() >= if close { 3 } else { 2 } {
                    contours.push(FlattenedContour {
                        points: std::mem::take(&mut current),
                        closed: close,
                    });
                } else {
                    current.clear();
                }
            }
            PathEvent::Quadratic { .. } | PathEvent::Cubic { .. } => {
                unreachable!("flattened path iteration should not yield curve events")
            }
        }
    }

    contours
}

fn build_open_aa_points(points: &[Point]) -> Vec<AaPathPoint> {
    if points.len() < 2 {
        return Vec::new();
    }

    if points.len() == 2 {
        let normal = vector_rot90(vector_normalize(points[1] - points[0]));
        return vec![
            AaPathPoint {
                position: points[0],
                normal,
            },
            AaPathPoint {
                position: points[1],
                normal,
            },
        ];
    }

    let mut aa_points = Vec::with_capacity(points.len() * 2);
    let mut previous_normal = vector_rot90(vector_normalize(points[1] - points[0]));
    aa_points.push(AaPathPoint {
        position: points[0],
        normal: previous_normal,
    });

    for index in 1..points.len() - 1 {
        let mut next_normal = vector_rot90(vector_normalize(points[index + 1] - points[index]));
        if vector_is_zero(previous_normal) {
            previous_normal = next_normal;
        } else if vector_is_zero(next_normal) {
            next_normal = previous_normal;
        }

        let averaged = scale_vector(previous_normal + next_normal, 0.5);
        let length_sq = vector_length_sq(averaged);
        if length_sq < 0.5 {
            let center_normal = vector_normalize(averaged);
            let previous_cut = scale_vector(previous_normal + center_normal, 0.5);
            let next_cut = scale_vector(next_normal + center_normal, 0.5);
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(
                    previous_cut,
                    1.0 / vector_length_sq(previous_cut).max(1.0e-6),
                ),
            });
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(next_cut, 1.0 / vector_length_sq(next_cut).max(1.0e-6)),
            });
        } else {
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(averaged, 1.0 / length_sq),
            });
        }

        previous_normal = next_normal;
    }

    aa_points.push(AaPathPoint {
        position: points[points.len() - 1],
        normal: vector_rot90(vector_normalize(
            points[points.len() - 1] - points[points.len() - 2],
        )),
    });
    aa_points
}

fn build_closed_aa_points(points: &[Point]) -> Vec<AaPathPoint> {
    if points.len() < 3 {
        return Vec::new();
    }

    let mut aa_points = Vec::with_capacity(points.len());
    let mut previous_normal = vector_rot90(vector_normalize(points[0] - points[points.len() - 1]));

    for index in 0..points.len() {
        let next_index = if index + 1 == points.len() {
            0
        } else {
            index + 1
        };
        let mut next_normal = vector_rot90(vector_normalize(points[next_index] - points[index]));
        if vector_is_zero(previous_normal) {
            previous_normal = next_normal;
        } else if vector_is_zero(next_normal) {
            next_normal = previous_normal;
        }

        let averaged = scale_vector(previous_normal + next_normal, 0.5);
        let length_sq = vector_length_sq(averaged).max(1.0e-6);
        aa_points.push(AaPathPoint {
            position: points[index],
            normal: scale_vector(averaged, 1.0 / length_sq),
        });
        previous_normal = next_normal;
    }

    aa_points
}

fn normals_point_to_transparent_side(
    contour: &FlattenedContour,
    contours: &[FlattenedContour],
    feather_width: f32,
) -> bool {
    for window in contour.points.windows(2) {
        let edge = window[1] - window[0];
        let edge_length_sq = vector_length_sq(edge);
        if edge_length_sq <= 1.0e-6 {
            continue;
        }

        let midpoint = Point::new(
            (window[0].x + window[1].x) * 0.5,
            (window[0].y + window[1].y) * 0.5,
        );
        let normal = vector_rot90(vector_normalize(edge));
        let sample = offset_point(midpoint, scale_vector(normal, -0.25 * feather_width));
        return point_in_filled_path(sample, contours);
    }

    true
}

fn point_in_filled_path(point: Point, contours: &[FlattenedContour]) -> bool {
    let mut inside = false;

    for contour in contours {
        if contour.closed && point_in_polygon(point, &contour.points) {
            inside = !inside;
        }
    }

    inside
}

fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    let mut previous = *polygon.last().unwrap_or(&Point::ZERO);

    for current in polygon {
        let intersects = ((current.y > point.y) != (previous.y > point.y))
            && (point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x);
        if intersects {
            inside = !inside;
        }
        previous = *current;
    }

    inside
}

fn points_nearly_equal(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= 1.0e-4 && (a.y - b.y).abs() <= 1.0e-4
}

fn vector_length_sq(vector: Vector) -> f32 {
    vector.x * vector.x + vector.y * vector.y
}

fn vector_is_zero(vector: Vector) -> bool {
    vector_length_sq(vector) <= 1.0e-6
}

fn vector_normalize(vector: Vector) -> Vector {
    let length_sq = vector_length_sq(vector);
    if length_sq <= 1.0e-6 {
        Vector::ZERO
    } else {
        let length = length_sq.sqrt();
        Vector::new(vector.x / length, vector.y / length)
    }
}

fn vector_rot90(vector: Vector) -> Vector {
    Vector::new(vector.y, -vector.x)
}

fn scale_vector(vector: Vector, factor: f32) -> Vector {
    Vector::new(vector.x * factor, vector.y * factor)
}

fn negate_vector(vector: Vector) -> Vector {
    Vector::new(-vector.x, -vector.y)
}

fn offset_point(point: Point, offset: Vector) -> Point {
    Point::new(point.x + offset.x, point.y + offset.y)
}

fn multiply_color_alpha(color: Color, factor: f32) -> Color {
    color.with_alpha(color.alpha * factor)
}

fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}

fn configure_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(adapter, size.0, size.1)
        .ok_or_else(|| Error::new("wgpu adapter does not support presenting to this surface"))?;
    config.format = preferred_surface_format(&surface.get_capabilities(adapter).formats)
        .unwrap_or(config.format);
    surface.configure(device, &config);
    Ok(config)
}

const SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const TEXTURED_SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
};

@group(0) @binding(0)
var image_sampler: sampler;

@group(0) @binding(1)
var image_texture: texture_2d<f32>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.tex_coords = tex_coords;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, in.tex_coords) * in.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::{
        ClipState, CompositionContainerId, DEFAULT_FEATHER_WIDTH, DrawOp, DrawOpArena,
        DrawOpKind, PreparedClipPath, PreparedDrawBatch, PreparedFrameBatches,
        PreparedPassBatch, PreparedVertices, RendererFrameStats, RetainedCompositorState,
        RetainedLayerRenderMode,
        RetainedPacketId, ScissorRect, TextEngine, VERTEX_SIZE, Vertex, WgpuRenderer,
        batch_draw_ops, build_vertices, prepare_frame_batches, shader_color, to_ndc,
    };
    use std::sync::Arc;
    use sui_core::{
        Color, FontHandle, ImageHandle, Path, Point, Rect, Size, Transform, WidgetId, WindowId,
    };
    use sui_scene::{
        ImageRegistry, ImageSource, LayerCachePolicy, LayerCompositionMode, RegisteredImage,
        Scene, SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor, SceneLayerId,
        SceneLayerUpdate, SceneLayerUpdateKind, StrokeStyle,
    };
    use sui_text::{FontRegistry, RegisteredFont, ShapedText, TextRun, TextStyle, TextSystem};

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
            .expect("system font available for renderer tests");

        font_db
            .with_face_data(font_id, |font_data, face_index| {
                RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
            })
            .expect("font data should be readable from system font database")
    }

    fn content_update(widget_id: WidgetId) -> SceneLayerUpdate {
        SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Content,
            SceneLayerDescriptor::new(SceneLayerId::from_widget(widget_id), widget_id, Rect::ZERO),
        )
        .with_damage(Rect::ZERO)
    }

    fn content_updates<const N: usize>(widget_ids: [WidgetId; N]) -> Vec<SceneLayerUpdate> {
        widget_ids.into_iter().map(content_update).collect()
    }

    fn prepare_with_compositor(
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        compositor: &mut RetainedCompositorState,
    ) -> sui_core::Result<DrawOpArena> {
        compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)
    }

    fn packet_signature(
        compositor: &RetainedCompositorState,
        container: CompositionContainerId,
    ) -> u64 {
        compositor.packets[&RetainedPacketId {
            container,
            segment_index: 0,
        }]
            .signature
    }

    #[test]
    fn build_vertices_applies_clip_and_transform_to_fill_rects() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::PushTransform {
            transform: Transform::translation(10.0, 5.0),
        });
        scene.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 16.0, 12.0),
        });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 3.0, 20.0, 10.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(1),
                viewport: Size::new(100.0, 100.0),
                surface_size: Size::new(100.0, 100.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        let expected_min = to_ndc(14.0, 8.0, Size::new(100.0, 100.0));
        let expected_max = to_ndc(26.0, 17.0, Size::new(100.0, 100.0));

        assert!(vertices.len() > 6);
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_min)
        );
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_max)
        );
        assert!(vertices.iter().any(|vertex| vertex.color[3] == 0.0));
    }

    #[test]
    fn build_vertices_supports_text_and_stroke_primitives() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 80.0, 24.0),
            text: "scene".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));
        scene.push(SceneCommand::StrokeRect {
            rect: Rect::new(2.0, 2.0, 20.0, 10.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
            stroke: StrokeStyle::new(2.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(2),
                viewport: Size::new(100.0, 80.0),
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 30);
    }

    #[test]
    fn build_vertices_supports_non_rect_paths() {
        let mut triangle = Path::builder();
        triangle
            .move_to(Point::new(10.0, 10.0))
            .line_to(Point::new(40.0, 10.0))
            .line_to(Point::new(24.0, 36.0))
            .close();

        let mut curve = Path::builder();
        curve
            .move_to(Point::new(8.0, 44.0))
            .quad_to(Point::new(24.0, 24.0), Point::new(48.0, 44.0));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillPath {
            path: triangle.build(),
            brush: Color::rgba(0.2, 0.8, 0.4, 1.0).into(),
        });
        scene.push(SceneCommand::StrokePath {
            path: curve.build(),
            brush: Color::rgba(0.9, 0.4, 0.2, 1.0).into(),
            stroke: StrokeStyle::new(3.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(5),
                viewport: Size::new(80.0, 60.0),
                surface_size: Size::new(80.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 12);
    }

    #[test]
    fn shader_color_linearizes_srgb_inputs() {
        let rgba = shader_color(Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0));

        assert!((rgba[0] - 0.05448).abs() < 0.0001);
        assert!((rgba[1] - 0.02315).abs() < 0.0001);
        assert!((rgba[2] - 0.66539).abs() < 0.0001);
        assert_eq!(rgba[3], 1.0);
    }

    #[test]
    fn retained_compositor_carries_active_path_clips() {
        let mut clip = Path::builder();
        clip.move_to(Point::new(8.0, 8.0))
            .line_to(Point::new(32.0, 8.0))
            .line_to(Point::new(20.0, 28.0))
            .close();

        let mut scene = Scene::new();
        scene.push(SceneCommand::PushClipPath { path: clip.build() });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let ops = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(6),
                viewport: Size::new(64.0, 64.0),
                surface_size: Size::new(64.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap();

        assert_eq!(ops.draw_ops.len(), 1);
        let op = &ops.draw_ops[0];
        let clip_state = &ops.clip_states[op.clip_state_index];
        assert!(op.clip_state_index > 0);
        assert_eq!(clip_state.clip_paths.len(), 1);
        assert!(clip_state.clip_paths[0].len > 0);
        assert_eq!(op.clip_rect, Some(Rect::new(8.0, 8.0, 24.0, 20.0)));
    }

    #[test]
    fn batch_draw_ops_merges_consecutive_matching_state() {
        let passes = batch_draw_ops(
            &DrawOpArena {
                scene_vertices: vec![
                    Vertex {
                        position: [0.0, 0.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                        tex_coords: [0.0, 0.0],
                    };
                    6
                ],
                clip_vertices: Vec::new(),
                clip_states: vec![ClipState {
                    clip_paths: Vec::new(),
                }],
                draw_ops: vec![
                    DrawOp {
                        kind: DrawOpKind::Solid,
                        vertices: PreparedVertices { start: 0, len: 3 },
                        clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                        clip_state_index: 0,
                    },
                    DrawOp {
                        kind: DrawOpKind::Solid,
                        vertices: PreparedVertices { start: 3, len: 3 },
                        clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                        clip_state_index: 0,
                    },
                ],
            },
            Size::new(50.0, 40.0),
            (100, 80),
        );

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].draws.len(), 1);
        assert_eq!(passes[0].draws[0].vertices.len, 6);
    }

    #[test]
    fn prepare_frame_batches_converts_clip_rects_to_scissors() {
        let prepared = prepare_frame_batches(
            DrawOpArena {
                scene_vertices: vec![
                    Vertex {
                        position: [0.0, 0.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                        tex_coords: [0.0, 0.0],
                    };
                    6
                ],
                clip_vertices: Vec::new(),
                clip_states: vec![ClipState {
                    clip_paths: Vec::new(),
                }],
                draw_ops: vec![DrawOp {
                    kind: DrawOpKind::Solid,
                    clip_rect: Some(Rect::new(5.0, 8.0, 20.0, 10.0)),
                    vertices: PreparedVertices { start: 0, len: 6 },
                    clip_state_index: 0,
                }],
            },
            Size::new(50.0, 40.0),
            (100, 80),
        );

        assert_eq!(prepared.passes.len(), 1);
        assert_eq!(
            prepared.passes[0].draws[0].clip_rect,
            Some(ScissorRect {
                x: 10,
                y: 16,
                width: 40,
                height: 20,
            })
        );
    }

    #[test]
    fn renderer_frame_stats_count_passes_draws_and_uploaded_vertices() {
        let vertex = Vertex {
            position: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            tex_coords: [0.0, 0.0],
        };
        let prepared = PreparedFrameBatches {
            scene_vertices: vec![vertex; 9],
            clip_vertices: vec![vertex; 6],
            passes: vec![
                PreparedPassBatch {
                    clip_state_index: 0,
                    clip_paths: vec![PreparedClipPath {
                        vertices: PreparedVertices { start: 0, len: 6 },
                    }],
                    draws: vec![
                        PreparedDrawBatch {
                            kind: DrawOpKind::Solid,
                            clip_rect: None,
                            vertices: PreparedVertices { start: 0, len: 3 },
                        },
                        PreparedDrawBatch {
                            kind: DrawOpKind::Solid,
                            clip_rect: Some(ScissorRect {
                                x: 0,
                                y: 0,
                                width: 10,
                                height: 10,
                            }),
                            vertices: PreparedVertices { start: 3, len: 6 },
                        },
                    ],
                },
                PreparedPassBatch {
                    clip_state_index: 1,
                    clip_paths: Vec::new(),
                    draws: vec![PreparedDrawBatch {
                        kind: DrawOpKind::Image {
                            handle: ImageHandle::new(1),
                        },
                        clip_rect: None,
                        vertices: PreparedVertices { start: 0, len: 3 },
                    }],
                },
            ],
        };

        let stats = RendererFrameStats::from_prepared_frame(&prepared);

        assert_eq!(stats.pass_count, 2);
        assert_eq!(stats.draw_count, 4);
        assert_eq!(stats.uploaded_vertex_bytes, 15 * VERTEX_SIZE);
        assert_eq!(stats.visible_tile_count, 0);
        assert_eq!(stats.tile_memory_bytes, 0);
    }

    #[test]
    fn text_engine_shapes_text_with_font_metrics() {
        let text = TextRun {
            rect: Rect::new(8.0, 10.0, 160.0, 32.0),
            text: "office".to_string(),
            style: TextStyle::new(Color::WHITE),
        };

        let text_engine = TextEngine::new().unwrap();
        let layout = text_engine
            .shape_text_run(&text, &FontRegistry::new())
            .unwrap();

        assert!(!layout.glyphs().is_empty());
        assert!(layout.measurement().width > 0.0);
        assert!(layout.measurement().height >= text.style.font_size);
    }

    #[test]
    fn build_vertices_supports_pre_shaped_text() {
        let layout = TextSystem::new()
            .shape_text(
                "scene",
                Size::new(80.0, 24.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawShapedText(ShapedText {
            origin: Point::new(4.0, 6.0),
            layout,
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(11),
                viewport: Size::new(100.0, 80.0),
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn text_engine_reuses_cached_glyph_meshes_across_repeated_builds() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "abc".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));

        let frame = SceneFrame {
            window_id: WindowId::new(12),
            viewport: Size::new(160.0, 60.0),
            surface_size: Size::new(160.0, 60.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let first = build_vertices(&frame, &mut text_engine).unwrap();
        assert!(!first.is_empty());
        assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));

        let second = build_vertices(&frame, &mut text_engine).unwrap();
        assert_eq!(first.len(), second.len());
        assert!(first.iter().zip(&second).all(|(left, right)| {
            left.position == right.position
                && left.color == right.color
                && left.tex_coords == right.tex_coords
        }));
        assert_eq!(text_engine.glyph_cache_stats(), (3, 3, 3));
    }

    #[test]
    fn retained_compositor_reuses_layer_packets_until_content_changes() {
        let layer_id = WidgetId::new(41);
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Clear(Color::BLACK));
        scene.push(SceneCommand::Layer(SceneLayer::new(
            layer_id,
            Rect::new(4.0, 6.0, 32.0, 24.0),
            child_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(21),
            viewport: Size::new(96.0, 64.0),
            surface_size: Size::new(96.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);
        let first_content_version = compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        frame.layer_updates.clear();
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_eq!(first_signature, packet_signature(&compositor, layer_container));

        frame.layer_updates = content_updates([layer_id]);
        let mut updated_child_scene = Scene::new();
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(12.0, 10.0, 8.0, 8.0),
            brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
        });
        assert!(frame.scene.replace_layer(
            layer_id,
            SceneLayer::new(layer_id, Rect::new(4.0, 6.0, 32.0, 24.0), updated_child_scene),
        ));
        let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let third_signature = packet_signature(&compositor, layer_container);
        let third_content_version = compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        assert!(third_content_version > first_content_version);
        assert_ne!(first_signature, third_signature);
        assert_ne!(first.scene_vertices, third.scene_vertices);
    }

    #[test]
    fn retained_compositor_reuses_parent_packets_when_only_child_content_changes() {
        let parent_id = WidgetId::new(51);
        let child_id = WidgetId::new(52);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 10.0, 10.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });

        let mut parent_scene = Scene::new();
        parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 4.0, 24.0, 24.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        parent_scene.push(SceneCommand::Layer(SceneLayer::new(
            child_id,
            Rect::new(8.0, 8.0, 10.0, 10.0),
            child_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::new(
            parent_id,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            parent_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(22),
            viewport: Size::new(64.0, 64.0),
            surface_size: Size::new(64.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([parent_id, child_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let parent_container = CompositionContainerId::Layer(SceneLayerId::from_widget(parent_id));
        let child_container = CompositionContainerId::Layer(SceneLayerId::from_widget(child_id));
        let parent_signature = packet_signature(&compositor, parent_container);
        let child_signature = packet_signature(&compositor, child_container);

        frame.layer_updates.clear();
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_eq!(parent_signature, packet_signature(&compositor, parent_container));
        assert_eq!(child_signature, packet_signature(&compositor, child_container));

        let mut updated_child_scene = Scene::new();
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 10.0, 10.0),
            brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
        });

        let mut updated_parent_scene = Scene::new();
        updated_parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 4.0, 24.0, 24.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        updated_parent_scene.push(SceneCommand::Layer(SceneLayer::new(
            child_id,
            Rect::new(8.0, 8.0, 10.0, 10.0),
            updated_child_scene,
        )));
        assert!(frame.scene.replace_layer(
            parent_id,
            SceneLayer::new(parent_id, Rect::new(4.0, 4.0, 24.0, 24.0), updated_parent_scene),
        ));

        frame.layer_updates = content_updates([child_id]);
        let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(parent_signature, packet_signature(&compositor, parent_container));
        assert_ne!(child_signature, packet_signature(&compositor, child_container));
        assert_ne!(first.scene_vertices, third.scene_vertices);
    }

    #[test]
    fn retained_compositor_reuses_direct_packets_across_layer_translation() {
        let layer_id = WidgetId::new(53);

        let build_layer = |x: f32| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(layer_id),
                layer_id,
                Rect::new(x, 10.0, 80.0, 36.0),
            )
            .with_content_bounds(Rect::new(x, 10.0, 80.0, 36.0))
            .with_paint_bounds(Rect::new(x, 10.0, 80.0, 36.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(x, 10.0, 80.0, 36.0),
                brush: Color::rgba(0.82, 0.36, 0.18, 1.0).into(),
            });

            SceneLayer::from_descriptor(descriptor, layer_scene)
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(build_layer(8.0)));

        let mut frame = SceneFrame {
            window_id: WindowId::new(24),
            viewport: Size::new(160.0, 80.0),
            surface_size: Size::new(160.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);
        let first_content_version = compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        frame.scene = {
            let mut next = Scene::new();
            next.push(SceneCommand::Layer(build_layer(44.0)));
            next
        };
        frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Transform,
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(layer_id),
                layer_id,
                Rect::new(44.0, 10.0, 80.0, 36.0),
            )
            .with_content_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
            .with_paint_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
            .with_cache_policy(LayerCachePolicy::Direct),
        )];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let second_signature = packet_signature(&compositor, layer_container);
        let second_content_version = compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        assert_eq!(first_signature, second_signature);
        assert_eq!(first_content_version, second_content_version);
        assert_eq!(compositor.last_frame_stats.direct_packets, 1);
        assert_ne!(first.scene_vertices, second.scene_vertices);
    }

    #[test]
    fn retained_compositor_prunes_removed_layers_and_packets() {
        let removed_id = WidgetId::new(61);
        let replacement_id = WidgetId::new(62);

        let mut first_scene = Scene::new();
        first_scene.push(SceneCommand::Layer(SceneLayer::new(
            removed_id,
            Rect::new(0.0, 0.0, 24.0, 24.0),
            {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 24.0, 24.0),
                    brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
                });
                scene
            },
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(23),
            viewport: Size::new(64.0, 64.0),
            surface_size: Size::new(64.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([removed_id]),
            scene: first_scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let removed_layer_id = SceneLayerId::from_widget(removed_id);
        let replacement_layer_id = SceneLayerId::from_widget(replacement_id);
        let removed_packet_id = RetainedPacketId {
            container: CompositionContainerId::Layer(removed_layer_id),
            segment_index: 0,
        };
        assert!(compositor.layers.contains_key(&removed_layer_id));
        assert!(compositor.packets.contains_key(&removed_packet_id));

        let mut second_scene = Scene::new();
        second_scene.push(SceneCommand::Layer(SceneLayer::new(
            replacement_id,
            Rect::new(8.0, 8.0, 24.0, 24.0),
            {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(8.0, 8.0, 24.0, 24.0),
                    brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
                });
                scene
            },
        )));
        frame.scene = second_scene;
        frame.layer_updates = content_updates([replacement_id]);

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(!compositor.layers.contains_key(&removed_layer_id));
        assert!(!compositor.packets.contains_key(&removed_packet_id));
        assert!(compositor.layers.contains_key(&replacement_layer_id));
        assert!(compositor.packets.contains_key(&RetainedPacketId {
            container: CompositionContainerId::Layer(replacement_layer_id),
            segment_index: 0,
        }));
    }

    #[test]
    fn retained_compositor_reuses_cached_tiles_until_damage_intersects_them() {
        let layer_id = WidgetId::new(71);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 128.0),
            brush: Color::rgba(0.2, 0.2, 0.2, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(31),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor.clone())
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(!first.draw_ops.is_empty());
        assert_eq!(
            compositor.layers[&SceneLayerId::from_widget(layer_id)].render_mode,
            RetainedLayerRenderMode::CachedTiles
        );
        assert_eq!(compositor.last_frame_stats.visible_tiles, 2);
        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 2);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 0);

        frame.layer_updates.clear();
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_eq!(compositor.last_frame_stats.visible_tiles, 2);
        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 0);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 2);

        let mut updated_layer_scene = Scene::new();
        updated_layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 128.0),
            brush: Color::rgba(0.2, 0.2, 0.2, 1.0).into(),
        });
        updated_layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(24.0, 24.0, 48.0, 48.0),
            brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
        });
        assert!(frame.scene.replace_layer(
            layer_id,
            SceneLayer::from_descriptor(descriptor.clone(), updated_layer_scene),
        ));
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor.clone())
                .with_damage(Rect::new(0.0, 0.0, 128.0, 128.0)),
        ];

        let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_ne!(second.scene_vertices, third.scene_vertices);
        assert_eq!(compositor.last_frame_stats.visible_tiles, 2);
        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 1);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 1);
    }

    #[test]
    fn retained_compositor_reuses_cached_tiles_across_layer_translation() {
        let layer_id = WidgetId::new(74);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let build_scene = |x: f32| {
            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::PushClip {
                rect: Rect::new(x, 0.0, 512.0, 128.0),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(x, 0.0, 512.0, 128.0),
                brush: Color::rgba(0.2, 0.2, 0.2, 1.0).into(),
            });
            layer_scene.push(SceneCommand::PopClip);

            let translated = descriptor
                .clone()
                .with_content_bounds(Rect::new(x, 0.0, 512.0, 128.0))
                .with_paint_bounds(Rect::new(x, 0.0, 512.0, 128.0));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(layer_id),
                    layer_id,
                    Rect::new(x, 0.0, 512.0, 128.0),
                )
                .with_content_bounds(Rect::new(x, 0.0, 512.0, 128.0))
                .with_paint_bounds(Rect::new(x, 0.0, 512.0, 128.0))
                .with_cache_policy(translated.cache_policy)
                .with_composition_mode(translated.composition_mode),
                layer_scene,
            )));
            scene
        };

        let mut frame = SceneFrame {
            window_id: WindowId::new(34),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor.clone())
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            ],
            scene: build_scene(0.0),
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 2);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 0);

        let translated_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(64.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(64.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(64.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached)
        .with_composition_mode(LayerCompositionMode::Scroll);
        frame.scene = build_scene(64.0);
        frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Transform,
            translated_descriptor,
        )];

        let _second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(compositor.last_frame_stats.visible_tiles, 2);
        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 0);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 2);
    }

    #[test]
    fn retained_compositor_routes_descendant_damage_into_cached_parent_tiles() {
        let parent_id = WidgetId::new(81);
        let child_id = WidgetId::new(82);

        let parent_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(parent_id),
            parent_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);
        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(300.0, 24.0, 48.0, 48.0),
        );

        let build_parent_scene = |child_brush: Color| {
            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(300.0, 24.0, 48.0, 48.0),
                brush: child_brush.into(),
            });

            let mut parent_scene = Scene::new();
            parent_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 512.0, 128.0),
                brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
            });
            parent_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor.clone(),
                child_scene,
            )));
            parent_scene
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            parent_descriptor.clone(),
            build_parent_scene(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(32),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, parent_descriptor.clone())
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor.clone())
                    .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 2);

        assert!(frame.scene.replace_layer(
            parent_id,
            SceneLayer::from_descriptor(
                parent_descriptor.clone(),
                build_parent_scene(Color::rgba(0.0, 1.0, 0.0, 1.0)),
            ),
        ));
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor.clone())
                .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
        ];

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(compositor.last_frame_stats.visible_tiles, 2);
        assert_eq!(compositor.last_frame_stats.regenerated_tiles, 1);
        assert_eq!(compositor.last_frame_stats.reused_tiles, 1);
    }

    #[test]
    fn retained_compositor_prunes_removed_cached_layer_tiles() {
        let removed_id = WidgetId::new(91);
        let removed_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(removed_id),
            removed_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);

        let mut first_scene = Scene::new();
        first_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            removed_descriptor.clone(),
            {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 512.0, 128.0),
                    brush: Color::rgba(0.8, 0.2, 0.2, 1.0).into(),
                });
                scene
            },
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(33),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, removed_descriptor.clone())
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            ],
            scene: first_scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert!(compositor
            .tiles
            .keys()
            .any(|address| address.layer == SceneLayerId::from_widget(removed_id)));

        frame.scene = Scene::new();
        frame.layer_updates.clear();

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(compositor
            .tiles
            .keys()
            .all(|address| address.layer != SceneLayerId::from_widget(removed_id)));
    }

    #[test]
    fn build_vertices_uses_registered_font_handle() {
        let handle = FontHandle::new(17);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "registered".to_string(),
            style: TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(3),
                viewport: Size::new(160.0, 60.0),
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(fonts),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn build_vertices_errors_for_unregistered_font_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "missing".to_string(),
            style: TextStyle {
                font: Some(FontHandle::new(404)),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let error = match build_vertices(
            &SceneFrame {
                window_id: WindowId::new(4),
                viewport: Size::new(160.0, 60.0),
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        ) {
            Ok(_) => panic!("expected missing font handle to fail during shaping"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("font handle 404 is not registered")
        );
    }

    #[test]
    fn retained_compositor_uses_registered_image_handle() {
        let handle = ImageHandle::new(23);
        let mut images = ImageRegistry::new();
        images.insert(
            handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ],
            )
            .unwrap(),
        );

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(handle),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let ops = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(7),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(images),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap();

        assert_eq!(ops.draw_ops.len(), 1);
        let op = &ops.draw_ops[0];
        assert!(matches!(op.kind, DrawOpKind::Image { handle: value } if value == handle));
        assert_eq!(op.vertices.len, 6);
    }

    #[test]
    fn retained_compositor_errors_for_unregistered_image_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(ImageHandle::new(88)),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let error = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(8),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("image handle 88 is not registered")
        );
    }

    #[test]
    fn renderer_feather_width_is_configurable() {
        let mut renderer = WgpuRenderer::new().with_feather_width(2.5);

        assert_eq!(renderer.feather_width(), 2.5);

        renderer.set_feather_width(-3.0);

        assert_eq!(renderer.feather_width(), 0.0);
    }
}
