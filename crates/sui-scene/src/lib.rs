#![forbid(unsafe_code)]

use std::{collections::HashMap, sync::Arc};

use sui_core::{
    Color, DirtyRegion, FontHandle, ImageHandle, Path, Rect, Size, Transform, WindowId,
};

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
pub struct TextStyle {
    pub font: Option<FontHandle>,
    pub font_size: f32,
    pub line_height: f32,
    pub color: Color,
}

impl TextStyle {
    pub fn new(color: Color) -> Self {
        Self {
            color,
            ..Self::default()
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 14.0,
            line_height: 18.0,
            color: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub rect: Rect,
    pub text: String,
    pub style: TextStyle,
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
}

impl From<TextRun> for SceneCommand {
    fn from(value: TextRun) -> Self {
        Self::DrawText(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFont {
    data: Arc<[u8]>,
    face_index: u32,
}

impl RegisteredFont {
    pub fn from_bytes(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: Arc::<[u8]>::from(data.into()),
            face_index: 0,
        }
    }

    pub const fn with_face_index(mut self, face_index: u32) -> Self {
        self.face_index = face_index;
        self
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontRegistry {
    fonts: HashMap<FontHandle, RegisteredFont>,
}

impl FontRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: FontHandle, font: RegisteredFont) -> Option<RegisteredFont> {
        self.fonts.insert(handle, font)
    }

    pub fn get(&self, handle: FontHandle) -> Option<&RegisteredFont> {
        self.fonts.get(&handle)
    }

    pub fn contains(&self, handle: FontHandle) -> bool {
        self.fonts.contains_key(&handle)
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneFrame {
    pub window_id: WindowId,
    pub viewport: Size,
    pub dirty_regions: Vec<DirtyRegion>,
    pub scene: Scene,
    pub font_registry: Arc<FontRegistry>,
}

impl SceneFrame {
    pub fn new(window_id: WindowId, viewport: Size) -> Self {
        Self {
            window_id,
            viewport,
            dirty_regions: Vec::new(),
            scene: Scene::new(),
            font_registry: Arc::new(FontRegistry::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Brush, FontRegistry, ImageSource, RegisteredFont, SceneCommand, SceneFrame, StrokeStyle,
        TextRun, TextStyle,
    };
    use std::sync::Arc;
    use sui_core::{Color, FontHandle, ImageHandle, Path, Rect, Transform, WindowId};

    #[test]
    fn scene_command_variants_store_extended_primitives() {
        let text = SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 8.0, 120.0, 24.0),
            text: "hello".to_string(),
            style: TextStyle::new(Color::WHITE),
        });
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

        assert!(matches!(text, SceneCommand::DrawText(_)));
        assert!(matches!(image, SceneCommand::DrawImage { .. }));
        assert!(matches!(stroke, SceneCommand::StrokeRect { .. }));
        assert!(matches!(path_fill, SceneCommand::FillPath { .. }));
        assert!(matches!(clip_path, SceneCommand::PushClipPath { .. }));
        assert!(matches!(transform, SceneCommand::PushTransform { .. }));
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
    }
}
