use super::support::{assert_rgba_images_match, prepare_with_compositor};
use crate::retained::RetainedCompositorState;
use crate::text::TextAtlasColorMode;
use crate::text_engine::TextEngine;
use crate::{TextCoveragePolicy, WgpuRenderer};
use std::sync::Arc;
use sui_core::{Color, FontHandle, Rect, Size, Transform, Vector, WidgetId, WindowId};
use sui_scene::{
    ImageRegistry, LayerCompositionMode, LayerProperties, Scene, SceneCommand, SceneFrame,
    SceneLayer, SceneLayerDescriptor, SceneLayerId, TextRenderPolicy, TextSubpixelOrder,
};
use sui_text::{FontRegistry, RegisteredFont, TextLayoutRegistry, TextRun, TextStyle};

fn frame(
    lcd: bool,
    foreground: Color,
    background: Option<Color>,
    transform: Transform,
    opacity: f32,
    effect: bool,
) -> SceneFrame {
    let handle = FontHandle::new(9910);
    let mut fonts = FontRegistry::new();
    fonts.insert(
        handle,
        RegisteredFont::from_bytes(sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT.to_vec()),
    );
    let mut text = Scene::new();
    text.push(SceneCommand::PushTextRenderPolicy {
        policy: TextRenderPolicy::new()
            .with_render_mode(if lcd {
                sui_scene::TextRenderMode::LcdSubpixel
            } else {
                sui_scene::TextRenderMode::Grayscale
            })
            .with_subpixel_order(TextSubpixelOrder::Rgb),
    });
    text.push(SceneCommand::DrawText(TextRun {
        rect: Rect::new(12.0, 12.0, 180.0, 32.0),
        text: "minimum ill".into(),
        style: TextStyle {
            font: Some(handle),
            font_size: 17.0,
            line_height: 24.0,
            color: foreground,
            ..TextStyle::default()
        },
    }));
    text.push(SceneCommand::PopTextRenderPolicy);
    let owner = WidgetId::new(9910);
    let descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(owner),
        owner,
        Rect::new(4.0, 4.0, 196.0, 56.0),
    )
    .with_properties(LayerProperties::new(opacity, Vector::ZERO))
    .with_composition_mode(if effect {
        LayerCompositionMode::Effect
    } else {
        LayerCompositionMode::Normal
    });
    let mut scene = Scene::new();
    if let Some(background) = background {
        scene.push(SceneCommand::Clear(background));
    }
    scene.push(SceneCommand::PushTransform { transform });
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        descriptor, text,
    )));
    scene.push(SceneCommand::PopTransform);
    SceneFrame {
        window_id: WindowId::new(9910),
        viewport: Size::new(320.0, 128.0),
        surface_size: Size::new(320.0, 128.0),
        scale_factor: 1.0,
        scene,
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        dirty_regions: vec![],
        layer_updates: vec![],
    }
}

#[test]
fn lcd_masks_require_opaque_supported_uniform_composition() {
    let cases = [
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::IDENTITY,
            1.0,
            false,
            true,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::scale(1.25, 1.25),
            1.0,
            false,
            true,
        ),
        (
            false,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::IDENTITY,
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            None,
            Transform::IDENTITY,
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::TRANSPARENT),
            Transform::IDENTITY,
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::rgba(0.0, 0.0, 0.0, 0.5),
            Some(Color::WHITE),
            Transform::IDENTITY,
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::IDENTITY,
            0.5,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::IDENTITY,
            1.0,
            true,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::scale(1.5, 1.0),
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::rotation(0.2),
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::BLACK,
            Some(Color::WHITE),
            Transform::scale(-1.0, 1.0).then(Transform::translation(200.0, 0.0)),
            1.0,
            false,
            false,
        ),
        (
            true,
            Color::linear_rgba(2.0, 1.0, 1.0, 1.0),
            Some(Color::WHITE),
            Transform::IDENTITY,
            1.0,
            false,
            false,
        ),
    ];
    for (support, fg, bg, transform, opacity, effect, expected) in cases {
        let mut engine = TextEngine::new().unwrap();
        engine.lcd_blending_supported = support;
        prepare_with_compositor(
            &frame(true, fg, bg, transform, opacity, effect),
            &mut engine,
            &mut RetainedCompositorState::default(),
        )
        .unwrap();
        assert!(!engine.glyph_cache.is_empty());
        assert!(
            engine
                .glyph_cache
                .iter()
                .filter(|(_, glyph)| !glyph.size.is_empty())
                .all(
                    |(key, _)| (key.atlas_color_mode == TextAtlasColorMode::LcdSubpixel)
                        == expected
                ),
            "unexpected LCD eligibility: support={support}, fg={fg:?}, bg={bg:?}, transform={transform:?}, opacity={opacity}, effect={effect}"
        );
    }
}

#[test]
fn lcd_is_disabled_for_gamut_conversion_and_hdr_output() {
    use crate::output::{
        DisplayColorPrimaries, DisplayTransferFunction, OutputStrategy, output_allows_lcd,
    };
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    assert!(output_allows_lcd(OutputStrategy::SdrSurface { format }));
    assert!(!output_allows_lcd(OutputStrategy::WideGamutSurface {
        format,
        primaries: DisplayColorPrimaries::DisplayP3
    }));
    assert!(!output_allows_lcd(OutputStrategy::HdrNativeSurface {
        format: wgpu::TextureFormat::Rgba16Float,
        primaries: DisplayColorPrimaries::Srgb,
        transfer: DisplayTransferFunction::LinearExtended
    }));
    assert!(!output_allows_lcd(
        OutputStrategy::HdrIntermediateThenToneMap {
            intermediate_format: wgpu::TextureFormat::Rgba16Float,
            surface_format: format,
            primaries: DisplayColorPrimaries::Srgb
        }
    ));
}

#[test]
fn changing_lcd_capability_rebuilds_text_without_resetting_the_atlas() {
    let mut engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let frame = frame(
        true,
        Color::BLACK,
        Some(Color::WHITE),
        Transform::IDENTITY,
        1.0,
        false,
    );
    let mut first_count = 0;
    for supported in [true, false, true] {
        engine.lcd_blending_supported = supported;
        let draw = prepare_with_compositor(&frame, &mut engine, &mut compositor).unwrap();
        assert!(
            draw.text_instances
                .iter()
                .all(|i| (i.coverage_flags[0] != 0) == supported)
        );
        if first_count == 0 {
            first_count = engine.glyph_cache.len();
        } else {
            assert!(
                engine.glyph_cache.len() > first_count,
                "changing support must preserve the previously cached masks"
            );
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn segoe_hint_range_is_resolved_before_raster_size_bucketing() {
    use crate::text::GlyphHintingTarget;
    let path = std::path::PathBuf::from(
        std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("Fonts/segoeui.ttf");
    let bytes = std::fs::read(path).expect("Windows Segoe UI font");
    let handle = FontHandle::new(9940);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, RegisteredFont::from_bytes(bytes));
    let mut frame = SceneFrame::new(WindowId::new(9940), Size::new(120.0, 100.0));
    frame.font_registry = Arc::new(fonts);
    frame.scene.push(SceneCommand::Clear(Color::WHITE));
    for (i, size) in [19.49, 19.51].into_iter().enumerate() {
        frame.scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(10.0, 8.0 + i as f32 * 32.0, 100.0, 28.0),
            text: "H".into(),
            style: TextStyle {
                font: Some(handle),
                font_size: size,
                line_height: 26.0,
                color: Color::BLACK,
                ..TextStyle::default()
            },
        }));
    }
    let mut engine = TextEngine::new().unwrap();
    prepare_with_compositor(&frame, &mut engine, &mut RetainedCompositorState::default()).unwrap();
    let keys: Vec<_> = engine.glyph_cache.keys().collect();
    assert_eq!(
        keys.len(),
        2,
        "different font-directed hint modes must not alias in the atlas"
    );
    assert_eq!(keys[0].scale_bucket, keys[1].scale_bucket);
    assert!(
        keys.iter()
            .any(|k| k.hinting_target == GlyphHintingTarget::Asymmetric)
    );
    assert!(
        keys.iter()
            .any(|k| k.hinting_target == GlyphHintingTarget::Symmetric)
    );
    engine.set_text_hinting(crate::TextHinting::None);
    prepare_with_compositor(&frame, &mut engine, &mut RetainedCompositorState::default()).unwrap();
    assert!(
        engine
            .glyph_cache
            .keys()
            .any(|k| k.hinting_target == GlyphHintingTarget::None)
    );
}

#[test]
fn unsupported_lcd_scope_renders_exact_grayscale_on_colored_background() {
    let mut actual = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    let fg = Color::rgba(0.1, 0.3, 0.8, 1.0);
    let bg = Color::rgba(0.9, 0.8, 0.65, 1.0);
    let empty = SceneFrame::new(WindowId::new(9910), Size::new(320.0, 128.0));
    for renderer in [&mut actual, &mut reference] {
        renderer.render(&empty).unwrap();
        // Exercise the actual single-source pipeline even on capable GPUs.
        renderer
            .shared
            .as_mut()
            .unwrap()
            .dual_source_blending_enabled = false;
    }
    actual
        .render(&frame(true, fg, Some(bg), Transform::IDENTITY, 1.0, false))
        .unwrap();
    reference
        .render(&frame(false, fg, Some(bg), Transform::IDENTITY, 1.0, false))
        .unwrap();
    assert_rgba_images_match(
        &actual.capture_last_frame_rgba(empty.window_id).unwrap(),
        &reference.capture_last_frame_rgba(empty.window_id).unwrap(),
    );
    assert!(
        actual
            .text_engine
            .as_ref()
            .unwrap()
            .glyph_cache
            .keys()
            .all(|k| k.atlas_color_mode == TextAtlasColorMode::Grayscale)
    );
}

#[test]
fn fading_retained_text_rebuilds_as_grayscale_and_restores_lcd() {
    let mut engine = TextEngine::new().unwrap();
    engine.lcd_blending_supported = true;
    let mut compositor = RetainedCompositorState::default();
    let fg = Color::rgba(0.1, 0.4, 0.7, 1.0);
    for opacity in [1.0, 0.5, 1.0] {
        let draw = prepare_with_compositor(
            &frame(
                true,
                fg,
                Some(Color::WHITE),
                Transform::IDENTITY,
                opacity,
                false,
            ),
            &mut engine,
            &mut compositor,
        )
        .unwrap();
        assert!(!draw.text_instances.is_empty());
        assert!(
            draw.text_instances
                .iter()
                .all(|i| (i.coverage_flags[0] != 0) == (opacity == 1.0))
        );
    }
}

#[test]
fn physical_rgb_sampling_covers_the_right_subpixel_first_at_a_left_edge() {
    use swash::zeno::{Mask, PathBuilder};
    let mut path = Vec::new();
    path.move_to((0.5, 0.0))
        .line_to((1.5, 0.0))
        .line_to((1.5, 2.0))
        .line_to((0.5, 2.0))
        .close();
    let (pixels, placement) = Mask::new(path.as_slice())
        .format(crate::text::lcd_bgra_format())
        .size(2, 2)
        .render();
    assert_eq!(placement.width, 2);
    let texel = [pixels[0], pixels[1], pixels[2], pixels[3]];
    let rgb = crate::text_engine::convert_subpixel_texel_for_mode(
        texel,
        crate::TextRenderMode::LcdSubpixel,
        TextSubpixelOrder::Rgb,
        0.0,
    );
    assert!(
        rgb[0] < rgb[1] && rgb[1] < rgb[2],
        "a stem entering the pixel from the right must cover blue more than red: {rgb:?}"
    );
    let bgr = crate::text_engine::convert_subpixel_texel_for_mode(
        texel,
        crate::TextRenderMode::LcdSubpixel,
        TextSubpixelOrder::Bgr,
        0.0,
    );
    assert_eq!([bgr[0], bgr[1], bgr[2]], [rgb[2], rgb[1], rgb[0]]);
}

#[test]
fn lcd_channel_curve_preserves_endpoints_and_differs_from_luminance_only() {
    use crate::text_policy::lcd_text_coverage as alpha;
    assert!((alpha(0.5, 0.0, 1.0) - 0.75).abs() < 1e-5);
    assert!((alpha(0.5, 1.0, 0.0) - 0.5).abs() < 1e-5);
    assert!((alpha(0.5, 0.1, 0.0) - alpha(0.5, 0.8, 0.0)).abs() > 0.05);
    for text in [0.0, 0.1, 0.49, 0.5, 0.51, 0.8, 1.0] {
        for background in [0.0, 0.2, 0.5, 0.8, 1.0, text] {
            assert_eq!(alpha(0.0, text, background), 0.0);
            assert_eq!(alpha(1.0, text, background), 1.0);
            let mut previous = 0.0;
            for i in 0..=255 {
                let a = alpha(i as f32 / 255.0, text, background);
                assert!(a.is_finite() && a >= previous - 1e-5 && (0.0..=1.0).contains(&a));
                previous = a;
            }
        }
    }
}

#[test]
fn lcd_shader_matches_channel_reference_on_colored_surfaces() {
    let mut renderer = WgpuRenderer::new();
    renderer.set_text_coverage_policy(TextCoveragePolicy::Linear);
    let mask_frame = frame(
        true,
        Color::WHITE,
        Some(Color::BLACK),
        Transform::IDENTITY,
        1.0,
        false,
    );
    renderer.render(&mask_frame).unwrap();
    if !renderer
        .shared
        .as_ref()
        .unwrap()
        .dual_source_blending_enabled
    {
        return;
    }
    let mask = renderer
        .capture_last_frame_rgba(mask_frame.window_id)
        .unwrap();
    let decode = |c: f32| Color::rgba(c, c, c, 1.0).to_linear_srgb().red;
    for (fg, bg) in [
        ([22u8, 125, 184], [255u8, 255, 255]),
        ([214, 100, 31], [17, 53, 79]),
        ([125, 211, 252], [18, 22, 31]),
    ] {
        let fg = fg.map(|v| f32::from(v) / 255.0);
        let bg = bg.map(|v| f32::from(v) / 255.0);
        renderer.set_text_coverage_policy(TextCoveragePolicy::Perceptual);
        renderer
            .render(&frame(
                true,
                Color::rgba(fg[0], fg[1], fg[2], 1.0),
                Some(Color::rgba(bg[0], bg[1], bg[2], 1.0)),
                Transform::IDENTITY,
                1.0,
                false,
            ))
            .unwrap();
        let output = renderer
            .capture_last_frame_rgba(mask_frame.window_id)
            .unwrap();
        let mut maximum = 0;
        for (raw, actual) in mask
            .pixels()
            .chunks_exact(4)
            .zip(output.pixels().chunks_exact(4))
        {
            for channel in 0..3 {
                let alpha = crate::text_policy::lcd_text_coverage(
                    decode(f32::from(raw[channel]) / 255.0),
                    fg[channel],
                    bg[channel],
                );
                let expected = crate::text_policy::linear_srgb_to_encoded_unit(
                    decode(fg[channel]) * alpha + decode(bg[channel]) * (1.0 - alpha),
                );
                maximum = maximum.max(actual[channel].abs_diff((expected * 255.0).round() as u8));
            }
        }
        assert!(
            maximum <= 4,
            "GPU per-channel correction differs from the independent linear-mask reference by {maximum}"
        );
    }
}
