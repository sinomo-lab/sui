use super::*;
use crate::{
    FontFeature, FontWeight, RegisteredFont, TextAlign, TextDirection, TextParagraph, TextSpan,
    TextWrap,
    prepared::{ByteCache, PreparationCaches},
};
use sui_core::{Color, FontHandle, Point};

fn request(document: &TextDocument, width: f32) -> TextLayoutRequest {
    TextLayoutRequest::new(document.clone()).with_box_size(Size::new(width, 300.0))
}

fn discard_final_layouts(system: &TextSystem) {
    *system.layout_cache.lock().unwrap() = TextLayoutCache::default();
}

fn discard_preparation(system: &TextSystem) {
    system
        .font_context
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .context
        .preparation = PreparationCaches::default();
}

#[test]
fn size_only_requests_remain_visible_to_the_runtime_profiler() {
    if !text_timing_enabled() {
        return;
    }
    let system = TextSystem::with_bundled_fonts();
    begin_text_timing_collection();
    system
        .measure_text_size("profiled", TextStyle::default(), &FontRegistry::new())
        .unwrap();
    let timing = take_text_timing_collection();
    assert_eq!(timing.request_count, 1);
    assert_eq!(timing.size_only_request_count, 1);
    assert_eq!(timing.cache_hit_count + timing.cache_miss_count, 0);
    assert_eq!(timing.total_time_us, timing.size_only_time_us);
}

fn assert_geometry(left: &TextLayout, right: &TextLayout) {
    assert_eq!(left.metadata(), right.metadata());
    assert_eq!(left.measurement(), right.measurement());
    assert_eq!(left.paragraphs(), right.paragraphs());
    assert_eq!(left.lines(), right.lines());
    assert_eq!(left.runs(), right.runs());
    assert_eq!(left.clusters(), right.clusters());
    assert_eq!(left.glyphs(), right.glyphs());
    for offset in left
        .text()
        .char_indices()
        .map(|(offset, _)| offset)
        .chain([left.text().len()])
    {
        assert_eq!(left.caret_rect(offset), right.caret_rect(offset));
        assert_eq!(
            left.selection_rects(0..offset),
            right.selection_rects(0..offset)
        );
        let caret = left.caret_rect(offset);
        let point = Point::new(caret.x(), caret.y() + caret.height() * 0.5);
        assert_eq!(left.hit_test_point(point), right.hit_test_point(point));
    }
}

#[test]
fn resizing_reuses_shaping_and_intrinsic_metrics_without_changing_published_geometry() {
    let system = TextSystem::with_bundled_fonts();
    let fonts = FontRegistry::new();
    let document = TextDocument::from_plain_text(
        "office affinity e\u{301} العربية שָׁלוֹם 123",
        TextStyle::default(),
    );
    system
        .measure_document_size(request(&document, 320.0), &fonts)
        .unwrap();
    let prepared = system.preparation_cache_snapshot();
    assert_eq!(prepared.paragraphs.misses, 1);
    assert_eq!(
        prepared.glyphs.entries, 0,
        "size-only must not compute glyph bounds"
    );
    assert_eq!(
        system.layout_cache_snapshot().entries,
        0,
        "size-only must not build final layouts"
    );
    let first = system
        .layout_document_persistent(None, request(&document, 190.0), &fonts)
        .unwrap();
    let first_stats = system.preparation_cache_snapshot();
    let second = system
        .layout_document_persistent(Some(first.handle()), request(&document, 95.0), &fonts)
        .unwrap();
    let second_stats = system.preparation_cache_snapshot();
    assert_eq!(second_stats.paragraphs.misses, 1);
    assert_eq!(second_stats.glyphs.misses, first_stats.glyphs.misses);
    assert!(second_stats.paragraphs.hits > first_stats.paragraphs.hits);
    assert_eq!(first.handle(), second.handle());
    assert_ne!(first.version(), second.version());
    discard_final_layouts(&system);
    discard_preparation(&system);
    let fresh = system
        .layout_document(request(&document, 95.0), &fonts)
        .unwrap();
    assert_geometry(second.layout(), &fresh);
    assert_eq!(
        system.text_layout_registry().get(second.handle()),
        Some(second.layout())
    );
}

#[test]
fn size_and_materialized_reflow_agree_across_styles_breaks_bidi_and_empty_text() {
    let system = TextSystem::with_bundled_fonts();
    let fonts = FontRegistry::new();
    for text in [
        "",
        "a\n\nlast\n",
        "office e\u{301}\tword\u{a0}glue",
        "אבגדה العربية 123 office",
        "👨‍👩‍👧‍👦 👩🏽‍💻 中文日本語 ไทย",
    ] {
        for wrap in [TextWrap::NoWrap, TextWrap::Word, TextWrap::Character] {
            for direction in [
                TextDirection::Auto,
                TextDirection::LeftToRight,
                TextDirection::RightToLeft,
            ] {
                for align in [TextAlign::Start, TextAlign::Center, TextAlign::Justified] {
                    let mut body = TextStyle {
                        font_size: 15.25,
                        line_height: 19.125,
                        ..TextStyle::default()
                    };
                    body.features.disable(FontFeature::STANDARD_LIGATURES);
                    let mut emphasis = body.clone();
                    emphasis.font_size = 23.75;
                    emphasis.line_height = 28.625;
                    emphasis.weight = FontWeight::BOLD;
                    let mut document = TextDocument::from_plain_text(text, body);
                    for paragraph in &mut document.paragraphs {
                        paragraph.style.wrap = wrap;
                        paragraph.style.direction = direction;
                        paragraph.style.align = align;
                        paragraph
                            .spans
                            .push(TextSpan::new(" tail", emphasis.clone()));
                    }
                    for width in [0.0, 73.25, 320.0] {
                        let size = system
                            .measure_document_size(request(&document, width), &fonts)
                            .unwrap();
                        let layout = system
                            .layout_document(request(&document, width), &fonts)
                            .unwrap();
                        assert_eq!(
                            size,
                            Size::new(layout.measurement().width, layout.measurement().height),
                            "text={text:?}, wrap={wrap:?}, direction={direction:?}, align={align:?}, width={width}"
                        );
                        discard_final_layouts(&system);
                    }
                }
            }
        }
    }
    for document in [
        TextDocument::new(),
        TextDocument::from_plain_text("", TextStyle::default()),
    ] {
        let size = system
            .measure_document_size(TextLayoutRequest::new(document.clone()), &fonts)
            .unwrap();
        let layout = system
            .layout_document(TextLayoutRequest::new(document), &fonts)
            .unwrap();
        assert_eq!(
            size,
            Size::new(layout.measurement().width, layout.measurement().height)
        );
    }
}

#[test]
fn paragraph_reuse_remaps_local_span_metadata_after_document_edits() {
    let system = TextSystem::with_bundled_fonts();
    let fonts = FontRegistry::new();
    let paragraph = TextParagraph::from_spans(vec![
        TextSpan::new("one ", TextStyle::default()),
        TextSpan::new(
            "two",
            TextStyle {
                font_size: 27.0,
                ..TextStyle::default()
            },
        ),
    ]);
    let initial = TextDocument {
        paragraphs: vec![paragraph.clone()],
    };
    system
        .layout_document(request(&initial, 180.0), &fonts)
        .unwrap();
    let edited = TextDocument {
        paragraphs: vec![
            TextParagraph::new("inserted\n", TextStyle::default()),
            paragraph,
        ],
    };
    let reused = system
        .layout_document(request(&edited, 120.0), &fonts)
        .unwrap();
    assert!(system.preparation_cache_snapshot().paragraphs.hits >= 1);
    discard_final_layouts(&system);
    discard_preparation(&system);
    let fresh = system
        .layout_document(request(&edited, 120.0), &fonts)
        .unwrap();
    assert_geometry(&reused, &fresh);
}

#[test]
fn font_registry_replacement_drops_prepared_shapes_and_metrics() {
    let system = TextSystem::with_bundled_fonts();
    let handle = FontHandle::new(4);
    let mut fonts = FontRegistry::new();
    let style = TextStyle {
        font: Some(handle),
        ..TextStyle::default()
    };
    for font in [
        include_bytes!("../../assets/NotoSans-Regular.ttf").as_slice(),
        include_bytes!("../../assets/NotoSansHebrew-Regular.ttf").as_slice(),
    ] {
        fonts.insert(handle, RegisteredFont::from_bytes(font));
        let layout = system
            .shape_text("אבגדה", Size::new(120.0, 40.0), style.clone(), &fonts)
            .unwrap();
        let stats = system.preparation_cache_snapshot();
        assert_eq!(stats.paragraphs.misses, 1);
        assert_eq!(stats.paragraphs.hits, 0);
        assert!(stats.glyphs.misses > 0);
        assert!(layout.faces().iter().any(|face| face.bytes() == font));
    }
    assert_eq!(system.font_context_build_count(), 2);
}

#[test]
fn color_changes_reuse_preparation_but_features_and_metrics_do_not() {
    let system = TextSystem::with_bundled_fonts();
    let fonts = FontRegistry::new();
    let mut style = TextStyle::default();
    system
        .measure_text_size("office", style.clone(), &fonts)
        .unwrap();
    style.color = Color::BLACK;
    system
        .measure_text_size("office", style.clone(), &fonts)
        .unwrap();
    assert_eq!(system.preparation_cache_snapshot().paragraphs.misses, 1);
    style.features.disable(FontFeature::STANDARD_LIGATURES);
    system
        .measure_text_size("office", style.clone(), &fonts)
        .unwrap();
    style.line_height += 3.0;
    system.measure_text_size("office", style, &fonts).unwrap();
    assert_eq!(system.preparation_cache_snapshot().paragraphs.misses, 3);
}

#[test]
fn cache_churn_is_byte_bounded_and_does_not_invalidate_pinned_layouts() {
    let system = TextSystem::with_bundled_fonts();
    let fonts = FontRegistry::new();
    system
        .measure_text_size("prime", TextStyle::default(), &fonts)
        .unwrap();
    {
        let mut cached = system.font_context.lock().unwrap();
        let preparation = &mut cached.as_mut().unwrap().context.preparation;
        preparation.paragraphs = ByteCache::new(100, 24 * 1024);
        preparation.glyphs = ByteCache::new(100, 512);
    }
    let pinned = system
        .shape_text_persistent(
            None,
            "pinned",
            Size::new(120.0, 40.0),
            TextStyle::default(),
            &fonts,
        )
        .unwrap();
    for index in 0..50 {
        system
            .shape_text(
                format!("change {index} abcdefghijklmnopqrstuvwxyz"),
                Size::new(80.0 + index as f32, 40.0),
                TextStyle::default(),
                &fonts,
            )
            .unwrap();
        let stats = system.preparation_cache_snapshot();
        assert!(stats.paragraphs.retained_bytes <= 24 * 1024);
        assert!(stats.glyphs.retained_bytes <= 512);
    }
    let before = system.preparation_cache_snapshot();
    assert!(before.paragraphs.evictions > 0 && before.glyphs.evictions > 0);
    system
        .measure_text_size("oversized ".repeat(4_000), TextStyle::default(), &fonts)
        .unwrap();
    assert_eq!(
        system.preparation_cache_snapshot().paragraphs.entries,
        before.paragraphs.entries
    );
    assert_eq!(
        system.text_layout_registry().get(pinned.handle()),
        Some(pinned.layout())
    );
}
