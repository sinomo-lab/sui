#![forbid(unsafe_code)]

use std::{collections::HashMap, sync::Arc};

use sui_core::{
    Color, DirtyRegion, Error, ImageHandle, Path, PathElement, Rect, Result, Size, Transform,
    Vector, WidgetId, WindowId,
};
use sui_text::{FontRegistry, ShapedText, ShapedTextWindow, TextLayoutRegistry, TextRun};

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    Solid(Color),
}

impl From<Color> for Brush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
}

impl StrokeStyle {
    pub const fn new(width: f32) -> Self {
        Self { width }
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self { width: 1.0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageSource {
    pub image: ImageHandle,
    pub source_rect: Option<Rect>,
    pub tint: Option<Color>,
}

impl ImageSource {
    pub const fn new(image: ImageHandle) -> Self {
        Self {
            image,
            source_rect: None,
            tint: None,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneLayerDescriptor {
    pub id: SceneLayerId,
    pub owner: WidgetId,
    pub bounds: Rect,
    pub content_bounds: Rect,
    pub paint_bounds: Rect,
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
    Layer(SceneLayer),
    Label {
        rect: Rect,
        text: String,
        color: Color,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    commands: Vec<SceneCommand>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: SceneCommand) {
        self.commands.push(command);
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
    }

    pub fn content_bounds(&self) -> Option<Rect> {
        self.compute_bounds(false)
    }

    pub fn paint_bounds(&self) -> Option<Rect> {
        self.compute_bounds(true)
    }

    pub fn replace_layer(&mut self, widget_id: WidgetId, replacement: SceneLayer) -> bool {
        for command in &mut self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    *command = SceneCommand::Layer(replacement);
                    return true;
                }
                SceneCommand::Layer(layer) => {
                    if layer.scene.replace_layer(widget_id, replacement.clone()) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }

    pub fn translate(&mut self, delta: Vector) {
        for command in &mut self.commands {
            translate_command(command, delta);
        }
    }

    pub fn translate_layer(&mut self, widget_id: WidgetId, delta: Vector) -> bool {
        for command in &mut self.commands {
            match command {
                SceneCommand::Layer(layer) if layer.widget_id() == widget_id => {
                    layer.translate(delta);
                    return true;
                }
                SceneCommand::Layer(layer) => {
                    if layer.scene.translate_layer(widget_id, delta) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
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
                if layer.descriptor.is_stack_surface {
                    if let Some(replacement) = sorted_layers.next() {
                        *layer = replacement;
                    }
                }
            }
        }
    }

    fn compute_bounds(&self, clipped: bool) -> Option<Rect> {
        let mut state = SceneBoundsState::default();
        let mut bounds: Option<Rect> = None;

        for command in &self.commands {
            if let Some(command_bounds) = state.command_bounds(command, clipped) {
                bounds = Some(match bounds {
                    Some(existing) => existing.union(command_bounds),
                    None => command_bounds,
                });
            }
        }

        bounds
    }
}

#[derive(Debug, Clone, Default)]
struct SceneBoundsState {
    transform: Transform,
    transform_stack: Vec<Transform>,
    clip_stack: Vec<Rect>,
}

impl SceneBoundsState {
    fn command_bounds(&mut self, command: &SceneCommand, clipped: bool) -> Option<Rect> {
        match command {
            SceneCommand::Clear(_) => self.current_clip(),
            SceneCommand::FillRect { rect, .. } => self.apply_rect(*rect, clipped),
            SceneCommand::StrokeRect { rect, stroke, .. } => self.apply_rect(
                rect.inflate(stroke.width * 0.5, stroke.width * 0.5),
                clipped,
            ),
            SceneCommand::FillPath { path, .. } => self.apply_rect(path.bounds(), clipped),
            SceneCommand::StrokePath { path, stroke, .. } => self.apply_rect(
                path.bounds()
                    .inflate(stroke.width * 0.5, stroke.width * 0.5),
                clipped,
            ),
            SceneCommand::DrawText(text) => self.apply_rect(text.rect, clipped),
            SceneCommand::DrawShapedText(text) => self.apply_rect(text.translated_bounds(), clipped),
            SceneCommand::DrawShapedTextWindow(text) => {
                self.apply_rect(text.translated_bounds(), clipped)
            }
            SceneCommand::DrawImage { rect, .. } => self.apply_rect(*rect, clipped),
            SceneCommand::PushClip { rect } => {
                let clip = self.transform.transform_rect_bbox(*rect);
                self.push_clip(clip);
                None
            }
            SceneCommand::PushClipPath { path } => {
                let clip = self.transform.transform_rect_bbox(path.bounds());
                self.push_clip(clip);
                None
            }
            SceneCommand::PopClip => {
                self.clip_stack.pop();
                None
            }
            SceneCommand::PushTransform { transform } => {
                self.transform_stack.push(self.transform);
                self.transform = self.transform.then(*transform);
                None
            }
            SceneCommand::PopTransform => {
                self.transform = self.transform_stack.pop().unwrap_or_default();
                None
            }
            SceneCommand::Layer(layer) => self.apply_rect(
                if clipped {
                    layer.descriptor.paint_bounds
                } else {
                    layer.descriptor.content_bounds
                },
                clipped,
            ),
            SceneCommand::Label { rect, .. } => self.apply_rect(*rect, clipped),
        }
    }

    fn apply_rect(&self, rect: Rect, clipped: bool) -> Option<Rect> {
        let transformed = self.transform.transform_rect_bbox(rect);
        if clipped {
            self.clip_rect(transformed)
        } else {
            Some(transformed)
        }
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
        SceneCommand::Clear(_) | SceneCommand::PopClip | SceneCommand::PopTransform => {}
        SceneCommand::FillRect { rect, .. }
        | SceneCommand::StrokeRect { rect, .. }
        | SceneCommand::DrawImage { rect, .. }
        | SceneCommand::PushClip { rect }
        | SceneCommand::Label { rect, .. } => {
            *rect = rect.translate(delta);
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
    }
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
}

impl RegisteredImage {
    pub fn from_rgba8(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Result<Self> {
        Self::from_pixels(width, height, RegisteredImageFormat::Rgba8, data)
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
        })
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
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageRegistry {
    images: HashMap<ImageHandle, RegisteredImage>,
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
        self.images.insert(handle, image)
    }

    pub fn get(&self, handle: ImageHandle) -> Option<&RegisteredImage> {
        self.images.get(&handle)
    }

    pub fn contains(&self, handle: ImageHandle) -> bool {
        self.images.contains_key(&handle)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneFrame {
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
        Brush, ImageRegistry, ImageSource, LayerCompositionMode, RegisteredImage, Scene,
        SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor, SceneLayerId,
        SceneLayerUpdate, SceneLayerUpdateKind, StrokeStyle,
    };
    use std::sync::Arc;
    use sui_core::{
        Color, FontHandle, ImageHandle, Path, Point, Rect, Transform, WidgetId, WindowId,
    };
    use sui_text::{
        FontRegistry, RegisteredFont, ShapedText, ShapedTextWindow, TextRun, TextStyle,
        TextSystem,
    };

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
        let layer = SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(9),
            Rect::new(1.0, 2.0, 30.0, 12.0),
            Scene::new(),
        ));

        assert!(matches!(text, SceneCommand::DrawText(_)));
        assert!(matches!(shaped_text, SceneCommand::DrawShapedText(_)));
        assert!(matches!(shaped_window, SceneCommand::DrawShapedTextWindow(_)));
        assert!(matches!(image, SceneCommand::DrawImage { .. }));
        assert!(matches!(stroke, SceneCommand::StrokeRect { .. }));
        assert!(matches!(path_fill, SceneCommand::FillPath { .. }));
        assert!(matches!(clip_path, SceneCommand::PushClipPath { .. }));
        assert!(matches!(transform, SceneCommand::PushTransform { .. }));
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
    fn registered_image_validates_pixel_buffer_length() {
        let error = RegisteredImage::from_rgba8(2, 2, vec![0, 1, 2]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("image data length 3 does not match")
        );
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
}
