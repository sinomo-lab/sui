use crate::{FontRegistry, TextFlowDirection, TextLayout, TextStyle, TextSystem};
use sui_core::{Color, Point, Size};

fn shape(system: &TextSystem, text: &str) -> TextLayout {
    let layout = system
        .shape_text(
            text,
            Size::new(800.0, 80.0),
            TextStyle {
                font_size: 28.0,
                line_height: 48.0,
                ..TextStyle::new(Color::BLACK)
            },
            &FontRegistry::new(),
        )
        .expect("bundled fonts should shape without system fonts");
    assert!(!layout.glyphs().is_empty());
    assert!(layout.glyphs().iter().all(|glyph| glyph.glyph_id != 0));
    layout
}

#[test]
fn bundled_fonts_cover_arabic_and_hebrew_letters_and_marks() {
    let arabic =
        ttf_parser::Face::parse(include_bytes!("../../assets/NotoSansArabic-Regular.ttf"), 0)
            .unwrap();
    let hebrew =
        ttf_parser::Face::parse(include_bytes!("../../assets/NotoSansHebrew-Regular.ttf"), 0)
            .unwrap();
    // Complete alphabets and combining marks, including Hebrew cantillation.
    // Persian/Urdu letters also exercise coverage beyond the demo's Arabic sample.
    for codepoint in (0x0621..=0x063a).chain(0x0641..=0x065f).chain([
        0x0679, 0x067e, 0x0686, 0x0688, 0x0698, 0x06af, 0x06ba, 0x06be, 0x06c1, 0x06cc, 0x06d2,
    ]) {
        let ch = char::from_u32(codepoint).unwrap();
        assert!(
            arabic.glyph_index(ch).is_some(),
            "missing U+{codepoint:04X}"
        );
    }
    for codepoint in (0x05d0..=0x05ea)
        .chain(0x0591..=0x05bd)
        .chain([0x05bf, 0x05c1, 0x05c2, 0x05c4, 0x05c5, 0x05c7])
    {
        let ch = char::from_u32(codepoint).unwrap();
        assert!(
            hebrew.glyph_index(ch).is_some(),
            "missing U+{codepoint:04X}"
        );
    }
}

#[test]
fn bundled_fonts_shape_mixed_scripts_using_portable_fallback_faces() {
    let system = TextSystem::with_bundled_fonts();
    let layout = shape(&system, "Latin 123 مَرْحَبًا שָׁלוֹם 456 فارسی اُردُو end");
    for expected in [
        include_bytes!("../../assets/NotoSans-Regular.ttf").as_slice(),
        include_bytes!("../../assets/NotoSansArabic-Regular.ttf").as_slice(),
        include_bytes!("../../assets/NotoSansHebrew-Regular.ttf").as_slice(),
    ] {
        assert!(
            layout
                .glyphs()
                .iter()
                .any(|glyph| { layout.glyph_face(glyph).shared_bytes().as_ref() == expected })
        );
    }
    for direction in [
        TextFlowDirection::LeftToRight,
        TextFlowDirection::RightToLeft,
    ] {
        assert!(layout.runs().iter().any(|run| run.direction == direction));
    }
    super::assert_cluster_run_ranges(&layout);
}

#[test]
fn bundled_arabic_uses_contextual_joining_forms() {
    let system = TextSystem::with_bundled_fonts();
    let isolated = shape(&system, "\u{0628}");
    let joined = shape(&system, "\u{0628}\u{0628}\u{0628}");
    // Noto Sans Arabic draws the dots separately; compare the spacing forms.
    let isolated_ids = isolated
        .glyphs()
        .iter()
        .filter(|glyph| glyph.advance.x > 0.0)
        .map(|glyph| glyph.glyph_id)
        .collect::<Vec<_>>();
    assert_eq!(isolated_ids.len(), 1);
    let joined_ids = joined
        .glyphs()
        .iter()
        .filter(|glyph| glyph.advance.x > 0.0)
        .map(|glyph| glyph.glyph_id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        joined_ids.len(),
        3,
        "initial, medial and final forms should differ"
    );
    assert!(!joined_ids.contains(&isolated_ids[0]));
}

#[test]
fn bundled_diacritics_stay_attached_to_their_base_clusters() {
    let system = TextSystem::with_bundled_fonts();
    for (plain, marked) in [
        ("\u{0628}", "\u{0628}\u{0650}"),
        ("\u{05e9}", "\u{05e9}\u{05b8}\u{05c1}"),
    ] {
        let base = shape(&system, plain);
        let layout = shape(&system, marked);
        assert_eq!(layout.clusters().len(), 1);
        assert_eq!(layout.clusters()[0].byte_range, 0..marked.len());
        assert!(layout.glyphs().len() > base.glyphs().len());
        assert!((layout.measurement().width - base.measurement().width).abs() < 0.01);
        let base_bounds = base.glyphs()[0].bounds.unwrap();
        assert!(
            layout.glyphs().iter().any(|glyph| {
                glyph.advance.x.abs() < 0.01
                    && glyph.bounds.is_some_and(|bounds| {
                        bounds.max_x() > base_bounds.x() && bounds.x() < base_bounds.max_x()
                    })
            }),
            "marks should have zero advance and sit over or under the base"
        );
    }
}

#[test]
fn bundled_rtl_carets_progress_left_and_hit_test_back_to_text() {
    let system = TextSystem::with_bundled_fonts();
    for text in ["مرحبا", "שלום"] {
        let layout = shape(&system, text);
        let offsets = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain([text.len()]);
        let mut previous_x = f32::INFINITY;
        for offset in offsets {
            let caret = layout.caret_rect(offset);
            assert!(
                caret.x() < previous_x,
                "RTL caret should move left: {text:?}, {offset}"
            );
            previous_x = caret.x();
            let hit =
                layout.hit_test_point(Point::new(caret.x(), caret.y() + caret.height() * 0.5));
            assert_eq!(hit.utf8_offset, offset, "RTL caret hit test: {text:?}");
        }
        let line = &layout.lines()[0];
        for (x, offset) in [
            (line.rect.x() - 10.0, text.len()),
            (line.rect.max_x() + 10.0, 0),
        ] {
            assert_eq!(
                layout
                    .hit_test_point(Point::new(x, line.rect.y() + 2.0))
                    .utf8_offset,
                offset,
                "hits outside RTL text should choose its visual edge"
            );
        }
    }
}

#[test]
fn bundled_mixed_direction_hit_tests_follow_visual_clusters() {
    let system = TextSystem::with_bundled_fonts();
    for text in ["abc שלום 123 مرحبا end", "שָׁלוֹם 123 مَرْحَبًا abc"] {
        let layout = shape(&system, text);
        for line in layout.lines() {
            for cluster in &line.clusters {
                if (cluster.x_end - cluster.x_start).abs() < 0.01 {
                    continue;
                }
                // Probe within each cluster, avoiding ambiguous bidi run boundaries.
                for (fraction, expected) in [(0.25, cluster.range.start), (0.75, cluster.range.end)]
                {
                    let x = cluster.x_start + (cluster.x_end - cluster.x_start) * fraction;
                    let hit = layout
                        .hit_test_point(Point::new(x, line.rect.y() + line.rect.height() * 0.5));
                    assert_eq!(
                        hit.utf8_offset, expected,
                        "hit at {x}: {text:?}, {cluster:?}"
                    );
                    assert!(text.is_char_boundary(hit.utf8_offset));
                }
            }
        }
    }
}
