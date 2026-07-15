#[cfg(not(target_arch = "wasm32"))]
use std::{env, path::PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use sui::{
    Application, Color, Constraints, Event, FontHandle, MeasureCtx, PaintCtx, Point, Rect,
    SemanticsCtx, SemanticsNode, SemanticsRole, Size, TextStyle, Widget, WindowBuilder,
    WindowEvent, WindowRenderOptions, WindowStemDarkening, WindowTextCoveragePolicy,
    WindowTextHinting, WindowTextSubpixelOrder, set_window_render_options,
};
#[cfg(not(target_arch = "wasm32"))]
use sui_testing::TestApp;

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: f32 = 480.0;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: f32 = 260.0;
#[cfg(not(target_arch = "wasm32"))]
const FONT_BYTES: &[u8] = sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT;

#[cfg(not(target_arch = "wasm32"))]
struct TextSample {
    text: &'static str,
    x: f32,
    y: f32,
    width: f32,
    font_size: f32,
    line_height: f32,
    color: Color,
}

#[cfg(not(target_arch = "wasm32"))]
const SAMPLES: &[TextSample] = &[
    TextSample {
        text: "minimum ill scroll",
        x: 32.0,
        y: 30.0,
        width: 416.0,
        font_size: 11.0,
        line_height: 14.0,
        color: Color::rgba(0.42, 0.49, 0.57, 1.0),
    },
    TextSample {
        text: "Toolbar 12 px glyph atlas",
        x: 32.0,
        y: 64.0,
        width: 416.0,
        font_size: 12.0,
        line_height: 15.0,
        color: Color::rgba(0.10, 0.14, 0.20, 1.0),
    },
    TextSample {
        text: "Status row 13 px / AVWA",
        x: 32.0,
        y: 100.0,
        width: 416.0,
        font_size: 13.0,
        line_height: 17.0,
        color: Color::rgba(0.18, 0.24, 0.32, 1.0),
    },
    TextSample {
        text: "Quick brown text renders in Noto Sans",
        x: 32.0,
        y: 140.0,
        width: 416.0,
        font_size: 14.0,
        line_height: 19.0,
        color: Color::rgba(0.12, 0.16, 0.22, 1.0),
    },
    TextSample {
        text: "Small UI text should not look fuzzy",
        x: 32.0,
        y: 184.0,
        width: 416.0,
        font_size: 16.0,
        line_height: 21.0,
        color: Color::rgba(0.10, 0.14, 0.20, 1.0),
    },
];

#[cfg(not(target_arch = "wasm32"))]
struct TextReferenceSurface {
    font: FontHandle,
}

#[cfg(not(target_arch = "wasm32"))]
impl TextReferenceSurface {
    fn style(&self, sample: &TextSample) -> TextStyle {
        TextStyle {
            font: Some(self.font),
            font_size: sample.font_size,
            line_height: sample.line_height,
            color: sample.color,
            ..TextStyle::default()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Widget for TextReferenceSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        Size::new(WIDTH, HEIGHT)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_rect(ctx.bounds(), Color::WHITE);
        for sample in SAMPLES {
            let rect = Rect::new(
                ctx.bounds().x() + sample.x,
                ctx.bounds().y() + sample.y,
                sample.width,
                sample.line_height,
            );
            ctx.draw_text(rect, sample.text, self.style(sample));
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some("Noto Sans browser comparison text".to_string());
        ctx.push(node);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn output_dir() -> PathBuf {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output"
            && let Some(value) = args.next()
        {
            return value.into();
        }
    }
    PathBuf::from("target/text-rendering-compare")
}

#[cfg(not(target_arch = "wasm32"))]
fn dpi_scale() -> f64 {
    env::var("SUI_TEXT_COMPARE_DPI_SCALE")
        .ok()
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_options() -> WindowRenderOptions {
    let mut options =
        WindowRenderOptions::new(true, 1.0).with_text_hinting(WindowTextHinting::default());

    if let Ok(value) = env::var("SUI_TEXT_COMPARE_HINTING")
        && value.eq_ignore_ascii_case("none")
    {
        options = options.with_text_hinting(WindowTextHinting::None);
    }

    if let Ok(value) = env::var("SUI_TEXT_COMPARE_STEM_DARKENING")
        && let Ok(amount) = value.parse::<f32>()
    {
        let max_ppem = env::var("SUI_TEXT_COMPARE_STEM_DARKENING_MAX_PPEM")
            .ok()
            .and_then(|raw| raw.parse::<f32>().ok())
            .unwrap_or(18.0);
        options = options.with_stem_darkening(WindowStemDarkening::Enabled { max_ppem, amount });
    }

    if let Ok(value) = env::var("SUI_TEXT_COMPARE_COVERAGE") {
        let policy = if value.eq_ignore_ascii_case("perceptual")
            || value.eq_ignore_ascii_case("browser")
            || value.eq_ignore_ascii_case("browser-like")
        {
            Some(WindowTextCoveragePolicy::Perceptual)
        } else if value.eq_ignore_ascii_case("linear") {
            Some(WindowTextCoveragePolicy::Linear)
        } else if value.eq_ignore_ascii_case("two") {
            Some(WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq)
        } else if let Some(gamma) = value
            .strip_prefix("gamma:")
            .and_then(|raw| raw.parse::<f32>().ok())
        {
            Some(WindowTextCoveragePolicy::Gamma(gamma))
        } else {
            value
                .strip_prefix("boost:")
                .and_then(|raw| raw.parse::<f32>().ok())
                .map(WindowTextCoveragePolicy::CoverageBoost)
        };
        if let Some(policy) = policy {
            options = options.with_text_coverage_policy(policy);
        }
    }

    if let Ok(value) = env::var("SUI_TEXT_COMPARE_SUBPIXEL_ORDER") {
        let order = if value.eq_ignore_ascii_case("rgb") {
            Some(WindowTextSubpixelOrder::Rgb)
        } else if value.eq_ignore_ascii_case("bgr") {
            Some(WindowTextSubpixelOrder::Bgr)
        } else if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("off") {
            Some(WindowTextSubpixelOrder::None)
        } else {
            None
        };
        if let Some(order) = order {
            options = options.with_text_subpixel_order(order);
        }
    }

    options
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> sui::Result<()> {
    let output_dir = output_dir();
    std::fs::create_dir_all(&output_dir).map_err(|error| {
        sui::Error::new(format!(
            "failed to create {}: {error}",
            output_dir.display()
        ))
    })?;

    let options = render_options();
    let mut app = Application::new().with_window_render_options(options);
    let font = app.register_font_bytes(FONT_BYTES.to_vec())?;
    let runtime = app
        .window(
            WindowBuilder::new()
                .title("SUI text rendering snapshot")
                .root(TextReferenceSurface { font }),
        )
        .build()?;
    for window_id in runtime.window_ids() {
        set_window_render_options(window_id, options);
    }
    let window = TestApp::from_runtime(runtime)?.main_window()?;
    let scale = dpi_scale();
    if (scale - 1.0).abs() > f64::EPSILON {
        let viewport = Size::new(WIDTH, HEIGHT);
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
                raw_dpi: Some((96.0 * scale) as f32),
                suggested_size: Some(viewport),
            }))?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::Resized(viewport)))?;
        window.run_until_idle()?;
    }
    let screenshot = window.capture_screenshot()?;
    let physical_size = Size::new(
        (WIDTH * scale as f32).round().max(1.0),
        (HEIGHT * scale as f32).round().max(1.0),
    );
    let screenshot = screenshot.crop(Rect::from_origin_size(Point::ZERO, physical_size))?;
    let path = output_dir.join("sui.png");
    screenshot.write_png(&path)?;

    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
