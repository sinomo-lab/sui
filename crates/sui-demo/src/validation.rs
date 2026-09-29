//! Text rendering, text editing, and HDR color validation surfaces.

#![forbid(unsafe_code)]

use std::rc::Rc;

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, TextDirection, TextStyle, TextSurface, TextWrap};

use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style, dev_theme_color,
};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;

pub const TEXT_RENDERING_COMPARISON_TITLE: &str = "SUI Text Rendering Comparison";
pub const COLOR_VALIDATION_VIEW_TITLE: &str = "SUI HDR and Color Validation";
pub const TEXT_VALIDATION_VIEW_TITLE: &str = "SUI Text Validation";
pub const TEXT_RENDERING_COMPARISON_SCROLL_NAME: &str = "Text rendering comparison scroll";
pub const TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME: &str =
    "Text rendering comparison vertical scroll bar";

pub const TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME: &str =
    "Text rendering comparison horizontal scroll bar";

pub const COLOR_VALIDATION_SCROLL_NAME: &str = "Color validation scroll";
pub const COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME: &str = "Color validation vertical scroll bar";
pub const COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME: &str =
    "Color validation horizontal scroll bar";

pub const TEXT_VALIDATION_SCROLL_NAME: &str = "Text validation scroll";
pub const TEXT_VALIDATION_EDITOR_NAME: &str = "Validation editor";
pub(crate) const TEXT_RENDERING_COMPARISON_MIN_WIDTH: f32 = 1094.0;
pub(crate) const TEXT_RENDERING_COMPARISON_CARD_WIDTH: f32 = 520.0;
pub(crate) const TEXT_RENDERING_SAMPLE_TILE_WIDTH: f32 = 232.0;
pub(crate) const TEXT_RENDERING_SAMPLE_TILE_HEIGHT: f32 = 108.0;
pub(crate) const TEXT_VALIDATION_CONTENT_WIDTH: f32 = 1040.0;
pub(crate) const TEXT_VALIDATION_PROBE_CARD_WIDTH: f32 = 320.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextRenderingModeSpec {
    title: &'static str,
    subtitle: &'static str,
    notes: &'static str,
    setting: &'static str,
    policy: TextRenderPolicy,
}

pub(crate) const TEXT_RENDERING_MODE_DATA: [TextRenderingModeSpec; 9] = [
    TextRenderingModeSpec {
        title: "Linear coverage",
        subtitle: "Coverage sampled without perceptual compensation.",
        notes: "Use as a control when art direction needs literal glyph coverage and no perceptual weight compensation.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Linear)",
        policy: TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Linear),
    },
    TextRenderingModeSpec {
        title: "Perceptual coverage",
        subtitle: "Default SUI policy with color-aware coverage boost.",
        notes: "This is the default text weight model and the best starting point for normal UI labels.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
    },
    TextRenderingModeSpec {
        title: "Perceptual no hinting",
        subtitle: "Default coverage without small-text grid fitting.",
        notes: "Use for canvas-like tools when free positioning matters more than pixel-grid stability.",
        setting: "TextRenderPolicy::new()\n    .with_hinting(TextRenderHinting::None)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: TextRenderPolicy::new()
            .with_hinting(TextRenderHinting::None)
            .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
    },
    TextRenderingModeSpec {
        title: "LCD subpixel",
        subtitle: "Per-channel atlas coverage for pixel-aligned text.",
        notes: "Use for dense UI text on LCD-safe axis-aligned output; transformed text falls back to grayscale glyph coverage.",
        setting: "TextRenderPolicy::new()\n    .with_render_mode(TextRenderMode::LcdSubpixel)\n    .with_subpixel_order(TextSubpixelOrder::Rgb)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: TextRenderPolicy::new()
            .with_render_mode(TextRenderMode::LcdSubpixel)
            .with_subpixel_order(TextSubpixelOrder::Rgb)
            .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
    },
    TextRenderingModeSpec {
        title: "Gamma 1.8 coverage",
        subtitle: "Diagnostic curve that lightens mid-coverage edges.",
        notes: "Use as a diagnostic option when comparing lighter antialiasing against perceptual coverage.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Gamma(1.8))",
        policy: TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Gamma(1.8)),
    },
    TextRenderingModeSpec {
        title: "Coverage boost 0.50",
        subtitle: "Fixed boost independent of foreground color.",
        notes: "Use when a graphics surface needs a repeatable fixed coverage curve across all foreground colors.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(0.50))",
        policy: TextRenderPolicy::new()
            .with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(0.50)),
    },
    TextRenderingModeSpec {
        title: "2c - c*c coverage",
        subtitle: "Maximum built-in boost curve for coverage pixels.",
        notes: "Use as an assertive diagnostic or for deliberately heavier small text in dense overlays.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq)",
        policy: TextRenderPolicy::new()
            .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq),
    },
    TextRenderingModeSpec {
        title: "Perceptual + stem darkening",
        subtitle: "Default coverage plus restrained small-text stem weight.",
        notes: "Use for tiny labels that need extra stem weight without changing layout or font weight.",
        setting: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)\n    .with_stem_darkening(TextRenderStemDarkening::Enabled { max_ppem: 18.0, amount: 0.20 })",
        policy: TextRenderPolicy::new()
            .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)
            .with_stem_darkening(TextRenderStemDarkening::Enabled {
                max_ppem: 18.0,
                amount: 0.20,
            }),
    },
    TextRenderingModeSpec {
        title: "LCD + stem darkening",
        subtitle: "Subpixel rendering plus small-text stem weight.",
        notes: "Use for dense desktop surfaces after checking that the target display path preserves physical RGB subpixels.",
        setting: "TextRenderPolicy::new()\n    .with_render_mode(TextRenderMode::LcdSubpixel)\n    .with_subpixel_order(TextSubpixelOrder::Rgb)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)\n    .with_stem_darkening(TextRenderStemDarkening::Enabled { max_ppem: 18.0, amount: 0.20 })",
        policy: TextRenderPolicy::new()
            .with_render_mode(TextRenderMode::LcdSubpixel)
            .with_subpixel_order(TextSubpixelOrder::Rgb)
            .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)
            .with_stem_darkening(TextRenderStemDarkening::Enabled {
                max_ppem: 18.0,
                amount: 0.20,
            }),
    },
];

pub fn build_text_rendering_comparison_surface() -> impl Widget {
    build_text_rendering_comparison_surface_with_theme(default_theme_reader())
}

pub fn build_text_rendering_comparison_surface_with_theme(
    theme_reader: DevThemeReader,
) -> impl Widget {
    let scroll_state = ScrollState::new();
    let mut mode_grid = Stack::vertical()
        .spacing(14.0)
        .alignment(Alignment::Stretch);

    for row in TEXT_RENDERING_MODE_DATA.chunks(2) {
        let mut row_stack = Stack::horizontal()
            .spacing(14.0)
            .alignment(Alignment::Start);
        for &spec in row {
            row_stack = row_stack.with_child(build_text_rendering_mode_card_with_theme(
                Rc::clone(&theme_reader),
                spec,
            ));
        }
        mode_grid = mode_grid.with_child(row_stack);
    }

    let content = MinimumWidth::new(
        TEXT_RENDERING_COMPARISON_MIN_WIDTH,
        Padding::all(
            20.0,
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(panel_with_theme(
                    Rc::clone(&theme_reader),
                    "Text rendering options",
                    "Direct-rendered samples showing per-text TextRenderPolicy overrides for graphics tools, inspectors, and dense UI surfaces.",
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Center)
                        .with_child(build_text_rendering_summary_metric_with_theme(
                            Rc::clone(&theme_reader),
                            "Modes",
                            "7",
                            "coverage, hinting, weight",
                        ))
                        .with_child(build_text_rendering_summary_metric_with_theme(
                            Rc::clone(&theme_reader),
                            "Pairs",
                            "2",
                            "light and dark direct text",
                        ))
                        .with_child(build_text_rendering_summary_metric_with_theme(
                            Rc::clone(&theme_reader),
                            "Stress",
                            "11-16 px",
                            "dense labels and status text",
                        )),
                ))
                .with_child(mode_grid),
        ),
    );

    TwoAxisScrollPane::new(
        scroll_state.clone(),
        ScrollView::both(content)
            .state(scroll_state.clone())
            .overlay_scroll_bars(false)
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Auto)
            .name(TEXT_RENDERING_COMPARISON_SCROLL_NAME),
        ScrollBar::vertical(scroll_state.clone())
            .name(TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
        ScrollBar::horizontal(scroll_state)
            .name(TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

pub fn build_text_rendering_comparison_application() -> Application {
    App::new()
        .window(Window::new(TEXT_RENDERING_COMPARISON_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_RENDERING_COMPARISON_TITLE,
                "Reference surface for applying per-text perceptual coverage, diagnostic coverage curves, hinting, and stem darkening overrides.",
                build_text_rendering_comparison_surface(),
            ),
        ))
        .into_application()
}

pub fn build_color_validation_surface() -> impl Widget {
    build_color_validation_surface_with_theme(default_theme_reader())
}

pub fn build_color_validation_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    const COLOR_VALIDATION_MIN_CONTENT_WIDTH: f32 = 780.0;
    const COLOR_VALIDATION_SWATCH_MIN_WIDTH: f32 = 150.0;

    let scroll_state = ScrollState::new();
    let content = MinimumWidth::new(
        COLOR_VALIDATION_MIN_CONTENT_WIDTH,
        Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "HDR brightness and clipping probes",
                "Start here when checking HDR. These rows show whether values above SDR reference white stay visually distinct. On SDR or clamp-heavy paths, the brighter swatches may collapse together. On HDR-capable paths, higher steps should remain separable and retain highlight structure.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(build_color_validation_quad_row_with_theme(
                        Rc::clone(&theme_reader),
                        "HDR white ladder",
                        "Reference white is 1.0. Higher linear-light steps intentionally exceed SDR range. If 2.0, 4.0, and 8.0 all look identical, the path is clipping or tone mapping aggressively.",
                        [
                            ("Reference white 1.0", Color::linear_rgba(1.0, 1.0, 1.0, 1.0)),
                            ("Highlight white 2.0", Color::linear_rgba(2.0, 2.0, 2.0, 1.0)),
                            ("Highlight white 4.0", Color::linear_rgba(4.0, 4.0, 4.0, 1.0)),
                            ("Highlight white 8.0", Color::linear_rgba(8.0, 8.0, 8.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_quad_row_with_theme(
                        Rc::clone(&theme_reader),
                        "HDR color highlight ladder",
                        "Colored highlights help catch cases where luminance is preserved but saturation shifts unexpectedly. Compare how orange and cyan energy above 1.0 behaves relative to SDR-bright controls.",
                        [
                            ("Orange highlight 1.0", Color::linear_rgba(1.0, 0.55, 0.18, 1.0)),
                            ("Orange highlight 2.0", Color::linear_rgba(2.0, 1.1, 0.36, 1.0)),
                            ("Cyan highlight 1.0", Color::linear_rgba(0.20, 0.80, 1.0, 1.0)),
                            ("Cyan highlight 2.0", Color::linear_rgba(0.40, 1.60, 2.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row_with_theme(
                        Rc::clone(&theme_reader),
                        "SDR clipping reference",
                        "This pair makes SDR clipping easy to spot. If the boosted sample looks no brighter than the baseline, the path is still constrained to SDR output at this stage.",
                        [
                            ("SDR white baseline", Color::linear_rgba(1.0, 1.0, 1.0, 1.0)),
                            ("SDR clipped white 2.0", Color::linear_rgba(2.0, 2.0, 2.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    )),
            ))
            .with_child(panel_with_theme(
                Rc::clone(&theme_reader),
                "Wide-gamut reference swatches",
                "Use these after the HDR ladder. This surface validates that sRGB and Display-P3 colors stay distinct in the renderer's linear working space before final display output.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(build_color_validation_row_with_theme(
                        Rc::clone(&theme_reader),
                        "Red primary",
                        "Display-P3 red should preserve its native primaries instead of being treated as an sRGB red with only transfer decoding.",
                        [
                            ("sRGB reference red", Color::rgba(1.0, 0.0, 0.0, 1.0)),
                            ("Display P3 reference red", Color::display_p3(1.0, 0.0, 0.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row_with_theme(
                        Rc::clone(&theme_reader),
                        "Green primary",
                        "The Display-P3 green sample intentionally lives outside the sRGB gamut. Compare it against the clipped sRGB control when checking wide-gamut correctness.",
                        [
                            ("sRGB clipped lime", Color::rgba(0.0, 1.0, 0.0, 1.0)),
                            ("Display P3 vivid lime", Color::display_p3(0.0, 1.0, 0.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    ))
                    .with_child(build_color_validation_row_with_theme(
                        Rc::clone(&theme_reader),
                        "Cyan accent mix",
                        "A mixed-color sample helps catch cases where Display-P3 is incorrectly reduced to transfer decoding only. The P3 version should retain a more vivid cyan accent on wide-gamut outputs.",
                        [
                            ("sRGB accent cyan", Color::rgba(0.0, 0.78, 1.0, 1.0)),
                            ("Display P3 accent cyan", Color::display_p3(0.0, 0.78, 1.0, 1.0)),
                        ],
                        COLOR_VALIDATION_SWATCH_MIN_WIDTH,
                    )),
            )),
    ));

    TwoAxisScrollPane::new(
        scroll_state.clone(),
        ScrollView::both(content)
            .state(scroll_state.clone())
            .overlay_scroll_bars(false)
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Auto)
            .name(COLOR_VALIDATION_SCROLL_NAME),
        ScrollBar::vertical(scroll_state.clone())
            .name(COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
        ScrollBar::horizontal(scroll_state)
            .name(COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

pub fn build_color_validation_application() -> Application {
    App::new()
        .window(Window::new(COLOR_VALIDATION_VIEW_TITLE).root(
            LivePerformanceRoot::new(
                COLOR_VALIDATION_VIEW_TITLE,
                "Reference surface for validating wide-gamut color handling, HDR brightness separation, and SDR clipping behavior while native HDR support lands in phases.",
                build_color_validation_surface(),
            ),
        ))
        .into_application()
}

pub(crate) fn build_color_validation_row_with_theme(
    theme_reader: DevThemeReader,
    title: &'static str,
    description: &'static str,
    swatches: [(&'static str, Color); 2],
    swatch_min_width: f32,
) -> impl Widget {
    let initial_theme = theme_reader();
    NamedSection::new(
        title,
        Background::new(
            initial_theme.palette.surface_raised,
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Stretch)
                    .with_child(demo_label(
                        &theme_reader,
                        title,
                        DemoTextRole::Emphasis,
                        DemoTextColor::Text,
                    ))
                    .with_child(demo_label(
                        &theme_reader,
                        description,
                        DemoTextRole::Body,
                        DemoTextColor::Muted,
                    ))
                    .with_child(
                        Stack::horizontal()
                            .spacing(18.0)
                            .alignment(Alignment::Center)
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[0].0,
                                swatches[0].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[1].0,
                                swatches[1].1,
                                swatch_min_width,
                            )),
                    ),
            ),
        )
        .brush_when(dev_theme_color(&theme_reader, |theme| {
            theme.palette.surface_raised
        })),
    )
}

pub(crate) fn build_color_validation_quad_row_with_theme(
    theme_reader: DevThemeReader,
    title: &'static str,
    description: &'static str,
    swatches: [(&'static str, Color); 4],
    swatch_min_width: f32,
) -> impl Widget {
    let initial_theme = theme_reader();
    NamedSection::new(
        title,
        Background::new(
            initial_theme.palette.surface_raised,
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Stretch)
                    .with_child(demo_label(
                        &theme_reader,
                        title,
                        DemoTextRole::Emphasis,
                        DemoTextColor::Text,
                    ))
                    .with_child(demo_label(
                        &theme_reader,
                        description,
                        DemoTextRole::Body,
                        DemoTextColor::Muted,
                    ))
                    .with_child(
                        Stack::horizontal()
                            .spacing(18.0)
                            .alignment(Alignment::Center)
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[0].0,
                                swatches[0].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[1].0,
                                swatches[1].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[2].0,
                                swatches[2].1,
                                swatch_min_width,
                            ))
                            .with_child(build_color_validation_swatch_with_theme(
                                Rc::clone(&theme_reader),
                                swatches[3].0,
                                swatches[3].1,
                                swatch_min_width,
                            )),
                    ),
            ),
        )
        .brush_when(dev_theme_color(&theme_reader, |theme| {
            theme.palette.surface_raised
        })),
    )
}

pub(crate) fn build_color_validation_swatch_with_theme(
    theme_reader: DevThemeReader,
    name: &'static str,
    color: Color,
    min_width: f32,
) -> impl Widget {
    MinimumWidth::new(
        min_width,
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Center)
            .with_child(
                ColorSwatch::new(name, color)
                    .size(Size::new(132.0, 56.0))
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
            )
            .with_child(demo_label(
                &theme_reader,
                name,
                DemoTextRole::Supporting,
                DemoTextColor::Text,
            )),
    )
}

pub(crate) fn build_text_rendering_mode_card_with_theme(
    theme_reader: DevThemeReader,
    spec: TextRenderingModeSpec,
) -> impl Widget {
    NamedSection::new(
        spec.title,
        SizedBox::new()
            .width(TEXT_RENDERING_COMPARISON_CARD_WIDTH)
            .with_child(
                StoryCard::new(
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(demo_label(
                            &theme_reader,
                            spec.title,
                            DemoTextRole::Emphasis,
                            DemoTextColor::Text,
                        ))
                        .with_child(MaximumWidth::new(
                            480.0,
                            demo_label(
                                &theme_reader,
                                spec.subtitle,
                                DemoTextRole::Metadata,
                                DemoTextColor::Muted,
                            ),
                        ))
                        .with_child(build_text_rendering_policy_snippet_with_theme(
                            Rc::clone(&theme_reader),
                            spec.setting,
                        ))
                        .with_child(
                            Stack::horizontal()
                                .spacing(10.0)
                                .alignment(Alignment::Start)
                                .with_child(build_text_rendering_sample_tile(
                                    text_rendering_sample_name(spec.title, false),
                                    "Light",
                                    false,
                                    spec,
                                ))
                                .with_child(build_text_rendering_sample_tile(
                                    text_rendering_sample_name(spec.title, true),
                                    "Dark",
                                    true,
                                    spec,
                                )),
                        )
                        .with_child(MaximumWidth::new(
                            480.0,
                            demo_label(
                                &theme_reader,
                                spec.notes,
                                DemoTextRole::Metadata,
                                DemoTextColor::Muted,
                            ),
                        )),
                )
                .theme_when(clone_dev_theme_reader(&theme_reader)),
            ),
    )
}

pub(crate) fn build_text_rendering_summary_metric_with_theme(
    theme_reader: DevThemeReader,
    label: &'static str,
    value: &'static str,
    caption: &'static str,
) -> impl Widget {
    SizedBox::new().width(210.0).with_child(
        StoryCard::new(
            Stack::vertical()
                .spacing(5.0)
                .alignment(Alignment::Start)
                .with_child(demo_label(
                    &theme_reader,
                    label,
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                ))
                .with_child(demo_label(
                    &theme_reader,
                    value,
                    DemoTextRole::Emphasis,
                    DemoTextColor::Text,
                ))
                .with_child(demo_label(
                    &theme_reader,
                    caption,
                    DemoTextRole::Metadata,
                    DemoTextColor::Muted,
                )),
        )
        .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

pub(crate) fn build_text_rendering_policy_snippet_with_theme(
    theme_reader: DevThemeReader,
    setting: &'static str,
) -> impl Widget {
    let initial_theme = theme_reader();
    Background::new(
        initial_theme.palette.control,
        Padding::all(
            8.0,
            demo_mono_label(&theme_reader, setting, DemoTextRole::Metadata, |theme| {
                theme.palette.text
            }),
        ),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.control
    }))
}

pub(crate) fn text_rendering_sample_name(title: &'static str, dark: bool) -> String {
    let surface = if dark { "dark" } else { "light" };
    format!("{title} {surface} sample")
}

pub(crate) struct TextRenderingSampleTile {
    name: String,
    label: &'static str,
    dark: bool,
    spec: TextRenderingModeSpec,
}

impl TextRenderingSampleTile {
    fn new(name: String, label: &'static str, dark: bool, spec: TextRenderingModeSpec) -> Self {
        Self {
            name,
            label,
            dark,
            spec,
        }
    }
}

impl Widget for TextRenderingSampleTile {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            TEXT_RENDERING_SAMPLE_TILE_WIDTH,
            TEXT_RENDERING_SAMPLE_TILE_HEIGHT,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.push_text_render_policy(self.spec.policy);
        paint_text_rendering_sample(ctx, ctx.bounds(), self.label, self.dark);
        ctx.pop_text_render_policy();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        ctx.push(node);
    }
}

pub(crate) fn build_text_rendering_sample_tile(
    name: String,
    label: &'static str,
    dark: bool,
    spec: TextRenderingModeSpec,
) -> impl Widget {
    TextRenderingSampleTile::new(name, label, dark, spec)
}

pub(crate) fn paint_text_rendering_sample(
    ctx: &mut PaintCtx,
    bounds: Rect,
    label: &'static str,
    dark: bool,
) {
    let background = if dark {
        Color::rgba(0.12, 0.16, 0.22, 1.0)
    } else {
        Color::rgba(0.995, 0.998, 1.0, 1.0)
    };
    let label_color = if dark {
        Color::rgba(0.70, 0.78, 0.86, 1.0)
    } else {
        Color::rgba(0.42, 0.49, 0.57, 1.0)
    };
    let primary_color = if dark {
        Color::rgba(0.96, 0.98, 1.0, 1.0)
    } else {
        Color::rgba(0.10, 0.14, 0.20, 1.0)
    };
    let secondary_color = if dark {
        Color::rgba(0.82, 0.88, 0.95, 1.0)
    } else {
        Color::rgba(0.18, 0.24, 0.32, 1.0)
    };

    ctx.fill_rect(bounds, background);
    ctx.stroke_rect(
        bounds,
        if dark {
            Color::rgba(1.0, 1.0, 1.0, 0.08)
        } else {
            Color::rgba(0.12, 0.16, 0.22, 0.08)
        },
        StrokeStyle::new(ctx.dpi().hairline_width()),
    );

    let x = bounds.x() + 12.0;
    let width = (bounds.width() - 24.0).max(1.0);
    draw_text_rendering_sample_line(
        ctx,
        Rect::new(x, bounds.y() + 12.0, width, 14.0),
        label,
        11.0,
        14.0,
        label_color,
    );
    draw_text_rendering_sample_line(
        ctx,
        Rect::new(x, bounds.y() + 33.0, width, 15.0),
        "minimum ill scroll",
        12.0,
        15.0,
        primary_color,
    );
    draw_text_rendering_sample_line(
        ctx,
        Rect::new(x, bounds.y() + 55.0, width, 17.0),
        "Toolbar 12 px glyph atlas",
        13.0,
        17.0,
        secondary_color,
    );
    draw_text_rendering_sample_line(
        ctx,
        Rect::new(x, bounds.y() + 81.0, width, 20.0),
        "Status row 16 px",
        16.0,
        20.0,
        primary_color,
    );
}

pub(crate) fn draw_text_rendering_sample_line(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &'static str,
    font_size: f32,
    line_height: f32,
    color: Color,
) {
    let mut style = TextStyle::new(color);
    style.font_size = font_size;
    style.line_height = line_height;
    ctx.draw_text(rect, text, style);
}

pub fn build_text_validation_surface() -> impl Widget {
    build_text_validation_surface_with_theme(default_theme_reader())
}

pub fn build_text_validation_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let content = Stack::vertical()
        .spacing(16.0)
        .alignment(Alignment::Stretch)
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "Text validation lab",
            "Focused smoke checks for shaping, wrapping, bidi boundaries, IME commits, and selection overlays.",
            Stack::horizontal()
                .spacing(14.0)
                .alignment(Alignment::Start)
                .with_child(build_text_validation_probe_card_with_theme(
                    Rc::clone(&theme_reader),
                    "Glyph coverage probe",
                    "Glyph coverage",
                    "Aa ill minimum | Cyrillic Привет",
                    "Checks Latin stems and one common fallback family without filling the page with missing-glyph blocks.",
                ))
                .with_child(build_text_validation_probe_card_with_theme(
                    Rc::clone(&theme_reader),
                    "Line wrapping probe",
                    "Line wrapping",
                    "wrap -> metrics -> caret -> overlay",
                    "Constrained text should reflow cleanly while selection geometry stays aligned to visible lines.",
                ))
                .with_child(build_text_validation_probe_card_with_theme(
                    Rc::clone(&theme_reader),
                    "Bidi caret probe",
                    "Bidi caret",
                    "abc 123 | שָׁלוֹם | مَرْحَبًا",
                    "Use the editor below for live RTL input while this card keeps the visual checklist compact.",
                )),
        ))
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "Interactive editor target",
            "Manual target for caret movement, selection ranges, scrolling, IME preedit, and fallback text entry.",
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    MaximumWidth::new(
                        960.0,
                        demo_label(
                            &theme_reader,
                            "Focus the editor, type with IME or keyboard input, extend selection with Shift+Arrow, and wheel-scroll to inspect visible-line extraction.",
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    ),
                )
                .with_child(
                    SizedBox::new()
                        .width(980.0)
                        .height(300.0)
                        .with_child(
                            TextSurface::new(TEXT_VALIDATION_EDITOR_NAME)
                                .value(text_validation_editor_seed())
                                .wrap(TextWrap::Word)
                                .direction(TextDirection::Auto)
                                .min_width(980.0)
                                .min_height(300.0)
                                .theme_when(clone_dev_theme_reader(&theme_reader))
                                .text_style_when(|theme| {
                                    demo_text_style(
                                        theme,
                                        DemoTextRole::Body,
                                        theme.palette.text,
                                    )
                                }),
                        ),
                ),
        ));

    ScrollView::vertical(Padding::all(
        24.0,
        SizedBox::new()
            .width(TEXT_VALIDATION_CONTENT_WIDTH)
            .with_child(content),
    ))
    .name(TEXT_VALIDATION_SCROLL_NAME)
    .theme_when(clone_dev_theme_reader(&theme_reader))
}

pub(crate) fn build_text_validation_probe_card_with_theme(
    theme_reader: DevThemeReader,
    name: &'static str,
    title: &'static str,
    sample: &'static str,
    caption: &'static str,
) -> impl Widget {
    NamedSection::new(
        name,
        SizedBox::new()
            .width(TEXT_VALIDATION_PROBE_CARD_WIDTH)
            .with_child(
                StoryCard::new(
                    Stack::vertical()
                        .spacing(8.0)
                        .alignment(Alignment::Start)
                        .with_child(demo_label(
                            &theme_reader,
                            title,
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ))
                        .with_child(demo_label(
                            &theme_reader,
                            sample,
                            DemoTextRole::Emphasis,
                            DemoTextColor::Text,
                        ))
                        .with_child(demo_label(
                            &theme_reader,
                            caption,
                            DemoTextRole::Metadata,
                            DemoTextColor::Muted,
                        )),
                )
                .theme_when(clone_dev_theme_reader(&theme_reader)),
            ),
    )
}

pub(crate) fn text_validation_editor_seed() -> String {
    [
        "Validation checklist",
        "- Shape: Latin stems and common fallback families should stay readable.",
        "- Wrapping: long diagnostics must reflow without selection gaps when the viewport narrows.",
        "- IME: composition commits should land near the caret instead of invalidating the whole surface.",
        "- Caret: moving across bidi boundaries should preserve stable layout handles and visible overlays.",
        "",
        "Fallback probes to paste, edit, or compare:",
        "Arabic: مرحبا | Hebrew: שלום | Hindi: नमस्ते | Han: 中文 | Emoji: 🙂",
        "Joining and marks: بِبِبِ | שָׁלוֹם | Mixed direction: abc שלום 123 مرحبا end",
        "",
        "Type here to confirm the runtime still exposes semantics-first text input for automated tests.",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests;
