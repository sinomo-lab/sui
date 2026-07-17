#![forbid(unsafe_code)]

use std::{collections::HashMap, sync::Arc};

use sui_core::{
    Color, ColorSpace, DirtyRegion, Error, ImageHandle, Path, PathElement, Point, Rect, Result,
    Size, Transform, Vector, WidgetId, WindowId,
};
use sui_text::{FontRegistry, ShapedText, ShapedTextWindow, TextLayoutRegistry, TextRun};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowParams {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl ShadowParams {
    /// logical-px reach beyond the rect edge; used to inflate the shadow quad & bounds.
    pub fn extent(&self) -> f32 {
        3.0 * self.blur.max(0.0)
            + self.spread.max(0.0)
            + self.offset_x.abs().max(self.offset_y.abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

pub const MAX_GRADIENT_STOPS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    Solid(Color),
    LinearGradient {
        start: Point,
        end: Point,
        stops: Vec<GradientStop>,
    },
}

impl From<Color> for Brush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StrokeJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
}

impl StrokeStyle {
    pub const fn new(width: f32) -> Self {
        Self {
            width,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
        }
    }

    pub const fn with_cap(mut self, cap: StrokeCap) -> Self {
        self.cap = cap;
        self
    }

    pub const fn with_join(mut self, join: StrokeJoin) -> Self {
        self.join = join;
        self
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageSource {
    pub image: ImageHandle,
    pub source_rect: Option<Rect>,
    pub tint: Option<Color>,
    pub sampling: ImageSampling,
    pub pixel_snap: ImagePixelSnap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImageSampling {
    Nearest,
    #[default]
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImagePixelSnap {
    None,
    #[default]
    Physical,
}

impl ImageSource {
    pub const fn new(image: ImageHandle) -> Self {
        Self {
            image,
            source_rect: None,
            tint: None,
            sampling: ImageSampling::Linear,
            pixel_snap: ImagePixelSnap::Physical,
        }
    }

    pub const fn with_source_rect(mut self, source_rect: Rect) -> Self {
        self.source_rect = Some(source_rect);
        self
    }

    pub const fn with_tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    pub const fn with_sampling(mut self, sampling: ImageSampling) -> Self {
        self.sampling = sampling;
        self
    }

    pub const fn with_pixel_snap(mut self, pixel_snap: ImagePixelSnap) -> Self {
        self.pixel_snap = pixel_snap;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetShader {
    ColorWheel,
    ColorPickerHueBar,
    ColorPickerSaturationValuePlane {
        color_space: ColorSpace,
        hue: f32,
        max_value: f32,
    },
    ColorPickerSaturationBar {
        color_space: ColorSpace,
        hue: f32,
        value: f32,
    },
    ColorPickerValueBar {
        color_space: ColorSpace,
        hue: f32,
        saturation: f32,
        max_value: f32,
    },
    ColorPickerAlphaBar {
        color: Color,
    },
    ColorPickerRgbChannelBar {
        color: Color,
        channel: u32,
        max_value: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRenderMode {
    Grayscale,
    LcdSubpixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSubpixelOrder {
    #[default]
    None,
    Rgb,
    Bgr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRenderHinting {
    None,
    Slight { max_ppem: f32 },
}

impl TextRenderHinting {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Slight { max_ppem } if max_ppem.is_finite() && max_ppem > 0.0 => {
                Self::Slight { max_ppem }
            }
            Self::Slight { .. } => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRenderStemDarkening {
    None,
    Enabled { max_ppem: f32, amount: f32 },
}

impl TextRenderStemDarkening {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Enabled { max_ppem, amount }
                if max_ppem.is_finite() && max_ppem > 0.0 && amount.is_finite() && amount > 0.0 =>
            {
                Self::Enabled {
                    max_ppem,
                    amount: amount.clamp(0.0, 1.0),
                }
            }
            Self::Enabled { .. } => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRenderCoveragePolicy {
    Perceptual,
    Linear,
    Gamma(f32),
    CoverageBoost(f32),
    TwoCoverageMinusCoverageSq,
}

impl TextRenderCoveragePolicy {
    pub fn normalized(self) -> Self {
        match self {
            Self::Perceptual => Self::Perceptual,
            Self::Linear => Self::Linear,
            Self::Gamma(gamma) if gamma.is_finite() && gamma > 0.0 => Self::Gamma(gamma),
            Self::Gamma(_) => Self::Linear,
            Self::CoverageBoost(amount) if amount.is_finite() && amount > 0.0 => {
                Self::CoverageBoost(amount.clamp(0.0, 1.0))
            }
            Self::CoverageBoost(_) => Self::Linear,
            Self::TwoCoverageMinusCoverageSq => Self::TwoCoverageMinusCoverageSq,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextRenderPolicy {
    pub render_mode: Option<TextRenderMode>,
    pub subpixel_order: Option<TextSubpixelOrder>,
    pub hinting: Option<TextRenderHinting>,
    pub stem_darkening: Option<TextRenderStemDarkening>,
    pub coverage_policy: Option<TextRenderCoveragePolicy>,
}

impl TextRenderPolicy {
    pub const fn new() -> Self {
        Self {
            render_mode: None,
            subpixel_order: None,
            hinting: None,
            stem_darkening: None,
            coverage_policy: None,
        }
    }

    pub const fn with_render_mode(mut self, mode: TextRenderMode) -> Self {
        self.render_mode = Some(mode);
        self
    }

    pub const fn with_subpixel_order(mut self, order: TextSubpixelOrder) -> Self {
        self.subpixel_order = Some(order);
        self
    }

    pub const fn with_hinting(mut self, hinting: TextRenderHinting) -> Self {
        self.hinting = Some(hinting);
        self
    }

    pub const fn with_stem_darkening(mut self, darkening: TextRenderStemDarkening) -> Self {
        self.stem_darkening = Some(darkening);
        self
    }

    pub const fn with_coverage_policy(mut self, policy: TextRenderCoveragePolicy) -> Self {
        self.coverage_policy = Some(policy);
        self
    }

    pub fn normalized(self) -> Self {
        Self {
            render_mode: self.render_mode,
            subpixel_order: self.subpixel_order,
            hinting: self.hinting.map(TextRenderHinting::normalized),
            stem_darkening: self.stem_darkening.map(TextRenderStemDarkening::normalized),
            coverage_policy: self
                .coverage_policy
                .map(TextRenderCoveragePolicy::normalized),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerProperties {
    pub opacity: f32,
    pub translation: Vector,
}

impl LayerProperties {
    pub const fn new(opacity: f32, translation: Vector) -> Self {
        Self {
            opacity,
            translation,
        }
    }

    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub const fn with_translation(mut self, translation: Vector) -> Self {
        self.translation = translation;
        self
    }
}

impl Default for LayerProperties {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            translation: Vector::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneLayerDescriptor {
    pub id: SceneLayerId,
    pub owner: WidgetId,
    pub bounds: Rect,
    pub content_bounds: Rect,
    pub paint_bounds: Rect,
    pub hit_test: bool,
    pub clip_to_ancestors: bool,
    pub properties: LayerProperties,
    pub stack_host: WidgetId,
    pub stack_order: usize,
    pub transient_owner_surface: Option<WidgetId>,
    pub is_stack_surface: bool,
    pub composition_mode: LayerCompositionMode,
}

impl SceneLayerDescriptor {
    pub fn new(id: SceneLayerId, owner: WidgetId, bounds: Rect) -> Self {
        Self {
            id,
            owner,
            bounds,
            content_bounds: bounds,
            paint_bounds: bounds,
            hit_test: true,
            clip_to_ancestors: true,
            properties: LayerProperties::default(),
            stack_host: owner,
            stack_order: 0,
            transient_owner_surface: None,
            is_stack_surface: false,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    pub const fn with_content_bounds(mut self, content_bounds: Rect) -> Self {
        self.content_bounds = content_bounds;
        self
    }

    pub const fn with_paint_bounds(mut self, paint_bounds: Rect) -> Self {
        self.paint_bounds = paint_bounds;
        self
    }

    pub const fn with_hit_test(mut self, hit_test: bool) -> Self {
        self.hit_test = hit_test;
        self
    }

    pub const fn with_clip_to_ancestors(mut self, clip: bool) -> Self {
        self.clip_to_ancestors = clip;
        self
    }

    pub const fn with_properties(mut self, properties: LayerProperties) -> Self {
        self.properties = properties;
        self
    }

    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.properties.opacity = opacity;
        self
    }

    pub const fn with_translation(mut self, translation: Vector) -> Self {
        self.properties.translation = translation;
        self
    }

    pub const fn with_stack_host(mut self, stack_host: WidgetId) -> Self {
        self.stack_host = stack_host;
        self
    }

    pub const fn with_stack_order(mut self, stack_order: usize) -> Self {
        self.stack_order = stack_order;
        self
    }

    pub const fn with_transient_owner_surface(
        mut self,
        transient_owner_surface: Option<WidgetId>,
    ) -> Self {
        self.transient_owner_surface = transient_owner_surface;
        self
    }

    pub const fn with_is_stack_surface(mut self, is_stack_surface: bool) -> Self {
        self.is_stack_surface = is_stack_surface;
        self
    }

    pub const fn with_composition_mode(mut self, composition_mode: LayerCompositionMode) -> Self {
        self.composition_mode = composition_mode;
        self
    }

    pub fn translate(mut self, delta: Vector) -> Self {
        self.bounds = self.bounds.translate(delta);
        self.content_bounds = self.content_bounds.translate(delta);
        self.paint_bounds = self.paint_bounds.translate(delta);
        self
    }

    pub fn presented_bounds(&self) -> Rect {
        self.bounds.translate(self.properties.translation)
    }

    pub fn presented_content_bounds(&self) -> Rect {
        self.content_bounds.translate(self.properties.translation)
    }

    pub fn presented_paint_bounds(&self) -> Rect {
        self.paint_bounds.translate(self.properties.translation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneLayerId(u64);

impl SceneLayerId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn from_widget(widget_id: WidgetId) -> Self {
        Self(widget_id.get())
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<WidgetId> for SceneLayerId {
    fn from(value: WidgetId) -> Self {
        Self::from_widget(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCompositionMode {
    Normal,
    Scroll,
    Overlay,
    Effect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneLayer {
    pub descriptor: SceneLayerDescriptor,
    pub scene: Box<Scene>,
}

impl SceneLayer {
    pub fn new(widget_id: WidgetId, bounds: Rect, scene: Scene) -> Self {
        let content_bounds = scene.content_bounds().unwrap_or(bounds);
        let paint_bounds = scene.paint_bounds().unwrap_or(bounds);
        Self {
            descriptor: SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                bounds,
            )
            .with_content_bounds(content_bounds)
            .with_paint_bounds(paint_bounds),
            scene: Box::new(scene),
        }
    }

    pub fn from_descriptor(descriptor: SceneLayerDescriptor, scene: Scene) -> Self {
        Self {
            descriptor,
            scene: Box::new(scene),
        }
    }

    pub const fn layer_id(&self) -> SceneLayerId {
        self.descriptor.id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.descriptor.owner
    }

    pub const fn bounds(&self) -> Rect {
        self.descriptor.bounds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneLayerUpdateKind {
    Ordering,
    Content,
    Transform,
    Clip,
    Effect,
    Visibility,
    Resources,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneLayerUpdate {
    pub layer_id: SceneLayerId,
    pub owner: WidgetId,
    pub kind: SceneLayerUpdateKind,
    pub bounds: Rect,
    pub content_bounds: Rect,
    pub paint_bounds: Rect,
    pub properties: LayerProperties,
    pub stack_host: WidgetId,
    pub stack_order: usize,
    pub transient_owner_surface: Option<WidgetId>,
    pub is_stack_surface: bool,
    pub damage: Option<Rect>,
}

impl SceneLayerUpdate {
    pub fn from_descriptor(kind: SceneLayerUpdateKind, descriptor: SceneLayerDescriptor) -> Self {
        Self {
            layer_id: descriptor.id,
            owner: descriptor.owner,
            kind,
            bounds: descriptor.bounds,
            content_bounds: descriptor.content_bounds,
            paint_bounds: descriptor.paint_bounds,
            properties: descriptor.properties,
            stack_host: descriptor.stack_host,
            stack_order: descriptor.stack_order,
            transient_owner_surface: descriptor.transient_owner_surface,
            is_stack_surface: descriptor.is_stack_surface,
            damage: None,
        }
    }

    pub const fn with_damage(mut self, damage: Rect) -> Self {
        self.damage = Some(damage);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        brush: Brush,
    },
    StrokeRect {
        rect: Rect,
        brush: Brush,
        stroke: StrokeStyle,
    },
    FillPath {
        path: Path,
        brush: Brush,
    },
    StrokePath {
        path: Path,
        brush: Brush,
        stroke: StrokeStyle,
    },
    DrawText(TextRun),
    DrawShapedText(ShapedText),
    DrawShapedTextWindow(ShapedTextWindow),
    DrawImage {
        rect: Rect,
        source: ImageSource,
    },
    DrawImageQuad {
        points: [Point; 4],
        source: ImageSource,
    },
    DrawShaderRect {
        rect: Rect,
        shader: WidgetShader,
    },
    PushClip {
        rect: Rect,
    },
    PushClipPath {
        path: Path,
    },
    PopClip,
    PushTransform {
        transform: Transform,
    },
    PopTransform,
    PushTextRenderPolicy {
        policy: TextRenderPolicy,
    },
    PopTextRenderPolicy,
    Layer(SceneLayer),
    FillRoundedRect {
        rect: Rect,
        radii: [f32; 4],
        brush: Brush,
        border: Option<Border>,
        shadow: Option<ShadowParams>,
    },
    Label {
        rect: Rect,
        text: String,
        color: Color,
    },
}

#[derive(Clone, Default)]
pub struct Scene {
    commands: Vec<SceneCommand>,
    bounds: SceneBoundsSummary,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scene")
            .field("commands", &self.commands)
            .finish()
    }
}

impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.commands == other.commands
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: SceneCommand) {
        self.bounds.push(&command);
        self.commands.push(command);
    }

    pub fn append(&mut self, mut scene: Scene) {
        if self.bounds.state.is_balanced() && scene.bounds.state.is_balanced() {
            self.bounds.extend(&scene.bounds);
        } else {
            for command in &scene.commands {
                self.bounds.push(command);
            }
        }
        self.commands.append(&mut scene.commands);
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.bounds = SceneBoundsSummary::default();
    }

    pub fn commands(&self) -> &[SceneCommand] {
        &self.commands
    }

    pub fn visit_commands(&self, visitor: &mut dyn FnMut(&SceneCommand)) {
        for command in &self.commands {
            visitor(command);
            if let SceneCommand::Layer(layer) = command {
                layer.scene.visit_commands(visitor);
            }
        }
    }

    pub fn visit_layers(&self, visitor: &mut dyn FnMut(&SceneLayer)) {
        for command in &self.commands {
            if let SceneCommand::Layer(layer) = command {
                visitor(layer);
                layer.scene.visit_layers(visitor);
            }
        }
    }

    pub fn visit_layers_mut(&mut self, visitor: &mut dyn FnMut(&mut SceneLayer)) {
        for command in &mut self.commands {
            if let SceneCommand::Layer(layer) = command {
                visitor(layer);
                layer.scene.visit_layers_mut(visitor);
            }
        }
        self.rebuild_bounds();
    }

    pub fn content_bounds(&self) -> Option<Rect> {
        self.bounds.content
    }

    pub fn paint_bounds(&self) -> Option<Rect> {
        self.bounds.paint
    }

    pub fn replace_layer(&mut self, widget_id: WidgetId, replacement: SceneLayer) -> bool {
        let mut replaced = false;
        for command in &mut self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    *command = SceneCommand::Layer(replacement);
                    replaced = true;
                    break;
                }
                SceneCommand::Layer(layer) => {
                    if layer.scene.replace_layer(widget_id, replacement.clone()) {
                        replaced = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if replaced {
            self.rebuild_bounds();
        }
        replaced
    }

    pub fn translate(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }
        for command in &mut self.commands {
            translate_command(command, delta);
        }
        self.rebuild_bounds();
    }

    pub fn translate_layer(&mut self, widget_id: WidgetId, delta: Vector) -> bool {
        let mut translated = false;
        for command in &mut self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    layer.translate(delta);
                    translated = true;
                    break;
                }
                SceneCommand::Layer(layer) => {
                    if layer.scene.translate_layer(widget_id, delta) {
                        translated = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if translated {
            self.rebuild_bounds();
        }
        translated
    }

    pub fn replace_layer_descriptor(
        &mut self,
        widget_id: WidgetId,
        descriptor: SceneLayerDescriptor,
    ) -> bool {
        let mut replaced = false;
        for command in &mut self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    layer.descriptor = descriptor;
                    replaced = true;
                    break;
                }
                SceneCommand::Layer(layer) => {
                    if layer
                        .scene
                        .replace_layer_descriptor(widget_id, descriptor.clone())
                    {
                        replaced = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if replaced {
            self.rebuild_bounds();
        }
        replaced
    }

    pub fn layer_scene(&self, widget_id: WidgetId) -> Option<&Scene> {
        for command in &self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    return Some(&layer.scene);
                }
                SceneCommand::Layer(layer) => {
                    if let Some(scene) = layer.scene.layer_scene(widget_id) {
                        return Some(scene);
                    }
                }
                _ => {}
            }
        }

        None
    }

    pub fn reorder_stack_surfaces(&mut self) {
        let mut stack_layers = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| match command {
                SceneCommand::Layer(layer) if layer.descriptor.is_stack_surface => {
                    Some((index, layer.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        stack_layers.sort_by_key(|(index, layer)| (layer.descriptor.stack_order, *index));

        let mut sorted_layers = stack_layers.into_iter().map(|(_, layer)| layer);
        for command in &mut self.commands {
            if let SceneCommand::Layer(layer) = command {
                layer.scene.reorder_stack_surfaces();
                if layer.descriptor.is_stack_surface
                    && let Some(replacement) = sorted_layers.next()
                {
                    *layer = replacement;
                }
            }
        }
        self.rebuild_bounds();
    }

    fn rebuild_bounds(&mut self) {
        self.bounds = SceneBoundsSummary::from_commands(&self.commands);
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct SceneBoundsSummary {
    state: SceneBoundsState,
    content: Option<Rect>,
    paint: Option<Rect>,
}

impl SceneBoundsSummary {
    fn from_commands(commands: &[SceneCommand]) -> Self {
        let mut summary = Self::default();
        for command in commands {
            summary.push(command);
        }
        summary
    }

    fn push(&mut self, command: &SceneCommand) {
        let (content, paint) = self.state.command_bounds(command);
        Self::include(&mut self.content, content);
        Self::include(&mut self.paint, paint);
    }

    fn extend(&mut self, other: &Self) {
        debug_assert!(self.state.is_balanced());
        debug_assert!(other.state.is_balanced());
        Self::include(&mut self.content, other.content);
        Self::include(&mut self.paint, other.paint);
    }

    fn include(bounds: &mut Option<Rect>, additional: Option<Rect>) {
        let Some(additional) = additional else {
            return;
        };
        *bounds = Some(match *bounds {
            Some(existing) => existing.union(additional),
            None => additional,
        });
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct SceneBoundsState {
    transform: Transform,
    transform_stack: Vec<Transform>,
    clip_stack: Vec<Rect>,
}

impl SceneBoundsState {
    fn is_balanced(&self) -> bool {
        self.transform.is_identity()
            && self.transform_stack.is_empty()
            && self.clip_stack.is_empty()
    }

    fn command_bounds(&mut self, command: &SceneCommand) -> (Option<Rect>, Option<Rect>) {
        match command {
            SceneCommand::Clear(_) => {
                let clip = self.current_clip();
                (clip, clip)
            }
            SceneCommand::FillRect { rect, .. } => self.apply_rect(*rect),
            SceneCommand::StrokeRect { rect, stroke, .. } => {
                self.apply_rect(rect.inflate(stroke.width * 0.5, stroke.width * 0.5))
            }
            SceneCommand::FillPath { path, .. } => self.apply_rect(path.bounds()),
            SceneCommand::StrokePath { path, stroke, .. } => self.apply_rect(
                path.bounds()
                    .inflate(stroke.width * 0.5, stroke.width * 0.5),
            ),
            SceneCommand::DrawText(text) => self.apply_rect(text.rect),
            SceneCommand::DrawShapedText(text) => self.apply_rect(text.translated_bounds()),
            SceneCommand::DrawShapedTextWindow(text) => self.apply_rect(text.translated_bounds()),
            SceneCommand::DrawImage { rect, .. } => self.apply_rect(*rect),
            SceneCommand::DrawImageQuad { points, .. } => self.apply_points(*points),
            SceneCommand::DrawShaderRect { rect, .. } => self.apply_rect(*rect),
            SceneCommand::PushClip { rect } => {
                let clip = self.transform.transform_rect_bbox(*rect);
                self.push_clip(clip);
                (None, None)
            }
            SceneCommand::PushClipPath { path } => {
                let clip = self.transform.transform_rect_bbox(path.bounds());
                self.push_clip(clip);
                (None, None)
            }
            SceneCommand::PopClip => {
                self.clip_stack.pop();
                (None, None)
            }
            SceneCommand::PushTransform { transform } => {
                self.transform_stack.push(self.transform);
                self.transform = self.transform.then(*transform);
                (None, None)
            }
            SceneCommand::PopTransform => {
                self.transform = self.transform_stack.pop().unwrap_or_default();
                (None, None)
            }
            SceneCommand::PushTextRenderPolicy { .. } | SceneCommand::PopTextRenderPolicy => {
                (None, None)
            }
            SceneCommand::Layer(layer) => self.apply_distinct_rects(
                layer.descriptor.content_bounds,
                layer.descriptor.paint_bounds,
            ),
            SceneCommand::FillRoundedRect { rect, shadow, .. } => {
                let bounds = match shadow {
                    Some(shadow) => {
                        let extent = shadow.extent();
                        let shadow_rect = rect
                            .inflate(extent, extent)
                            .translate(Vector::new(shadow.offset_x, shadow.offset_y));
                        rect.union(shadow_rect)
                    }
                    None => *rect,
                };
                self.apply_rect(bounds)
            }
            SceneCommand::Label { rect, .. } => self.apply_rect(*rect),
        }
    }

    fn apply_rect(&self, rect: Rect) -> (Option<Rect>, Option<Rect>) {
        let transformed = self.transform.transform_rect_bbox(rect);
        (Some(transformed), self.clip_rect(transformed))
    }

    fn apply_distinct_rects(&self, content: Rect, paint: Rect) -> (Option<Rect>, Option<Rect>) {
        let content = self.transform.transform_rect_bbox(content);
        let paint = self.transform.transform_rect_bbox(paint);
        (Some(content), self.clip_rect(paint))
    }

    fn apply_points(&self, points: [Point; 4]) -> (Option<Rect>, Option<Rect>) {
        let transformed = points.map(|point| self.transform.transform_point(point));
        let bounds = points_bounds(&transformed);
        (Some(bounds), self.clip_rect(bounds))
    }

    fn push_clip(&mut self, clip: Rect) {
        let clip = self.clip_rect(clip).unwrap_or(Rect::ZERO);
        self.clip_stack.push(clip);
    }

    fn clip_rect(&self, rect: Rect) -> Option<Rect> {
        let mut clipped = rect;
        for clip in &self.clip_stack {
            clipped = clipped.intersection(*clip)?;
        }
        Some(clipped)
    }

    fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }
}

impl SceneLayer {
    fn translate(&mut self, delta: Vector) {
        self.descriptor = self.descriptor.clone().translate(delta);
        self.scene.translate(delta);
    }
}

fn translate_command(command: &mut SceneCommand, delta: Vector) {
    match command {
        SceneCommand::Clear(_)
        | SceneCommand::PopClip
        | SceneCommand::PopTransform
        | SceneCommand::PushTextRenderPolicy { .. }
        | SceneCommand::PopTextRenderPolicy => {}
        SceneCommand::FillRect { rect, .. }
        | SceneCommand::StrokeRect { rect, .. }
        | SceneCommand::DrawImage { rect, .. }
        | SceneCommand::DrawShaderRect { rect, .. }
        | SceneCommand::PushClip { rect }
        | SceneCommand::Label { rect, .. } => {
            *rect = rect.translate(delta);
        }
        SceneCommand::DrawImageQuad { points, .. } => {
            for point in points {
                *point += delta;
            }
        }
        SceneCommand::FillPath { path, .. }
        | SceneCommand::StrokePath { path, .. }
        | SceneCommand::PushClipPath { path } => {
            *path = translate_path(path, delta);
        }
        SceneCommand::DrawText(text) => {
            text.rect = text.rect.translate(delta);
        }
        SceneCommand::DrawShapedText(text) => {
            text.origin += delta;
        }
        SceneCommand::DrawShapedTextWindow(text) => {
            text.origin += delta;
        }
        SceneCommand::PushTransform { .. } => {}
        SceneCommand::Layer(layer) => {
            layer.translate(delta);
        }
        SceneCommand::FillRoundedRect { rect, brush, .. } => {
            *rect = rect.translate(delta);
            if let Brush::LinearGradient { start, end, .. } = brush {
                *start += delta;
                *end += delta;
            }
        }
    }
}

fn points_bounds(points: &[Point]) -> Rect {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    Rect::from_points(Point::new(min_x, min_y), Point::new(max_x, max_y))
}

fn translate_path(path: &Path, delta: Vector) -> Path {
    let mut builder = Path::builder();
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                builder.move_to(*point + delta);
            }
            PathElement::LineTo(point) => {
                builder.line_to(*point + delta);
            }
            PathElement::QuadTo { ctrl, to } => {
                builder.quad_to(*ctrl + delta, *to + delta);
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                builder.cubic_to(*ctrl1 + delta, *ctrl2 + delta, *to + delta);
            }
            PathElement::Close => {
                builder.close();
            }
        }
    }
    builder.build()
}

impl From<TextRun> for SceneCommand {
    fn from(value: TextRun) -> Self {
        Self::DrawText(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredImageFormat {
    Rgba8,
}

impl RegisteredImageFormat {
    const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredImage {
    data: Arc<[u8]>,
    width: u32,
    height: u32,
    format: RegisteredImageFormat,
    svg: Option<Arc<[u8]>>,
    mipmaps_enabled: bool,
}

/// Renderer-neutral metadata for an image whose pixels are owned outside the
/// frame's CPU image registry.
///
/// The renderer backend is responsible for resolving the matching
/// [`ImageHandle`] to a backend resource. Keeping only dimensions in the scene
/// frame lets widgets participate in normal image layout, clipping, sampling,
/// and retained rendering without making the scene model depend on a GPU API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredExternalImage {
    width: u32,
    height: u32,
}

impl RegisteredExternalImage {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::new(
                "external image dimensions must both be non-zero",
            ));
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

impl RegisteredImage {
    pub fn from_rgba8(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Result<Self> {
        Self::from_pixels(width, height, RegisteredImageFormat::Rgba8, data)
    }

    pub fn from_svg(svg: impl AsRef<[u8]>) -> Result<Self> {
        let svg = Arc::<[u8]>::from(svg.as_ref());
        let tree = parse_svg_image(&svg)?;
        let size = tree.size().to_int_size();
        let mut image = Self::from_svg_tree_at_size(&tree, size.width(), size.height())?;
        image.svg = Some(svg);
        Ok(image)
    }

    pub fn from_svg_at_size(width: u32, height: u32, svg: impl AsRef<[u8]>) -> Result<Self> {
        let svg = Arc::<[u8]>::from(svg.as_ref());
        let tree = parse_svg_image(&svg)?;
        let mut image = Self::from_svg_tree_at_size(&tree, width, height)?;
        image.svg = Some(svg);
        Ok(image)
    }

    pub fn from_pixels(
        width: u32,
        height: u32,
        format: RegisteredImageFormat,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self> {
        let data = data.into();
        let expected_len = width as usize * height as usize * format.bytes_per_pixel();
        if data.len() != expected_len {
            return Err(Error::new(format!(
                "image data length {} does not match expected size {} for a {}x{} {:?} image",
                data.len(),
                expected_len,
                width,
                height,
                format
            )));
        }

        Ok(Self {
            data: Arc::<[u8]>::from(data),
            width,
            height,
            format,
            svg: None,
            mipmaps_enabled: true,
        })
    }

    /// Disables mipmap generation for this image.
    ///
    /// This is useful for frequently updated bitmap resources such as paint
    /// canvases, where rebuilding a full mip pyramid for every content update
    /// costs more than base-level linear filtering.
    pub const fn without_mipmaps(mut self) -> Self {
        self.mipmaps_enabled = false;
        self
    }

    pub const fn mipmaps_enabled(&self) -> bool {
        self.mipmaps_enabled
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn format(&self) -> RegisteredImageFormat {
        self.format
    }

    pub fn is_svg(&self) -> bool {
        self.svg.is_some()
    }

    pub fn svg_bytes(&self) -> Option<&[u8]> {
        self.svg.as_deref()
    }

    pub fn rasterize_svg_at_size(&self, width: u32, height: u32) -> Result<Option<Self>> {
        let Some(svg) = &self.svg else {
            return Ok(None);
        };
        let tree = parse_svg_image(svg)?;
        Self::from_svg_tree_at_size(&tree, width, height).map(Some)
    }

    fn from_svg_tree_at_size(tree: &resvg::usvg::Tree, width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::new("SVG raster dimensions must be non-zero"));
        }

        let source_size = tree.size();
        let scale_x = width as f32 / source_size.width();
        let scale_y = height as f32 / source_size.height();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| Error::new("failed to allocate SVG raster pixmap"))?;
        resvg::render(
            tree,
            resvg::tiny_skia::Transform::from_scale(scale_x, scale_y),
            &mut pixmap.as_mut(),
        );

        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in pixmap.pixels() {
            if pixel.alpha() == 0 {
                data.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let color = pixel.demultiply();
                data.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
            }
        }

        Self::from_rgba8(width, height, data)
    }
}

fn parse_svg_image(svg: &[u8]) -> Result<resvg::usvg::Tree> {
    let options = resvg::usvg::Options::default();
    resvg::usvg::Tree::from_data(svg, &options)
        .map_err(|err| Error::new(format!("failed to parse SVG image: {err}")))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageRegistry {
    images: HashMap<ImageHandle, RegisteredImage>,
    external_images: HashMap<ImageHandle, RegisteredExternalImage>,
}

impl ImageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        handle: ImageHandle,
        image: RegisteredImage,
    ) -> Option<RegisteredImage> {
        self.external_images.remove(&handle);
        self.images.insert(handle, image)
    }

    pub fn insert_external(
        &mut self,
        handle: ImageHandle,
        image: RegisteredExternalImage,
    ) -> Option<RegisteredExternalImage> {
        self.images.remove(&handle);
        self.external_images.insert(handle, image)
    }

    pub fn get(&self, handle: ImageHandle) -> Option<&RegisteredImage> {
        self.images.get(&handle)
    }

    pub fn get_external(&self, handle: ImageHandle) -> Option<&RegisteredExternalImage> {
        self.external_images.get(&handle)
    }

    pub fn dimensions(&self, handle: ImageHandle) -> Option<(u32, u32)> {
        self.get(handle)
            .map(|image| (image.width(), image.height()))
            .or_else(|| {
                self.get_external(handle)
                    .map(|image| (image.width(), image.height()))
            })
    }

    pub fn contains(&self, handle: ImageHandle) -> bool {
        self.images.contains_key(&handle) || self.external_images.contains_key(&handle)
    }

    pub fn iter(&self) -> impl Iterator<Item = (ImageHandle, &RegisteredImage)> {
        self.images.iter().map(|(handle, image)| (*handle, image))
    }

    pub fn iter_external(&self) -> impl Iterator<Item = (ImageHandle, &RegisteredExternalImage)> {
        self.external_images
            .iter()
            .map(|(handle, image)| (*handle, image))
    }

    pub fn len(&self) -> usize {
        self.images.len() + self.external_images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty() && self.external_images.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneFrame {
    /// Host render target for this frame. The target may be a platform window
    /// or an embedded viewport/region represented by SUI's `WindowId`.
    pub window_id: WindowId,
    pub viewport: Size,
    pub surface_size: Size,
    pub scale_factor: f32,
    pub dirty_regions: Vec<DirtyRegion>,
    pub layer_updates: Vec<SceneLayerUpdate>,
    pub scene: Scene,
    pub font_registry: Arc<FontRegistry>,
    pub image_registry: Arc<ImageRegistry>,
    pub text_layout_registry: Arc<TextLayoutRegistry>,
}

impl SceneFrame {
    pub fn new(window_id: WindowId, viewport: Size) -> Self {
        Self {
            window_id,
            viewport,
            surface_size: viewport,
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: Scene::new(),
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Brush, ImageRegistry, ImageSource, LayerCompositionMode, LayerProperties,
        RegisteredExternalImage, RegisteredImage, RegisteredImageFormat, Scene, SceneBoundsSummary,
        SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor, SceneLayerId, SceneLayerUpdate,
        SceneLayerUpdateKind, StrokeStyle, TextRenderCoveragePolicy, TextRenderMode,
        TextRenderPolicy, TextSubpixelOrder, WidgetShader,
    };
    use std::sync::Arc;
    use sui_core::{
        Color, FontHandle, ImageHandle, Path, Point, Rect, Transform, Vector, WidgetId, WindowId,
    };
    use sui_text::{
        FontRegistry, RegisteredFont, ShapedText, ShapedTextWindow, TextRun, TextStyle, TextSystem,
    };

    fn assert_send_sync<T: Send + Sync>() {}

    fn assert_bounds_summary_matches_commands(scene: &Scene) {
        let recomputed = SceneBoundsSummary::from_commands(&scene.commands);
        assert_eq!(scene.bounds, recomputed);
        assert_eq!(scene.content_bounds(), recomputed.content);
        assert_eq!(scene.paint_bounds(), recomputed.paint);
    }

    #[test]
    fn scene_frame_is_send_sync() {
        assert_send_sync::<SceneFrame>();
    }

    #[test]
    fn scene_command_variants_store_extended_primitives() {
        let text = SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 8.0, 120.0, 24.0),
            text: "hello".to_string(),
            style: TextStyle::new(Color::WHITE),
        });
        let shaped_layout = TextSystem::new()
            .shape_text_persistent(
                None,
                "hello",
                sui_core::Size::new(120.0, 24.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();
        let shaped_text = SceneCommand::DrawShapedText(ShapedText {
            origin: Point::new(4.0, 8.0),
            layout_handle: shaped_layout.handle(),
            layout_version: shaped_layout.version(),
            bounds: shaped_layout.measurement().bounds,
            color_override: None,
        });
        let shaped_window = SceneCommand::DrawShapedTextWindow(ShapedTextWindow::new(
            Point::new(4.0, 8.0),
            &shaped_layout,
            0..1,
        ));
        let image = SceneCommand::DrawImage {
            rect: Rect::new(0.0, 0.0, 32.0, 32.0),
            source: ImageSource::new(ImageHandle::new(7))
                .with_tint(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        };
        let shader = SceneCommand::DrawShaderRect {
            rect: Rect::new(0.0, 0.0, 32.0, 8.0),
            shader: WidgetShader::ColorPickerHueBar,
        };
        let stroke = SceneCommand::StrokeRect {
            rect: Rect::new(1.0, 2.0, 30.0, 12.0),
            brush: Brush::Solid(Color::BLACK),
            stroke: StrokeStyle::new(2.0),
        };
        let path_fill = SceneCommand::FillPath {
            path: Path::from(Rect::new(3.0, 4.0, 12.0, 10.0)),
            brush: Brush::Solid(Color::WHITE),
        };
        let clip_path = SceneCommand::PushClipPath {
            path: Path::circle(sui_core::Point::new(10.0, 10.0), 4.0),
        };
        let transform = SceneCommand::PushTransform {
            transform: Transform::translation(3.0, 5.0),
        };
        let text_policy = SceneCommand::PushTextRenderPolicy {
            policy: TextRenderPolicy::new()
                .with_render_mode(TextRenderMode::LcdSubpixel)
                .with_subpixel_order(TextSubpixelOrder::Rgb)
                .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq),
        };
        let layer = SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(9),
            Rect::new(1.0, 2.0, 30.0, 12.0),
            Scene::new(),
        ));

        assert!(matches!(text, SceneCommand::DrawText(_)));
        assert!(matches!(shaped_text, SceneCommand::DrawShapedText(_)));
        assert!(matches!(
            shaped_window,
            SceneCommand::DrawShapedTextWindow(_)
        ));
        assert!(matches!(image, SceneCommand::DrawImage { .. }));
        assert!(matches!(shader, SceneCommand::DrawShaderRect { .. }));
        assert!(matches!(stroke, SceneCommand::StrokeRect { .. }));
        assert!(matches!(path_fill, SceneCommand::FillPath { .. }));
        assert!(matches!(clip_path, SceneCommand::PushClipPath { .. }));
        assert!(matches!(transform, SceneCommand::PushTransform { .. }));
        assert!(matches!(
            text_policy,
            SceneCommand::PushTextRenderPolicy { .. }
        ));
        assert!(matches!(layer, SceneCommand::Layer(_)));
    }

    #[test]
    fn scene_frame_can_share_font_registry_snapshots() {
        let mut registry = FontRegistry::new();
        registry.insert(
            FontHandle::new(9),
            RegisteredFont::from_bytes(vec![1, 2, 3]),
        );

        let mut frame = SceneFrame::new(WindowId::new(4), sui_core::Size::new(32.0, 24.0));
        frame.font_registry = Arc::new(registry);

        assert_eq!(frame.font_registry.len(), 1);
        assert!(frame.font_registry.contains(FontHandle::new(9)));
        assert!(frame.layer_updates.is_empty());
    }

    #[test]
    fn scene_frame_can_share_image_registry_snapshots() {
        let mut registry = ImageRegistry::new();
        registry.insert(
            ImageHandle::new(5),
            RegisteredImage::from_rgba8(1, 1, vec![255, 128, 64, 255]).unwrap(),
        );

        let mut frame = SceneFrame::new(WindowId::new(8), sui_core::Size::new(48.0, 32.0));
        frame.image_registry = Arc::new(registry);

        assert_eq!(frame.image_registry.len(), 1);
        assert!(frame.image_registry.contains(ImageHandle::new(5)));
        assert!(frame.layer_updates.is_empty());

        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::new(12),
            WidgetId::new(4),
            Rect::new(2.0, 3.0, 40.0, 20.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 80.0, 32.0))
        .with_paint_bounds(Rect::new(2.0, 3.0, 40.0, 20.0))
        .with_composition_mode(LayerCompositionMode::Scroll);
        let update = SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Transform, descriptor)
            .with_damage(Rect::new(1.0, 2.0, 42.0, 24.0));

        assert_eq!(update.layer_id.get(), 12);
        assert_eq!(update.owner, WidgetId::new(4));
        assert_eq!(update.kind, SceneLayerUpdateKind::Transform);
        assert_eq!(update.damage, Some(Rect::new(1.0, 2.0, 42.0, 24.0)));
    }

    #[test]
    fn image_registry_tracks_external_dimensions_and_one_backing_per_handle() {
        let handle = ImageHandle::new(15);
        let mut registry = ImageRegistry::new();
        registry.insert_external(handle, RegisteredExternalImage::new(640, 360).unwrap());

        assert!(registry.contains(handle));
        assert_eq!(registry.dimensions(handle), Some((640, 360)));
        assert!(registry.get(handle).is_none());
        assert!(registry.get_external(handle).is_some());

        registry.insert(
            handle,
            RegisteredImage::from_rgba8(1, 1, vec![1, 2, 3, 255]).unwrap(),
        );
        assert_eq!(registry.dimensions(handle), Some((1, 1)));
        assert!(registry.get(handle).is_some());
        assert!(registry.get_external(handle).is_none());
    }

    #[test]
    fn scene_layer_descriptor_defaults_to_identity_layer_properties() {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::new(19),
            WidgetId::new(7),
            Rect::new(10.0, 12.0, 48.0, 24.0),
        );

        assert_eq!(descriptor.properties, LayerProperties::default());
        assert_eq!(descriptor.properties.opacity, 1.0);
        assert_eq!(descriptor.properties.translation, Vector::ZERO);
        assert_eq!(descriptor.presented_bounds(), descriptor.bounds);
        assert_eq!(descriptor.presented_paint_bounds(), descriptor.paint_bounds);
    }

    #[test]
    fn scene_layer_update_preserves_dynamic_layer_properties() {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::new(27),
            WidgetId::new(11),
            Rect::new(4.0, 6.0, 32.0, 18.0),
        )
        .with_content_bounds(Rect::new(4.0, 6.0, 32.0, 18.0))
        .with_paint_bounds(Rect::new(2.0, 4.0, 36.0, 22.0))
        .with_properties(
            LayerProperties::default()
                .with_opacity(0.35)
                .with_translation(Vector::new(14.0, -3.0)),
        );
        let update =
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Effect, descriptor.clone());

        assert_eq!(update.properties.opacity, 0.35);
        assert_eq!(update.properties.translation, Vector::new(14.0, -3.0));
        assert_eq!(
            descriptor.presented_paint_bounds(),
            Rect::new(16.0, 1.0, 36.0, 22.0)
        );
    }

    #[test]
    fn registered_image_validates_pixel_buffer_length() {
        let error = RegisteredImage::from_rgba8(2, 2, vec![0, 1, 2]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("image data length 3 does not match")
        );
    }

    #[test]
    fn registered_image_rasterizes_svg_at_natural_size() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2" viewBox="0 0 4 2"><rect width="4" height="2" fill="#336699"/></svg>"##;

        let image = RegisteredImage::from_svg(svg).unwrap();

        assert_eq!(image.width(), 4);
        assert_eq!(image.height(), 2);
        assert_eq!(image.format(), RegisteredImageFormat::Rgba8);
        assert_eq!(image.bytes().len(), 4 * 2 * 4);
        assert_eq!(&image.bytes()[0..4], &[0x33, 0x66, 0x99, 0xff]);
        assert!(image.is_svg());
        assert_eq!(image.svg_bytes(), Some(svg.as_slice()));

        let larger = image
            .rasterize_svg_at_size(12, 6)
            .unwrap()
            .expect("retained SVG should rerasterize");
        assert_eq!((larger.width(), larger.height()), (12, 6));
        assert!(!larger.is_svg());
    }

    #[test]
    fn registered_image_rasterizes_svg_at_requested_size() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><circle cx="2" cy="2" r="2" fill="white"/></svg>"##;

        let image = RegisteredImage::from_svg_at_size(16, 8, svg).unwrap();

        assert_eq!(image.width(), 16);
        assert_eq!(image.height(), 8);
        assert_eq!(image.bytes().len(), 16 * 8 * 4);
        assert!(image.bytes().chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn registered_image_rejects_invalid_svg() {
        let error = RegisteredImage::from_svg(b"not svg").unwrap_err();

        assert!(error.to_string().contains("failed to parse SVG image"));
    }

    #[test]
    fn scene_replace_layer_updates_nested_widget_scene() {
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(2.0, 2.0, 6.0, 6.0),
            brush: Brush::Solid(Color::WHITE),
        });

        let mut root = Scene::new();
        root.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            brush: Brush::Solid(Color::BLACK),
        });
        root.push(SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(2),
            Rect::new(1.0, 1.0, 10.0, 10.0),
            child_scene,
        )));

        let mut replacement = Scene::new();
        replacement.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 4.0, 8.0, 8.0),
            brush: Brush::Solid(Color::rgba(0.0, 1.0, 0.0, 1.0)),
        });

        assert!(root.replace_layer(
            WidgetId::new(2),
            SceneLayer::new(
                WidgetId::new(2),
                Rect::new(1.0, 1.0, 10.0, 10.0),
                replacement
            ),
        ));

        let mut command_count = 0usize;
        let mut saw_nested_fill = false;
        root.visit_commands(&mut |command| {
            command_count += 1;
            if let SceneCommand::FillRect { rect, .. } = command
                && *rect == Rect::new(4.0, 4.0, 8.0, 8.0)
            {
                saw_nested_fill = true;
            }
        });

        assert_eq!(command_count, 3);
        assert!(saw_nested_fill);
    }

    #[test]
    fn scene_bounds_summary_stays_exact_through_all_mutation_paths() {
        let mut scene = Scene::default();
        assert_eq!(scene.content_bounds(), None);
        assert_eq!(scene.paint_bounds(), None);
        assert_bounds_summary_matches_commands(&scene);

        scene.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
        });
        scene.push(SceneCommand::PushTransform {
            transform: Transform::translation(10.0, 5.0),
        });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(30.0, 30.0, 40.0, 40.0),
            brush: Brush::Solid(Color::WHITE),
        });
        assert_eq!(
            scene.content_bounds(),
            Some(Rect::new(40.0, 35.0, 40.0, 40.0))
        );
        assert_eq!(
            scene.paint_bounds(),
            Some(Rect::new(40.0, 35.0, 10.0, 15.0))
        );
        assert_bounds_summary_matches_commands(&scene);

        let mut appended = Scene::new();
        appended.push(SceneCommand::FillRect {
            rect: Rect::new(-8.0, -6.0, 4.0, 3.0),
            brush: Brush::Solid(Color::BLACK),
        });
        appended.push(SceneCommand::PushClip {
            rect: Rect::new(-10.0, -10.0, 20.0, 20.0),
        });
        appended.push(SceneCommand::Clear(Color::BLACK));
        appended.push(SceneCommand::PopClip);
        assert_bounds_summary_matches_commands(&appended);

        // The destination has an open clip and transform, so append must replay the
        // incoming commands against that state rather than just unioning its bounds.
        scene.append(appended);
        scene.push(SceneCommand::PopTransform);
        scene.push(SceneCommand::PopClip);
        assert_bounds_summary_matches_commands(&scene);

        let child_id = WidgetId::new(41);
        let grandchild_id = WidgetId::new(42);
        let mut grandchild_scene = Scene::new();
        grandchild_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 12.0, 8.0),
            brush: Brush::Solid(Color::WHITE),
        });
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::Layer(SceneLayer::new(
            grandchild_id,
            Rect::new(4.0, 6.0, 12.0, 8.0),
            grandchild_scene,
        )));
        scene.push(SceneCommand::Layer(SceneLayer::new(
            child_id,
            Rect::new(2.0, 3.0, 24.0, 18.0),
            child_scene,
        )));
        assert_bounds_summary_matches_commands(&scene);

        let mut replacement_scene = Scene::new();
        replacement_scene.push(SceneCommand::FillRect {
            rect: Rect::new(20.0, 22.0, 14.0, 10.0),
            brush: Brush::Solid(Color::BLACK),
        });
        assert!(scene.replace_layer(
            grandchild_id,
            SceneLayer::new(
                grandchild_id,
                Rect::new(20.0, 22.0, 14.0, 10.0),
                replacement_scene,
            ),
        ));
        assert_bounds_summary_matches_commands(&scene);

        assert!(scene.translate_layer(grandchild_id, Vector::new(7.0, -2.0)));
        assert_bounds_summary_matches_commands(&scene);

        let replacement_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(grandchild_id),
            grandchild_id,
            Rect::new(27.0, 20.0, 14.0, 10.0),
        )
        .with_content_bounds(Rect::new(25.0, 18.0, 18.0, 14.0))
        .with_paint_bounds(Rect::new(24.0, 17.0, 20.0, 16.0));
        assert!(scene.replace_layer_descriptor(grandchild_id, replacement_descriptor));
        assert_bounds_summary_matches_commands(&scene);

        scene.visit_layers_mut(&mut |layer| {
            layer.descriptor.paint_bounds = layer.descriptor.paint_bounds.inflate(1.0, 2.0);
            layer.scene.push(SceneCommand::Label {
                rect: Rect::new(1.0, 2.0, 3.0, 4.0),
                text: "cache mutation".to_string(),
                color: Color::WHITE,
            });
        });
        assert_bounds_summary_matches_commands(&scene);

        let mut reordered = Scene::new();
        reordered.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        });
        reordered.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::new(91),
                WidgetId::new(91),
                Rect::new(0.0, 0.0, 100.0, 10.0),
            )
            .with_is_stack_surface(true)
            .with_stack_order(1),
            Scene::new(),
        )));
        reordered.push(SceneCommand::PopClip);
        reordered.push(SceneCommand::PushClip {
            rect: Rect::new(50.0, 0.0, 10.0, 10.0),
        });
        reordered.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::new(92),
                WidgetId::new(92),
                Rect::new(50.0, 0.0, 100.0, 10.0),
            )
            .with_is_stack_surface(true)
            .with_stack_order(0),
            Scene::new(),
        )));
        reordered.push(SceneCommand::PopClip);
        assert_eq!(
            reordered.paint_bounds(),
            Some(Rect::new(0.0, 0.0, 60.0, 10.0))
        );
        reordered.reorder_stack_surfaces();
        assert_eq!(
            reordered.paint_bounds(),
            Some(Rect::new(50.0, 0.0, 10.0, 10.0))
        );
        assert_bounds_summary_matches_commands(&reordered);
        scene.append(reordered);
        assert_bounds_summary_matches_commands(&scene);

        scene.translate(Vector::new(-3.0, 9.0));
        assert_bounds_summary_matches_commands(&scene);

        // A balanced scene can be appended by merging summaries without replaying
        // its command stream. The result must still match a complete recomputation.
        let mut balanced = Scene::new();
        balanced.push(SceneCommand::PushTransform {
            transform: Transform::translation(2.0, 4.0),
        });
        balanced.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 5.0, 7.0),
            brush: Brush::Solid(Color::WHITE),
        });
        balanced.push(SceneCommand::PopTransform);
        scene.append(balanced);
        assert_bounds_summary_matches_commands(&scene);

        let clone = scene.clone();
        assert_eq!(scene, clone);
        assert_bounds_summary_matches_commands(&clone);

        scene.clear();
        assert_eq!(scene, Scene::default());
        assert_eq!(scene.content_bounds(), None);
        assert_eq!(scene.paint_bounds(), None);
        assert_bounds_summary_matches_commands(&scene);
    }
}
