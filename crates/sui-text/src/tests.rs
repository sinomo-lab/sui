use crate::{
    FontRegistry, RegisteredFont, TextDocument, TextLayoutCacheSnapshot, TextLayoutRequest,
    TextParagraph, TextSelection, TextSpan, TextStyle, TextSystem,
};
use sui_core::{Color, FontHandle, Size};

fn load_test_font() -> RegisteredFont {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    let families = [fontdb::Family::SansSerif];
    let font_id = font_db
        .query(&fontdb::Query {
            families: &families,
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .or_else(|| font_db.faces().next().map(|face| face.id))
        .expect("system font available for text tests");

    font_db
        .with_face_data(font_id, |font_data, face_index| {
            RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
        })
        .expect("font data should be readable from system font database")
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

    assert_eq!(layout.face().face_index(), fonts.get(handle).unwrap().face_index());
    assert_eq!(layout.face().shared_bytes(), fonts.get(handle).unwrap().shared_bytes());
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
    assert_eq!(second.glyphs(), layout.glyphs());
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