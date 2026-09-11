use crate::draw::DrawOpArena;
use crate::resources::DEFAULT_FEATHER_WIDTH;
use crate::retained::CompositionContainerId;
use crate::retained::RetainedCompositorState;
use crate::retained::RetainedPacketId;
use crate::text_engine::TextEngine;
use std::sync::Arc;
use sui_core::Color;
use sui_core::Path;
use sui_core::Rect;
use sui_core::Size;
use sui_core::WidgetId;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::LayerCompositionMode;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_scene::SceneLayer;
use sui_scene::SceneLayerDescriptor;
use sui_scene::SceneLayerId;
use sui_scene::SceneLayerUpdate;
use sui_scene::SceneLayerUpdateKind;
use sui_text::FontRegistry;
use sui_text::RegisteredFont;
use sui_text::TextLayoutRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerCachePolicy {
    Auto,
    Direct,
    Cached,
}

pub(crate) trait TestSceneLayerDescriptorExt {
    fn with_cache_policy(self, _cache_policy: LayerCachePolicy) -> Self;
}

impl TestSceneLayerDescriptorExt for SceneLayerDescriptor {
    fn with_cache_policy(self, _cache_policy: LayerCachePolicy) -> Self {
        self
    }
}

pub(crate) fn load_test_font() -> RegisteredFont {
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

pub(crate) fn content_update(widget_id: WidgetId) -> SceneLayerUpdate {
    SceneLayerUpdate::from_descriptor(
        SceneLayerUpdateKind::Content,
        SceneLayerDescriptor::new(SceneLayerId::from_widget(widget_id), widget_id, Rect::ZERO),
    )
    .with_damage(Rect::ZERO)
}

pub(crate) fn content_updates<const N: usize>(widget_ids: [WidgetId; N]) -> Vec<SceneLayerUpdate> {
    widget_ids.into_iter().map(content_update).collect()
}

pub(crate) fn prepare_with_compositor(
    frame: &SceneFrame,
    text_engine: &mut TextEngine,
    compositor: &mut RetainedCompositorState,
) -> sui_core::Result<DrawOpArena> {
    compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)
}

pub(crate) fn packet_signature(
    compositor: &RetainedCompositorState,
    container: CompositionContainerId,
) -> u64 {
    compositor.packets[&RetainedPacketId {
        container,
        segment_index: 0,
    }]
        .signature
}

pub(crate) const RGBA_CHANNEL_TOLERANCE: u8 = 1;

pub(crate) fn rgba_channels_match_with_tolerance(left: &[u8], right: &[u8], tolerance: u8) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| left.abs_diff(*right) <= tolerance)
}

pub(crate) fn assert_rgba_images_match(
    left: &crate::capture::RgbaImage,
    right: &crate::capture::RgbaImage,
) {
    assert_eq!(left.width(), right.width(), "image widths differ");
    assert_eq!(left.height(), right.height(), "image heights differ");

    let mut diff_count = 0usize;
    let mut diff_bounds: Option<(u32, u32, u32, u32)> = None;
    let mut max_channel_diff = 0u8;
    let width = left.width();
    for (index, (left_px, right_px)) in left
        .pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .enumerate()
    {
        let pixel_max_channel_diff = left_px
            .iter()
            .zip(right_px.iter())
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .unwrap_or(0);
        max_channel_diff = max_channel_diff.max(pixel_max_channel_diff);
        if pixel_max_channel_diff > RGBA_CHANNEL_TOLERANCE {
            diff_count += 1;
            let x = (index as u32) % width;
            let y = (index as u32) / width;
            diff_bounds = Some(match diff_bounds {
                Some((min_x, min_y, max_x, max_y)) => {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }
                None => (x, y, x, y),
            });
        }
    }

    if diff_count != 0 {
        let (min_x, min_y, max_x, max_y) = diff_bounds.expect("diff bounds present");
        panic!(
            "images differ beyond {RGBA_CHANNEL_TOLERANCE} channel value at {} pixels within bounds ({}, {})..({}, {}); max channel diff {}",
            diff_count, min_x, min_y, max_x, max_y, max_channel_diff
        );
    }
}

pub(crate) fn rgba_image_diff_count(
    left: &crate::capture::RgbaImage,
    right: &crate::capture::RgbaImage,
) -> usize {
    assert_eq!(left.width(), right.width(), "image widths differ");
    assert_eq!(left.height(), right.height(), "image heights differ");

    left.pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .filter(|(left_px, right_px)| {
            !rgba_channels_match_with_tolerance(left_px, right_px, RGBA_CHANNEL_TOLERANCE)
        })
        .count()
}

pub(crate) fn ink_pixel_count(image: &crate::capture::RgbaImage, rect: Rect) -> usize {
    let min_x = rect.x().floor().max(0.0) as u32;
    let min_y = rect.y().floor().max(0.0) as u32;
    let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
    let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
    let pixels = image.pixels();
    let width = image.width() as usize;

    let mut count = 0usize;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let index = ((y as usize * width) + x as usize) * 4;
            let red = pixels[index] as i32;
            let green = pixels[index + 1] as i32;
            let blue = pixels[index + 2] as i32;
            let alpha = pixels[index + 3] as i32;
            if alpha > 0 && (red + green + blue) < 680 {
                count += 1;
            }
        }
    }
    count
}

pub(crate) fn non_white_pixel_count(image: &crate::capture::RgbaImage, rect: Rect) -> usize {
    let min_x = rect.x().floor().max(0.0) as u32;
    let min_y = rect.y().floor().max(0.0) as u32;
    let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
    let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
    let pixels = image.pixels();
    let width = image.width() as usize;

    let mut count = 0usize;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let index = ((y as usize * width) + x as usize) * 4;
            if !rgba_channels_match_with_tolerance(
                &pixels[index..index + 4],
                &[255, 255, 255, 255],
                RGBA_CHANNEL_TOLERANCE,
            ) {
                count += 1;
            }
        }
    }
    count
}

pub(crate) fn non_white_row_count(image: &crate::capture::RgbaImage, rect: Rect) -> usize {
    let min_x = rect.x().floor().max(0.0) as u32;
    let min_y = rect.y().floor().max(0.0) as u32;
    let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
    let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
    let pixels = image.pixels();
    let width = image.width() as usize;

    let mut rows = 0usize;
    for y in min_y..max_y {
        let mut row_has_ink = false;
        for x in min_x..max_x {
            let index = ((y as usize * width) + x as usize) * 4;
            if !rgba_channels_match_with_tolerance(
                &pixels[index..index + 4],
                &[255, 255, 255, 255],
                RGBA_CHANNEL_TOLERANCE,
            ) {
                row_has_ink = true;
                break;
            }
        }
        rows += row_has_ink as usize;
    }
    rows
}

pub(crate) fn rgba_pixel(image: &crate::capture::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let width = image.width() as usize;
    let index = ((y as usize * width) + x as usize) * 4;
    let pixels = image.pixels();
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

pub(crate) fn assert_rgba_pixel_near(
    image: &crate::capture::RgbaImage,
    x: u32,
    y: u32,
    expected: [u8; 4],
    tolerance: u8,
) {
    let actual = rgba_pixel(image, x, y);
    for channel in 0..4 {
        assert!(
            actual[channel].abs_diff(expected[channel]) <= tolerance,
            "pixel ({x}, {y}) channel {channel} differed by more than {tolerance}: got {}, expected {}",
            actual[channel],
            expected[channel]
        );
    }
}

pub(crate) fn assert_rgba_channels_near(actual: &[u8], expected: [u8; 4], tolerance: u8) {
    assert_eq!(
        actual.len(),
        4,
        "expected exactly one RGBA pixel, got {} channels",
        actual.len()
    );
    for channel in 0..4 {
        assert!(
            actual[channel].abs_diff(expected[channel]) <= tolerance,
            "channel {channel} differed by more than {tolerance}: got {}, expected {}",
            actual[channel],
            expected[channel]
        );
    }
}

pub(crate) fn assert_rgba_pixels_near(actual: &[u8], expected: &[u8], tolerance: u8) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "RGBA buffer length mismatch: got {}, expected {}",
        actual.len(),
        expected.len()
    );
    assert_eq!(
        actual.len() % 4,
        0,
        "RGBA buffer length must be divisible by 4"
    );

    for (pixel_index, (actual_pixel, expected_pixel)) in actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .enumerate()
    {
        for channel in 0..4 {
            assert!(
                actual_pixel[channel].abs_diff(expected_pixel[channel]) <= tolerance,
                "pixel {pixel_index} channel {channel} differed by more than {tolerance}: got {}, expected {}",
                actual_pixel[channel],
                expected_pixel[channel]
            );
        }
    }
}

pub(crate) fn build_translucent_scroll_child_frame(
    window_id: WindowId,
    scroll_cache_policy: LayerCachePolicy,
    child_cache_policy: LayerCachePolicy,
    child_y: f32,
    update_kind: SceneLayerUpdateKind,
) -> SceneFrame {
    let shell_id = WidgetId::new(210);
    let scroll_id = WidgetId::new(211);
    let child_id = WidgetId::new(212);
    let scroll_bounds = Rect::new(24.0, 24.0, 382.0, 292.0);
    let child_bounds = Rect::new(42.0, child_y, 360.0, 220.0);
    let selected_row = Rect::new(42.0, child_y + 32.0, 360.0, 28.0);
    let thumb = Rect::new(396.0, child_y + 14.0, 4.0, 58.0);

    let shell_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(shell_id),
        shell_id,
        Rect::new(0.0, 0.0, 430.0, 360.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
    .with_cache_policy(LayerCachePolicy::Direct);

    let scroll_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(scroll_id),
        scroll_id,
        scroll_bounds,
    )
    .with_content_bounds(scroll_bounds)
    .with_paint_bounds(scroll_bounds)
    .with_cache_policy(scroll_cache_policy)
    .with_composition_mode(LayerCompositionMode::Scroll);

    let child_descriptor =
        SceneLayerDescriptor::new(SceneLayerId::from_widget(child_id), child_id, child_bounds)
            .with_content_bounds(child_bounds)
            .with_paint_bounds(child_bounds)
            .with_cache_policy(child_cache_policy);

    let mut child_scene = Scene::new();
    child_scene.push(SceneCommand::FillRect {
        rect: child_bounds,
        brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
    });
    child_scene.push(SceneCommand::PushClip { rect: child_bounds });
    child_scene.push(SceneCommand::FillPath {
        path: Path::rounded_rect(selected_row, 6.0),
        brush: Color::rgba(0.09, 0.40, 0.92, 0.14).into(),
    });
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(58.0, child_y + 40.0, 172.0, 12.0),
        brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
    });
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(58.0, child_y + 64.0, 140.0, 10.0),
        brush: Color::rgba(0.45, 0.52, 0.61, 1.0).into(),
    });
    child_scene.push(SceneCommand::PopClip);
    child_scene.push(SceneCommand::FillPath {
        path: Path::rounded_rect(thumb, 2.0),
        brush: Color::rgba(0.54, 0.60, 0.68, 0.75).into(),
    });

    let mut scroll_scene = Scene::new();
    scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        child_descriptor.clone(),
        child_scene,
    )));

    let mut shell_scene = Scene::new();
    shell_scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 430.0, 360.0),
        brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
    });
    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        scroll_descriptor.clone(),
        scroll_scene,
    )));

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        shell_descriptor,
        shell_scene,
    )));

    let update_damage = if update_kind == SceneLayerUpdateKind::Transform {
        Rect::new(42.0, 0.0, 360.0, 320.0)
    } else {
        child_bounds
    };

    SceneFrame {
        window_id,
        viewport: Size::new(430.0, 360.0),
        surface_size: Size::new(430.0, 360.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(update_kind, child_descriptor)
                .with_damage(update_damage),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    }
}

pub(crate) fn logical_x_from_ndc(ndc_x: f32, viewport: Size) -> f32 {
    ((ndc_x + 1.0) * 0.5) * viewport.width
}

pub(crate) fn logical_y_from_ndc(ndc_y: f32, viewport: Size) -> f32 {
    ((1.0 - ndc_y) * 0.5) * viewport.height
}

pub(crate) const PHYSICAL_PIXEL_ALIGNMENT_EPSILON: f32 = 0.005;

pub(crate) fn is_physically_pixel_aligned(value: f32, scale_factor: f32) -> bool {
    let physical = value * scale_factor;
    (physical - physical.round()).abs() <= PHYSICAL_PIXEL_ALIGNMENT_EPSILON
}

/// Minimal RGBA8 PNG encoder using a single zlib "stored" (uncompressed) block, so the
/// capture test can persist a screenshot without pulling in an image/png dependency.
pub(crate) fn encode_png_rgba8(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in bytes {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn adler32(bytes: &[u8]) -> u32 {
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        for &byte in bytes {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut crc_input = Vec::with_capacity(4 + data.len());
        crc_input.extend_from_slice(kind);
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&crc_input);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    }

    // Raw image data: one filter byte (0 = None) per scanline, then RGBA pixels.
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[row * stride..(row + 1) * stride]);
    }

    // zlib stream: 0x78 0x01 header, stored deflate blocks, adler32 trailer.
    let mut zlib = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < raw.len() {
        let block = (raw.len() - offset).min(0xFFFF);
        let last = offset + block >= raw.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend_from_slice(&(block as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(block as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + block]);
        offset += block;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth 8, color type 6 (RGBA)
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);
    png
}
