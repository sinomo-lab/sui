use crate::WgpuRenderer;
use crate::capture::DebugCaptureArtifact;
use crate::capture::HdrRgbaImage;
use crate::capture::encode_hdr_debug_artifact;
use crate::capture::hdr_image_to_sdr_rgba;
use crate::output::ColorManagementMode;
use crate::output::DebugCaptureEncoding;
use crate::output::DebugCaptureRequest;
use crate::output::DebugCaptureStage;
use crate::output::DebugSdrVisualization;
use crate::output::DisplayCapabilities;
use crate::output::DisplayColorPrimaries;
use crate::output::DisplayTransferFunction;
use crate::output::DynamicRangeMode;
use crate::output::OutputStrategy;
use crate::output::RequestedColorManagementMode;
use crate::output::RequestedDynamicRangeMode;
use crate::output::RequestedOutputColorPrimaries;
use crate::output::RequestedToneMappingMode;
use crate::output::apply_output_transform_for_testing;
use crate::output::output_transform_requires_intermediate;
use crate::output::select_output_strategy;
use crate::output::shader_color;
use crate::output::tone_map_linear_color;
use crate::tests::support::{
    RGBA_CHANNEL_TOLERANCE, assert_rgba_channels_near, assert_rgba_pixel_near,
    assert_rgba_pixels_near,
};
use std::sync::Arc;
use sui_core::Color;
use sui_core::Rect;
use sui_core::Size;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_text::FontRegistry;
use sui_text::TextLayoutRegistry;

#[test]
pub(crate) fn debug_capture_stage_helpers_classify_hdr_and_final_outputs() {
    assert!(DebugCaptureStage::HdrIntermediate.is_hdr_capable());
    assert!(DebugCaptureStage::HdrIntermediate.uses_hdr_intermediate());
    assert!(!DebugCaptureStage::FinalComposed.is_hdr_capable());
    assert!(!DebugCaptureStage::FinalComposed.uses_hdr_intermediate());

    assert_eq!(
        DebugCaptureStage::default(),
        DebugCaptureStage::FinalComposed
    );
    assert_eq!(DebugCaptureEncoding::default(), DebugCaptureEncoding::Png);
    assert_eq!(
        DebugSdrVisualization::default(),
        DebugSdrVisualization::ToneMappedColor
    );
    assert_eq!(
        DebugCaptureRequest::default(),
        DebugCaptureRequest {
            stage: DebugCaptureStage::FinalComposed,
            encoding: DebugCaptureEncoding::Png,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        }
    );
}

#[test]
pub(crate) fn hdr_png_capture_normalizes_native_hdr_reference_white() {
    let image = HdrRgbaImage::new(
        3,
        1,
        vec![
            2.5, 2.5, 2.5, 1.0, //
            1.25, 1.25, 1.25, 0.5, //
            5.0, 0.0, 0.0, 1.0,
        ],
    )
    .unwrap();

    let sdr = hdr_image_to_sdr_rgba(
        &image,
        DebugSdrVisualization::ToneMappedColor,
        2.5,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();

    assert_rgba_channels_near(
        &sdr.pixels()[0..4],
        [255, 255, 255, 255],
        RGBA_CHANNEL_TOLERANCE,
    );
    assert!(sdr.pixels()[4] < 255);
    assert!(
        sdr.pixels()[4].abs_diff(sdr.pixels()[5]) <= RGBA_CHANNEL_TOLERANCE,
        "normalized grayscale channels differed by more than {RGBA_CHANNEL_TOLERANCE}: got {} and {}",
        sdr.pixels()[4],
        sdr.pixels()[5]
    );
    assert!(
        sdr.pixels()[7].abs_diff(128) <= RGBA_CHANNEL_TOLERANCE,
        "alpha channel differed by more than {RGBA_CHANNEL_TOLERANCE}: got {}, expected 128",
        sdr.pixels()[7]
    );
    assert_rgba_channels_near(
        &sdr.pixels()[8..12],
        [255, 0, 0, 255],
        RGBA_CHANNEL_TOLERANCE,
    );
}

#[test]
pub(crate) fn hdr_png_capture_preserves_srgb_bytes_after_hdr_scale_and_half_readback() {
    let reference_white = 203.0 / 80.0;
    let mut pixels = Vec::with_capacity(256 * 4);
    let mut expected = Vec::with_capacity(256 * 4);

    for value in 0..=255u8 {
        let encoded = value as f32 / 255.0;
        let linear = shader_color(Color::srgba(encoded, encoded, encoded, 1.0));
        let captured = half::f16::from_f32(linear[0] * reference_white).to_f32();
        pixels.extend_from_slice(&[captured, captured, captured, 1.0]);
        expected.extend_from_slice(&[value, value, value, 255]);
    }

    let image = HdrRgbaImage::new(256, 1, pixels).unwrap();
    let sdr = hdr_image_to_sdr_rgba(
        &image,
        DebugSdrVisualization::ToneMappedColor,
        reference_white,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();

    assert_rgba_pixels_near(sdr.pixels(), expected.as_slice(), RGBA_CHANNEL_TOLERANCE);
}

#[test]
pub(crate) fn sdr_png_capture_transform_preserves_srgb_bytes_regardless_of_sdr_brightness() {
    let strategy = OutputStrategy::SdrSurface {
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
    };

    for value in 0..=255u8 {
        let encoded = value as f32 / 255.0;
        let transformed = apply_output_transform_for_testing(
            shader_color(Color::srgba(encoded, encoded, encoded, 1.0)),
            strategy,
            RequestedToneMappingMode::Clamp,
            10_000.0,
            None,
        );
        let captured = crate::capture::linear_to_srgb_capture_u8(transformed[0]);
        assert_eq!(captured, value, "sRGB byte {value} should round-trip");
    }

    let clipped = apply_output_transform_for_testing(
        [4.0, 2.0, 0.5, 1.0],
        strategy,
        RequestedToneMappingMode::Clamp,
        10_000.0,
        None,
    );
    assert_eq!(crate::capture::linear_to_srgb_capture_u8(clipped[0]), 255);
    assert_eq!(crate::capture::linear_to_srgb_capture_u8(clipped[1]), 255);
    assert_eq!(crate::capture::linear_to_srgb_capture_u8(clipped[2]), 188);
}

#[test]
pub(crate) fn sdr_png_capture_readback_preserves_srgb_bytes_and_clips_hdr() {
    let window_id = WindowId::new(4520);
    let viewport = Size::new(32.0, 16.0);
    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::srgba(0.0, 0.0, 0.0, 1.0)));
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 16.0, 16.0),
        brush: Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0).into(),
    });
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(16.0, 0.0, 16.0, 16.0),
        brush: Color::linear_rgba(4.0, 2.0, 0.5, 1.0).into(),
    });

    let frame = SceneFrame {
        window_id,
        viewport,
        surface_size: viewport,
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::new();
    renderer.render(&frame).unwrap();
    let image = renderer.capture_last_frame_rgba(window_id).unwrap();

    assert_rgba_pixel_near(&image, 8, 8, [66, 42, 213, 255], RGBA_CHANNEL_TOLERANCE);
    assert_rgba_pixel_near(
        &image,
        24,
        8,
        [
            255,
            255,
            crate::capture::linear_to_srgb_capture_u8(0.5),
            255,
        ],
        RGBA_CHANNEL_TOLERANCE,
    );
}

#[test]
pub(crate) fn hdr_png_capture_converts_display_p3_final_output_back_to_srgb() {
    let color = Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0);
    let strategy = OutputStrategy::HdrNativeSurface {
        format: wgpu::TextureFormat::Rgba16Float,
        primaries: DisplayColorPrimaries::DisplayP3,
        transfer: DisplayTransferFunction::LinearExtended,
    };
    let transformed = apply_output_transform_for_testing(
        shader_color(color),
        strategy,
        RequestedToneMappingMode::Automatic,
        203.0,
        None,
    );
    let image = HdrRgbaImage::new(
        1,
        1,
        vec![
            transformed[0],
            transformed[1],
            transformed[2],
            transformed[3],
        ],
    )
    .unwrap();

    let sdr = hdr_image_to_sdr_rgba(
        &image,
        DebugSdrVisualization::ToneMappedColor,
        203.0 / 80.0,
        DisplayColorPrimaries::DisplayP3,
    )
    .unwrap();

    assert_rgba_channels_near(
        &sdr.pixels()[0..4],
        [66, 42, 213, 255],
        RGBA_CHANNEL_TOLERANCE,
    );
}

#[test]
pub(crate) fn hdr_png_capture_visualizations_use_sdr_reference_white() {
    let image = HdrRgbaImage::new(
        2,
        1,
        vec![
            2.0, 2.0, 2.0, 1.0, //
            2.01, 0.0, 0.0, 1.0,
        ],
    )
    .unwrap();

    let mask = hdr_image_to_sdr_rgba(
        &image,
        DebugSdrVisualization::ClipMask,
        2.0,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();
    assert_rgba_channels_near(&mask.pixels()[0..4], [0, 0, 0, 255], RGBA_CHANNEL_TOLERANCE);
    assert_rgba_channels_near(
        &mask.pixels()[4..8],
        [255, 64, 64, 255],
        RGBA_CHANNEL_TOLERANCE,
    );

    let heatmap = hdr_image_to_sdr_rgba(
        &image,
        DebugSdrVisualization::HeadroomHeatmap,
        2.0,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();
    assert!(heatmap.pixels()[4] >= heatmap.pixels()[0]);
    assert!(
        heatmap.pixels()[7].abs_diff(255) <= RGBA_CHANNEL_TOLERANCE,
        "heatmap alpha channel differed by more than {RGBA_CHANNEL_TOLERANCE}: got {}, expected 255",
        heatmap.pixels()[7]
    );
}

#[test]
pub(crate) fn hdr_debug_artifact_encoding_preserves_exr_and_maps_png_to_sdr() {
    let image = HdrRgbaImage::new(1, 1, vec![2.5, 1.25, 0.0, 1.0]).unwrap();

    let png = encode_hdr_debug_artifact(
        image.clone(),
        DebugCaptureRequest {
            stage: DebugCaptureStage::FinalComposed,
            encoding: DebugCaptureEncoding::Png,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        },
        2.5,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();
    let DebugCaptureArtifact::SdrRgba8(png) = png else {
        panic!("PNG HDR debug capture should be converted to SDR RGBA");
    };
    assert!(
        png.pixels()[0].abs_diff(255) <= RGBA_CHANNEL_TOLERANCE,
        "PNG red channel differed by more than {RGBA_CHANNEL_TOLERANCE}: got {}, expected 255",
        png.pixels()[0]
    );
    assert!(png.pixels()[1] < 255);

    let exr = encode_hdr_debug_artifact(
        image,
        DebugCaptureRequest {
            stage: DebugCaptureStage::FinalComposed,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        },
        2.5,
        DisplayColorPrimaries::Srgb,
    )
    .unwrap();
    let DebugCaptureArtifact::HdrLinearRgbaF32(exr) = exr else {
        panic!("EXR HDR debug capture should preserve HDR linear RGBA");
    };
    assert_eq!(exr.pixels()[0], 2.5);
}

#[test]
pub(crate) fn shader_color_preserves_extended_linear_srgb_values_for_hdr_content() {
    let rgba = shader_color(Color::linear_rgba(2.0, 4.0, 8.0, 1.0));

    assert_eq!(rgba, [2.0, 4.0, 8.0, 1.0]);
}

#[test]
pub(crate) fn select_output_strategy_prefers_wide_gamut_when_requested_and_supported() {
    let strategy = select_output_strategy(
        &[wgpu::TextureFormat::Bgra8UnormSrgb],
        DisplayCapabilities {
            supports_wide_gamut: true,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode {
            mode: RequestedColorManagementMode::PreferWideGamut,
            output_primaries: RequestedOutputColorPrimaries::DisplayP3,
            dynamic_range: RequestedDynamicRangeMode::Automatic,
            tone_mapping: RequestedToneMappingMode::Automatic,
            ..ColorManagementMode::default()
        },
    );

    assert_eq!(
        strategy,
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_automatic_uses_wide_gamut_when_supported() {
    let strategy = select_output_strategy(
        &[wgpu::TextureFormat::Bgra8UnormSrgb],
        DisplayCapabilities {
            supports_wide_gamut: true,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode::default(),
    );

    assert_eq!(
        strategy,
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_automatic_uses_native_hdr_when_supported() {
    let strategy = select_output_strategy(
        &[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::Srgb,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: true,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode::default(),
    );

    assert_eq!(
        strategy,
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::Srgb,
            transfer: DisplayTransferFunction::LinearExtended,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_automatic_ignores_float16_without_native_hdr_support() {
    let strategy = select_output_strategy(
        &[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            preferred_dynamic_range: DynamicRangeMode::StandardDynamicRange,
            native_hdr_presentation_supported: false,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode::default(),
    );

    assert_eq!(
        strategy,
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_hdr_support_without_native_uses_sdr_despite_float16() {
    let strategy = select_output_strategy(
        &[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::Srgb,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: false,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode::default(),
    );

    assert_eq!(
        strategy,
        OutputStrategy::SdrSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_automatic_hdr_falls_back_to_sdr_without_native_hdr() {
    let strategy = select_output_strategy(
        &[wgpu::TextureFormat::Bgra8UnormSrgb],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: false,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode::default(),
    );

    assert_eq!(
        strategy,
        OutputStrategy::SdrSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_explicit_sdr_disables_automatic_hdr() {
    let strategy = select_output_strategy(
        &[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: false,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode {
            dynamic_range: RequestedDynamicRangeMode::StandardDynamicRange,
            ..ColorManagementMode::default()
        },
    );

    assert_eq!(
        strategy,
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_uses_sdr_when_hdr_is_requested_without_native_support() {
    let strategy = select_output_strategy(
        &[wgpu::TextureFormat::Bgra8UnormSrgb],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: false,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode {
            mode: RequestedColorManagementMode::PreferHdr,
            output_primaries: RequestedOutputColorPrimaries::DisplayP3,
            dynamic_range: RequestedDynamicRangeMode::HighDynamicRange,
            tone_mapping: RequestedToneMappingMode::Automatic,
            ..ColorManagementMode::default()
        },
    );

    assert_eq!(
        strategy,
        OutputStrategy::SdrSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    );
}

#[test]
pub(crate) fn select_output_strategy_uses_native_hdr_scrgb_surface_when_supported() {
    let strategy = select_output_strategy(
        &[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ],
        DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: true,
            preferred_primaries: DisplayColorPrimaries::Srgb,
            preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
            native_hdr_presentation_supported: true,
            ..DisplayCapabilities::default()
        },
        ColorManagementMode {
            mode: RequestedColorManagementMode::PreferHdr,
            output_primaries: RequestedOutputColorPrimaries::DisplayP3,
            dynamic_range: RequestedDynamicRangeMode::HighDynamicRange,
            tone_mapping: RequestedToneMappingMode::Automatic,
            ..ColorManagementMode::default()
        },
    );

    assert_eq!(
        strategy,
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::Srgb,
            transfer: DisplayTransferFunction::LinearExtended,
        }
    );
}

#[test]
pub(crate) fn hdr_output_transform_requires_intermediate_for_hdr_strategies() {
    assert!(output_transform_requires_intermediate(
        OutputStrategy::HdrIntermediateThenToneMap {
            intermediate_format: wgpu::TextureFormat::Rgba16Float,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    ));
    assert!(output_transform_requires_intermediate(
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::DisplayP3,
            transfer: DisplayTransferFunction::LinearExtended,
        }
    ));
    assert!(!output_transform_requires_intermediate(
        OutputStrategy::SdrSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    ));
}

#[test]
pub(crate) fn wide_gamut_output_transform_runs_even_for_srgb_surface_formats() {
    assert!(output_transform_requires_intermediate(
        OutputStrategy::WideGamutSurface {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        }
    ));
}

#[test]
pub(crate) fn reinhard_tone_mapping_compresses_extended_linear_values() {
    let transformed =
        tone_map_linear_color([4.0, 1.0, 0.5, 1.0], RequestedToneMappingMode::Reinhard);

    assert!(transformed[0] < 1.0);
    assert!(transformed[0] > transformed[1]);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn clamp_tone_mapping_limits_linear_values_to_sdr_range() {
    let transformed = tone_map_linear_color([2.5, 1.25, 0.5, 1.0], RequestedToneMappingMode::Clamp);

    assert_eq!(transformed, [1.0, 1.0, 0.5, 1.0]);
}

#[test]
pub(crate) fn hdr_output_transform_scales_reference_white_to_requested_sdr_content_brightness() {
    let transformed = apply_output_transform_for_testing(
        [1.0, 1.0, 1.0, 1.0],
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::Srgb,
            transfer: DisplayTransferFunction::LinearExtended,
        },
        RequestedToneMappingMode::Automatic,
        203.0,
        None,
    );

    let expected = 203.0 / 80.0;
    assert!((transformed[0] - expected).abs() < 0.0001);
    assert!((transformed[1] - expected).abs() < 0.0001);
    assert!((transformed[2] - expected).abs() < 0.0001);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn native_hdr_output_preserves_requested_reference_white_even_with_manual_tone_mapping_modes()
 {
    let strategy = OutputStrategy::HdrNativeSurface {
        format: wgpu::TextureFormat::Rgba16Float,
        primaries: DisplayColorPrimaries::Srgb,
        transfer: DisplayTransferFunction::LinearExtended,
    };
    let expected = 203.0 / 80.0;

    for mode in [
        RequestedToneMappingMode::Automatic,
        RequestedToneMappingMode::Clamp,
        RequestedToneMappingMode::Reinhard,
    ] {
        let transformed =
            apply_output_transform_for_testing([1.0, 1.0, 1.0, 1.0], strategy, mode, 203.0, None);
        assert!((transformed[0] - expected).abs() < 0.0001, "mode={mode:?}");
        assert!((transformed[1] - expected).abs() < 0.0001, "mode={mode:?}");
        assert!((transformed[2] - expected).abs() < 0.0001, "mode={mode:?}");
        assert_eq!(transformed[3], 1.0, "mode={mode:?}");
    }
}

#[test]
pub(crate) fn native_hdr_output_uses_sc_rgb_reference_white_when_display_sdr_white_is_reported() {
    let strategy = OutputStrategy::HdrNativeSurface {
        format: wgpu::TextureFormat::Rgba16Float,
        primaries: DisplayColorPrimaries::DisplayP3,
        transfer: DisplayTransferFunction::LinearExtended,
    };
    let transformed = apply_output_transform_for_testing(
        [1.0, 1.0, 1.0, 1.0],
        strategy,
        RequestedToneMappingMode::Automatic,
        101.5,
        Some(203.0),
    );

    let expected = 101.5 / 80.0;
    assert!((transformed[0] - expected).abs() < 0.0001);
    assert!((transformed[1] - expected).abs() < 0.0001);
    assert!((transformed[2] - expected).abs() < 0.0001);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn output_transform_maps_linear_srgb_to_display_p3_canvas_primaries() {
    let transformed = apply_output_transform_for_testing(
        [1.0, 0.55, 0.18, 1.0],
        OutputStrategy::HdrNativeSurface {
            format: wgpu::TextureFormat::Rgba16Float,
            primaries: DisplayColorPrimaries::DisplayP3,
            transfer: DisplayTransferFunction::Srgb,
        },
        RequestedToneMappingMode::Automatic,
        80.0,
        None,
    );

    assert!((transformed[0] - 0.920_107_84).abs() < 0.0001);
    assert!((transformed[1] - 0.564_937_35).abs() < 0.0001);
    assert!((transformed[2] - 0.220_794_81).abs() < 0.0001);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn hdr_tone_mapped_output_preserves_sdr_reference_white_by_default() {
    let transformed = apply_output_transform_for_testing(
        [1.0, 1.0, 1.0, 1.0],
        OutputStrategy::HdrIntermediateThenToneMap {
            intermediate_format: wgpu::TextureFormat::Rgba16Float,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::DisplayP3,
        },
        RequestedToneMappingMode::Automatic,
        203.0,
        None,
    );

    assert!((transformed[0] - 1.0).abs() < 0.0001);
    assert!((transformed[1] - 1.0).abs() < 0.0001);
    assert!((transformed[2] - 1.0).abs() < 0.0001);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn hdr_tone_mapped_output_keeps_reinhard_as_explicit_opt_in() {
    let transformed = apply_output_transform_for_testing(
        [1.0, 1.0, 1.0, 1.0],
        OutputStrategy::HdrIntermediateThenToneMap {
            intermediate_format: wgpu::TextureFormat::Rgba16Float,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            primaries: DisplayColorPrimaries::Srgb,
        },
        RequestedToneMappingMode::Reinhard,
        203.0,
        None,
    );

    let expected = 0.5;
    assert!((transformed[0] - expected).abs() < 0.0001);
    assert!((transformed[1] - expected).abs() < 0.0001);
    assert!((transformed[2] - expected).abs() < 0.0001);
    assert_eq!(transformed[3], 1.0);
}

#[test]
pub(crate) fn shader_color_preserves_linear_display_p3_channels_before_gamut_conversion() {
    let encoded = shader_color(Color::display_p3(0.5, 0.25, 0.75, 1.0));
    let linear = shader_color(Color::linear_display_p3(
        0.21404114, 0.05087609, 0.52252156, 1.0,
    ));

    for index in 0..3 {
        assert!((encoded[index] - linear[index]).abs() < 0.0001);
    }
    assert_eq!(linear[3], 1.0);
}

#[test]
pub(crate) fn text_atlas_shader_outputs_premultiplied_alpha() {
    let color = shader_color(Color::srgba(
        66.0 / 255.0,
        42.0 / 255.0,
        213.0 / 255.0,
        0.75,
    ));
    let coverage = 0.5;
    let alpha = color[3] * coverage;
    let premultiplied = [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha];

    assert!((premultiplied[0] - 0.02043).abs() < 0.0001);
    assert!((premultiplied[1] - 0.00868).abs() < 0.0001);
    assert!((premultiplied[2] - 0.24952).abs() < 0.0001);
    assert!((premultiplied[3] - 0.375).abs() < 0.0001);
}

#[test]
pub(crate) fn color_text_atlas_shader_outputs_sampled_premultiplied_alpha() {
    // The atlas now holds sRGB; the shader linearizes the sampled color before premultiplying.
    fn srgb_to_linear(channel: f32) -> f32 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    let sampled_srgb = [66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0];
    let linear = [
        srgb_to_linear(sampled_srgb[0]),
        srgb_to_linear(sampled_srgb[1]),
        srgb_to_linear(sampled_srgb[2]),
    ];
    let sampled_alpha = 0.5;
    let opacity = 0.75;
    let alpha = sampled_alpha * opacity;
    let premultiplied = [
        linear[0] * alpha,
        linear[1] * alpha,
        linear[2] * alpha,
        alpha,
    ];

    assert!((premultiplied[0] - linear[0] * 0.375).abs() < 0.0001);
    assert!((premultiplied[1] - linear[1] * 0.375).abs() < 0.0001);
    assert!((premultiplied[2] - linear[2] * 0.375).abs() < 0.0001);
    assert!((premultiplied[3] - 0.375).abs() < 0.0001);
}
