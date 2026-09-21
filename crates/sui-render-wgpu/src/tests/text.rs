use crate::WgpuRenderer;
use crate::capture::RgbaImage;
use crate::draw::DrawOpKind;
use crate::interop::WgpuExternalTextureRegistry;
use crate::output::shader_color;
use crate::retained::RetainedCompositorState;
use crate::scene::build_vertices;
use crate::shaders::TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE;
use crate::shaders::TEXT_ATLAS_SHADER_SOURCE;
use crate::tests::support::{
    RGBA_CHANNEL_TOLERANCE, assert_rgba_channels_near, assert_rgba_pixel_near,
    assert_rgba_pixels_near, encode_png_rgba8, is_physically_pixel_aligned, load_test_font,
    logical_x_from_ndc, logical_y_from_ndc, prepare_with_compositor, rgba_image_diff_count,
};
use crate::text::CachedGlyphAtlas;
use crate::text::GlyphCacheKey;
use crate::text::GlyphFaceCacheKey;
use crate::text::GlyphSubpixelOffsetKey;
use crate::text::TextAtlasColorMode;
use crate::text::TextAtlasPages;
use crate::text_engine::TextEngine;
use crate::text_engine::allows_lcd_text;
use crate::text_engine::append_cached_glyph_atlas;
use crate::text_engine::apply_stem_darkening_to_coverage;
use crate::text_engine::convert_subpixel_texel_for_mode;
use crate::text_engine::glyph_raster_offset;
use crate::text_engine::glyph_subpixel_offset;
use crate::text_engine::swash_image_to_rgba;
use crate::text_policy::StemDarkening;
use crate::text_policy::TextCoveragePolicy;
use crate::text_policy::TextHinting;
use crate::text_policy::TextRenderMode;
use std::sync::Arc;
use sui_core::Color;
use sui_core::FontHandle;
use sui_core::ImageHandle;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::ImageSource;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_scene::StrokeStyle;
use sui_scene::TextRenderCoveragePolicy;
use sui_scene::TextRenderPolicy;
use sui_scene::TextSubpixelOrder;
use sui_text::FontRegistry;
use sui_text::ShapedGlyph;
use sui_text::ShapedText;
use sui_text::ShapedTextWindow;
use sui_text::TextLayoutRegistry;
use sui_text::TextRun;
use sui_text::TextStyle;
use sui_text::TextSystem;
use swash::scale::Source as SwashSource;
use swash::scale::StrikeWith as SwashStrikeWith;
use swash::scale::image::Content as SwashImageContent;
use tiny_skia::PathBuilder as TinySkiaPathBuilder;

#[test]
pub(crate) fn build_vertices_supports_text_and_stroke_primitives() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
    )
    .unwrap();

    assert!(!vertices.is_empty());
    assert!(vertices.len() >= 30);
}

#[test]
pub(crate) fn cached_glyph_atlas_linearizes_srgb_inputs() {
    let atlas = CachedGlyphAtlas {
        scale: 12.0,
        offset: Vector::new(1.0, 2.0),
        size: Size::new(8.0, 10.0),
        uv_min: [0.25, 0.5],
        uv_max: [0.5, 0.75],
        color_mode: TextAtlasColorMode::Grayscale,
        is_color: false,
        page_index: 0,
    };
    let glyph = ShapedGlyph {
        glyph_id: 42,
        cluster: 0,
        span_id: sui_text::TextSpanId {
            paragraph_index: 0,
            span_index: 0,
        },
        run_index: 0,
        line_index: 0,
        face_index: 0,
        origin_x: 12.0,
        origin_y: 20.0,
        advance: Vector::new(8.0, 0.0),
        scale: 12.0,
        bounds: Some(Rect::new(13.0, 22.0, 8.0, 10.0)),
    };

    let color = Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 0.75);
    let mut vertices = Vec::new();
    append_cached_glyph_atlas(
        &mut vertices,
        &atlas,
        &glyph,
        color,
        Transform::IDENTITY,
        Size::new(64.0, 64.0),
        1.0,
    );

    assert_eq!(vertices.len(), 6);
    let expected = shader_color(color);
    for vertex in vertices {
        assert!((vertex.color[0] - expected[0]).abs() < 0.0001);
        assert!((vertex.color[1] - expected[1]).abs() < 0.0001);
        assert!((vertex.color[2] - expected[2]).abs() < 0.0001);
        assert!((vertex.color[3] - expected[3]).abs() < 0.0001);
    }
}

#[test]
pub(crate) fn cached_glyph_atlas_places_quad_from_subpixel_phase_integer() {
    let atlas = CachedGlyphAtlas {
        scale: 12.0,
        offset: Vector::ZERO,
        size: Size::new(8.0, 10.0),
        uv_min: [0.25, 0.5],
        uv_max: [0.5, 0.75],
        color_mode: TextAtlasColorMode::Grayscale,
        is_color: false,
        page_index: 0,
    };
    let glyph = ShapedGlyph {
        glyph_id: 42,
        cluster: 0,
        span_id: sui_text::TextSpanId {
            paragraph_index: 0,
            span_index: 0,
        },
        run_index: 0,
        line_index: 0,
        face_index: 0,
        origin_x: 10.75,
        origin_y: 20.0,
        advance: Vector::new(8.0, 0.0),
        scale: 12.0,
        bounds: None,
    };
    let viewport = Size::new(64.0, 64.0);
    let mut vertices = Vec::new();
    append_cached_glyph_atlas(
        &mut vertices,
        &atlas,
        &glyph,
        Color::WHITE,
        Transform::IDENTITY,
        viewport,
        1.0,
    );

    assert_eq!(vertices.len(), 6);
    let left = logical_x_from_ndc(vertices[0].position[0], viewport);
    assert!((left - 10.0).abs() < 0.0001);
}

#[test]
pub(crate) fn swash_placement_offsets_are_converted_to_screen_space() {
    let offset = glyph_raster_offset(
        &swash::zeno::Placement {
            left: 6,
            top: 10,
            width: 12,
            height: 14,
        },
        2.0,
    );

    assert_eq!(offset, Vector::new(3.0, -5.0));
}

#[test]
pub(crate) fn cached_color_glyph_atlas_uses_opacity_sentinel() {
    let atlas = CachedGlyphAtlas {
        scale: 12.0,
        offset: Vector::new(1.0, 2.0),
        size: Size::new(8.0, 10.0),
        uv_min: [0.25, 0.5],
        uv_max: [0.5, 0.75],
        color_mode: TextAtlasColorMode::Grayscale,
        is_color: true,
        page_index: 0,
    };
    let glyph = ShapedGlyph {
        glyph_id: 42,
        cluster: 0,
        span_id: sui_text::TextSpanId {
            paragraph_index: 0,
            span_index: 0,
        },
        run_index: 0,
        line_index: 0,
        face_index: 0,
        origin_x: 12.0,
        origin_y: 20.0,
        advance: Vector::new(8.0, 0.0),
        scale: 12.0,
        bounds: Some(Rect::new(13.0, 22.0, 8.0, 10.0)),
    };

    let mut vertices = Vec::new();
    append_cached_glyph_atlas(
        &mut vertices,
        &atlas,
        &glyph,
        Color::srgba(0.2, 0.4, 0.6, 0.75),
        Transform::IDENTITY,
        Size::new(64.0, 64.0),
        1.0,
    );

    assert_eq!(vertices.len(), 6);
    for vertex in vertices {
        assert_eq!(vertex.color[0], 1.0);
        assert_eq!(vertex.color[1], 1.0);
        assert_eq!(vertex.color[2], 1.0);
        assert_eq!(vertex.color[3], -0.75);
    }
}

#[test]
pub(crate) fn swash_color_glyph_images_store_srgb_for_text_atlas() {
    let image = swash::scale::image::Image {
        source: SwashSource::ColorBitmap(SwashStrikeWith::BestFit),
        content: SwashImageContent::Color,
        placement: swash::zeno::Placement {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        },
        data: vec![66, 42, 213, 128],
    };

    let rasterized = swash_image_to_rgba(
        &image,
        14.0,
        TextRenderMode::Grayscale,
        TextSubpixelOrder::None,
        StemDarkening::None,
    )
    .expect("color glyph should convert into atlas pixels");

    assert!(rasterized.is_color);
    // The atlas stores sRGB verbatim; the fragment shader linearizes at sample time.
    assert_rgba_pixels_near(
        &rasterized.pixels,
        &[66, 42, 213, 128],
        RGBA_CHANNEL_TOLERANCE,
    );
}

#[test]
pub(crate) fn text_coverage_policy_matches_egui_reference_formulas() {
    assert!((TextCoveragePolicy::Linear.apply(0.5) - 0.5).abs() < 0.0001);
    assert!((TextCoveragePolicy::Gamma(2.0).apply(0.5) - 0.25).abs() < 0.0001);
    assert!((TextCoveragePolicy::CoverageBoost(0.5).apply(0.5) - 0.625).abs() < 0.0001);
    assert!((TextCoveragePolicy::TwoCoverageMinusCoverageSq.apply(0.5) - 0.75).abs() < 0.0001);
}

#[test]
pub(crate) fn text_coverage_policy_defaults_to_perceptual_luminance_curve() {
    assert_eq!(
        TextCoveragePolicy::default(),
        TextCoveragePolicy::Perceptual
    );

    let dark = TextCoveragePolicy::Perceptual.resolved_for_text_color(Color::BLACK);
    let light = TextCoveragePolicy::Perceptual.resolved_for_text_color(Color::WHITE);
    assert!(
        dark.apply(0.5) > 0.5,
        "dark stems need contrast compensation"
    );
    assert!(
        light.apply(0.5) < 0.5,
        "light edges must not be brightened twice"
    );
    // The same accent needs different compensation on opposite surfaces.
    let accent = Color::rgba(0.1, 0.5, 0.8, 1.0);
    let on_white =
        TextCoveragePolicy::Perceptual.resolved_for_text_background(accent, Some(Color::WHITE));
    let on_black =
        TextCoveragePolicy::Perceptual.resolved_for_text_background(accent, Some(Color::BLACK));
    assert!(on_white.apply(0.5) > on_black.apply(0.5));
    for text in [
        Color::BLACK,
        Color::WHITE,
        accent,
        Color::rgba(0.5, 0.5, 0.5, 1.0),
    ] {
        for bg in [Color::BLACK, Color::WHITE, text] {
            let policy =
                TextCoveragePolicy::Perceptual.resolved_for_text_background(text, Some(bg));
            let mut previous = 0.0;
            for i in 0..=255 {
                let alpha = policy.apply(i as f32 / 255.0);
                assert!(alpha.is_finite() && alpha >= previous && alpha <= 1.0);
                previous = alpha;
            }
            assert_eq!(policy.apply(0.0), 0.0);
            assert_eq!(policy.apply(1.0), 1.0);
        }
    }
    assert_eq!(
        TextCoveragePolicy::Perceptual
            .resolved_for_text_color(Color::linear_rgba(2.0, 1.0, 1.0, 1.0)),
        TextCoveragePolicy::Linear
    );
}

#[test]
pub(crate) fn text_render_mode_defaults_to_grayscale() {
    assert_eq!(TextRenderMode::default(), TextRenderMode::Grayscale);
}

#[test]
fn transformed_text_rasterizes_at_display_resolution() {
    use sui_core::WidgetId;
    use sui_scene::SceneLayer;
    let handle = FontHandle::new(9001);
    let mut fonts = FontRegistry::new();
    fonts.insert(
        handle,
        sui_text::RegisteredFont::from_bytes(sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT.to_vec()),
    );
    let fonts = Arc::new(fonts);
    let render = |zoom: f32, dpi: f32, transformed: bool, retained: bool| {
        let mut content = Scene::new();
        let size_scale = if transformed { 1.0 } else { zoom };
        content.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(
                12.125 * size_scale,
                8.25 * size_scale,
                220.0 * size_scale,
                26.0 * size_scale,
            ),
            text: "minimum AVWA 012345".to_string(),
            style: TextStyle {
                font: Some(handle),
                font_size: 15.0 * size_scale,
                line_height: 22.0 * size_scale,
                color: Color::BLACK,
                ..TextStyle::default()
            },
        }));
        let mut scene = Scene::new();
        scene.push(SceneCommand::Clear(Color::WHITE));
        if transformed {
            scene.push(SceneCommand::PushTransform {
                transform: Transform::scale(zoom, zoom),
            });
        }
        if retained {
            let widget = WidgetId::new(9001);
            scene.push(SceneCommand::Layer(SceneLayer::new(
                widget,
                Rect::new(8.0, 4.0, 230.0, 40.0),
                content,
            )));
        } else {
            for command in content.commands() {
                scene.push(command.clone());
            }
        }
        if transformed {
            scene.push(SceneCommand::PopTransform);
        }
        let window_id = WindowId::new(9001);
        let viewport = Size::new(500.0, 100.0);
        let frame = SceneFrame {
            window_id,
            viewport,
            surface_size: Size::new(viewport.width * dpi, viewport.height * dpi),
            scale_factor: dpi,
            dirty_regions: vec![],
            layer_updates: vec![],
            scene,
            font_registry: Arc::clone(&fonts),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };
        let mut renderer = WgpuRenderer::new();
        renderer.render(&frame).unwrap();
        renderer.capture_last_frame_rgba(window_id).unwrap()
    };
    for dpi in [1.0, 1.25, 2.0] {
        for zoom in [0.5, 1.25, 1.5, 2.0] {
            let reference = render(zoom, dpi, false, false);
            for retained in [false, true] {
                let scaled = render(zoom, dpi, true, retained);
                let differing = scaled
                    .pixels()
                    .chunks_exact(4)
                    .zip(reference.pixels().chunks_exact(4))
                    .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 3))
                    .count();
                let max_delta = scaled
                    .pixels()
                    .iter()
                    .zip(reference.pixels())
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                // Separate allocations can quantize packed atlas UVs differently;
                // allow a small edge-channel error, not the large differences
                // caused by magnifying a lower-resolution mask.
                assert!(
                    max_delta <= 10,
                    "scaled text differs from directly sized text: zoom={zoom}, dpi={dpi}, retained={retained}, pixels={differing}, max_delta={max_delta}"
                );
            }
        }
    }
}

#[test]
fn text_raster_resolution_covers_rotated_stretched_and_sheared_axes() {
    use crate::text_engine::text_transform_scale;
    assert!(
        (text_transform_scale(Transform::scale(2.0, 2.0).then(Transform::rotation(0.6))) - 2.0)
            .abs()
            < 1e-5
    );
    assert!((text_transform_scale(Transform::scale(0.5, 0.75)) - 0.75).abs() < 1e-5);
    assert!((text_transform_scale(Transform::scale(-2.0, 1.0)) - 2.0).abs() < 1e-5);
    assert!(
        (text_transform_scale(Transform::new(1.0, 0.0, 1.0, 1.0, 0.0, 0.0)) - 1.618034).abs()
            < 1e-5
    );
}

#[test]
fn perceptual_text_uses_inherited_backdrop_and_refreshes_when_it_changes() {
    use sui_core::WidgetId;
    use sui_scene::SceneLayer;
    let mut actual = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    let foreground = Color::rgba(0.08, 0.49, 0.72, 1.0);
    let window = WindowId::new(9020);
    for background in [
        Color::WHITE,
        Color::rgba(0.07, 0.09, 0.12, 1.0),
        Color::WHITE,
    ] {
        let mut text = Scene::new();
        text.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(16.0, 16.0, 250.0, 26.0),
            text: "Colored minimum 012345".into(),
            style: TextStyle {
                font_size: 15.0,
                line_height: 22.0,
                color: foreground,
                ..TextStyle::default()
            },
        }));
        let mut panel = Scene::new();
        panel.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 284.0, 60.0),
            brush: background.into(),
        });
        panel.push(SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(9022),
            Rect::new(16.0, 16.0, 250.0, 26.0),
            text,
        )));
        let mut frame = SceneFrame::new(window, Size::new(300.0, 80.0));
        frame
            .scene
            .push(SceneCommand::Clear(Color::rgba(0.3, 0.1, 0.2, 1.0)));
        frame.scene.push(SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(9021),
            Rect::new(8.0, 8.0, 284.0, 60.0),
            panel,
        )));
        reference.set_text_coverage_policy(
            TextCoveragePolicy::Perceptual
                .resolved_for_text_background(foreground, Some(background)),
        );
        actual.render(&frame).unwrap();
        reference.render(&frame).unwrap();
        crate::tests::support::assert_rgba_images_match(
            &actual.capture_last_frame_rgba(window).unwrap(),
            &reference.capture_last_frame_rgba(window).unwrap(),
        );
    }
}

#[test]
pub(crate) fn text_subpixel_order_defaults_to_none() {
    assert_eq!(TextSubpixelOrder::default(), TextSubpixelOrder::None);
    assert_eq!(
        WgpuRenderer::new().text_subpixel_order(),
        TextSubpixelOrder::None
    );
}

#[test]
fn translated_retained_text_resolves_the_backdrop_at_its_presented_position() {
    use sui_core::WidgetId;
    use sui_scene::{LayerProperties, SceneLayer, SceneLayerDescriptor, SceneLayerId};
    let mut actual = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    let window = WindowId::new(9030);
    let foreground = Color::rgba(0.08, 0.49, 0.72, 1.0);
    let dark = Color::rgba(0.07, 0.09, 0.12, 1.0);
    for translation in [0.0, 200.0, 0.0] {
        let mut text = Scene::new();
        text.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(16.0, 16.0, 150.0, 26.0),
            text: "minimum".into(),
            style: TextStyle {
                font_size: 15.0,
                line_height: 22.0,
                color: foreground,
                ..TextStyle::default()
            },
        }));
        let owner = WidgetId::new(9030);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(owner),
            owner,
            Rect::new(16.0, 16.0, 150.0, 26.0),
        )
        .with_properties(LayerProperties::new(1.0, Vector::new(translation, 0.0)));
        let mut frame = SceneFrame::new(window, Size::new(400.0, 80.0));
        frame.scene.push(SceneCommand::Clear(Color::WHITE));
        frame.scene.push(SceneCommand::FillRect {
            rect: Rect::new(200.0, 0.0, 200.0, 80.0),
            brush: dark.into(),
        });
        frame
            .scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor, text,
            )));
        reference.set_text_coverage_policy(
            TextCoveragePolicy::Perceptual.resolved_for_text_background(
                foreground,
                Some(if translation == 0.0 {
                    Color::WHITE
                } else {
                    dark
                }),
            ),
        );
        actual.render(&frame).unwrap();
        reference.render(&frame).unwrap();
        crate::tests::support::assert_rgba_images_match(
            &actual.capture_last_frame_rgba(window).unwrap(),
            &reference.capture_last_frame_rgba(window).unwrap(),
        );
    }
}

#[test]
pub(crate) fn slight_hinting_enables_below_threshold() {
    let config = TextHinting::Slight { max_ppem: 18.0 };
    assert!(config.should_hint(14.0));
    assert!(!config.should_hint(24.0));
}

#[test]
pub(crate) fn stem_darkening_boosts_partial_coverage() {
    let darkened = apply_stem_darkening_to_coverage(128, 0.1);
    assert!(darkened > 128);
}

#[test]
pub(crate) fn lcd_text_render_mode_has_distinct_cache_identity() {
    assert_ne!(
        TextAtlasColorMode::from(TextRenderMode::Grayscale),
        TextAtlasColorMode::from(TextRenderMode::LcdSubpixel),
    );
}

#[test]
pub(crate) fn subpixel_mask_preserves_distinct_rgb_channels_in_lcd_mode() {
    let converted = convert_subpixel_texel_for_mode(
        [255, 128, 32, 255],
        TextRenderMode::LcdSubpixel,
        TextSubpixelOrder::Rgb,
        0.0,
    );
    assert_rgba_channels_near(&converted, [32, 128, 255, 255], RGBA_CHANNEL_TOLERANCE);
}

#[test]
pub(crate) fn subpixel_mask_can_reverse_physical_order_for_bgr_lcd() {
    let converted = convert_subpixel_texel_for_mode(
        [255, 128, 32, 255],
        TextRenderMode::LcdSubpixel,
        TextSubpixelOrder::Bgr,
        0.0,
    );
    assert_rgba_channels_near(&converted, [255, 128, 32, 255], RGBA_CHANNEL_TOLERANCE);
}

#[test]
pub(crate) fn subpixel_mask_uses_grayscale_when_subpixel_order_is_none() {
    let converted = convert_subpixel_texel_for_mode(
        [255, 128, 32, 255],
        TextRenderMode::LcdSubpixel,
        TextSubpixelOrder::None,
        0.0,
    );
    assert_rgba_channels_near(&converted, [255, 255, 255, 138], RGBA_CHANNEL_TOLERANCE);
}

#[test]
pub(crate) fn glyph_subpixel_offset_tracks_quarter_pixel_phase() {
    let glyph = ShapedGlyph {
        glyph_id: 42,
        cluster: 0,
        span_id: sui_text::TextSpanId {
            paragraph_index: 0,
            span_index: 0,
        },
        run_index: 0,
        line_index: 0,
        face_index: 0,
        origin_x: 10.25,
        origin_y: 20.0,
        advance: Vector::new(8.0, 0.0),
        scale: 12.0,
        bounds: None,
    };

    assert_eq!(
        glyph_subpixel_offset(Transform::IDENTITY, Vector::ZERO, &glyph, 1.0),
        GlyphSubpixelOffsetKey::new(1, 0)
    );
    assert_eq!(
        glyph_subpixel_offset(Transform::IDENTITY, Vector::ZERO, &glyph, 2.0),
        GlyphSubpixelOffsetKey::new(2, 0)
    );
    assert_eq!(
        glyph_subpixel_offset(Transform::rotation(0.25), Vector::ZERO, &glyph, 1.0),
        GlyphSubpixelOffsetKey::default()
    );
}

#[test]
pub(crate) fn glyph_cache_key_includes_subpixel_offset() {
    let face = GlyphFaceCacheKey {
        data_ptr: 0x1000,
        data_len: 128,
        face_index: 0,
    };
    let first = GlyphCacheKey::new(
        face,
        7,
        1024,
        GlyphSubpixelOffsetKey::new(0, 0),
        TextRenderMode::Grayscale,
        TextSubpixelOrder::None,
        TextHinting::None,
        StemDarkening::None,
        400,
    );
    let second = GlyphCacheKey::new(
        face,
        7,
        1024,
        GlyphSubpixelOffsetKey::new(1, 0),
        TextRenderMode::Grayscale,
        TextSubpixelOrder::None,
        TextHinting::None,
        StemDarkening::None,
        400,
    );
    assert_ne!(first, second);
}

#[test]
pub(crate) fn glyph_cache_key_includes_weight() {
    let face = GlyphFaceCacheKey {
        data_ptr: 0x1000,
        data_len: 128,
        face_index: 0,
    };
    let key = |weight| {
        GlyphCacheKey::new(
            face,
            7,
            1024,
            GlyphSubpixelOffsetKey::new(0, 0),
            TextRenderMode::Grayscale,
            TextSubpixelOrder::None,
            TextHinting::None,
            StemDarkening::None,
            weight,
        )
    };
    // Different weights of a variable font rasterize differently -> distinct cache entries.
    assert_ne!(key(400), key(700));
    assert_eq!(key(700), key(700));
}

#[test]
pub(crate) fn glyph_cache_key_includes_subpixel_order() {
    let face = GlyphFaceCacheKey {
        data_ptr: 0x1000,
        data_len: 128,
        face_index: 0,
    };
    let key = |order| {
        GlyphCacheKey::new(
            face,
            7,
            1024,
            GlyphSubpixelOffsetKey::new(0, 0),
            TextRenderMode::LcdSubpixel,
            order,
            TextHinting::None,
            StemDarkening::None,
            400,
        )
    };
    assert_ne!(key(TextSubpixelOrder::Rgb), key(TextSubpixelOrder::Bgr));
    assert_eq!(key(TextSubpixelOrder::None), key(TextSubpixelOrder::None));
}

#[test]
pub(crate) fn text_atlas_shaders_use_sampled_coverage_and_dual_source_blending() {
    assert!(TEXT_ATLAS_SHADER_SOURCE.contains("fn apply_text_coverage"));
    assert!(TEXT_ATLAS_SHADER_SOURCE.contains("apply_text_coverage(sampled.a"));
    assert!(!TEXT_ATLAS_SHADER_SOURCE.contains("TEXT_COVERAGE_GAMMA"));
    assert!(TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE.contains("@blend_src(0)"));
    assert!(TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE.contains("@blend_src(1)"));
    assert!(TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE.contains("fn apply_text_coverage"));
    assert!(TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE.contains("apply_text_coverage(sampled.a"));
    assert!(!TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE.contains("TEXT_COVERAGE_GAMMA"));
}

#[test]
pub(crate) fn lcd_text_requires_axis_aligned_pixel_snapped_path() {
    assert!(allows_lcd_text(Transform::IDENTITY));
    assert!(!allows_lcd_text(Transform::rotation(
        std::f32::consts::FRAC_PI_4
    )));
    assert!(!allows_lcd_text(Transform::scale(-1.0, 1.0)));
    assert!(!allows_lcd_text(Transform::rotation(std::f32::consts::PI)));
}

#[test]
pub(crate) fn atlas_text_snaps_repeated_stems_to_physical_pixels() {
    let handle = FontHandle::new(31);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let viewport = Size::new(260.0, 52.0);
    let frame = SceneFrame {
        window_id: WindowId::new(98),
        viewport,
        surface_size: Size::new(390.0, 78.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 10.0, 220.0, 24.0),
                text: "scroll".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 14.0,
                    line_height: 18.0,
                    color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                    ..TextStyle::default()
                },
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let vertices = build_vertices(&frame, &mut text_engine).unwrap();

    assert!(
        vertices.len() >= 12,
        "expected atlas vertices for repeated l glyphs"
    );

    let first_l_left = logical_x_from_ndc(vertices[24].position[0], viewport);
    let second_l_left = logical_x_from_ndc(vertices[30].position[0], viewport);

    assert!(
        is_physically_pixel_aligned(first_l_left, frame.scale_factor),
        "first l did not snap to the physical pixel grid: x={first_l_left}"
    );
    assert!(
        is_physically_pixel_aligned(second_l_left, frame.scale_factor),
        "second l did not snap to the physical pixel grid: x={second_l_left}"
    );
}

#[test]
pub(crate) fn atlas_text_is_position_invariant_at_matching_fractional_dpi_phase() {
    let handle = FontHandle::new(311);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let viewport = Size::new(260.0, 140.0);
    let outer_origin = Point::new(39.0, 2.3333333);
    let inner_origin = Point::new(58.333332, 97.666664);
    let delta = inner_origin - outer_origin;
    let frame = SceneFrame {
        window_id: WindowId::new(99),
        viewport,
        surface_size: Size::new(390.0, 210.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            for origin in [outer_origin, inner_origin] {
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(origin.x, origin.y, 180.0, 22.0),
                    text: "Light preview live updates".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 14.0,
                        line_height: 20.0,
                        color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                        ..TextStyle::default()
                    },
                }));
            }
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let vertices = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(
        vertices.len() % 2,
        0,
        "expected identical text runs to produce an even vertex count"
    );

    let split = vertices.len() / 2;
    let (outer_vertices, inner_vertices) = vertices.split_at(split);
    assert_eq!(outer_vertices.len(), inner_vertices.len());

    for (index, (outer, inner)) in outer_vertices.iter().zip(inner_vertices.iter()).enumerate() {
        let outer_x = logical_x_from_ndc(outer.position[0], viewport);
        let outer_y = logical_y_from_ndc(outer.position[1], viewport);
        let inner_x = logical_x_from_ndc(inner.position[0], viewport);
        let inner_y = logical_y_from_ndc(inner.position[1], viewport);
        let normalized_inner_x = inner_x - delta.x;
        let normalized_inner_y = inner_y - delta.y;

        assert!(
            (outer_x - normalized_inner_x).abs() < 0.0001,
            "vertex {index} changed x after translation normalization: outer={outer_x}, inner={inner_x}, delta_x={} ",
            delta.x,
        );
        assert!(
            (outer_y - normalized_inner_y).abs() < 0.0001,
            "vertex {index} changed y after translation normalization: outer={outer_y}, inner={inner_y}, delta_y={} ",
            delta.y,
        );
        assert_eq!(
            outer.tex_coords, inner.tex_coords,
            "vertex {index} UVs differ"
        );
        assert_eq!(outer.color, inner.color, "vertex {index} colors differ");
    }
}

#[test]
pub(crate) fn renderer_text_coverage_policy_shares_glyph_cache_entries_for_explicit_policies() {
    let handle = FontHandle::new(33);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let frame = SceneFrame {
        window_id: WindowId::new(201),
        viewport: Size::new(320.0, 84.0),
        surface_size: Size::new(320.0, 84.0),
        scale_factor: 1.25,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 320.0, 84.0),
                brush: Color::BLACK.into(),
            });
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(12.0, 8.0, 296.0, 64.0),
                text: "Reusable".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 55.5,
                    line_height: 59.5,
                    color: Color::WHITE,
                    ..TextStyle::default()
                },
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    text_engine.set_text_coverage_policy(TextCoveragePolicy::Linear);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();
    let linear_stats = text_engine.glyph_cache_stats();

    text_engine.set_text_coverage_policy(TextCoveragePolicy::TwoCoverageMinusCoverageSq);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();
    let dark_stats = text_engine.glyph_cache_stats();

    assert!(
        linear_stats.0 > 0,
        "linear policy should populate the glyph cache"
    );
    assert!(
        linear_stats.2 > 0,
        "first pass should record glyph cache misses"
    );
    assert!(
        dark_stats.1 > linear_stats.1,
        "switching policy should reuse existing glyph cache entries"
    );
    assert_eq!(dark_stats.0, linear_stats.0);
    assert_eq!(dark_stats.2, linear_stats.2);
}

#[test]
pub(crate) fn text_render_policy_scope_overrides_and_restores_glyph_cache_policy() {
    let handle = FontHandle::new(36);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let text_style = TextStyle {
        font: Some(handle),
        font_size: 24.0,
        line_height: 28.0,
        color: Color::WHITE,
        ..TextStyle::default()
    };

    let frame = SceneFrame {
        window_id: WindowId::new(204),
        viewport: Size::new(120.0, 120.0),
        surface_size: Size::new(120.0, 120.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 8.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style.clone(),
            }));
            scene.push(SceneCommand::PushTextRenderPolicy {
                policy: TextRenderPolicy::new()
                    .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq),
            });
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 44.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style.clone(),
            }));
            scene.push(SceneCommand::PopTextRenderPolicy);
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 80.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style,
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    text_engine.set_text_coverage_policy(TextCoveragePolicy::Linear);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(text_engine.glyph_cache_stats(), (1, 2, 1));
}

#[test]
pub(crate) fn text_render_policy_scope_overrides_and_restores_render_mode() {
    let handle = FontHandle::new(37);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let text_style = TextStyle {
        font: Some(handle),
        font_size: 24.0,
        line_height: 28.0,
        color: Color::WHITE,
        ..TextStyle::default()
    };

    let frame = SceneFrame {
        window_id: WindowId::new(205),
        viewport: Size::new(120.0, 120.0),
        surface_size: Size::new(120.0, 120.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::Clear(Color::BLACK));
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 8.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style.clone(),
            }));
            scene.push(SceneCommand::PushTextRenderPolicy {
                policy: TextRenderPolicy::new()
                    .with_render_mode(sui_scene::TextRenderMode::LcdSubpixel)
                    .with_subpixel_order(TextSubpixelOrder::Rgb),
            });
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 44.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style.clone(),
            }));
            scene.push(SceneCommand::PopTextRenderPolicy);
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 80.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style,
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    text_engine.lcd_blending_supported = true;
    text_engine.set_text_render_mode(TextRenderMode::Grayscale);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(text_engine.glyph_cache_stats(), (2, 1, 2));
}

#[test]
pub(crate) fn unsafe_lcd_text_transform_uses_grayscale_glyph_cache_entry() {
    let handle = FontHandle::new(38);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let text_style = TextStyle {
        font: Some(handle),
        font_size: 24.0,
        line_height: 28.0,
        color: Color::WHITE,
        ..TextStyle::default()
    };

    let frame = SceneFrame {
        window_id: WindowId::new(206),
        viewport: Size::new(160.0, 120.0),
        surface_size: Size::new(160.0, 120.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 8.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style.clone(),
            }));
            scene.push(SceneCommand::PushTextRenderPolicy {
                policy: TextRenderPolicy::new()
                    .with_render_mode(sui_scene::TextRenderMode::LcdSubpixel)
                    .with_subpixel_order(TextSubpixelOrder::Rgb),
            });
            scene.push(SceneCommand::PushTransform {
                transform: Transform::rotation(0.25),
            });
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(20.0, 20.0, 80.0, 28.0),
                text: "I".to_string(),
                style: text_style,
            }));
            scene.push(SceneCommand::PopTransform);
            scene.push(SceneCommand::PopTextRenderPolicy);
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    text_engine.set_text_render_mode(TextRenderMode::Grayscale);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(text_engine.glyph_cache_stats(), (1, 1, 1));
}

#[test]
pub(crate) fn linear_text_coverage_policy_shares_glyph_cache_entries_for_light_and_dark_text() {
    let handle = FontHandle::new(34);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let frame = SceneFrame {
        window_id: WindowId::new(202),
        viewport: Size::new(240.0, 96.0),
        surface_size: Size::new(240.0, 96.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 8.0, 100.0, 32.0),
                text: "I".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 24.0,
                    line_height: 28.0,
                    color: Color::BLACK,
                    ..TextStyle::default()
                },
            }));
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 48.0, 100.0, 32.0),
                text: "I".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 24.0,
                    line_height: 28.0,
                    color: Color::WHITE,
                    ..TextStyle::default()
                },
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    text_engine.set_text_coverage_policy(TextCoveragePolicy::Linear);
    let _ = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(text_engine.glyph_cache_stats(), (1, 1, 1));
}

#[test]
pub(crate) fn default_text_coverage_policy_shares_glyph_cache_entries_across_text_colors() {
    let handle = FontHandle::new(35);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let frame = SceneFrame {
        window_id: WindowId::new(203),
        viewport: Size::new(240.0, 96.0),
        surface_size: Size::new(240.0, 96.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 8.0, 100.0, 32.0),
                text: "I".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 24.0,
                    line_height: 28.0,
                    color: Color::BLACK,
                    ..TextStyle::default()
                },
            }));
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 48.0, 100.0, 32.0),
                text: "I".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 24.0,
                    line_height: 28.0,
                    color: Color::WHITE,
                    ..TextStyle::default()
                },
            }));
            scene
        },
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let _ = build_vertices(&frame, &mut text_engine).unwrap();

    assert_eq!(text_engine.glyph_cache_stats(), (1, 1, 1));
}

#[test]
pub(crate) fn text_engine_shapes_text_with_font_metrics() {
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
pub(crate) fn build_vertices_supports_pre_shaped_text() {
    let text_system = TextSystem::new();
    let layout = text_system
        .shape_text_persistent(
            None,
            "scene",
            Size::new(80.0, 24.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShapedText(ShapedText::new(
        Point::new(4.0, 6.0),
        &layout,
    )));

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
            text_layout_registry: text_system.text_layout_registry(),
        },
        &mut text_engine,
    )
    .unwrap();

    assert!(!vertices.is_empty());
}

#[test]
pub(crate) fn shaped_text_color_override_paints_cached_layout_without_changing_style() {
    let text_system = TextSystem::new();
    let layout_color = Color::rgba(0.92, 0.18, 0.16, 1.0);
    let paint_color = Color::rgba(0.12, 0.48, 0.86, 1.0);
    let layout = text_system
        .shape_text_persistent(
            None,
            "state",
            Size::new(96.0, 24.0),
            TextStyle::new(layout_color),
            &FontRegistry::new(),
        )
        .unwrap();

    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShapedText(
        ShapedText::new(Point::new(4.0, 6.0), &layout).with_color(paint_color),
    ));

    let mut text_engine = TextEngine::new().unwrap();
    let vertices = build_vertices(
        &SceneFrame {
            window_id: WindowId::new(311),
            viewport: Size::new(120.0, 80.0),
            surface_size: Size::new(120.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: text_system.text_layout_registry(),
        },
        &mut text_engine,
    )
    .unwrap();

    assert_eq!(layout.style().color, layout_color);
    assert!(!vertices.is_empty());
    assert_eq!(vertices[0].color, shader_color(paint_color));
}

#[test]
pub(crate) fn shaped_text_from_runtime_draw_text_snaps_to_physical_pixels_at_fractional_dpi() {
    let handle = FontHandle::new(312);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let text_system = TextSystem::new();
    let layout = text_system
        .shape_text_persistent(
            None,
            "Widget text",
            Size::new(132.0, 24.0),
            TextStyle {
                font: Some(handle),
                font_size: 14.0,
                line_height: 19.0,
                color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                ..TextStyle::default()
            },
            &fonts,
        )
        .unwrap();
    let viewport = Size::new(220.0, 72.0);
    let origin = Point::new(18.333_332, 10.666_667);
    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShapedText(ShapedText::new(
        origin, &layout,
    )));

    let frame = SceneFrame {
        window_id: WindowId::new(312),
        viewport,
        surface_size: Size::new(330.0, 108.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: text_system.text_layout_registry(),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor)
        .expect("shaped text frame should prepare");
    let text_op = draw_ops
        .draw_ops
        .iter()
        .find(|op| matches!(op.kind, DrawOpKind::TextAtlas))
        .expect("shaped text should emit atlas instances");
    let start = text_op.vertices.start as usize;
    let end = start + text_op.vertices.len as usize;
    let instances = &draw_ops.text_instances[start..end];
    assert!(!instances.is_empty());
    for (index, instance) in instances.iter().enumerate() {
        let x = logical_x_from_ndc(instance.top_left[0], viewport);
        let y = logical_y_from_ndc(instance.top_left[1], viewport);
        assert!(
            is_physically_pixel_aligned(x, frame.scale_factor),
            "shaped text instance {index} x was not physically aligned: {x}"
        );
        assert!(
            is_physically_pixel_aligned(y, frame.scale_factor),
            "shaped text instance {index} y was not physically aligned: {y}"
        );
    }
}

#[test]
pub(crate) fn shaped_text_window_snaps_to_physical_pixels_at_fractional_dpi() {
    let handle = FontHandle::new(313);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let text_system = TextSystem::new();
    let layout = text_system
        .shape_text_persistent(
            None,
            "First line wraps into a second visible line",
            Size::new(84.0, 72.0),
            TextStyle {
                font: Some(handle),
                font_size: 14.0,
                line_height: 19.0,
                color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                ..TextStyle::default()
            },
            &fonts,
        )
        .unwrap();
    assert!(
        layout.lines().len() > 1,
        "test layout must produce a non-empty line window"
    );
    assert!(
        layout.line_window(1..2).glyph_instances().len() > 0,
        "test layout line window must contain glyphs"
    );
    let viewport = Size::new(220.0, 96.0);
    let origin = Point::new(18.333_332, 10.666_667);
    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShapedTextWindow(ShapedTextWindow::new(
        origin,
        &layout,
        1..2,
    )));

    let frame = SceneFrame {
        window_id: WindowId::new(313),
        viewport,
        surface_size: Size::new(330.0, 144.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: text_system.text_layout_registry(),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor)
        .expect("windowed shaped text frame should prepare");
    let text_op = draw_ops
        .draw_ops
        .iter()
        .find(|op| matches!(op.kind, DrawOpKind::TextAtlas))
        .expect("windowed shaped text should emit atlas instances");
    let start = text_op.vertices.start as usize;
    let end = start + text_op.vertices.len as usize;
    let instances = &draw_ops.text_instances[start..end];
    assert!(!instances.is_empty());
    for (index, instance) in instances.iter().enumerate() {
        let x = logical_x_from_ndc(instance.top_left[0], viewport);
        let y = logical_y_from_ndc(instance.top_left[1], viewport);
        assert!(
            is_physically_pixel_aligned(x, frame.scale_factor),
            "windowed shaped text instance {index} x was not physically aligned: {x}"
        );
        assert!(
            is_physically_pixel_aligned(y, frame.scale_factor),
            "windowed shaped text instance {index} y was not physically aligned: {y}"
        );
    }
}

#[test]
pub(crate) fn text_engine_reuses_cached_glyph_atlas_entries_across_repeated_builds() {
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
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let first = build_vertices(&frame, &mut text_engine).unwrap();
    assert!(!first.is_empty());
    assert_eq!(first.len(), 18);
    assert!(first.iter().any(|vertex| vertex.tex_coords != [0.0, 0.0]));
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
pub(crate) fn text_engine_parses_swash_face_once_per_text_run_when_glyphs_miss() {
    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawText(TextRun {
        rect: Rect::new(4.0, 6.0, 120.0, 28.0),
        text: "abc".to_string(),
        style: TextStyle::new(Color::WHITE),
    }));

    let frame = SceneFrame {
        window_id: WindowId::new(15),
        viewport: Size::new(160.0, 60.0),
        surface_size: Size::new(160.0, 60.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let first = build_vertices(&frame, &mut text_engine).unwrap();
    assert!(!first.is_empty());
    assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));
    assert_eq!(text_engine.swash_face_parse_count(), 1);

    let second = build_vertices(&frame, &mut text_engine).unwrap();
    assert_eq!(first.len(), second.len());
    assert_eq!(text_engine.glyph_cache_stats(), (3, 3, 3));
    assert_eq!(text_engine.swash_face_parse_count(), 1);
}

#[test]
pub(crate) fn text_engine_buckets_cached_glyph_atlas_entries_by_scale() {
    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawText(TextRun {
        rect: Rect::new(4.0, 6.0, 120.0, 28.0),
        text: "abc".to_string(),
        style: TextStyle::new(Color::WHITE),
    }));

    let base_frame = SceneFrame {
        window_id: WindowId::new(13),
        viewport: Size::new(160.0, 60.0),
        surface_size: Size::new(160.0, 60.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut scaled_scene = Scene::new();
    scaled_scene.push(SceneCommand::DrawText(TextRun {
        rect: Rect::new(4.0, 6.0, 120.0, 56.0),
        text: "abc".to_string(),
        style: TextStyle {
            font_size: 28.0,
            line_height: 32.0,
            ..TextStyle::new(Color::WHITE)
        },
    }));

    let scaled_frame = SceneFrame {
        window_id: WindowId::new(14),
        viewport: Size::new(160.0, 80.0),
        surface_size: Size::new(160.0, 80.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: scaled_scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let first = build_vertices(&base_frame, &mut text_engine).unwrap();
    assert!(!first.is_empty());
    assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));

    let second = build_vertices(&scaled_frame, &mut text_engine).unwrap();
    assert!(!second.is_empty());
    assert_eq!(text_engine.glyph_cache_stats(), (6, 0, 6));
}

#[test]
pub(crate) fn glyph_raster_bounds_expand_fractional_edges() {
    let mut builder = TinySkiaPathBuilder::new();
    builder.move_to(0.6, -0.2);
    builder.line_to(10.2, -0.2);
    builder.line_to(10.2, 4.4);
    builder.line_to(0.6, 4.4);
    builder.close();
    let path = builder.finish().expect("fractional rectangle path");

    let bounds =
        crate::text_engine::glyph_raster_bounds(&path).expect("bounds for fractional rectangle");

    assert!((bounds.logical_min_x - 0.6).abs() < 0.0001);
    assert!((bounds.logical_min_y + 0.2).abs() < 0.0001);
    assert!((bounds.logical_width - 9.6).abs() < 0.0001);
    assert!((bounds.logical_height - 4.6).abs() < 0.0001);
    assert_eq!(bounds.raster_min_x, 0.0);
    assert_eq!(bounds.raster_min_y, -1.0);
    assert_eq!(bounds.raster_width, 11);
    assert_eq!(bounds.raster_height, 6);
}

#[test]
pub(crate) fn atlas_text_keeps_terminal_glyphs_at_fractional_scale() {
    let handle = FontHandle::new(30);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let build_frame = |text: &str| SceneFrame {
        window_id: WindowId::new(96),
        viewport: Size::new(260.0, 52.0),
        surface_size: Size::new(390.0, 78.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 260.0, 52.0),
                brush: Color::WHITE.into(),
            });
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(8.0, 10.0, 220.0, 24.0),
                text: text.to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 14.0,
                    line_height: 18.0,
                    color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                    ..TextStyle::default()
                },
            }));
            scene
        },
        font_registry: Arc::new(fonts.clone()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    let without_terminal = build_frame("inspecto");
    renderer.render(&without_terminal).unwrap();
    let without_terminal_pixels = renderer
        .capture_last_frame_rgba(without_terminal.window_id)
        .unwrap();

    let with_terminal = build_frame("inspector");
    renderer.render(&with_terminal).unwrap();
    let with_terminal_pixels = renderer
        .capture_last_frame_rgba(with_terminal.window_id)
        .unwrap();

    let diff_count = rgba_image_diff_count(&without_terminal_pixels, &with_terminal_pixels);

    assert!(
        diff_count > 0,
        "terminal glyph vanished at fractional scale (diff_count={diff_count})"
    );
}

#[test]
pub(crate) fn renderer_grows_atlas_pages_when_text_atlas_fills_mid_frame() {
    let mut scene = Scene::new();
    let text: String = (33u8..=126).map(char::from).collect();
    scene.push(SceneCommand::DrawText(TextRun {
        rect: Rect::new(4.0, 6.0, 1800.0, 32.0),
        text,
        style: TextStyle {
            font_size: 18.0,
            line_height: 22.0,
            ..TextStyle::new(Color::WHITE)
        },
    }));

    let frame = SceneFrame {
        window_id: WindowId::new(16),
        viewport: Size::new(1800.0, 64.0),
        surface_size: Size::new(1800.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::new();
    let mut text_engine = TextEngine::new().unwrap();
    // Tiny pages with room for several layers: the printable-ASCII run overflows the first
    // page, so the atlas must grow onto additional texture-array layers rather than reset.
    text_engine.atlas = TextAtlasPages::new(96, 96, 4);
    renderer.text_engine = Some(text_engine);

    renderer.render(&frame).unwrap();

    let active_text_engine = renderer
        .text_engine
        .as_ref()
        .expect("renderer keeps the same text engine -- no nuclear reset");
    // The engine was NOT replaced: it still has the small page size we configured (a reset
    // would have rebuilt it at the default page size).
    assert_eq!(active_text_engine.atlas.page_size(), (96, 96));
    // Overflowing one page grew the atlas onto more pages instead of resetting.
    assert!(
        active_text_engine.atlas.page_count() >= 2,
        "atlas should have grown onto multiple pages"
    );
    assert!(active_text_engine.glyph_cache_stats().0 > 32);

    let stats = renderer
        .last_frame_stats(frame.window_id)
        .expect("renderer should record frame stats");
    assert!(stats.text_glyph_instance_count > 0);

    let image = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    assert!(
        image
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > RGBA_CHANNEL_TOLERANCE),
        "frame should render visible text across atlas pages"
    );
}

#[test]
pub(crate) fn multi_page_atlas_is_stable_across_frames() {
    // Rendering the same overflowing scene twice must not re-rasterize glyphs or churn pages.
    // This is the property that replaces the old "atlas full -> full reset every frame"
    // stutter: once warm, repeat frames are pure cache hits.
    let text: String = (33u8..=126).map(char::from).collect();
    let make_frame = || {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 1800.0, 32.0),
            text: text.clone(),
            style: TextStyle {
                font_size: 18.0,
                line_height: 22.0,
                ..TextStyle::new(Color::WHITE)
            },
        }));
        SceneFrame {
            window_id: WindowId::new(17),
            viewport: Size::new(1800.0, 64.0),
            surface_size: Size::new(1800.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::new();
    let mut text_engine = TextEngine::new().unwrap();
    text_engine.atlas = TextAtlasPages::new(96, 96, 4);
    renderer.text_engine = Some(text_engine);

    renderer.render(&make_frame()).unwrap();
    let (entries_after_first, pages_after_first) = {
        let text_engine = renderer.text_engine.as_ref().unwrap();
        (
            text_engine.glyph_cache_stats().0,
            text_engine.atlas.page_count(),
        )
    };
    assert!(pages_after_first >= 2, "glyphs should span multiple pages");

    renderer.render(&make_frame()).unwrap();
    let text_engine = renderer.text_engine.as_ref().unwrap();
    assert_eq!(
        text_engine.atlas.page_count(),
        pages_after_first,
        "page count must be stable across frames (no growth/thrash)"
    );
    assert_eq!(
        text_engine.atlas.page_size(),
        (96, 96),
        "engine must not have been reset"
    );
    assert_eq!(
        text_engine.glyph_cache_stats().0,
        entries_after_first,
        "no glyphs should be re-rasterized on the second frame"
    );

    let image = renderer.capture_last_frame_rgba(WindowId::new(17)).unwrap();
    assert!(
        image
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > RGBA_CHANNEL_TOLERANCE)
    );
}

#[test]
pub(crate) fn multi_page_atlas_evicts_without_corruption_under_pressure() {
    // A tight 2-page budget with a rotating glyph set forces LRU eviction across frames. The
    // atlas must never exceed its budget, the glyph cache must never dangle past the live
    // pages (coupled entries are dropped on eviction), and rendering must keep working.
    let texts = [
        "the quick brown fox",
        "JUMPS OVER THE LAZY",
        "0123456789 !?#@&*()",
        "Pack my box w/ jugs",
        "Sphinx of black quartz",
    ];

    let mut renderer = WgpuRenderer::new();
    let mut text_engine = TextEngine::new().unwrap();
    text_engine.atlas = TextAtlasPages::new(64, 64, 2);
    renderer.text_engine = Some(text_engine);

    for label in texts {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 600.0, 28.0),
            text: label.to_string(),
            style: TextStyle {
                font_size: 18.0,
                line_height: 22.0,
                ..TextStyle::new(Color::WHITE)
            },
        }));
        let frame = SceneFrame {
            window_id: WindowId::new(21),
            viewport: Size::new(600.0, 40.0),
            surface_size: Size::new(600.0, 40.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        renderer.render(&frame).unwrap();

        let text_engine = renderer.text_engine.as_ref().unwrap();
        let pages = text_engine.atlas.page_count();
        assert!(pages <= 2, "atlas must respect the page budget");
        assert!(
            text_engine
                .glyph_cache
                .values()
                .all(|cached| cached.page_index < pages),
            "every cached glyph must reference a live page after eviction"
        );
    }

    let image = renderer.capture_last_frame_rgba(WindowId::new(21)).unwrap();
    assert!(
        image
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > RGBA_CHANNEL_TOLERANCE)
    );
}

#[test]
pub(crate) fn renderer_samples_persistent_external_texture_updates() {
    let handle = ImageHandle::new(2301);
    let window_id = WindowId::new(2301);
    let viewport = Size::new(16.0, 16.0);
    let registry = WgpuExternalTextureRegistry::new();
    let mut renderer = WgpuRenderer::new().with_external_texture_registry(registry.clone());

    // The first target initialization publishes the renderer-owned device
    // and queue to the shared registry.
    renderer
        .render(&SceneFrame::new(window_id, viewport))
        .unwrap();
    let context = registry.context().expect("renderer context attached");
    let expected_adapter_info = renderer
        .shared
        .as_ref()
        .expect("renderer device initialized")
        .adapter
        .get_info();
    assert_eq!(context.adapter_info(), &expected_adapter_info);

    let direct_handle = ImageHandle::new(2302);
    let direct_texture = context.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("SUI direct external texture test"),
        size: wgpu::Extent3d {
            width: 4,
            height: 3,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let direct_descriptor = registry
        .register_texture(direct_handle, direct_texture)
        .unwrap();
    assert_eq!(
        (direct_descriptor.width(), direct_descriptor.height()),
        (4, 3)
    );
    assert!(registry.unregister(direct_handle));

    let red = Arc::<[u8]>::from(
        [255, 0, 0, 255]
            .into_iter()
            .cycle()
            .take(2 * 2 * 4)
            .collect::<Vec<_>>(),
    );
    let descriptor = registry.upload_rgba8(handle, 2, 2, red, 1).unwrap();
    let first_binding_revision = registry.resolve(handle).unwrap().binding_revision;
    let mut images = ImageRegistry::new();
    images.insert_external(handle, descriptor);

    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::BLACK));
    scene.push(SceneCommand::DrawImage {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        source: ImageSource::new(handle),
    });
    let mut frame = SceneFrame::new(window_id, viewport);
    frame.scene = scene;
    frame.image_registry = Arc::new(images);

    renderer.render(&frame).unwrap();
    let first = renderer.capture_last_frame_rgba(window_id).unwrap();
    assert_rgba_pixel_near(&first, 8, 8, [255, 0, 0, 255], RGBA_CHANNEL_TOLERANCE);

    let green = Arc::<[u8]>::from(
        [0, 255, 0, 255]
            .into_iter()
            .cycle()
            .take(2 * 2 * 4)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        registry.upload_rgba8(handle, 2, 2, green, 2).unwrap(),
        descriptor
    );
    assert_eq!(
        registry.resolve(handle).unwrap().binding_revision,
        first_binding_revision,
        "same-size updates must reuse the persistent GPU allocation"
    );

    // The retained scene is unchanged; rendering again samples the newly
    // uploaded contents through the same image handle and bind group.
    renderer.render(&frame).unwrap();
    let second = renderer.capture_last_frame_rgba(window_id).unwrap();
    assert_rgba_pixel_near(&second, 8, 8, [0, 255, 0, 255], RGBA_CHANNEL_TOLERANCE);
}

/// Diagnostic microbenchmark for text policy cache behavior. It measures CPU-side scene
/// preparation through the real text engine and reports cache behavior across coverage,
/// color, LCD, and unsafe-LCD fallback scenarios. Run with `--ignored --nocapture`.
#[test]
#[ignore = "diagnostic benchmark for text render policy cache and atlas behavior"]
pub(crate) fn text_render_policy_cache_benchmark() {
    let handle = FontHandle::new(7101);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let fonts = Arc::new(fonts);
    let images = Arc::new(ImageRegistry::new());
    let layouts = Arc::new(TextLayoutRegistry::default());
    let viewport = Size::new(640.0, 760.0);

    let frame = |window_id: u64,
                 policy: TextRenderPolicy,
                 color: Color,
                 transform: Option<Transform>|
     -> SceneFrame {
        let mut scene = Scene::new();
        scene.push(SceneCommand::Clear(Color::BLACK));
        scene.push(SceneCommand::PushTextRenderPolicy { policy });
        if let Some(transform) = transform {
            scene.push(SceneCommand::PushTransform { transform });
        }
        for row in 0..44 {
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(18.0, 12.0 + row as f32 * 16.0, 600.0, 17.0),
                text: format!("Atlas policy benchmark row {row:02}: minimum ill glyphs 1234567890"),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 12.0,
                    line_height: 16.0,
                    color,
                    ..TextStyle::default()
                },
            }));
        }
        if transform.is_some() {
            scene.push(SceneCommand::PopTransform);
        }
        scene.push(SceneCommand::PopTextRenderPolicy);
        SceneFrame {
            window_id: WindowId::new(window_id),
            viewport,
            surface_size: viewport,
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::clone(&fonts),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::clone(&layouts),
        }
    };

    let scenarios = [
        (
            "grayscale-perceptual",
            frame(
                7201,
                TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                None,
            ),
        ),
        (
            "coverage-linear",
            frame(
                7202,
                TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Linear),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                None,
            ),
        ),
        (
            "coverage-boost",
            frame(
                7203,
                TextRenderPolicy::new()
                    .with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(0.55)),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                None,
            ),
        ),
        (
            "coverage-gamma",
            frame(
                7204,
                TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Gamma(1.8)),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                None,
            ),
        ),
        (
            "color-churn-light",
            frame(
                7205,
                TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
                Color::rgba(0.88, 0.90, 0.94, 1.0),
                None,
            ),
        ),
        (
            "lcd-rgb",
            frame(
                7206,
                TextRenderPolicy::new()
                    .with_render_mode(sui_scene::TextRenderMode::LcdSubpixel)
                    .with_subpixel_order(TextSubpixelOrder::Rgb)
                    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                None,
            ),
        ),
        (
            "grayscale-rotated",
            frame(
                7207,
                TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                Some(Transform::rotation(0.08)),
            ),
        ),
        (
            "unsafe-lcd-fallback",
            frame(
                7208,
                TextRenderPolicy::new()
                    .with_render_mode(sui_scene::TextRenderMode::LcdSubpixel)
                    .with_subpixel_order(TextSubpixelOrder::Rgb)
                    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
                Color::rgba(0.08, 0.10, 0.14, 1.0),
                Some(Transform::rotation(0.08)),
            ),
        ),
    ];

    let mut text_engine = TextEngine::new().expect("text engine should initialize");
    text_engine.lcd_blending_supported = true;
    println!("\n=== Text Render Policy Cache Benchmark ===");
    for (name, frame) in scenarios {
        let before = text_engine.glyph_cache_stats();
        let first_start = std::time::Instant::now();
        let vertices = build_vertices(&frame, &mut text_engine)
            .expect("benchmark frame should build")
            .len();
        let first_us = first_start.elapsed().as_secs_f64() * 1_000_000.0;
        let after_first = text_engine.glyph_cache_stats();

        let warm_repeats = 12usize;
        let warm_start = std::time::Instant::now();
        for _ in 0..warm_repeats {
            let _ = build_vertices(&frame, &mut text_engine)
                .expect("warm benchmark frame should build");
        }
        let warm_avg_us = warm_start.elapsed().as_secs_f64() * 1_000_000.0 / warm_repeats as f64;
        let after_warm = text_engine.glyph_cache_stats();

        println!(
            "{name:22} first={first_us:8.1}us warm_avg={warm_avg_us:8.1}us vertices={vertices:5} entries={} (+{}) hits=+{} misses=+{} first_misses=+{}",
            after_warm.0,
            after_warm.0.saturating_sub(before.0),
            after_warm.1.saturating_sub(before.1),
            after_warm.2.saturating_sub(before.2),
            after_first.2.saturating_sub(before.2),
        );
    }
    println!("==========================================\n");
}

/// Renders a body-text + small-label sample across DPR, light/dark surfaces, and coverage
/// policies. The validation reports core stem luma plus an edge-coverage ratio instead of raw
/// pixel diffs, because small-text quality changes are perceptual and do not track total
/// changed-pixel percentages well.
#[test]
pub(crate) fn text_coverage_quality_matrix_capture() {
    let handle = FontHandle::new(7001);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let fonts = Arc::new(fonts);

    // Body paragraph + a couple of small UI labels.
    let lines: [(&str, f32, f32); 5] = [
        ("The quick brown fox jumps over the lazy dog.", 20.0, 16.0),
        ("Pack my box with five dozen liquor jugs.", 20.0, 44.0),
        (
            "Body text at a typical reading size, 1234567890.",
            16.0,
            72.0,
        ),
        ("Small UI label", 12.0, 98.0),
        ("settings  ·  profile  ·  sign out", 11.0, 118.0),
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixPolicy {
        Linear,
        Perceptual,
        LcdRgb,
    }

    impl MatrixPolicy {
        fn name(self) -> &'static str {
            match self {
                Self::Linear => "linear",
                Self::Perceptual => "perceptual",
                Self::LcdRgb => "lcd-rgb",
            }
        }

        fn configure(self, renderer: &mut WgpuRenderer) {
            match self {
                Self::Linear => {
                    renderer.set_text_render_mode(TextRenderMode::Grayscale);
                    renderer.set_text_subpixel_order(TextSubpixelOrder::None);
                    renderer.set_text_coverage_policy(TextCoveragePolicy::Linear);
                }
                Self::Perceptual => {
                    renderer.set_text_render_mode(TextRenderMode::Grayscale);
                    renderer.set_text_subpixel_order(TextSubpixelOrder::None);
                    renderer.set_text_coverage_policy(TextCoveragePolicy::Perceptual);
                }
                Self::LcdRgb => {
                    renderer.set_text_render_mode(TextRenderMode::LcdSubpixel);
                    renderer.set_text_subpixel_order(TextSubpixelOrder::Rgb);
                    renderer.set_text_coverage_policy(TextCoveragePolicy::Perceptual);
                }
            }
        }
    }

    let render_sample = |policy: MatrixPolicy, scale: f32, bg: Color, fg: Color| -> RgbaImage {
        let policy_id = match policy {
            MatrixPolicy::Linear => 1,
            MatrixPolicy::Perceptual => 2,
            MatrixPolicy::LcdRgb => 3,
        };
        let window_id = WindowId::new(7100 + policy_id + (scale * 10.0) as u64);
        let viewport = Size::new(420.0, 140.0);
        let mut scene = Scene::new();
        scene.push(SceneCommand::Clear(bg));
        for (text, size, y) in lines {
            scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(12.0, y, 400.0, size + 8.0),
                text: text.to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: size,
                    line_height: size + 6.0,
                    color: fg,
                    ..TextStyle::default()
                },
            }));
        }
        let frame = SceneFrame {
            window_id,
            viewport,
            surface_size: Size::new(viewport.width * scale, viewport.height * scale),
            scale_factor: scale,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::clone(&fonts),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };
        let mut renderer = WgpuRenderer::new();
        policy.configure(&mut renderer);
        renderer
            .render(&frame)
            .expect("headless text render should succeed");
        renderer
            .capture_last_frame_rgba(window_id)
            .expect("capture of rendered text frame should succeed")
    };

    #[derive(Debug, Clone, Copy)]
    struct InkStats {
        mean_luma: f32,
        core_luma: f32,
        foreground_weight: f32,
        edge_ratio: f32,
        inked_pixels: u64,
    }

    fn luma(rgb: [u8; 3]) -> f32 {
        0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32
    }

    fn ink_stats(image: &RgbaImage, bg: [u8; 3], fg: [u8; 3], dark_text: bool) -> InkStats {
        let bg_luma = luma(bg);
        let fg_luma = luma(fg);
        let contrast = (fg_luma - bg_luma).abs().max(1.0);
        let mut lumas: Vec<f32> = Vec::new();
        let mut edge_pixels = 0u64;
        let mut core_pixels = 0u64;
        for px in image.pixels().chunks_exact(4) {
            let d = (px[0] as i32 - bg[0] as i32).abs()
                + (px[1] as i32 - bg[1] as i32).abs()
                + (px[2] as i32 - bg[2] as i32).abs();
            if d > 10 {
                let pixel_luma = luma([px[0], px[1], px[2]]);
                let coverage = ((pixel_luma - bg_luma).abs() / contrast).clamp(0.0, 1.0);
                if coverage >= 0.85 {
                    core_pixels += 1;
                } else if coverage >= 0.10 {
                    edge_pixels += 1;
                }
                lumas.push(pixel_luma);
            }
        }
        let count = lumas.len() as u64;
        if count == 0 {
            return InkStats {
                mean_luma: 0.0,
                core_luma: 0.0,
                foreground_weight: 0.0,
                edge_ratio: 0.0,
                inked_pixels: 0,
            };
        }
        let mean = lumas.iter().sum::<f32>() / count as f32;
        if dark_text {
            lumas.sort_by(|a, b| a.partial_cmp(b).unwrap());
        } else {
            lumas.sort_by(|a, b| b.partial_cmp(a).unwrap());
        }
        let q = (lumas.len() / 4).max(1);
        let core = lumas[..q].iter().sum::<f32>() / q as f32;
        let foreground_weight = if dark_text {
            bg_luma - core
        } else {
            core - bg_luma
        };
        InkStats {
            mean_luma: mean,
            core_luma: core,
            foreground_weight,
            edge_ratio: edge_pixels as f32 / core_pixels.max(1) as f32,
            inked_pixels: count,
        }
    }

    let light_bg = Color::rgba(0.98, 0.98, 0.99, 1.0);
    let dark_bg = Color::rgba(0.08, 0.09, 0.11, 1.0);
    let light_bg8 = [250u8, 250, 252];
    let dark_bg8 = [22u8, 24, 28];
    let light_fg = Color::rgba(0.05, 0.05, 0.06, 1.0);
    let dark_fg = Color::rgba(0.95, 0.95, 0.96, 1.0);
    let light_fg8 = [13u8, 13, 15];
    let dark_fg8 = [242u8, 242, 245];
    let policies = [
        MatrixPolicy::Linear,
        MatrixPolicy::Perceptual,
        MatrixPolicy::LcdRgb,
    ];
    let scales = [1.0f32, 1.5, 2.0];
    let mut write_dir = None;
    if std::env::var_os("SUI_TEXT_COVERAGE_WRITE_PNGS").is_some() {
        let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir.push("../../target/text-coverage-matrix");
        std::fs::create_dir_all(&dir).expect("creating text coverage capture dir should work");
        write_dir = Some(dir);
    }

    for scale in scales {
        let mut linear_light_weight = None;
        let mut perceptual_light_weight = None;
        let mut linear_dark_weight = None;
        let mut perceptual_dark_weight = None;

        for policy in policies {
            let light = render_sample(policy, scale, light_bg, light_fg);
            let dark = render_sample(policy, scale, dark_bg, dark_fg);
            let light_stats = ink_stats(&light, light_bg8, light_fg8, true);
            let dark_stats = ink_stats(&dark, dark_bg8, dark_fg8, false);
            assert!(light_stats.inked_pixels > 200);
            assert!(dark_stats.inked_pixels > 200);
            assert!(light_stats.edge_ratio.is_finite());
            assert!(dark_stats.edge_ratio.is_finite());

            eprintln!(
                "[text-quality {policy} @{scale:.1}x light] mean={:.1} core={:.1} weight={:.1} edge/core={:.2} n={}",
                light_stats.mean_luma,
                light_stats.core_luma,
                light_stats.foreground_weight,
                light_stats.edge_ratio,
                light_stats.inked_pixels,
                policy = policy.name(),
            );
            eprintln!(
                "[text-quality {policy} @{scale:.1}x dark] mean={:.1} core={:.1} weight={:.1} edge/core={:.2} n={}",
                dark_stats.mean_luma,
                dark_stats.core_luma,
                dark_stats.foreground_weight,
                dark_stats.edge_ratio,
                dark_stats.inked_pixels,
                policy = policy.name(),
            );

            match policy {
                MatrixPolicy::Linear => {
                    linear_light_weight = Some(light_stats.foreground_weight);
                    linear_dark_weight = Some(dark_stats.foreground_weight);
                }
                MatrixPolicy::Perceptual => {
                    perceptual_light_weight = Some(light_stats.foreground_weight);
                    perceptual_dark_weight = Some(dark_stats.foreground_weight);
                }
                MatrixPolicy::LcdRgb => {}
            }

            if let Some(dir) = write_dir.as_ref() {
                for (image, surface) in [(&light, "light"), (&dark, "dark")] {
                    let png = encode_png_rgba8(image.width(), image.height(), image.pixels());
                    let mut path = dir.clone();
                    path.push(format!("text-{surface}-{}-{scale:.1}x.png", policy.name()));
                    std::fs::write(&path, &png).expect("writing text capture PNG should succeed");
                }
            }
        }

        assert!(
            perceptual_light_weight.expect("perceptual light stats")
                >= linear_light_weight.expect("linear light stats") * 0.98
        );
        assert!(
            perceptual_dark_weight.expect("perceptual dark stats")
                >= linear_dark_weight.expect("linear dark stats") * 0.98
        );
    }
}
