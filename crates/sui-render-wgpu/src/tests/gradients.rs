//! Linear gradients through more than two stops, on rectangles and rounded
//! rectangles, in every direction.

use crate::WgpuRenderer;
use crate::capture::RgbaImage;
use crate::primitives::{GradientRegion, gradient_band_code, gradient_regions};
use crate::tests::support::{assert_rgba_pixel_near, rgba_pixel};
use std::sync::Arc;
use sui_core::Color;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::WindowId;
use sui_scene::Brush;
use sui_scene::GradientStop;
use sui_scene::ImageRegistry;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_text::FontRegistry;
use sui_text::TextLayoutRegistry;

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::rgba(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);
const TOLERANCE: u8 = 8;

fn stops(stops: &[(f32, Color)]) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|&(offset, color)| GradientStop { offset, color })
        .collect()
}

fn rgb() -> Vec<GradientStop> {
    stops(&[(0.0, RED), (0.5, GREEN), (1.0, BLUE)])
}

fn render(viewport: Size, commands: Vec<SceneCommand>) -> RgbaImage {
    let window_id = WindowId::new(7310);
    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)));
    for command in commands {
        scene.push(command);
    }
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
        .expect("headless gradient render succeeds");
    renderer
        .capture_last_frame_rgba(window_id)
        .expect("the frame can be captured")
}

fn fill(rect: Rect, start: Point, end: Point, stops: Vec<GradientStop>) -> SceneCommand {
    SceneCommand::FillRect {
        rect,
        brush: Brush::LinearGradient { start, end, stops },
    }
}

fn fill_rounded(rect: Rect, start: Point, end: Point, stops: Vec<GradientStop>) -> SceneCommand {
    SceneCommand::FillRoundedRect {
        rect,
        radii: [12.0; 4],
        brush: Brush::LinearGradient { start, end, stops },
        border: None,
        shadow: None,
    }
}

/// Halfway between two full-strength channels. Gradients blend in linear light, so
/// the midpoint encodes brighter than 128.
const HALF: u8 = 188;

// Each gradient spans 301 px so its middle stop falls exactly on a pixel center.

#[test]
fn horizontal_three_stop_gradient_renders_its_middle_stop() {
    let image = render(
        Size::new(320.0, 60.0),
        vec![fill(
            Rect::new(10.0, 10.0, 301.0, 40.0),
            Point::new(10.0, 30.0),
            Point::new(311.0, 30.0),
            rgb(),
        )],
    );
    assert_rgba_pixel_near(&image, 160, 30, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 85, 30, [HALF, HALF, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 235, 30, [0, HALF, HALF, 255], TOLERANCE);
}

#[test]
fn reversed_and_vertical_three_stop_gradients_render_their_middle_stop() {
    let image = render(
        Size::new(200.0, 320.0),
        vec![
            fill(
                Rect::new(10.0, 10.0, 60.0, 301.0),
                Point::new(40.0, 10.0),
                Point::new(40.0, 311.0),
                rgb(),
            ),
            fill(
                Rect::new(100.0, 10.0, 60.0, 301.0),
                Point::new(130.0, 311.0),
                Point::new(130.0, 10.0),
                rgb(),
            ),
        ],
    );
    assert_rgba_pixel_near(&image, 40, 160, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 40, 85, [HALF, HALF, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 130, 160, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 130, 85, [0, HALF, HALF, 255], TOLERANCE);
}

#[test]
fn diagonal_three_stop_gradient_renders_its_middle_stop() {
    let image = render(
        Size::new(240.0, 240.0),
        vec![fill(
            Rect::new(20.0, 20.0, 201.0, 201.0),
            Point::new(20.0, 20.0),
            Point::new(221.0, 221.0),
            rgb(),
        )],
    );
    assert_rgba_pixel_near(&image, 120, 120, [0, 255, 0, 255], TOLERANCE);
    // Along the perpendicular through the middle, the color stays at the middle stop.
    assert_rgba_pixel_near(&image, 60, 180, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 180, 60, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 70, 70, [HALF, HALF, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 170, 170, [0, HALF, HALF, 255], TOLERANCE);
}

#[test]
fn rounded_three_stop_gradients_keep_their_corners() {
    let image = render(
        Size::new(320.0, 200.0),
        vec![
            fill_rounded(
                Rect::new(10.0, 10.0, 301.0, 60.0),
                Point::new(10.0, 40.0),
                Point::new(311.0, 40.0),
                rgb(),
            ),
            fill_rounded(
                Rect::new(10.0, 90.0, 301.0, 101.0),
                Point::new(10.0, 90.0),
                Point::new(311.0, 191.0),
                rgb(),
            ),
        ],
    );
    assert_rgba_pixel_near(&image, 160, 40, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 160, 140, [0, 255, 0, 255], TOLERANCE);
    // The rounded corners stay transparent over the black background.
    assert_rgba_pixel_near(&image, 11, 11, [0, 0, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 309, 11, [0, 0, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 11, 189, [0, 0, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 309, 189, [0, 0, 0, 255], TOLERANCE);
}

/// A translucent color repeated at irregular offsets must paint every pixel once:
/// a gap between regions shows the background, an overlap blends twice.
fn assert_uniform(image: &RgbaImage, area: Rect) {
    let expected = rgba_pixel(image, area.x() as u32, area.y() as u32);
    for y in area.y() as u32..area.max_y() as u32 {
        for x in area.x() as u32..area.max_x() as u32 {
            let pixel = rgba_pixel(image, x, y);
            assert!(
                pixel
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
                "pixel ({x}, {y}) is {pixel:?}, not {expected:?}: a seam between regions"
            );
        }
    }
}

fn translucent_stops() -> Vec<GradientStop> {
    let wash = Color::rgba(1.0, 1.0, 1.0, 0.4);
    stops(&[
        (0.0, wash),
        (0.13, wash),
        (0.29, wash),
        (0.5, wash),
        (0.61, wash),
        (0.83, wash),
        (1.0, wash),
    ])
}

#[test]
fn multi_stop_gradients_have_no_seams_between_stops() {
    let image = render(
        Size::new(320.0, 320.0),
        vec![
            fill(
                Rect::new(7.3, 10.0, 290.6, 40.0),
                Point::new(7.3, 30.0),
                Point::new(297.9, 30.0),
                translucent_stops(),
            ),
            fill(
                Rect::new(20.0, 70.0, 230.0, 230.0),
                Point::new(20.0, 70.0),
                Point::new(250.0, 300.0),
                translucent_stops(),
            ),
        ],
    );
    assert_uniform(&image, Rect::new(9.0, 12.0, 286.0, 36.0));
    assert_uniform(&image, Rect::new(22.0, 72.0, 226.0, 226.0));
    // 40% white over black in linear light.
    assert_rgba_pixel_near(&image, 100, 30, [170, 170, 170, 255], TOLERANCE);
}

#[test]
fn gradients_hold_their_end_colors_and_honor_hard_stops() {
    let image = render(
        Size::new(220.0, 80.0),
        vec![
            fill(
                Rect::new(10.0, 10.0, 200.0, 20.0),
                Point::new(10.0, 20.0),
                Point::new(210.0, 20.0),
                stops(&[(0.25, RED), (0.75, BLUE)]),
            ),
            fill(
                Rect::new(10.0, 40.0, 200.0, 20.0),
                Point::new(10.0, 50.0),
                Point::new(210.0, 50.0),
                stops(&[
                    (0.0, RED),
                    (0.4, GREEN),
                    (0.5, GREEN),
                    (0.5, BLUE),
                    (1.0, BLUE),
                ]),
            ),
        ],
    );
    assert_rgba_pixel_near(&image, 20, 20, [255, 0, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 55, 20, [255, 0, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 165, 20, [0, 0, 255, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 205, 20, [0, 0, 255, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 108, 50, [0, 255, 0, 255], TOLERANCE);
    assert_rgba_pixel_near(&image, 112, 50, [0, 0, 255, 255], TOLERANCE);
}

#[test]
fn gradient_regions_tile_the_axis_with_pads_and_hard_edges() {
    let solid = |color, start, end, at| GradientRegion {
        start,
        end,
        ramp: (at, at),
        from: color,
        to: color,
    };
    assert_eq!(
        gradient_regions(&[]),
        vec![solid(Color::TRANSPARENT, None, None, 0.0)]
    );
    assert_eq!(
        gradient_regions(&stops(&[(0.3, GREEN)])),
        vec![solid(GREEN, None, None, 0.3)]
    );
    assert_eq!(
        gradient_regions(&stops(&[(0.2, RED), (0.8, BLUE)])),
        vec![GradientRegion {
            start: None,
            end: None,
            ramp: (0.2, 0.8),
            from: RED,
            to: BLUE,
        }],
        "one ramp pads both ends by clamping"
    );
    assert_eq!(
        gradient_regions(&rgb()),
        vec![
            GradientRegion {
                start: None,
                end: Some(0.5),
                ramp: (0.0, 0.5),
                from: RED,
                to: GREEN,
            },
            GradientRegion {
                start: Some(0.5),
                end: None,
                ramp: (0.5, 1.0),
                from: GREEN,
                to: BLUE,
            },
        ]
    );
    assert_eq!(
        gradient_regions(&stops(&[(0.5, RED), (0.5, BLUE)])),
        vec![
            solid(RED, None, Some(0.5), 0.5),
            solid(BLUE, Some(0.5), None, 0.5)
        ],
        "stops sharing an offset make a hard edge"
    );
    assert_eq!(
        gradient_regions(&stops(&[(0.0, RED), (0.0, GREEN), (1.0, BLUE), (1.0, RED)])),
        vec![
            solid(RED, None, Some(0.0), 0.0),
            GradientRegion {
                start: Some(0.0),
                end: Some(1.0),
                ramp: (0.0, 1.0),
                from: GREEN,
                to: BLUE,
            },
            solid(RED, Some(1.0), None, 1.0),
        ],
        "hard edges at the ends get pads in the outer colors"
    );
    assert_eq!(
        gradient_regions(&stops(&[(0.6, RED), (0.2, GREEN), (1.4, BLUE)])),
        vec![
            solid(RED, None, Some(0.6), 0.6),
            GradientRegion {
                start: Some(0.6),
                end: None,
                ramp: (0.6, 1.0),
                from: GREEN,
                to: BLUE,
            },
        ],
        "offsets clamp into range and never step backwards"
    );
}

#[test]
fn gradient_band_codes_are_exact_and_share_boundaries() {
    let regions = gradient_regions(&stops(&[
        (0.0, RED),
        (0.123_456, GREEN),
        (0.5, BLUE),
        (0.5, RED),
        (1.0, GREEN),
    ]));
    let decode = |region: &GradientRegion| {
        let code = gradient_band_code(region);
        assert_eq!(code.fract(), 0.0);
        assert!((1.0..=16_777_216.0).contains(&code), "{code}");
        let code = code as u32 - 1;
        (
            code & 0x80_0000 != 0,
            code & 0x40_0000 != 0,
            (code >> 11) & 0x7FF,
            code & 0x7FF,
        )
    };
    let decoded = regions.iter().map(decode).collect::<Vec<_>>();
    assert!(decoded[0].0, "the first region is open at its start");
    assert!(
        decoded.last().unwrap().1,
        "the last region is open at its end"
    );
    for pair in decoded.windows(2) {
        assert_eq!(
            pair[0].3, pair[1].2,
            "neighbors share their boundary exactly"
        );
    }
    let largest = GradientRegion {
        start: None,
        end: None,
        ramp: (1.0, 1.0),
        from: RED,
        to: RED,
    };
    assert_eq!(gradient_band_code(&largest), 16_777_216.0);
}
