use crate::WgpuRenderer;
use crate::capture::decode_rgba16f_pixels;
use crate::capture::strip_padded_readback_rows;
use crate::diagnostics::RendererFrameStats;
use crate::draw::ClipState;
use crate::draw::DrawOp;
use crate::draw::DrawOpArena;
use crate::draw::DrawOpKind;
use crate::draw::PreparedClipPath;
use crate::draw::PreparedDrawBatch;
use crate::draw::PreparedDrawKind;
use crate::draw::PreparedFrameBatches;
use crate::draw::PreparedPassBatch;
use crate::draw::PreparedVertices;
use crate::draw::ScissorRect;
use crate::geometry::CachedGlyphMesh;
use crate::gpu::Vertex;
use crate::output::shader_color;
use crate::paths::append_cached_path_mesh;
use crate::retained::RetainedCompositorState;
use crate::scene::build_vertices;
use crate::shaders::ANALYTIC_PATH_SHADER_SOURCE;
use crate::submission::COMPACT_VERTEX_SIZE;
use crate::submission::SOLID_VERTEX_SIZE;
use crate::submission::batch_draw_ops;
use crate::tests::support::{
    RGBA_CHANNEL_TOLERANCE, encode_png_rgba8, ink_pixel_count, load_test_font,
    non_white_pixel_count, non_white_row_count, prepare_with_compositor,
    rgba_channels_match_with_tolerance, rgba_pixel,
};
use crate::text_engine::TextEngine;
use crate::text_engine::apply_stem_darkening_to_coverage;
use crate::text_policy::StemDarkening;
use std::sync::Arc;
use sui_core::Color;
use sui_core::FontHandle;
use sui_core::ImageHandle;
use sui_core::Path;
use sui_core::PathBuilder;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::WindowId;
use sui_scene::Border;
use sui_scene::Brush;
use sui_scene::GradientStop;
use sui_scene::ImageRegistry;
use sui_scene::ImageSampling;
use sui_scene::ImageSource;
use sui_scene::RegisteredImage;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_scene::ShadowParams;
use sui_scene::StrokeStyle;
use sui_scene::WidgetShader;
use sui_text::FontRegistry;
use sui_text::TextLayoutRegistry;
use sui_text::TextRun;
use sui_text::TextStyle;

#[test]
pub(crate) fn strip_padded_float_readback_rows_preserves_pixel_order() {
    let mapped = vec![1u8, 2, 3, 4, 9, 9, 9, 9, 5, 6, 7, 8, 8, 8, 8, 8];
    let stripped = strip_padded_readback_rows(&mapped, 4, 8, 2);
    assert_eq!(stripped, vec![1u8, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
pub(crate) fn decode_rgba16f_pixels_converts_half_float_channels_to_f32() {
    let samples = [0.0f32, 1.0, 2.0, 0.5, 4.0, 8.0, 0.25, 1.0];
    let mut raw = Vec::new();
    for sample in samples {
        raw.extend_from_slice(&half::f16::from_f32(sample).to_bits().to_le_bytes());
    }
    let decoded = decode_rgba16f_pixels(&raw);
    assert_eq!(decoded.len(), 8);
    for (actual, expected) in decoded.into_iter().zip(samples) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected {expected}, got {actual}"
        );
    }
}

#[test]
pub(crate) fn renderer_draws_widget_shader_rect_gradient() {
    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShaderRect {
        rect: Rect::new(0.0, 0.0, 96.0, 24.0),
        shader: WidgetShader::ColorPickerHueBar,
    });

    let frame = SceneFrame {
        window_id: WindowId::new(240),
        viewport: Size::new(96.0, 24.0),
        surface_size: Size::new(96.0, 24.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let left = rgba_pixel(&pixels, 4, 12);
    let right = rgba_pixel(&pixels, 72, 12);

    assert!(left[3] > 200);
    assert!(right[3] > 200);
    assert_ne!(left, right);
}

#[test]
pub(crate) fn build_vertices_supports_non_rect_paths() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
    )
    .unwrap();

    assert!(!vertices.is_empty());
    assert!(vertices.len() >= 12);
}

#[test]
pub(crate) fn shader_color_linearizes_srgb_inputs() {
    let rgba = shader_color(Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0));

    assert!((rgba[0] - 0.05448).abs() < 0.0001);
    assert!((rgba[1] - 0.02315).abs() < 0.0001);
    assert!((rgba[2] - 0.66539).abs() < 0.0001);
    assert_eq!(rgba[3], 1.0);
}

#[test]
pub(crate) fn shader_color_converts_display_p3_primaries_into_linear_srgb_working_space() {
    let rgba = shader_color(Color::display_p3(1.0, 0.0, 0.0, 1.0));

    assert!((rgba[0] - 1.22494).abs() < 0.0001);
    assert!((rgba[1] + 0.04205).abs() < 0.0001);
    assert!((rgba[2] + 0.01963).abs() < 0.0001);
    assert_eq!(rgba[3], 1.0);
}

#[test]
pub(crate) fn cached_path_mesh_linearizes_srgb_inputs() {
    let mut mesh = CachedGlyphMesh::default();
    let a = mesh.push_vertex(Point::new(0.0, 0.0), 1.0);
    let b = mesh.push_vertex(Point::new(10.0, 0.0), 1.0);
    let c = mesh.push_vertex(Point::new(0.0, 10.0), 1.0);
    mesh.add_triangle(a, b, c);

    let color = Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0);
    let mut vertices = Vec::new();
    append_cached_path_mesh(&mut vertices, &mesh, color, Size::new(32.0, 32.0));

    assert_eq!(vertices.len(), 3);
    let expected = shader_color(color);
    for vertex in vertices {
        assert!((vertex.color[0] - expected[0]).abs() < 0.0001);
        assert!((vertex.color[1] - expected[1]).abs() < 0.0001);
        assert!((vertex.color[2] - expected[2]).abs() < 0.0001);
        assert_eq!(vertex.color[3], 1.0);
    }
}

#[test]
pub(crate) fn stem_darkening_applies_only_below_threshold() {
    let config = StemDarkening::Enabled {
        max_ppem: 18.0,
        amount: 0.08,
    };
    assert!(config.effective_amount(14.0) > 0.0);
    assert_eq!(config.effective_amount(24.0), 0.0);
}

#[test]
pub(crate) fn stem_darkening_preserves_transparent_and_opaque_endpoints() {
    // A transparent pixel must stay transparent — otherwise every glyph cell's background
    // gets lifted to `amount` opacity, painting a gray box behind each glyph. A fully
    // opaque pixel must stay opaque. This must hold even at a large darkening amount.
    for amount in [0.1, 0.5, 0.6, 1.0] {
        assert_eq!(apply_stem_darkening_to_coverage(0, amount), 0);
        assert_eq!(apply_stem_darkening_to_coverage(255, amount), 255);
    }
}

#[test]
pub(crate) fn analytic_path_fill_shader_uses_symmetric_signed_distance() {
    assert!(ANALYTIC_PATH_SHADER_SOURCE.contains("let signed_distance"));
    assert!(ANALYTIC_PATH_SHADER_SOURCE.contains("0.5 - (signed_distance"));
    let derivative = ANALYTIC_PATH_SHADER_SOURCE
        .find("let scene_dx = dpdx(in.scene_position)")
        .expect("analytic shader should derive AA width from scene position");
    let path_lookup = ANALYTIC_PATH_SHADER_SOURCE
        .find("let path_meta = path_metas[in.path_index]")
        .expect("analytic shader should load path metadata");
    assert!(
        derivative < path_lookup,
        "derivatives must precede path-indexed non-uniform control flow"
    );
    assert!(ANALYTIC_PATH_SHADER_SOURCE.contains("let scene_dy = dpdy(in.scene_position)"));
    assert!(!ANALYTIC_PATH_SHADER_SOURCE.contains("dpdx(min_distance)"));
    assert!(!ANALYTIC_PATH_SHADER_SOURCE.contains("dpdy(min_distance)"));
    assert!(!ANALYTIC_PATH_SHADER_SOURCE.contains("fwidth(point.x)"));
    assert!(!ANALYTIC_PATH_SHADER_SOURCE.contains("1.0 - (min_distance"));
}

#[test]
pub(crate) fn analytic_path_shader_passes_wgsl_uniformity_validation() {
    let module = naga::front::wgsl::parse_str(ANALYTIC_PATH_SHADER_SOURCE)
        .expect("analytic path WGSL should parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("analytic path WGSL should satisfy uniform-control-flow validation");
}

#[test]
pub(crate) fn batch_draw_ops_merges_consecutive_matching_state() {
    let passes = batch_draw_ops(
        &DrawOpArena {
            scene_vertices: vec![
                Vertex::basic(
                    [0.0, 0.0],
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0],
                    [0.0; 4],
                );
                6
            ],
            clip_vertices: Vec::new(),
            text_instances: Vec::new(),
            clip_states: vec![ClipState {
                clip_paths: Vec::new(),
            }],
            draw_ops: vec![
                DrawOp {
                    kind: DrawOpKind::Solid,
                    vertices: PreparedVertices { start: 0, len: 3 },
                    clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                    clip_state_index: 0,
                    image: None,
                },
                DrawOp {
                    kind: DrawOpKind::Solid,
                    vertices: PreparedVertices { start: 3, len: 3 },
                    clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                    clip_state_index: 0,
                    image: None,
                },
            ],
            analytic_paths: std::collections::HashMap::new(),
            next_analytic_path_id: 0,
        },
        Size::new(50.0, 40.0),
        (100, 80),
    );

    assert_eq!(passes.len(), 1);
    assert_eq!(passes[0].draws.len(), 1);
    assert_eq!(passes[0].draws[0].vertices.len, 6);
}

#[test]
pub(crate) fn renderer_frame_stats_count_passes_draws_and_uploaded_vertices() {
    let vertex = Vertex::basic([0.0, 0.0], [1.0, 1.0, 1.0, 1.0], [0.0, 0.0], [0.0; 4]);
    let prepared = PreparedFrameBatches {
        solid_vertices: Vec::new(),
        scene_vertices: vec![vertex.into(); 9],
        analytic_vertices: Vec::new(),
        extended_vertices: Vec::new(),
        clip_vertices: vec![vertex.into(); 6],
        text_instances: Vec::new(),
        passes: vec![
            PreparedPassBatch {
                clip_paths: vec![PreparedClipPath {
                    vertices: PreparedVertices { start: 0, len: 6 },
                }],
                draws: vec![
                    PreparedDrawBatch {
                        kind: PreparedDrawKind::Solid,
                        clip_rect: None,
                        vertices: PreparedVertices { start: 0, len: 3 },
                    },
                    PreparedDrawBatch {
                        kind: PreparedDrawKind::Solid,
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
                clip_paths: Vec::new(),
                draws: vec![PreparedDrawBatch {
                    kind: PreparedDrawKind::Image {
                        handle: ImageHandle::new(1),
                        sampling: ImageSampling::Linear,
                        raster_size: crate::draw::ImageRasterSize {
                            width: 1,
                            height: 1,
                        },
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
    assert_eq!(
        stats.uploaded_vertex_bytes,
        9 * COMPACT_VERTEX_SIZE + 6 * SOLID_VERTEX_SIZE
    );
    assert_eq!(stats.visible_layer_count, 0);
    assert_eq!(stats.retained_state_update_time_us, 0);
}

#[test]
pub(crate) fn analytic_stroke_rect_renders_at_fractional_scale() {
    let frame = SceneFrame {
        window_id: WindowId::new(97),
        viewport: Size::new(220.0, 64.0),
        surface_size: Size::new(330.0, 96.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 220.0, 64.0),
                brush: Color::WHITE.into(),
            });
            scene.push(SceneCommand::StrokeRect {
                rect: Rect::new(12.0, 12.0, 180.0, 32.0),
                brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
                stroke: StrokeStyle::new(1.0),
            });
            scene
        },
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

    let changed_pixels = pixels
        .pixels()
        .chunks_exact(4)
        .filter(|pixel| {
            !rgba_channels_match_with_tolerance(
                pixel,
                &[255, 255, 255, 255],
                RGBA_CHANNEL_TOLERANCE,
            )
        })
        .count();

    assert!(
        changed_pixels > 500,
        "analytic stroke rect disappeared at fractional scale (changed_pixels={changed_pixels})"
    );
}

#[test]
pub(crate) fn analytic_stroke_path_keeps_nominal_line_thickness() {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(24.0, 32.0))
        .line_to(Point::new(196.0, 32.0));

    let frame = SceneFrame {
        window_id: WindowId::new(98),
        viewport: Size::new(220.0, 64.0),
        surface_size: Size::new(220.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 220.0, 64.0),
                brush: Color::WHITE.into(),
            });
            scene.push(SceneCommand::StrokePath {
                path: builder.build(),
                brush: Color::BLACK.into(),
                stroke: StrokeStyle::new(1.0),
            });
            scene
        },
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let visible_rows = non_white_row_count(&pixels, Rect::new(20.0, 26.0, 180.0, 12.0));

    assert!(
        visible_rows <= 3,
        "analytic one-pixel stroke expanded across too many rows (visible_rows={visible_rows})"
    );
}

#[test]
pub(crate) fn round_stroke_path_uses_analytic_antialiasing() {
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(6.0, 25.0))
        .line_to(Point::new(25.0, 6.0));

    let frame = SceneFrame {
        window_id: WindowId::new(9901),
        viewport: Size::new(32.0, 32.0),
        surface_size: Size::new(32.0, 32.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: {
            let mut scene = Scene::new();
            scene.push(SceneCommand::StrokePath {
                path: builder.build(),
                brush: Color::WHITE.into(),
                stroke: StrokeStyle::new(2.0)
                    .with_cap(sui_scene::StrokeCap::Round)
                    .with_join(sui_scene::StrokeJoin::Round),
            });
            scene
        },
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert_eq!(draw_ops.analytic_paths.len(), 1);
    assert_eq!(compositor.path_cache.stats(), (0, 0, 0));
    assert!(
        draw_ops
            .draw_ops
            .iter()
            .any(|draw| matches!(draw.kind, DrawOpKind::AnalyticPath { .. }))
    );

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let partial_coverage = pixels
        .pixels()
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 0 && pixel[3] < 255)
        .count();
    let opaque_coverage = pixels
        .pixels()
        .chunks_exact(4)
        .filter(|pixel| pixel[3] == 255)
        .count();

    assert!(
        partial_coverage > 0,
        "round stroke edges should contain partial alpha coverage"
    );
    assert!(
        opaque_coverage > 0,
        "round stroke center should retain fully covered pixels"
    );
}

#[test]
pub(crate) fn soft_rounded_border_retains_most_ink_when_feathering_is_enabled() {
    let build_frame = || {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 240.0, 72.0),
            brush: Color::WHITE.into(),
        });
        scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(Rect::new(12.0, 16.0, 196.0, 36.0), 8.0),
            brush: Color::rgba(1.0, 1.0, 1.0, 1.0).into(),
        });
        scene.push(SceneCommand::StrokePath {
            path: Path::rounded_rect(Rect::new(12.0, 16.0, 196.0, 36.0), 8.0),
            brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
            stroke: StrokeStyle::new(1.0 / 1.5),
        });

        SceneFrame {
            window_id: WindowId::new(99),
            viewport: Size::new(240.0, 72.0),
            surface_size: Size::new(360.0, 108.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let frame = build_frame();

    let mut feathered = WgpuRenderer::default()
        .with_feathering_enabled(true)
        .with_feather_width(1.0);
    feathered.render(&frame).unwrap();
    let feathered_pixels = feathered.capture_last_frame_rgba(frame.window_id).unwrap();

    let mut hard = WgpuRenderer::default().with_feathering_enabled(false);
    hard.render(&frame).unwrap();
    let hard_pixels = hard.capture_last_frame_rgba(frame.window_id).unwrap();

    let crop = Rect::new(10.0, 14.0, 200.0, 40.0);
    let feathered_ink = ink_pixel_count(&feathered_pixels, crop);
    let hard_ink = ink_pixel_count(&hard_pixels, crop);

    assert!(
        feathered_ink * 5 >= hard_ink * 4,
        "feathered rounded border lost too much ink at fractional scale (feathered_ink={feathered_ink}, hard_ink={hard_ink})"
    );
}

#[test]
pub(crate) fn soft_control_border_and_chevrons_retain_visible_ink_when_feathering_is_enabled() {
    fn line_path(start: Point, end: Point) -> Path {
        let mut builder = PathBuilder::new();
        builder.move_to(start).line_to(end);
        builder.build()
    }

    fn chevron_path(bounds: Rect, direction: f32) -> Path {
        let center = Point::new(
            bounds.x() + (bounds.width() * 0.5),
            bounds.y() + (bounds.height() * 0.5),
        );
        let mut builder = PathBuilder::new();
        if direction.is_sign_positive() {
            builder
                .move_to(Point::new(bounds.x(), bounds.y() + (bounds.height() * 0.3)))
                .line_to(Point::new(
                    center.x,
                    bounds.max_y() - (bounds.height() * 0.3),
                ))
                .line_to(Point::new(
                    bounds.max_x(),
                    bounds.y() + (bounds.height() * 0.3),
                ));
        } else {
            builder
                .move_to(Point::new(
                    bounds.x(),
                    bounds.max_y() - (bounds.height() * 0.3),
                ))
                .line_to(Point::new(center.x, bounds.y() + (bounds.height() * 0.3)))
                .line_to(Point::new(
                    bounds.max_x(),
                    bounds.max_y() - (bounds.height() * 0.3),
                ));
        }
        builder.build()
    }

    let build_frame = || {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 220.0, 72.0),
            brush: Color::WHITE.into(),
        });
        scene.push(SceneCommand::StrokePath {
            path: Path::rounded_rect(Rect::new(10.0, 16.0, 170.0, 40.0), 6.0),
            brush: Color::rgba(0.47, 0.49, 0.53, 1.0).into(),
            stroke: StrokeStyle::new(1.0),
        });
        scene.push(SceneCommand::StrokePath {
            path: line_path(Point::new(154.0, 22.0), Point::new(154.0, 50.0)),
            brush: Color::rgba(0.73, 0.73, 0.75, 1.0).into(),
            stroke: StrokeStyle::new(1.0),
        });
        scene.push(SceneCommand::StrokePath {
            path: chevron_path(Rect::new(156.0, 18.0, 16.0, 14.0), -1.0),
            brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
            stroke: StrokeStyle::new(1.8),
        });
        scene.push(SceneCommand::StrokePath {
            path: chevron_path(Rect::new(156.0, 40.0, 16.0, 14.0), 1.0),
            brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
            stroke: StrokeStyle::new(1.8),
        });
        scene.push(SceneCommand::StrokePath {
            path: Path::rounded_rect(Rect::new(10.0, 16.0, 196.0, 40.0), 6.0),
            brush: Color::rgba(0.73, 0.73, 0.75, 1.0).into(),
            stroke: StrokeStyle::new(1.0),
        });
        scene.push(SceneCommand::StrokePath {
            path: chevron_path(Rect::new(178.0, 27.0, 18.0, 18.0), 1.0),
            brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
            stroke: StrokeStyle::new(1.8),
        });

        SceneFrame {
            window_id: WindowId::new(102),
            viewport: Size::new(220.0, 72.0),
            surface_size: Size::new(220.0, 72.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let frame = build_frame();

    let mut feathered = WgpuRenderer::default()
        .with_feathering_enabled(true)
        .with_feather_width(1.0);
    feathered.render(&frame).unwrap();
    let feathered_pixels = feathered.capture_last_frame_rgba(frame.window_id).unwrap();

    let mut hard = WgpuRenderer::default().with_feathering_enabled(false);
    hard.render(&frame).unwrap();
    let hard_pixels = hard.capture_last_frame_rgba(frame.window_id).unwrap();

    let number_input_crop = Rect::new(8.0, 14.0, 172.0, 44.0);
    let select_crop = Rect::new(8.0, 14.0, 200.0, 44.0);
    let feathered_number_input_ink = ink_pixel_count(&feathered_pixels, number_input_crop);
    let hard_number_input_ink = ink_pixel_count(&hard_pixels, number_input_crop);
    let feathered_select_ink = ink_pixel_count(&feathered_pixels, select_crop);
    let hard_select_ink = ink_pixel_count(&hard_pixels, select_crop);

    assert!(
        feathered_number_input_ink * 3 >= hard_number_input_ink,
        "feathered number-input border or chevrons lost too much ink (feathered={feathered_number_input_ink}, hard={hard_number_input_ink})"
    );
    assert!(
        feathered_select_ink * 3 >= hard_select_ink,
        "feathered select border or chevron lost too much ink (feathered={feathered_select_ink}, hard={hard_select_ink})"
    );
}

#[test]
pub(crate) fn analytic_splitter_rects_stay_at_divider_location() {
    let divider = Rect::new(140.0, 10.0, 12.0, 84.0);
    let handle = Rect::new(144.0, 38.0, 4.0, 28.0);
    let mut scene = Scene::new();
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 220.0, 108.0),
        brush: Color::WHITE.into(),
    });
    scene.push(SceneCommand::FillRect {
        rect: divider,
        brush: Color::rgba(0.94, 0.955, 0.975, 1.0).into(),
    });
    scene.push(SceneCommand::StrokeRect {
        rect: divider,
        brush: Color::rgba(0.58, 0.62, 0.68, 1.0).into(),
        stroke: StrokeStyle::new(1.0),
    });
    scene.push(SceneCommand::FillRect {
        rect: handle,
        brush: Color::rgba(0.58, 0.62, 0.68, 0.9).into(),
    });

    let frame = SceneFrame {
        window_id: WindowId::new(103),
        viewport: Size::new(220.0, 108.0),
        surface_size: Size::new(220.0, 108.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

    let misplaced_left_pixels = non_white_pixel_count(&pixels, Rect::new(0.0, 0.0, 100.0, 108.0));
    let divider_pixels = non_white_pixel_count(&pixels, Rect::new(138.0, 8.0, 18.0, 88.0));

    assert_eq!(
        misplaced_left_pixels, 0,
        "splitter coverage rendered away from the divider"
    );
    assert!(
        divider_pixels > 250,
        "splitter divider did not render enough visible pixels at its expected location (divider_pixels={divider_pixels})"
    );
}

#[test]
pub(crate) fn build_vertices_uses_registered_font_handle() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
    )
    .unwrap();

    assert!(!vertices.is_empty());
}

#[test]
pub(crate) fn build_vertices_errors_for_unregistered_font_handle() {
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
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
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
pub(crate) fn retained_svg_draws_track_physical_size_and_snap_after_dpi_changes() {
    let handle = ImageHandle::new(24);
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" viewBox="0 0 4 4"><path d="M0 0h4v4H0z" fill="#fff"/></svg>"##;
    let mut images = ImageRegistry::new();
    images.insert(handle, RegisteredImage::from_svg(svg).unwrap());
    let images = Arc::new(images);

    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawImage {
        rect: Rect::new(10.2, 5.3, 14.0, 12.0),
        source: ImageSource::new(handle),
    });

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let make_frame = |surface_size| SceneFrame {
        window_id: WindowId::new(8),
        viewport: Size::new(100.0, 100.0),
        surface_size,
        scale_factor: surface_size.width / 100.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene: scene.clone(),
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::clone(&images),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let one_x = prepare_with_compositor(
        &make_frame(Size::new(100.0, 100.0)),
        &mut text_engine,
        &mut compositor,
    )
    .unwrap();
    let DrawOpKind::Image {
        raster_size: one_x_raster,
        ..
    } = one_x.draw_ops[0].kind
    else {
        panic!("expected image draw op");
    };
    assert_eq!((one_x_raster.width, one_x_raster.height), (14, 12));

    let two_x = prepare_with_compositor(
        &make_frame(Size::new(200.0, 200.0)),
        &mut text_engine,
        &mut compositor,
    )
    .unwrap();
    let op = &two_x.draw_ops[0];
    let DrawOpKind::Image { raster_size, .. } = op.kind else {
        panic!("expected image draw op");
    };
    assert_eq!((raster_size.width, raster_size.height), (28, 24));
    assert_eq!(
        op.image.expect("image metadata").bounds,
        Rect::new(10.0, 5.5, 14.0, 12.0)
    );
}

#[test]
pub(crate) fn renderer_feather_width_is_configurable() {
    let mut renderer = WgpuRenderer::new().with_feather_width(2.5);

    assert_eq!(renderer.feather_width(), 2.5);
    assert!(!renderer.feathering_enabled());

    renderer.set_feathering_enabled(true);

    assert!(renderer.feathering_enabled());

    renderer.set_feather_width(-3.0);

    assert_eq!(renderer.feather_width(), 0.0);

    renderer.set_feathering_enabled(false);

    assert!(!renderer.feathering_enabled());
}

#[test]
pub(crate) fn rounded_rect_primitives_render_to_png_capture() {
    // Exercises the new rounded-rect (per-corner radii + border + soft shadow) and the
    // linear-gradient brush end to end: build a scene, render headless, persist a PNG.
    let window_id = WindowId::new(4242);
    let viewport = Size::new(320.0, 240.0);

    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::rgba(0.12, 0.13, 0.16, 1.0)));

    // A shadowed, bordered rounded card inside a larger clip. The clip is wide enough
    // that the soft shadow remains visible around the card.
    scene.push(SceneCommand::PushClip {
        rect: Rect::new(16.0, 16.0, 180.0, 150.0),
    });
    scene.push(SceneCommand::FillRoundedRect {
        rect: Rect::new(40.0, 44.0, 120.0, 84.0),
        radii: [16.0; 4],
        brush: Brush::Solid(Color::rgba(0.20, 0.55, 0.95, 1.0)),
        border: Some(Border {
            width: 3.0,
            color: Color::rgba(0.95, 0.97, 1.0, 1.0),
        }),
        shadow: Some(ShadowParams {
            offset_x: 0.0,
            offset_y: 6.0,
            blur: 8.0,
            spread: 1.0,
            color: Color::rgba(0.0, 0.0, 0.0, 0.55),
        }),
    });
    scene.push(SceneCommand::PopClip);

    // A per-corner-radii rounded rect (sharp tl/br, round tr/bl).
    scene.push(SceneCommand::FillRoundedRect {
        rect: Rect::new(210.0, 30.0, 90.0, 70.0),
        radii: [2.0, 20.0, 2.0, 20.0],
        brush: Brush::Solid(Color::rgba(0.95, 0.45, 0.30, 1.0)),
        border: None,
        shadow: None,
    });

    // A horizontal linear-gradient rounded rect.
    scene.push(SceneCommand::FillRoundedRect {
        rect: Rect::new(40.0, 160.0, 240.0, 56.0),
        radii: [10.0; 4],
        brush: Brush::LinearGradient {
            start: Point::new(40.0, 188.0),
            end: Point::new(280.0, 188.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::rgba(0.10, 0.80, 0.55, 1.0),
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::rgba(0.55, 0.20, 0.85, 1.0),
                },
            ],
        },
        border: None,
        shadow: None,
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
    renderer
        .render(&frame)
        .expect("headless render of rounded-rect primitives should succeed");

    let image = renderer
        .capture_last_frame_rgba(window_id)
        .expect("capture of rendered frame should succeed");

    // The frame must contain visibly painted content (the clear color is opaque, so we
    // additionally check that some pixels differ from the background).
    let bg = [
        (0.12_f32.powf(1.0 / 2.2) * 255.0) as u8,
        (0.13_f32.powf(1.0 / 2.2) * 255.0) as u8,
        (0.16_f32.powf(1.0 / 2.2) * 255.0) as u8,
    ];
    let non_background = image.pixels().chunks_exact(4).any(|pixel| {
        (pixel[0] as i32 - bg[0] as i32).abs()
            + (pixel[1] as i32 - bg[1] as i32).abs()
            + (pixel[2] as i32 - bg[2] as i32).abs()
            > 24
    });
    assert!(
        non_background,
        "rendered frame should contain the painted primitives"
    );

    let png = encode_png_rgba8(image.width(), image.height(), image.pixels());
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target");
    let _ = std::fs::create_dir_all(&path);
    path.push("rounded_rect_primitives_capture.png");
    std::fs::write(&path, &png).expect("writing capture PNG should succeed");
    eprintln!("wrote capture PNG to {}", path.display());
}
