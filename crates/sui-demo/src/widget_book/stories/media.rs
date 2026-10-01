use sui::prelude::*;
use sui::{BUILTIN_ICON_GLYPHS, ColorSpace, Rect, Vector};

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{Section, SpecimenGrid, boxed, example, strip};
use super::super::{COLOR_SWATCH_NAME, DEMO_IMAGE_LABEL, WIDGET_BOOK_IMAGE_HANDLE};
use super::sized;
use crate::app::DemoTextRole;

pub(super) const STORIES: [Story; 10] = [
    Story {
        id: "typography",
        title: "Typography",
        api: "Label, ThemeTextScale",
        summary: "The semantic text roles used across the demos, from page titles to metadata.",
        keywords: "text label font heading body",
        category: Category::Media,
        build: typography,
    },
    Story {
        id: "icons",
        title: "Icons",
        api: "Icon, IconGlyph",
        summary: "Every built-in glyph, drawn from the bundled Lucide set.",
        keywords: "glyph lucide symbol",
        category: Category::Media,
        build: icons,
    },
    Story {
        id: "link",
        title: "Link and rich text",
        api: "Link, RichText",
        summary: "Inline links with URL semantics and styled, wrapping text runs.",
        keywords: "hyperlink url rich text",
        category: Category::Media,
        build: link,
    },
    Story {
        id: "rich-document",
        title: "Rich document",
        api: "RichDocumentView",
        summary: "Retained Markdown blocks with selection, code actions, and structured results.",
        keywords: "markdown document code block",
        category: Category::Media,
        build: rich_document,
    },
    Story {
        id: "color-swatch",
        title: "Color swatch and palette",
        api: "ColorSwatch, ColorPalette",
        summary: "Color chips, including translucent ones over a checkerboard, and selectable palettes.",
        keywords: "color chip palette swatches",
        category: Category::Media,
        build: color_swatch,
    },
    Story {
        id: "color-picker",
        title: "Color picker",
        api: "ColorPicker, SimpleColorPicker",
        summary: "Full HDR-aware picking and compact channel sliders in HSL, HSV, RGB, or OKLCH.",
        keywords: "color picker hdr hsl rgb oklch perceptual",
        category: Category::Media,
        build: color_picker,
    },
    Story {
        id: "brush-preview",
        title: "Brush preview",
        api: "BrushPreview",
        summary: "A live stroke sample for brush size, opacity, and shape.",
        keywords: "brush stroke paint",
        category: Category::Media,
        build: brush_preview,
    },
    Story {
        id: "image",
        title: "Image",
        api: "Image, ImageFit",
        summary: "Registered images with fit modes, rounded corners, and backgrounds.",
        keywords: "picture thumbnail fit cover contain",
        category: Category::Media,
        build: image,
    },
    Story {
        id: "canvas",
        title: "Canvas and ruler",
        api: "Canvas, CanvasRuler",
        summary: "A zoomable vector canvas that can host widgets, with rulers that follow its viewport.",
        keywords: "vector canvas ruler zoom shapes",
        category: Category::Media,
        build: canvas,
    },
    Story {
        id: "pixel-canvas",
        title: "Pixel canvas",
        api: "PixelCanvas",
        summary: "An editable raster grid for sprites and masks.",
        keywords: "pixel raster sprite paint",
        category: Category::Media,
        build: pixel_canvas,
    },
];

fn typography(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let roles = [
        ("Page title", DemoTextRole::PageTitle, "Widget Book"),
        (
            "Section title",
            DemoTextRole::SectionTitle,
            "Layout and surfaces",
        ),
        ("Emphasis", DemoTextRole::Emphasis, "Export complete"),
        ("Card title", DemoTextRole::CardTitle, "Project settings"),
        (
            "Body",
            DemoTextRole::Body,
            "Body copy reads comfortably in panels and dialogs.",
        ),
        (
            "Supporting",
            DemoTextRole::Supporting,
            "Supporting text explains a control or a value.",
        ),
        (
            "Metadata",
            DemoTextRole::Metadata,
            "Updated 2 min ago · 12 MB",
        ),
    ];
    let rows = roles
        .into_iter()
        .map(|(label, role, sample)| (label, vec![boxed(ctx.styled(sample, role))]))
        .chain(std::iter::once((
            "Monospace",
            vec![boxed(ctx.mono("let pending = cache_hits + 1;"))],
        )))
        .collect();
    vec![Section::new(
        "Roles",
        SpecimenGrid::new(theme, &[""], rows, false),
    )]
}

fn icons(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let names: Vec<String> = BUILTIN_ICON_GLYPHS
        .iter()
        .map(|glyph| format!("{glyph:?}"))
        .collect();
    let items = BUILTIN_ICON_GLYPHS
        .iter()
        .zip(&names)
        .map(|(glyph, name)| {
            (
                name.as_str(),
                boxed(
                    SizedBox::new().width(72.0).child(Align::center(
                        Icon::new(*glyph)
                            .size(20.0)
                            .color(theme.palette.text)
                            .label(name)
                            .theme(theme),
                    )),
                ),
            )
        })
        .collect();
    vec![strip(theme, "", items)]
}

fn link(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "Link",
                boxed(Link::new("Read the SUI docs", "https://example.invalid/sui").theme(theme)),
            ),
            (
                "Disabled link",
                boxed(
                    Link::new("Release notes", "https://example.invalid/notes")
                        .enabled(false)
                        .theme(theme),
                ),
            ),
            (
                "Rich text",
                boxed(sized(
                    300.0,
                    RichText::from_plain_text(
                        "Rich text wraps across lines and keeps its layout retained between frames.",
                        TextStyle {
                            color: theme.palette.text,
                            ..theme.body_text_style()
                        },
                    ),
                )),
            ),
        ],
    )]
}

fn rich_document(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let document = RichDocumentModel::from_markdown(
        r#"## Retained rich document

Select across blocks, open the [application link](https://example.invalid/sui), or copy the code.

```rust
document.append_markdown("incremental tail");
```
"#,
    );
    let mut operation = RichExtensionBlock::new("operation-log", "Indexed workspace");
    operation.status = RichDocumentStatus::Success;
    operation.summary = Some("2,000 keyed rows ready".into());
    operation.body = "documents  184\nassets      37\nerrors       0".into();
    document.append_extension(operation);
    vec![example(
        "",
        SizedBox::new().width(620.0).height(320.0).child(
            ScrollView::vertical(RichDocumentView::new(document).theme(theme))
                .retain_content_layer()
                .theme(theme),
        ),
    )]
}

fn color_swatch(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let decorative = |hue| theme.decorative.get(hue).solid;
    let swatch = |name: &str, color: Color| {
        ColorSwatch::new(name, color)
            .size(Size::new(56.0, 32.0))
            .theme(theme)
    };
    vec![
        strip(
            theme,
            "Swatches",
            vec![
                (
                    "Opaque",
                    boxed(swatch(COLOR_SWATCH_NAME, theme.colors.primary)),
                ),
                (
                    "Translucent",
                    boxed(swatch("Shadow swatch", Color::rgba(0.08, 0.10, 0.14, 0.55))),
                ),
                (
                    "Wide gamut",
                    boxed(swatch(
                        "Display P3 swatch",
                        Color::display_p3(0.96, 0.30, 0.18, 1.0),
                    )),
                ),
                (
                    "Read only",
                    boxed(swatch("Locked swatch", theme.colors.secondary).read_only(true)),
                ),
            ],
        ),
        example(
            "Palette",
            ColorPalette::new("Document palette")
                .columns(9)
                .swatch_size(28.0)
                .swatches(
                    DecorativeHue::ALL
                        .map(|hue| ColorPaletteSwatch::new(format!("{hue:?}"), decorative(hue))),
                )
                .selected(6)
                .theme(theme),
        ),
    ]
}

fn color_picker(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![
        example(
            "HDR color picker",
            SizedBox::new().width(420.0).height(440.0).child(
                ColorPicker::from_color(
                    "Accent picker",
                    Color::new(ColorSpace::LinearSrgb, 2.0, 0.65, 0.4, 1.0),
                )
                .theme(theme),
            ),
        ),
        strip(
            theme,
            "Simple color picker",
            vec![
                (
                    "HSL",
                    boxed(sized(
                        220.0,
                        SimpleColorPicker::from_color(
                            "Simple HSL color picker",
                            Color::rgba(0.20, 0.58, 0.86, 1.0),
                        )
                        .mode(SimpleColorPickerMode::Hsl)
                        .show_alpha(false)
                        .theme(theme),
                    )),
                ),
                (
                    "HSV",
                    boxed(sized(
                        220.0,
                        SimpleColorPicker::from_color(
                            "Simple HSV color picker",
                            Color::rgba(0.42, 0.72, 0.30, 1.0),
                        )
                        .mode(SimpleColorPickerMode::Hsv)
                        .theme(theme),
                    )),
                ),
                (
                    "RGB in Display P3",
                    boxed(sized(
                        220.0,
                        SimpleColorPicker::from_color(
                            "Simple RGB color picker",
                            Color::display_p3(0.86, 0.42, 0.20, 0.70),
                        )
                        .mode(SimpleColorPickerMode::Rgb)
                        .color_space(ColorSpace::DisplayP3)
                        .theme(theme),
                    )),
                ),
                (
                    "OKLCH",
                    boxed(sized(
                        220.0,
                        SimpleColorPicker::from_color(
                            "Simple OKLCH color picker",
                            Color::oklch(0.62, 0.17, 300.0),
                        )
                        .mode(SimpleColorPickerMode::Oklch)
                        .theme(theme),
                    )),
                ),
            ],
        ),
    ]
}

fn brush_preview(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let decorative = |hue| theme.decorative.get(hue).solid;
    let brush = |name: &str, color: Color, size: f32, opacity: f32, shape| {
        BrushPreview::new(name)
            .spec(BrushPreviewSpec::new(color, size, opacity, shape))
            .size(Size::new(240.0, 56.0))
            .theme(theme)
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Round, opaque",
                boxed(brush(
                    "Round brush",
                    decorative(DecorativeHue::Blue),
                    24.0,
                    1.0,
                    BrushPreviewShape::Round,
                )),
            ),
            (
                "Round, 40% opacity",
                boxed(brush(
                    "Soft brush",
                    decorative(DecorativeHue::Magenta),
                    32.0,
                    0.4,
                    BrushPreviewShape::Round,
                )),
            ),
            (
                "Square",
                boxed(brush(
                    "Square brush",
                    decorative(DecorativeHue::Green),
                    16.0,
                    0.85,
                    BrushPreviewShape::Square,
                )),
            ),
        ],
    )]
}

fn image(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let framed = |label: &str, fit: ImageFit| {
        Image::new(WIDGET_BOOK_IMAGE_HANDLE)
            .label(label)
            .fit(fit)
            .size(Size::new(120.0, 84.0))
            .background(theme.palette.control)
            .corner_radius(8.0)
            .theme(theme)
    };
    vec![strip(
        theme,
        "Fit",
        vec![
            (
                "Contain",
                boxed(framed(DEMO_IMAGE_LABEL, ImageFit::Contain)),
            ),
            ("Cover", boxed(framed("Cover image", ImageFit::Cover))),
            ("Fill", boxed(framed("Fill image", ImageFit::Fill))),
            ("None", boxed(framed("Unscaled image", ImageFit::None))),
        ],
    )]
}

fn canvas(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let blue = theme.decorative.get(DecorativeHue::Blue).solid;
    let orange = theme.decorative.get(DecorativeHue::Orange).solid;
    // The document origin maps to the canvas center; pan so the drawing's
    // center (256, 146) lands there.
    let zoom = 0.9;
    let viewport = CanvasViewport::new()
        .zoom(zoom)
        .pan(Vector::new(-256.0 * zoom, -146.0 * zoom));
    let document = Size::new(640.0, 420.0);
    vec![example(
        "",
        Stack::vertical()
            .gap(0.0)
            .alignment(Alignment::Start)
            .with_child(sized(
                440.0,
                CanvasRuler::horizontal("Canvas horizontal ruler", document)
                    .viewport(viewport, Size::new(440.0, 26.0))
                    .theme(theme),
            ))
            .with_child(
                SizedBox::new().width(440.0).height(220.0).child(
                    Canvas::new("Vector canvas")
                        .desired_size(Size::new(440.0, 220.0))
                        .viewport(viewport)
                        .shape(CanvasShape::rect(
                            Rect::new(80.0, 70.0, 180.0, 100.0),
                            Some(blue.with_alpha(0.22)),
                            Some(CanvasStroke::new(blue, 2.0)),
                        ))
                        .shape(CanvasShape::circle(
                            Point::new(380.0, 170.0),
                            52.0,
                            Some(orange.with_alpha(0.24)),
                            Some(CanvasStroke::new(orange, 2.0)),
                        ))
                        .widget(
                            Rect::new(100.0, 96.0, 150.0, 40.0),
                            Button::new("Hosted widget").theme(theme),
                        )
                        .theme(theme),
                ),
            ),
    )]
}

fn pixel_canvas(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let blue = theme.decorative.get(DecorativeHue::Blue).solid;
    let amber = theme.decorative.get(DecorativeHue::Amber).solid;
    vec![example(
        "",
        SizedBox::new().width(360.0).height(180.0).child(
            PixelCanvas::from_fn("Pixel canvas", 16, 12, |x, y| {
                if (x + y) % 2 == 0 { blue } else { amber }
            })
            .state(PixelCanvasState::new())
            .desired_size(Size::new(360.0, 180.0))
            .fit_on_first_layout()
            .theme(theme),
        ),
    )]
}
