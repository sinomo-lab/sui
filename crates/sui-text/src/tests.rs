use crate::{
    FontRegistry, RegisteredFont, TextDirection, TextDocument, TextFlowDirection,
    TextLayoutCacheSnapshot, TextLayoutRequest, TextParagraph, TextParagraphStyle,
    TextSelection, TextSpan, TextStyle, TextSystem,
};
use sui_core::{Color, FontHandle, Point, Size};

fn load_test_font() -> RegisteredFont {
    load_system_font_for_family(fontdb::Family::SansSerif).expect("system sans-serif font available for text tests")
}

fn load_system_font_for_family(family: fontdb::Family<'static>) -> Option<RegisteredFont> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    let families = [family];
    let font_id = font_db
        .query(&fontdb::Query {
            families: &families,
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .or_else(|| font_db.faces().next().map(|face| face.id))?;

    font_db
        .with_face_data(font_id, |font_data, face_index| {
            RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
        })
}

fn load_distinct_test_fonts() -> Option<(RegisteredFont, RegisteredFont)> {
    let candidates = [
        fontdb::Family::SansSerif,
        fontdb::Family::Serif,
        fontdb::Family::Monospace,
        fontdb::Family::Cursive,
    ];
    let fonts = candidates
        .into_iter()
        .filter_map(load_system_font_for_family)
        .collect::<Vec<_>>();

    for left_index in 0..fonts.len() {
        for right_index in (left_index + 1)..fonts.len() {
            if fonts[left_index].shared_bytes() != fonts[right_index].shared_bytes()
                || fonts[left_index].face_index() != fonts[right_index].face_index()
            {
                return Some((fonts[left_index].clone(), fonts[right_index].clone()));
            }
        }
    }

    None
}

fn find_fallback_font_case() -> Option<(RegisteredFont, char)> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    let candidates = ['🙂', '中', 'Ж', 'א', 'م', 'क'];

    for candidate in candidates {
        let mut missing_font_id = None;
        let mut fallback_found = false;

        for face_info in font_db.faces() {
            let Some(Some((supports_ascii, supports_candidate))) = font_db.with_face_data(
                face_info.id,
                |font_data, face_index| {
                    let face = ttf_parser::Face::parse(font_data, face_index).ok()?;
                    Some((
                        face.glyph_index('A').is_some(),
                        face.glyph_index(candidate).is_some(),
                    ))
                },
            ) else {
                continue;
            };

            fallback_found |= supports_candidate;
            if missing_font_id.is_none() && supports_ascii && !supports_candidate {
                missing_font_id = Some(face_info.id);
            }

            if missing_font_id.is_some() && fallback_found {
                break;
            }
        }

        if let Some(font_id) = missing_font_id.filter(|_| fallback_found) {
            if let Some(font) = font_db.with_face_data(font_id, |font_data, face_index| {
                RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
            }) {
                return Some((font, candidate));
            }
        }
    }

    None
}

fn overlapping_range_indices(
    cluster_range: std::ops::Range<usize>,
    run_ranges: &[std::ops::Range<usize>],
) -> Option<std::ops::Range<usize>> {
    let overlaps = run_ranges
        .iter()
        .enumerate()
        .filter_map(|(index, run_range)| {
            if byte_ranges_overlap(&cluster_range, run_range) {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let start = *overlaps.first()?;
    let end = overlaps.last().copied().unwrap_or(start) + 1;
    Some(start..end)
}

fn byte_ranges_overlap(left: &std::ops::Range<usize>, right: &std::ops::Range<usize>) -> bool {
    if left.is_empty() {
        return right.start <= left.start && left.start <= right.end;
    }
    if right.is_empty() {
        return left.start <= right.start && right.start <= left.end;
    }
    left.start < right.end && right.start < left.end
}

fn assert_cluster_run_ranges(layout: &crate::TextLayout) {
    let run_ranges = layout
        .runs()
        .iter()
        .map(|run| run.byte_range.clone())
        .collect::<Vec<_>>();

    for cluster in layout.clusters() {
        assert!(cluster.glyph_range.start <= cluster.glyph_range.end);
        assert!(cluster.glyph_range.end <= layout.glyphs().len());
        assert!(cluster.run_range.start <= cluster.run_range.end);
        assert!(cluster.run_range.end <= layout.runs().len());
        assert_eq!(
            Some(cluster.run_range.clone()),
            overlapping_range_indices(cluster.byte_range.clone(), &run_ranges),
        );
    }
}

#[test]
fn text_system_shapes_text_and_reports_geometry() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "hello\nworld",
            Size::new(120.0, 48.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(layout.box_size(), Size::new(120.0, 48.0));
    assert_eq!(layout.paragraphs().len(), 2);
    assert_eq!(layout.lines().len(), 2);
    assert_eq!(layout.runs().len(), 2);
    assert!(!layout.glyphs().is_empty());
    assert!(layout.measurement().width > 0.0);
    assert!(layout.measurement().height >= layout.style().font_size);
    assert_eq!(layout.caret_rect(3).width(), 1.0);
    assert!(!layout.selection_rects(1..8).is_empty());
    assert!(layout.selection_bounds(1..8).is_some());
    assert!(layout
        .selection_geometry(&TextSelection::new(Default::default(), Default::default()))
        .rects
        .is_empty());
}

#[test]
fn text_system_wraps_multi_line_paragraphs() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "hello wrapped world",
            Size::new(70.0, 80.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(layout.paragraphs().len(), 1);
    assert!(layout.lines().len() >= 2);
    assert!(layout.runs().len() >= 2);
    assert!(layout.measurement().height > layout.style().line_height);
}

#[test]
fn text_system_uses_registered_font_handles() {
    let system = TextSystem::new();
    let handle = FontHandle::new(19);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let layout = system
        .shape_text(
            "registered",
            Size::new(160.0, 28.0),
            TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
            &fonts,
        )
        .unwrap();

    assert_eq!(layout.runs().len(), 1);
    assert_eq!(
        layout.run_face(0).face_index(),
        fonts.get(handle).unwrap().face_index()
    );
    assert_eq!(
        layout.run_face(0).shared_bytes(),
        fonts.get(handle).unwrap().shared_bytes()
    );
}

#[test]
fn text_system_reuses_cached_layouts_across_color_changes() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "cached",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(
        system.layout_cache_snapshot(),
        TextLayoutCacheSnapshot {
            entries: 1,
            hits: 0,
            misses: 1,
        }
    );
    assert_eq!(layout.style().color, Color::WHITE);

    let second = system
        .shape_text(
            "cached",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::rgba(0.2, 0.7, 0.9, 1.0)),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(
        system.layout_cache_snapshot(),
        TextLayoutCacheSnapshot {
            entries: 1,
            hits: 1,
            misses: 1,
        }
    );
    assert_eq!(second.style().color, Color::rgba(0.2, 0.7, 0.9, 1.0));
    assert!(second.shares_storage_with(&layout));
    assert_eq!(second.id(), layout.id());
    assert_eq!(second.version(), layout.version());
    assert_eq!(second.glyphs(), layout.glyphs());
}

#[test]
fn text_system_reuses_cached_registered_font_layouts_across_color_changes() {
    let system = TextSystem::new();
    let handle = FontHandle::new(29);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let layout = system
        .shape_text(
            "registered cached",
            Size::new(180.0, 28.0),
            TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
            &fonts,
        )
        .unwrap();

    assert_eq!(
        system.layout_cache_snapshot(),
        TextLayoutCacheSnapshot {
            entries: 1,
            hits: 0,
            misses: 1,
        }
    );

    let second = system
        .shape_text(
            "registered cached",
            Size::new(180.0, 28.0),
            TextStyle {
                font: Some(handle),
                color: Color::rgba(0.3, 0.8, 0.4, 1.0),
                ..TextStyle::default()
            },
            &fonts,
        )
        .unwrap();

    assert_eq!(
        system.layout_cache_snapshot(),
        TextLayoutCacheSnapshot {
            entries: 1,
            hits: 1,
            misses: 1,
        }
    );
    assert_eq!(second.style().color, Color::rgba(0.3, 0.8, 0.4, 1.0));
    assert!(second.shares_storage_with(&layout));
    assert_eq!(second.id(), layout.id());
    assert_eq!(second.version(), layout.version());
    assert_eq!(second.glyphs(), layout.glyphs());
    assert_eq!(second.run_face(0).shared_bytes(), layout.run_face(0).shared_bytes());
}

#[test]
fn layout_document_keeps_paragraph_and_span_structure() {
    let system = TextSystem::new();
    let document = TextDocument {
        paragraphs: vec![
            TextParagraph {
                style: Default::default(),
                spans: vec![
                    TextSpan::new("hel", TextStyle::new(Color::WHITE)),
                    TextSpan::new("lo", TextStyle::new(Color::BLACK)),
                ],
            },
            TextParagraph::new("world", TextStyle::new(Color::WHITE)),
        ],
    };

    let layout = system
        .layout_document(
            TextLayoutRequest::new(document).with_box_size(Size::new(200.0, 64.0)),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(layout.document().paragraphs.len(), 2);
    assert_eq!(layout.paragraphs().len(), 2);
    assert_eq!(layout.lines().len(), 2);
    assert_eq!(layout.runs().len(), 3);
    assert_eq!(layout.run_style(0).color, Color::WHITE);
    assert_eq!(layout.run_style(1).color, Color::BLACK);
    assert_eq!(layout.text(), "hello\nworld");
    assert_eq!(layout.runs()[0].byte_range, 0..3);
    assert_eq!(layout.runs()[1].byte_range, 3..5);
}

#[test]
fn layout_document_preserves_mixed_faces_on_runs_and_glyphs() {
    let Some((left_font, right_font)) = load_distinct_test_fonts() else {
        return;
    };

    let system = TextSystem::new();
    let left_handle = FontHandle::new(101);
    let right_handle = FontHandle::new(102);
    let mut fonts = FontRegistry::new();
    fonts.insert(left_handle, left_font.clone());
    fonts.insert(right_handle, right_font.clone());

    let document = TextDocument {
        paragraphs: vec![TextParagraph {
            style: Default::default(),
            spans: vec![
                TextSpan::new(
                    "left",
                    TextStyle {
                        font: Some(left_handle),
                        ..TextStyle::new(Color::WHITE)
                    },
                ),
                TextSpan::new(
                    "right",
                    TextStyle {
                        font: Some(right_handle),
                        color: Color::BLACK,
                        ..TextStyle::default()
                    },
                ),
            ],
        }],
    };

    let layout = system
        .layout_document(
            TextLayoutRequest::new(document).with_box_size(Size::new(240.0, 40.0)),
            &fonts,
        )
        .unwrap();

    let distinct_faces = layout
        .runs()
        .iter()
        .map(|run| run.face_index)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(distinct_faces.len() >= 2);
    assert!(layout.glyphs().iter().any(|glyph| glyph.face_index != layout.glyphs()[0].face_index));
    assert_eq!(layout.glyphs()[0].span_id.paragraph_index, 0);
    assert_eq!(layout.glyphs()[0].span_id.span_index, 0);
    assert!(layout
        .glyphs()
        .iter()
        .any(|glyph| glyph.span_id.span_index == 1));
    assert_cluster_run_ranges(&layout);
}

#[test]
fn fallback_layout_exposes_non_explicit_faces_on_runs_and_glyphs() {
    let Some((explicit_font, fallback_char)) = find_fallback_font_case() else {
        return;
    };

    let system = TextSystem::new();
    let handle = FontHandle::new(103);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, explicit_font.clone());

    let layout = system
        .shape_text(
            format!("A{fallback_char}B"),
            Size::new(200.0, 32.0),
            TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
            &fonts,
        )
        .unwrap();

    let primary_bytes = explicit_font.shared_bytes();
    assert!(layout.faces().len() >= 2);
    assert!(layout.runs().iter().enumerate().any(|(index, _)| {
        layout.run_face(index).shared_bytes() != primary_bytes
    }));
    assert!(layout
        .glyphs()
        .iter()
        .any(|glyph| layout.glyph_face(glyph).shared_bytes() != primary_bytes));
    for glyph in layout.glyphs() {
        assert_eq!(glyph.face_index, layout.runs()[glyph.run_index].face_index);
    }
    assert_cluster_run_ranges(&layout);
}

#[test]
fn fallback_faces_keep_stable_shared_bytes_across_layout_builds() {
    let Some((explicit_font, fallback_char)) = find_fallback_font_case() else {
        return;
    };

    let system = TextSystem::new();
    let handle = FontHandle::new(104);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, explicit_font.clone());
    let style = TextStyle {
        font: Some(handle),
        ..TextStyle::new(Color::WHITE)
    };

    let first = system
        .shape_text(
            format!("A{fallback_char}B"),
            Size::new(200.0, 32.0),
            style.clone(),
            &fonts,
        )
        .unwrap();
    let second = system
        .shape_text(
            format!("A{fallback_char}B"),
            Size::new(201.0, 32.0),
            style,
            &fonts,
        )
        .unwrap();

    let primary_bytes = explicit_font.shared_bytes();
    let first_fallback = first
        .glyphs()
        .iter()
        .find_map(|glyph| {
            let face = first.glyph_face(glyph);
            (face.shared_bytes() != primary_bytes).then(|| face.clone())
        })
        .expect("expected fallback glyph face in first layout");
    let second_fallback = second
        .glyphs()
        .iter()
        .find_map(|glyph| {
            let face = second.glyph_face(glyph);
            (face.shared_bytes() != primary_bytes).then(|| face.clone())
        })
        .expect("expected fallback glyph face in second layout");

    assert_eq!(first_fallback.face_index(), second_fallback.face_index());
    assert_eq!(first_fallback.data_len(), second_fallback.data_len());
    assert_eq!(first_fallback.data_ptr(), second_fallback.data_ptr());
    assert_eq!(first_fallback.shared_bytes(), second_fallback.shared_bytes());
}

#[test]
fn run_views_and_glyph_instances_expose_renderer_context() {
    let system = TextSystem::new();
    let document = TextDocument {
        paragraphs: vec![TextParagraph {
            style: Default::default(),
            spans: vec![
                TextSpan::new("hello", TextStyle::new(Color::WHITE)),
                TextSpan::new(" world", TextStyle::new(Color::BLACK)),
            ],
        }],
    };

    let layout = system
        .layout_document(
            TextLayoutRequest::new(document).with_box_size(Size::new(200.0, 40.0)),
            &FontRegistry::new(),
        )
        .unwrap();

    let run_views = layout.run_views().collect::<Vec<_>>();
    assert_eq!(run_views.len(), layout.runs().len());
    assert_eq!(run_views[0].style.color, Color::WHITE);
    assert_eq!(run_views[1].style.color, Color::BLACK);
    assert_eq!(run_views[0].glyphs.len(), layout.runs()[0].glyph_range.len());
    assert_eq!(run_views[1].clusters.len(), layout.runs()[1].cluster_range.len());

    let glyph_instances = layout.glyph_instances().collect::<Vec<_>>();
    assert_eq!(glyph_instances.len(), layout.glyphs().len());
    for instance in glyph_instances {
        assert_eq!(instance.run.line_index, instance.glyph.line_index);
        assert_eq!(instance.line.paragraph_index, instance.run.paragraph_index);
        assert_eq!(instance.run.face_index, instance.glyph.face_index);
        assert_eq!(instance.face.face_index(), layout.glyph_face(instance.glyph).face_index());
        assert_eq!(instance.style.color, layout.glyph_style(instance.glyph).color);
    }
}

#[test]
fn line_window_exposes_visible_layout_slice() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "this is a wrapped line window test",
            Size::new(75.0, 120.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert!(layout.lines().len() >= 2);
    let window = layout.line_window(0..1);
    assert_eq!(window.lines().len(), 1);
    assert_eq!(window.glyphs().len(), layout.lines()[0].glyph_range.len());
    assert_eq!(window.glyph_instances().count(), window.glyphs().len());
    assert!(window.bounds().height() > 0.0);
}

#[test]
fn hit_testing_maps_points_back_to_text_offsets() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "this is a wrapped hit test line",
            Size::new(75.0, 120.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert!(layout.lines().len() >= 2);
    let first_line = &layout.lines()[0];
    let second_line = &layout.lines()[1];

    let start = layout.hit_test_point(Point::new(first_line.rect.x(), first_line.rect.y() + 2.0));
    let middle = layout.hit_test_point(Point::new(first_line.rect.x() + 24.0, first_line.rect.y() + 2.0));
    let next_line = layout.hit_test_point(Point::new(
        second_line.rect.x() + (second_line.rect.width() * 0.5),
        second_line.rect.y() + (second_line.rect.height() * 0.5),
    ));

    assert_eq!(start.utf8_offset, 0);
    assert!(middle.utf8_offset > start.utf8_offset);
    assert!(next_line.utf8_offset > middle.utf8_offset);
}

#[test]
fn auto_direction_layout_exposes_mixed_ltr_and_rtl_runs() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "cursor abc אבג 123 مرحبا",
            Size::new(320.0, 40.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    let directions = layout
        .runs()
        .iter()
        .map(|run| run.direction)
        .collect::<std::collections::HashSet<_>>();

    assert!(directions.contains(&TextFlowDirection::LeftToRight));
    assert!(directions.contains(&TextFlowDirection::RightToLeft));
    assert_cluster_run_ranges(&layout);
}

#[test]
fn explicit_rtl_paragraph_places_caret_progression_from_the_right() {
    let system = TextSystem::new();
    let document = TextDocument {
        paragraphs: vec![TextParagraph {
            style: TextParagraphStyle {
                direction: TextDirection::RightToLeft,
                ..Default::default()
            },
            spans: vec![TextSpan::new("שלום עולם", TextStyle::new(Color::WHITE))],
        }],
    };

    let layout = system
        .layout_document(
            TextLayoutRequest::new(document).with_box_size(Size::new(240.0, 40.0)),
            &FontRegistry::new(),
        )
        .unwrap();

    let start_caret = layout.caret_rect(0);
    let end_caret = layout.caret_rect(layout.text().len());

    assert_eq!(layout.lines()[0].direction, TextFlowDirection::RightToLeft);
    assert!(
        start_caret.x() >= end_caret.x(),
        "expected RTL caret progression to begin at the right edge, start={start_caret:?} end={end_caret:?}"
    );
}

#[test]
fn wrapped_selection_geometry_spans_multiple_lines() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "The validation surface should preserve selection rectangles across wrapped lines without collapsing everything into one strip.",
            Size::new(96.0, 160.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    let rects = layout.selection_rects(10..72);
    let bounds = layout.selection_bounds(10..72);

    assert!(layout.lines().len() >= 2);
    assert!(rects.len() >= 2, "expected multi-line selection geometry, got {rects:?}");
    assert!(bounds.is_some());
}

#[test]
fn persistent_layout_handle_survives_relayout() {
    let system = TextSystem::new();
    let first = system
        .shape_text_persistent(
            None,
            "alpha",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    let second = system
        .shape_text_persistent(
            Some(first.handle()),
            "beta beta",
            Size::new(160.0, 24.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    assert_eq!(first.handle(), second.handle());
    assert_ne!(first.version(), second.version());

    let registry = system.text_layout_registry();
    let resolved = registry
        .get(second.handle())
        .expect("persistent layout handle should resolve from the registry snapshot");
    assert_eq!(resolved.text(), "beta beta");
    assert_eq!(resolved.version(), second.version());
}

#[test]
fn adopted_layouts_register_content_derived_handles() {
    let system = TextSystem::new();
    let layout = system
        .shape_text(
            "adopted",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::WHITE),
            &FontRegistry::new(),
        )
        .unwrap();

    let persistent = system.adopt_layout(layout.clone());
    assert_eq!(persistent.handle().get(), layout.id().get());

    let registry = system.text_layout_registry();
    let resolved = registry
        .get(persistent.handle())
        .expect("adopted layout should be inserted into the registry snapshot");
    assert_eq!(resolved.text(), layout.text());
    assert_eq!(resolved.version(), layout.version());
}