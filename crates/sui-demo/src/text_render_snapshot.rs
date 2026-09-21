#[cfg(not(target_arch = "wasm32"))]
use std::{env, path::PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use sui::{
    Application, Color, Constraints, Event, FontHandle, MeasureCtx, PaintCtx, Point, Rect,
    SemanticsCtx, SemanticsNode, SemanticsRole, Size, TextRenderCoveragePolicy, TextRenderPolicy,
    TextStyle, Widget, WindowBuilder, WindowEvent, WindowRenderOptions, WindowStemDarkening,
    WindowTextCoveragePolicy, WindowTextHinting, WindowTextSubpixelOrder,
    set_window_render_options,
};
#[cfg(not(target_arch = "wasm32"))]
use sui_testing::TestApp;

#[cfg(not(target_arch = "wasm32"))]
const WIDTH: f32 = 480.0;
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT: f32 = 460.0;
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
    dark_color: Color,
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
        dark_color: Color::rgba(166.0 / 255.0, 178.0 / 255.0, 200.0 / 255.0, 1.0),
    },
    TextSample {
        text: "Toolbar 12 px glyph atlas",
        x: 32.0,
        y: 64.0,
        width: 416.0,
        font_size: 12.0,
        line_height: 15.0,
        color: Color::rgba(0.10, 0.14, 0.20, 1.0),
        dark_color: Color::rgba(0.92, 0.94, 0.98, 1.0),
    },
    TextSample {
        text: "Status row 13 px / AVWA",
        x: 32.0,
        y: 100.0,
        width: 416.0,
        font_size: 13.0,
        line_height: 17.0,
        color: Color::rgba(0.18, 0.24, 0.32, 1.0),
        dark_color: Color::rgba(0.75, 0.80, 0.88, 1.0),
    },
    TextSample {
        text: "Quick brown text renders in Noto Sans",
        x: 32.0,
        y: 140.0,
        width: 416.0,
        font_size: 14.0,
        line_height: 19.0,
        color: Color::rgba(0.12, 0.16, 0.22, 1.0),
        dark_color: Color::rgba(0.92, 0.94, 0.98, 1.0),
    },
    TextSample {
        text: "Small UI text should not look fuzzy",
        x: 32.0,
        y: 184.0,
        width: 416.0,
        font_size: 16.0,
        line_height: 21.0,
        color: Color::rgba(0.10, 0.14, 0.20, 1.0),
        dark_color: Color::rgba(0.92, 0.94, 0.98, 1.0),
    },
    TextSample {
        text: "Accent blue / Settings 012345",
        x: 32.0,
        y: 224.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::rgba(0.031, 0.486, 0.643, 1.0),
        dark_color: Color::rgba(125.0 / 255.0, 211.0 / 255.0, 252.0 / 255.0, 1.0),
    },
    TextSample {
        text: "Success green / minimum 012345",
        x: 32.0,
        y: 264.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::rgba(0.086, 0.639, 0.290, 1.0),
        dark_color: Color::rgba(74.0 / 255.0, 222.0 / 255.0, 128.0 / 255.0, 1.0),
    },
    TextSample {
        text: "Error red / minimum 012345",
        x: 32.0,
        y: 304.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::rgba(0.863, 0.149, 0.149, 1.0),
        dark_color: Color::rgba(248.0 / 255.0, 113.0 / 255.0, 113.0 / 255.0, 1.0),
    },
    TextSample {
        text: "Purple labels / AVWA minimum",
        x: 32.0,
        y: 344.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::rgba(0.576, 0.200, 0.918, 1.0),
        dark_color: Color::rgba(192.0 / 255.0, 132.0 / 255.0, 252.0 / 255.0, 1.0),
    },
    TextSample {
        text: "Warning orange / minimum 012345",
        x: 32.0,
        y: 384.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::rgba(154.0 / 255.0, 103.0 / 255.0, 0.0, 1.0),
        dark_color: Color::rgba(251.0 / 255.0, 146.0 / 255.0, 60.0 / 255.0, 1.0),
    },
    TextSample {
        text: "RGB edge probe | minimum ill",
        x: 32.0,
        y: 424.0,
        width: 416.0,
        font_size: 15.0,
        line_height: 21.0,
        color: Color::BLACK,
        dark_color: Color::WHITE,
    },
];

#[cfg(not(target_arch = "wasm32"))]
struct TextReferenceSurface {
    font: FontHandle,
    dark: bool,
    legacy_coverage: bool,
    render_mode: sui_scene::TextRenderMode,
    subpixel_order: sui_scene::TextSubpixelOrder,
}

#[cfg(not(target_arch = "wasm32"))]
impl TextReferenceSurface {
    fn style(&self, sample: &TextSample) -> TextStyle {
        TextStyle {
            font: Some(self.font),
            font_size: sample.font_size,
            line_height: sample.line_height,
            color: if self.dark {
                sample.dark_color
            } else {
                sample.color
            },
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
        ctx.fill_rect(
            ctx.bounds(),
            if self.dark {
                Color::rgba(18.0 / 255.0, 22.0 / 255.0, 31.0 / 255.0, 1.0)
            } else {
                Color::WHITE
            },
        );
        for sample in SAMPLES {
            let rect = Rect::new(
                ctx.bounds().x() + sample.x,
                ctx.bounds().y() + sample.y,
                sample.width,
                sample.line_height,
            );
            let style = self.style(sample);
            let mut policy = TextRenderPolicy::new()
                .with_render_mode(self.render_mode)
                .with_subpixel_order(self.subpixel_order);
            if self.legacy_coverage {
                let color = style.color;
                let luminance = 0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue;
                policy = policy.with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(
                    (1.0 - luminance).clamp(0.45, 0.92),
                ));
            }
            ctx.push_text_render_policy(policy);
            ctx.draw_text(rect, sample.text, style);
            ctx.pop_text_render_policy();
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
    let font_bytes = match env::var_os("SUI_TEXT_COMPARE_FONT") {
        Some(path) => std::fs::read(path)
            .map_err(|error| sui::Error::new(format!("reading comparison font: {error}")))?,
        None => FONT_BYTES.to_vec(),
    };
    let font = app.register_font_bytes(font_bytes)?;
    let dark = env::var("SUI_TEXT_COMPARE_SURFACE").is_ok_and(|v| v == "dark");
    let mode_name = env::var("SUI_TEXT_COMPARE_MODE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let render_mode = match mode_name.as_str() {
        "lcd" => sui_scene::TextRenderMode::LcdSubpixel,
        _ => sui_scene::TextRenderMode::Grayscale,
    };
    let order_name = env::var("SUI_TEXT_COMPARE_SUBPIXEL_ORDER")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let subpixel_order = match order_name.as_str() {
        "bgr" => sui_scene::TextSubpixelOrder::Bgr,
        "none" | "off" => sui_scene::TextSubpixelOrder::None,
        "rgb" => sui_scene::TextSubpixelOrder::Rgb,
        "" if render_mode == sui_scene::TextRenderMode::LcdSubpixel => {
            sui_scene::TextSubpixelOrder::Rgb
        }
        _ => sui_scene::TextSubpixelOrder::None,
    };
    let legacy_coverage =
        env::var("SUI_TEXT_COMPARE_COVERAGE").is_ok_and(|v| v == "legacy-perceptual");
    // Share the exact static ASCII corpus and unrounded channels with Chrome.
    let rows = SAMPLES.iter().map(|s| {
        let c = if dark { s.dark_color } else { s.color };
        assert!(s.text.is_ascii());
        format!(r#"{{"text":{:?},"x":{},"y":{},"width":{},"fontSize":{},"lineHeight":{},"color":[{},{},{},{}]}}"#,
            s.text,s.x,s.y,s.width,s.font_size,s.line_height,c.red*255.0,c.green*255.0,c.blue*255.0,c.alpha)
    }).collect::<Vec<_>>().join(",\n");
    let background = if dark { "[18,22,31]" } else { "[255,255,255]" };
    std::fs::write(
        output_dir.join("samples.json"),
        format!(
            r#"{{"width":{WIDTH},"height":{HEIGHT},"background":{background},"samples":[{rows}]}}"#
        ),
    )
    .map_err(|error| sui::Error::new(format!("writing sample manifest: {error}")))?;
    let runtime = app
        .window(
            WindowBuilder::new()
                .title("SUI text rendering snapshot")
                .root(TextReferenceSurface {
                    font,
                    dark,
                    legacy_coverage,
                    render_mode,
                    subpixel_order,
                }),
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
