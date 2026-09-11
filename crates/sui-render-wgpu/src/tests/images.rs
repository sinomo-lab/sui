use crate::WgpuRenderer;
use crate::draw::DrawOpKind;
use crate::primitives::to_ndc;
use crate::resources::image_mip_chain;
use crate::resources::image_mip_level_count;
use crate::retained::RetainedCompositorState;
use crate::scene::build_vertices;
use crate::tests::support::prepare_with_compositor;
use crate::text_engine::TextEngine;
use std::sync::Arc;
use sui_core::ImageHandle;
use sui_core::Rect;
use sui_core::Size;
use sui_core::Transform;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::ImageSampling;
use sui_scene::ImageSource;
use sui_scene::RegisteredImage;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_text::FontRegistry;
use sui_text::TextLayoutRegistry;

#[test]
pub(crate) fn bitmap_mip_chain_reaches_one_pixel_and_preserves_transparent_edge_color() {
    let image = RegisteredImage::from_rgba8(
        2,
        2,
        vec![255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0],
    )
    .unwrap();

    assert_eq!(image_mip_level_count(8, 4), 4);
    let levels = image_mip_chain(&image, true);
    assert_eq!(levels.len(), 2);
    assert_eq!((levels[1].width, levels[1].height), (1, 1));
    assert_eq!(&levels[1].pixels[0..3], &[255, 0, 0]);
    assert_eq!(levels[1].pixels[3], 64);
    assert_eq!(image_mip_chain(&image, false).len(), 1);
}

#[test]
pub(crate) fn retained_compositor_uses_registered_image_handle() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
        &mut compositor,
    )
    .unwrap();

    assert_eq!(ops.draw_ops.len(), 1);
    let op = &ops.draw_ops[0];
    assert!(
        matches!(op.kind, DrawOpKind::Image { handle: value, sampling: ImageSampling::Linear, .. } if value == handle)
    );
    assert_eq!(op.vertices.len, 6);
}

#[test]
pub(crate) fn renderer_caches_physical_svg_variants_and_bitmap_mip_levels() {
    let svg_handle = ImageHandle::new(25);
    let bitmap_handle = ImageHandle::new(26);
    let dynamic_bitmap_handle = ImageHandle::new(27);
    let mut images = ImageRegistry::new();
    images.insert(
            svg_handle,
            RegisteredImage::from_svg(
                br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" viewBox="0 0 4 4"><circle cx="2" cy="2" r="2" fill="#fff"/></svg>"##,
            )
            .unwrap(),
        );
    images.insert(
        bitmap_handle,
        RegisteredImage::from_rgba8(8, 4, vec![255; 8 * 4 * 4]).unwrap(),
    );
    images.insert(
        dynamic_bitmap_handle,
        RegisteredImage::from_rgba8(8, 4, vec![255; 8 * 4 * 4])
            .unwrap()
            .without_mipmaps(),
    );

    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawImage {
        rect: Rect::new(2.0, 2.0, 14.0, 12.0),
        source: ImageSource::new(svg_handle),
    });
    scene.push(SceneCommand::DrawImage {
        rect: Rect::new(20.0, 2.0, 8.0, 4.0),
        source: ImageSource::new(bitmap_handle),
    });
    scene.push(SceneCommand::DrawImage {
        rect: Rect::new(30.0, 2.0, 8.0, 4.0),
        source: ImageSource::new(dynamic_bitmap_handle),
    });
    let frame = SceneFrame {
        window_id: WindowId::new(9),
        viewport: Size::new(50.0, 30.0),
        surface_size: Size::new(100.0, 60.0),
        scale_factor: 2.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(images),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::new();
    renderer.render(&frame).unwrap();

    let (svg_key, svg_texture) = renderer
        .image_cache
        .iter()
        .find(|(key, _)| key.handle == svg_handle)
        .expect("physical SVG texture cached");
    let svg_size = svg_key.raster_size.expect("SVG cache key has raster size");
    assert_eq!((svg_size.width, svg_size.height), (28, 24));
    assert_eq!(
        (svg_texture.texture.width(), svg_texture.texture.height()),
        (28, 24)
    );
    assert_eq!(svg_texture.texture.mip_level_count(), 1);

    let (bitmap_key, bitmap_texture) = renderer
        .image_cache
        .iter()
        .find(|(key, _)| key.handle == bitmap_handle)
        .expect("bitmap texture cached");
    assert_eq!(bitmap_key.raster_size, None);
    assert!(bitmap_key.mipmapped);
    assert_eq!(bitmap_texture.texture.mip_level_count(), 4);

    let (dynamic_bitmap_key, dynamic_bitmap_texture) = renderer
        .image_cache
        .iter()
        .find(|(key, _)| key.handle == dynamic_bitmap_handle)
        .expect("dynamic bitmap texture cached");
    assert_eq!(dynamic_bitmap_key.raster_size, None);
    assert!(!dynamic_bitmap_key.mipmapped);
    assert_eq!(dynamic_bitmap_texture.texture.mip_level_count(), 1);
}

#[test]
pub(crate) fn retained_compositor_preserves_transformed_image_quad() {
    let handle = ImageHandle::new(24);
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

    let rect = Rect::new(4.0, 6.0, 32.0, 24.0);
    let transform = Transform::translation(40.0, 10.0).then(Transform::rotation(0.35));
    let mut scene = Scene::new();
    scene.push(SceneCommand::PushTransform { transform });
    scene.push(SceneCommand::DrawImage {
        rect,
        source: ImageSource::new(handle),
    });
    scene.push(SceneCommand::PopTransform);

    let mut text_engine = TextEngine::new().unwrap();
    let vertices = build_vertices(
        &SceneFrame {
            window_id: WindowId::new(7),
            viewport: Size::new(128.0, 96.0),
            surface_size: Size::new(128.0, 96.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
    )
    .unwrap();

    let expected = transform.transform_point(rect.origin);
    let expected = to_ndc(expected.x, expected.y, Size::new(128.0, 96.0));
    assert!((vertices[0].position[0] - expected[0]).abs() < 0.001);
    assert!((vertices[0].position[1] - expected[1]).abs() < 0.001);
}

#[test]
pub(crate) fn retained_compositor_errors_for_unregistered_image_handle() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
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
