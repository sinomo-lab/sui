//! Native CPU text workloads. See docs/text-layout-performance.md for methodology.
use std::{hint::black_box, time::Instant};

use sui_core::{Color, FontHandle, Size};
use sui_text::{
    BUNDLED_NOTO_SANS_REGULAR_FONT, FontRegistry, RegisteredFont, TextDocument, TextLayoutRequest,
    TextStyle, TextSystem,
};

const SAMPLES: usize = 7;
const STEPS: usize = 8;

fn measure_size(system: &TextSystem, request: TextLayoutRequest, fonts: &FontRegistry) -> Size {
    system.measure_document_size(request, fonts).unwrap()
}

fn label_passes(
    system: &TextSystem,
    text: &str,
    width: f32,
    style: &TextStyle,
    fonts: &FontRegistry,
) {
    // The wrapped Label::measure request sequence; geometry uses the measured height.
    black_box(
        system
            .measure_text_size(text.to_owned(), style.clone(), fonts)
            .unwrap(),
    );
    let request = TextLayoutRequest::new(TextDocument::from_plain_text(
        text.to_owned(),
        style.clone(),
    ))
    .with_box_size(Size::new(width, 1.0));
    let measured = measure_size(system, request, fonts);
    let layout = system
        .shape_text_persistent(
            None,
            text.to_owned(),
            Size::new(width, measured.height),
            style.clone(),
            fonts,
        )
        .unwrap();
    black_box(layout.measurement());
    // Keep registry lifetime bounded, as the runtime does after a frame.
    system.retain_persistent_layouts(&Default::default());
}

fn run(name: &str, texts: &[String], style: &TextStyle, fonts: &FontRegistry) {
    let system = TextSystem::new();
    // Exclude font discovery/context creation, and prime the unchanged-width control.
    for text in texts {
        black_box(
            system
                .shape_text(text.clone(), Size::new(320.0, 1.0), style.clone(), fonts)
                .unwrap(),
        );
    }
    let before = system.layout_cache_snapshot();
    let mut samples = Vec::new();
    let steps = if name == "fresh_text" { 1 } else { STEPS };
    for sample in 0..SAMPLES {
        let started = Instant::now();
        for step in 0..steps {
            // No repeated widths: full-layout cache hits must not hide reflow work.
            let width = 260.125 + (sample * steps + step) as f32 * 2.375;
            for (index, text) in texts.iter().enumerate() {
                match name {
                    "same_width" => {
                        black_box(
                            system
                                .shape_text(
                                    text.clone(),
                                    Size::new(320.0, 1.0),
                                    style.clone(),
                                    fonts,
                                )
                                .unwrap(),
                        );
                    }
                    "fresh_text" => {
                        let text = format!("fresh-{sample}-{index}: {text}");
                        black_box(
                            system
                                .shape_text(text, Size::new(width, 1.0), style.clone(), fonts)
                                .unwrap(),
                        );
                    }
                    "size_only_resize" => {
                        let request = TextLayoutRequest::new(TextDocument::from_plain_text(
                            text.clone(),
                            style.clone(),
                        ))
                        .with_box_size(Size::new(width, 1.0));
                        black_box(measure_size(&system, request, fonts));
                    }
                    "wrapped_label_passes" => label_passes(&system, text, width, style, fonts),
                    _ => {
                        black_box(
                            system
                                .shape_text(
                                    text.clone(),
                                    Size::new(width, 1.0),
                                    style.clone(),
                                    fonts,
                                )
                                .unwrap(),
                        );
                    }
                }
            }
        }
        samples.push(started.elapsed().as_secs_f64() * 1e6 / (steps * texts.len()) as f64);
    }
    let after = system.layout_cache_snapshot();
    let raw = samples
        .iter()
        .map(|value| format!("{value:.3}"))
        .collect::<Vec<_>>()
        .join(";");
    samples.sort_by(f64::total_cmp);
    println!(
        "{name},{},{},{:.3},{:.3},{},{},{}",
        texts.len(),
        steps,
        samples[SAMPLES / 2],
        samples[SAMPLES - 1],
        after.hits - before.hits,
        after.misses - before.misses,
        raw
    );
}

fn main() {
    let mut fonts = FontRegistry::new();
    let handle = FontHandle::new(1);
    fonts.insert(
        handle,
        RegisteredFont::from_bytes(BUNDLED_NOTO_SANS_REGULAR_FONT),
    );
    let style = TextStyle {
        font: Some(handle),
        font_size: 16.0,
        line_height: 20.0,
        ..TextStyle::new(Color::BLACK)
    };
    let short: Vec<_> = (0..200).map(|index| format!(
        "Message {index}: Retaining shaped paragraphs lets the interface change its width without repeating font selection, Unicode analysis, and glyph shaping."
    )).collect();
    let long: Vec<_> = [
        "Latin office affinity AVATAR and e\u{301} combining characters. ",
        "مرحبا بالعالم English 123 مع العربية وتغيير عرض النص. ",
        "中文排版と日本語の文章。改行幅を変更します。 ",
        "English אבגדה العربية 123 👨‍👩‍👧‍👦 👩🏽‍💻 ไทย. ",
    ]
    .into_iter()
    .map(|text| text.repeat(30))
    .collect();
    println!(
        "workload,texts,steps,median_us_per_text,max_sample_us_per_text,layout_hits,layout_misses,samples_us_per_text"
    );
    for name in [
        "same_width",
        "fresh_text",
        "resize",
        "size_only_resize",
        "wrapped_label_passes",
    ] {
        run(name, &short, &style, &fonts);
    }
    run("multilingual_resize", &long, &style, &fonts);
}
